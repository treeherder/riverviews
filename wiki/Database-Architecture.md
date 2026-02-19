# Database Architecture

FloPro uses PostgreSQL as its primary data store with a multi-schema design optimized for both historical analysis and real-time monitoring.

## Why PostgreSQL?

### Decision Rationale

✅ **Time Series Optimized** - Native timestamp types, efficient range queries, window functions  
✅ **ACID Guarantees** - Critical for flood alerting (no partial data)  
✅ **Advanced Indexing** - B-tree, partial indexes for recent data  
✅ **Mature & Stable** - 30+ years of development, production-tested  
✅ **Free & Open Source** - No vendor lock-in  
✅ **Rich Ecosystem** - Extensions, tools, Rust client libraries  
✅ **Geospatial Ready** - PostGIS available for future map features  

### Alternatives Considered

**TimescaleDB:**
- ⚠️ Adds complexity with chunking
- ⚠️ Overkill for 8 stations × 87 years (~700K records)
- ✅ Could migrate later if scaling to 1000+ stations

**InfluxDB:**
- ❌ Eventual consistency (not acceptable for flood alerts)
- ❌ Limited SQL support
- ❌ Weaker Rust client libraries
- ✅ Better for metrics, not transactional data

**SQLite:**
- ❌ No built-in replication
- ❌ Single-writer limitation
- ❌ Limited concurrent reader performance
- ✅ Fine for single-machine prototype

## Schema Overview

### Multi-Schema Design

FloPro uses **separate schemas** for different data sources and processing stages:

```sql
CREATE SCHEMA usgs_raw;    -- Raw USGS NWIS data
CREATE SCHEMA nws;         -- NWS forecasts (future)
CREATE SCHEMA noaa;        -- Weather data (future)
CREATE SCHEMA usace;       -- Lock/dam operations (future)
-- public schema:          -- Unified views, processed data
```

**Why separate schemas?**
- **Source tracking**: Always know data origin
- **Namespace isolation**: Prevents table name conflicts
- **Permission control**: Different access levels per source
- **Clear dependencies**: Public views reference raw schemas
- **Easy cleanup**: `DROP SCHEMA usgs_raw CASCADE` for testing

## Core Tables

### 1. usgs_raw.sites — Station Metadata

**Purpose:** Master registry of monitored USGS gauge stations

```sql
CREATE TABLE usgs_raw.sites (
    site_code VARCHAR(8) PRIMARY KEY,     -- 05568500
    site_name TEXT NOT NULL,              -- "Illinois River at Kingston Mines, IL"
    latitude NUMERIC(10, 7) NOT NULL,     -- WGS84
    longitude NUMERIC(11, 7) NOT NULL,
    description TEXT,
    active BOOLEAN NOT NULL DEFAULT true,
    first_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Key Design Decisions:**
- **site_code as PK**: Natural key (USGS's identifier)
- **active flag**: Soft delete (preserve historical references)
- **Timestamps**: Track when stations added/modified
- **NOT NULL constraints**: Required fields cannot be omitted

**Current Data:** 8 stations (see [[Station Registry]])

### 2. usgs_raw.gauge_readings — Time Series Data

**Purpose:** Store actual hydrological measurements (discharge, stage)

```sql
CREATE TABLE usgs_raw.gauge_readings (
    id BIGSERIAL PRIMARY KEY,
    site_code VARCHAR(8) NOT NULL,
    parameter_code VARCHAR(5) NOT NULL,        -- 00060, 00065
    value NUMERIC(12, 4) NOT NULL,             -- Actual measurement
    unit VARCHAR(10) NOT NULL,                 -- ft, ft3/s
    qualifier VARCHAR(1) NOT NULL DEFAULT 'P', -- P=provisional, A=approved
    reading_time TIMESTAMPTZ NOT NULL,         -- Measurement timestamp
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_reading UNIQUE (site_code, parameter_code, reading_time)
);
```

**Critical Design Principles:**

#### ✅ Valid Measurements Only
- **NO NULL values** in `value` column
- **NO sentinel values** (-999999) stored
- **NO placeholder records** for offline stations
- Parser filters invalid data before INSERT
- See [[Data Storage Strategy|docs/DATA_STORAGE_STRATEGY.md]] for rationale

#### ✅ Prevent Duplicates
```sql
CONSTRAINT unique_reading UNIQUE (site_code, parameter_code, reading_time)
```
- Enables idempotent ingestion (safe to re-run)
- `ON CONFLICT DO NOTHING` for incremental updates
- Prevents double-counting in analytics

#### ✅ Preserve Measurement Time vs Ingestion Time
- `reading_time`: When USGS measured the water
- `ingested_at`: When we stored it in our DB
- Enables staleness detection
- Supports backfill without losing provenance

**Storage Estimates:**
- **DV historical**: ~508,080 daily records (87 years × 8 sites × 2 params)
- **IV recent**: ~184,320 15-min records (120 days × 8 sites × 2 params)
- **Total initial load**: ~692,400 readings
- **Daily growth**: ~1,152 new readings (8 sites × 2 params × 96 readings/day)

### 3. usgs_raw.monitoring_state — Staleness Tracking

**Purpose:** Track polling metadata and station health (separate from measurements)

```sql
CREATE TABLE usgs_raw.monitoring_state (
    site_code VARCHAR(8),
    parameter_code VARCHAR(5),
    
    -- Polling metadata
    last_poll_attempted TIMESTAMPTZ,
    last_poll_succeeded TIMESTAMPTZ,
    last_data_received TIMESTAMPTZ,
    
    -- Latest reading details
    latest_reading_time TIMESTAMPTZ,
    latest_reading_value NUMERIC(12, 4),
    
    -- Health tracking
    consecutive_failures INTEGER DEFAULT 0,
    status VARCHAR(20) DEFAULT 'active',  -- active, degraded, offline
    status_since TIMESTAMPTZ DEFAULT NOW(),
    
    -- Staleness configuration
    is_stale BOOLEAN DEFAULT false,
    stale_since TIMESTAMPTZ,
    staleness_threshold_minutes INTEGER DEFAULT 60,
    
    PRIMARY KEY (site_code, parameter_code)
);
```

**Why Separate Table?**

This table tracks **absence of data**, not data itself:
- Records poll attempts even when no readings received
- Increments `consecutive_failures` on empty responses
- Maintains `latest_reading_time` (unchanged when offline)
- Enables staleness detection without querying time series

**Status State Machine:**
```
active → degraded → offline
  ↑         ↓          ↓
  └─────────┴──────────┘
      (fresh data received)
