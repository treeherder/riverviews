# Pre-Ingestion Database Strategy

**Created:** February 19, 2026  
**Status:** Ready for review before historical data ingestion

## Executive Summary

Before loading 87 years of historical gauge data (1939–2026), we should add infrastructure to:

1. **Store NWS flood thresholds in the database** (currently hardcoded in Rust)
2. **Track historical flood events** as relational data
3. **Capture discharge-stage correlations** for multi-parameter flood detection
4. **Enable automatic flood event detection** during data ingestion

## Why This Matters

### Current State (Hardcoded in Rust)
```rust
// src/stations.rs
thresholds: Some(FloodThresholds {
    action_stage_ft: 14.0,
    flood_stage_ft: 16.0,
    moderate_flood_stage_ft: 20.0,
    major_flood_stage_ft: 24.0,
}),
```

**Problems:**
- Can't update thresholds without recompiling
- No historical record of threshold changes (NWS revises after major floods)
- No way to query "which stations have major flood thresholds below 20 ft?"
- Threshold verification requires code review, not SQL queries

### Proposed State (Database-Driven)
```sql
SELECT site_code, site_name, flood_stage_ft, major_flood_stage_ft
FROM nws.flood_thresholds t
JOIN usgs_raw.sites s USING (site_code)
WHERE major_flood_stage_ft < 23.0
ORDER BY flood_stage_ft;
```

**Benefits:**
- Dynamic threshold updates (no redeployment)
- Audit trail via `nws.flood_threshold_history`
- SQL-driven alerting and reporting
- Supports web dashboard queries

## Schema Additions

See [sql/003_flood_metadata.sql](../sql/003_flood_metadata.sql) for full implementation.

### 1. Flood Thresholds Table

```sql
CREATE TABLE nws.flood_thresholds (
    site_code VARCHAR(8) PRIMARY KEY,
    action_stage_ft NUMERIC(6, 2),      -- Prepare (e.g., 14 ft)
    flood_stage_ft NUMERIC(6, 2),       -- Minor flooding (16 ft)
    moderate_flood_stage_ft NUMERIC(6, 2),  -- Significant damage (20 ft)
    major_flood_stage_ft NUMERIC(6, 2),     -- Severe flooding (24 ft)
    effective_date DATE,
    last_verified TIMESTAMPTZ,
    notes TEXT
);
```

**Initialized with current values from stations.rs:**
- Kingston Mines (05568500): 14/16/20/24 ft
- Chillicothe (05568000): 13/15/19/23 ft  
- Henry (05557000): 13/15/19/22 ft
- Marseilles (05552500): 12/14/18/22 ft

### 2. Historical Flood Events Table

```sql
CREATE TABLE nws.flood_events (
    site_code VARCHAR(8),
    event_start TIMESTAMPTZ,      -- When stage exceeded flood threshold
    event_end TIMESTAMPTZ,         -- When stage dropped below threshold
    crest_time TIMESTAMPTZ,        -- Peak of flood
    peak_stage_ft NUMERIC(6, 2),   -- Maximum stage during event
    severity VARCHAR(20),          -- 'flood', 'moderate', 'major'
    event_name TEXT,               -- e.g., "May 2013 Historic Flood"
    notes TEXT
);
```

**Use Cases:**
- "How many times has Kingston Mines exceeded 20 ft since 1993?"
- "What was the average duration of major floods at Peoria?"
- "Show me all flood events where Marseilles crested 12 hours before Kingston Mines"
- Train ML models on pre-flood upstream patterns

### 3. Discharge Thresholds (Optional)

Some stations have both stage AND discharge thresholds:

```sql
CREATE TABLE nws.discharge_thresholds (
    site_code VARCHAR(8),
    flood_discharge_cfs NUMERIC(10, 2),  -- Flow rate producing flood stage
    moderate_flood_discharge_cfs NUMERIC(10, 2),
    -- ...
);
```

**Why This Matters:**  
If a station's stage sensor fails but discharge is working, we can still detect flooding by flow rate.