```

- **active**: Fresh data (age < threshold)
- **degraded**: Stale but exists (age > threshold)
- **offline**: No data in recent polls

See [[Staleness Tracking]] for complete implementation.

### 4. public.latest_readings — Materialized View

**Purpose:** Fast access to most recent value per station (for dashboards)

```sql
CREATE MATERIALIZED VIEW public.latest_readings AS
WITH ranked_readings AS (
    SELECT 
        gr.site_code,
        gr.parameter_code,
        gr.value,
        gr.reading_time,
        s.site_name,
        ROW_NUMBER() OVER (
            PARTITION BY gr.site_code, gr.parameter_code 
            ORDER BY gr.reading_time DESC
        ) as rn
    FROM usgs_raw.gauge_readings gr
    INNER JOIN usgs_raw.sites s ON gr.site_code = s.site_code
    WHERE s.active = true
      AND gr.reading_time > NOW() - INTERVAL '6 hours'
)
SELECT * FROM ranked_readings WHERE rn = 1;

CREATE UNIQUE INDEX idx_latest_readings_site_param 
    ON public.latest_readings(site_code, parameter_code);
```

**Refresh Strategy:**
```sql
-- Call every 15 minutes (after ingestion)
REFRESH MATERIALIZED VIEW CONCURRENTLY public.latest_readings;
```

**Why Materialized?**
- ✅ Instant dashboard queries (no complex CTEs)
- ✅ Pre-computed ROW_NUMBER() window function
- ✅ Consistent snapshot for all concurrent readers
- ✅ CONCURRENTLY allows reads during refresh

## Indexing Strategy

### Performance Indexes

```sql
-- Most recent readings for a site (used in alerts)
CREATE INDEX idx_gauge_readings_site_time 
    ON usgs_raw.gauge_readings(site_code, reading_time DESC);

-- Site + parameter queries (most common)
CREATE INDEX idx_gauge_readings_site_param_time 
    ON usgs_raw.gauge_readings(site_code, parameter_code, reading_time DESC);

-- Global time-based queries
CREATE INDEX idx_gauge_readings_time 
    ON usgs_raw.gauge_readings(reading_time DESC);
```

### Partial Index for Hot Data

```sql
-- Most queries focus on recent data
CREATE INDEX idx_gauge_readings_recent 
    ON usgs_raw.gauge_readings(site_code, parameter_code, reading_time DESC)
    WHERE reading_time > NOW() - INTERVAL '30 days';