## Ingestion Strategy

### Phase 1: Schema Migration (NOW)
```bash
psql -U your_user -d flomon < sql/003_flood_metadata.sql
```

Adds tables and initializes thresholds from current Rust code.

### Phase 2: Historical Data Ingestion
```bash
cargo run --bin historical_ingest
```

Loads 87 years of gauge readings into `usgs_raw.gauge_readings`.  
**This takes several hours and stores millions of rows.**

### Phase 3: Flood Event Detection (AFTER INGESTION)
```sql
-- Example: Detect all times Kingston Mines exceeded 16 ft for 6+ hours
SELECT 
    MIN(reading_time) as flood_start,
    MAX(reading_time) as flood_end,
    MAX(value) as peak_stage_ft
FROM usgs_raw.gauge_readings
WHERE site_code = '05568500' 
  AND parameter_code = '00065'  -- stage
  AND value >= 16.0  -- flood threshold
GROUP BY date_trunc('day', reading_time)  -- Simplified grouping
HAVING MAX(value) >= 16.0;
```

We can write a PL/pgSQL function to automate this and populate `nws.flood_events`.

### Phase 4: Validation & Analysis
```sql
-- How many flood events per station?
SELECT * FROM nws.flood_event_summary;

-- Compare upstream/downstream timing
SELECT 
    e1.site_code as upstream,
    e2.site_code as downstream,
    e1.crest_time as upstream_crest,
    e2.crest_time as downstream_crest,
    EXTRACT(HOUR FROM (e2.crest_time - e1.crest_time)) as lag_hours
FROM nws.flood_events e1
JOIN nws.flood_events e2 
  ON DATE(e1.crest_time) = DATE(e2.crest_time)
WHERE e1.site_code = '05552500'  -- Marseilles
  AND e2.site_code = '05568500'  -- Kingston Mines
  AND e1.peak_stage_ft > 14  -- Marseilles exceeded flood stage
ORDER BY e1.crest_time;
```

## Data Sources for NWS Thresholds

### Official NWS AHPS Pages (Verified Feb 2026)

- **Kingston Mines:** https://water.weather.gov/ahps2/hydrograph.php?wfo=ilx&gage=kini2
- **Chillicothe:** https://water.weather.gov/ahps2/hydrograph.php?wfo=ilx&gage=chti2  
- **Henry:** https://water.weather.gov/ahps2/hydrograph.php?wfo=ilx&gage=heni2
- **Marseilles:** https://water.weather.gov/ahps2/hydrograph.php?wfo=lot&gage=mrsi2

Each page shows:
- Current stage vs. flood stage graphic
- Official action/flood/moderate/major thresholds
- Historical crest data (highest floods on record)
- Flood impact statements (what happens at each stage)

### Verification Process

1. Visit NWS AHPS page for each station
2. Record thresholds from "Flood Categories" table
3. Cross-reference with current values in [stations.rs](../src/stations.rs)
4. Note any discrepancies (may indicate threshold revision)
5. Update `sql/003_flood_metadata.sql` with official values
6. Document source and verification date

## Historical Flood Research

### Known Major Events (for validation)

Research these events to populate `nws.flood_events` manually for verification:

1. **May 2013 - Record Flood**
   - Kingston Mines crested near 29 ft (record)
   - Caused by prolonged spring rainfall + snowmelt
   - Peoria area evacuations, I-474 closed

2. **Spring 2019 - Prolonged Flood**
   - Extended high water throughout basin
   - Agricultural impacts, levee stress

3. **1993 - Great Midwest Flood**
   - Historic Mississippi/Illinois River flood
   - Kingston Mines likely exceeded 25 ft

4. **1943-1945 - War Years Floods**
   - Multiple events in USGS historical record
   - Pre-dam regulation era

### Research Sources

- **NWS Event Database:** https://water.weather.gov/ahps2/crests.php?gage=kini2
- **USGS Flood Peak Database:** https://nwis.waterdata.usgs.gov/usa/nwis/peak
- **Local Historical Societies:** Peoria Historical Society archives
- **News Archives:** Peoria Journal Star flood coverage

## Migration Checklist

- [ ] Review [sql/003_flood_metadata.sql](../sql/003_flood_metadata.sql)
- [ ] Verify NWS threshold values against official AHPS pages
- [ ] Update INSERT statements with verified thresholds
- [ ] Run migration: `psql -d flomon < sql/003_flood_metadata.sql`
- [ ] Verify tables created: `\dt nws.*`
- [ ] Check threshold data: `SELECT * FROM nws.flood_thresholds;`
- [ ] Run historical ingestion: `cargo run --bin historical_ingest`
- [ ] Implement flood event detection function (Phase 3)
- [ ] Populate `nws.flood_events` from gauge readings
- [ ] Refresh materialized view: `REFRESH MATERIALIZED VIEW nws.flood_event_summary;`
- [ ] Validate against known historical floods (May 2013, etc.)

## Code Changes Required

### 1. Update Rust threshold loading

**Before (hardcoded):**
```rust
// src/stations.rs
pub thresholds: Option<FloodThresholds>,
```

**After (database-backed):**
```rust
// src/alert/thresholds.rs
pub fn load_thresholds_from_db(conn: &postgres::Client) -> HashMap<String, FloodThresholds> {
    let rows = conn.query(
        "SELECT site_code, action_stage_ft, flood_stage_ft, 
                moderate_flood_stage_ft, major_flood_stage_ft
         FROM nws.flood_thresholds",
        &[]
    ).unwrap();
    
    rows.iter().map(|row| {
        let site_code: String = row.get(0);
        let thresholds = FloodThresholds {
            action_stage_ft: row.get(1),
            flood_stage_ft: row.get(2),
            moderate_flood_stage_ft: row.get(3),
            major_flood_stage_ft: row.get(4),
        };
        (site_code, thresholds)
    }).collect()
}
```

### 2. Cache thresholds in monitoring service

```rust
// src/monitor/mod.rs
pub struct MonitoringCache {
    // ... existing fields ...
    flood_thresholds: HashMap<String, FloodThresholds>,
}

impl MonitoringCache {
    pub fn refresh_from_db(&mut self, conn: &mut postgres::Client) {
        // Load monitoring_state (already implemented)
        // ...
        
        // Load flood thresholds
        self.flood_thresholds = load_thresholds_from_db(conn);
    }
}
```

### 3. Optional: Keep stations.rs thresholds as fallback

```rust
// src/stations.rs - keep for offline testing
pub thresholds: Option<FloodThresholds>,  // Fallback if DB unavailable
```

Application startup:
1. Try to load from database
2. If DB connection fails, use hardcoded fallback
3. Log warning if using fallback values

## Benefits Summary

### Before (Hardcoded)
- Thresholds in Rust code only
- No historical flood record
- Can't answer "how often does X flood?"
- Threshold changes require recompile/redeploy

### After (Database-Driven)
- ✅ Dynamic threshold updates via SQL
- ✅ Historical flood event analysis
- ✅ Upstream/downstream correlation queries
- ✅ Training data for ML models
- ✅ Web dashboard: "Show flood history for this station"
- ✅ Audit trail of threshold changes
- ✅ Discharge-based flood detection (if stage sensor fails)

## Recommendation

**YES - Add this infrastructure before ingestion.**

**Rationale:**
1. It's much easier to add tables BEFORE loading millions of rows
2. We can detect flood events DURING ingestion (one pass through data)
3. Historical flood analysis will validate our threshold values
4. This data will be invaluable for ML training and trend analysis
5. The migration is low-risk (adds tables, doesn't modify existing schema)

**Timeline:**
- Schema migration: 5 minutes
- Threshold verification: 30 minutes (visit NWS AHPS pages)
- Historical ingestion: 2-4 hours (unchanged)
- Flood event detection: 1-2 hours (run after ingestion)

**Total delay: ~4-6 hours, provides years of value**