```

**Why Partial?**
- 70% smaller than full index
- Covers 95% of queries (recent data)
- Faster updates (only recent rows)
- Automatically maintained by PostgreSQL

## Data Integrity Constraints

### Foreign Key Relationships

```sql
-- Ensure site exists before storing readings
ALTER TABLE usgs_raw.gauge_readings
    ADD CONSTRAINT fk_site_code 
    FOREIGN KEY (site_code) REFERENCES usgs_raw.sites(site_code);
```

**Why Not More FKs?**
- Parameter codes (00060, 00065) are USGS standards (no local table needed)
- Over-normalization adds complexity for minimal benefit
- Trust USGS to maintain parameter definitions

### Check Constraints

```sql
-- Latitude must be valid
ALTER TABLE usgs_raw.sites 
    ADD CONSTRAINT valid_latitude 
    CHECK (latitude BETWEEN -90 AND 90);

-- Longitude must be valid
ALTER TABLE usgs_raw.sites 
    ADD CONSTRAINT valid_longitude 
    CHECK (longitude BETWEEN -180 AND 180);

-- Value must be realistic (no sentinel values)
ALTER TABLE usgs_raw.gauge_readings
    ADD CONSTRAINT value_not_sentinel
    CHECK (value > -999000);  -- Sentinel is -999999
```

## Transaction Strategy

### Bulk Inserts with Transactions

```rust
// historical_ingest.rs
let mut transaction = client.transaction()?;

for reading in readings {
    transaction.execute(
        "INSERT INTO usgs_raw.gauge_readings (...) VALUES (...) 
         ON CONFLICT DO NOTHING",
        &[...]
    )?;
}

transaction.commit()?;  // All or nothing
```

**Why Transactions?**
- ✅ Atomic: All readings from a poll succeed or none do
- ✅ Consistent: No partial hourly batches
- ✅ Rollback on error: Failed API parse doesn't leave partial data
- ✅ Performance: Batched commits faster than individual INSERTs

### Idempotent Operations

```sql
-- Safe to run multiple times
INSERT INTO usgs_raw.gauge_readings (...)
VALUES (...)
ON CONFLICT (site_code, parameter_code, reading_time) 
DO NOTHING;
```

**Benefits:**
- Can re-run historical ingestion without duplicates
- Resumable after failures
- Safe for eventual consistency scenarios

## Maintenance & Optimization

### Auto-Vacuum Configuration

```sql
-- Aggressive vacuuming for high-volume table
ALTER TABLE usgs_raw.gauge_readings SET (
    autovacuum_vacuum_scale_factor = 0.05,  -- Vacuum at 5% change (vs 20% default)
    autovacuum_analyze_scale_factor = 0.02  -- Analyze at 2% change (vs 10% default)
);
```

**Why?**
- Table receives 1,152 inserts/day
- Keeps statistics fresh for query planner
- Prevents bloat from dead tuples

### Partitioning (Future)

**Not Currently Implemented** - single table sufficient for current scale

**When to Partition:**
- Table exceeds 10M rows (~14 years of 15-min data)
- Individual index exceeds 1GB
- Need to drop old data frequently

**Partitioning Strategy (when needed):**
```sql
-- Partition by year for easy archival
CREATE TABLE gauge_readings_2026 
    PARTITION OF gauge_readings
    FOR VALUES FROM ('2026-01-01') TO ('2027-01-01');
```

## Backup Strategy

### Point-in-Time Recovery (PITR)

**Recommended Configuration:**
```ini
# postgresql.conf
wal_level = replica
archive_mode = on
archive_command = 'cp %p /backup/wal/%f'
```

**Recovery Point Objective (RPO):** < 15 minutes  
**Recovery Time Objective (RTO):** < 1 hour

### pg_dump for Disaster Recovery

```bash
# Daily full backup
pg_dump -h localhost -U flopro_admin flomon_db \
    --format=custom \
    --file=/backup/flomon_$(date +%Y%m%d).dump

# Restore
pg_restore -h localhost -U flopro_admin -d flomon_db \
    /backup/flomon_20260219.dump
```

## Schema Migration Pattern

### Version-Controlled Migrations

```
sql/
├── 001_initial_schema.sql          # Sites + gauge_readings tables
├── 002_monitoring_metadata.sql     # Staleness tracking
└── 003_future_migration.sql        # Next schema change
```

**Migration Approach:**
- Sequential numbering (001, 002, 003...)
- Each file is idempotent (`CREATE TABLE IF NOT EXISTS`)
- Track applied migrations in `schema_version` table (future)
- Never modify existing migrations (create new ones)

---

**Related Pages:**
- [[Data Sources]] - What we're storing
- [[Staleness Tracking]] - monitoring_state table usage
- [[Technology Stack]] - Why PostgreSQL over alternatives
- [[Database Setup]] - Installation and initialization
