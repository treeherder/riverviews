# FloPro Project Status

**Last Updated:** February 20, 2026  
**Project:** Flood Monitoring Service (flomon_service) - Peoria Illinois River  
**Architecture:** Rust daemon + Python analysis  
**Language:** Rust (daemon) | Python (analysis - planned) | **Database:** PostgreSQL 18.2

## Quick Context

Flood monitoring system for Peoria, IL area. **Rust daemon** handles data ingestion, curation, and simple threshold monitoring. **Python scripts** (planned) handle complex statistical analysis, regression, and ML modeling.

**Refactoring (Feb 20, 2026):** Separated complex analysis from Rust codebase. Daemon now focuses on data curation and simple threshold monitoring. Python FloML package handles statistical analysis and ML. See [docs/REFACTORING_PLAN.md](flomon_service/docs/REFACTORING_PLAN.md).

## Database Status: ✅ PRODUCTION READY

**Connection:** TCP/IP authentication (not peer auth)
```bash
PGPASSWORD=flopro_dev_2026 psql -h localhost -U flopro_admin -d flopro_db
```

**DATABASE_URL:** `postgresql://flopro_admin:flopro_dev_2026@localhost/flopro_db` (in `.env`)

**Schemas:** usgs_raw, nws, usace, flood_analysis (for Python output), monitoring, public  
**Migrations:** 5 files in `sql/` (001-005)  
**Validation:** `./scripts/validate_db_setup.sh`

**Seeded Data:**
- 8 USGS monitoring sites
- 4 NWS flood thresholds
- Ready for historical backfill

## Test Status

**Integration Tests:** ✅ 9/9 passing (100%) - `cargo test --test peak_flow_integration`  
**Unit Tests:** ⚠️ Some failing (intentional - unimplemented stubs for monitoring features)

**Unimplemented Features:**
- Staleness detection - time-based data freshness checks
- Site grouping - organizing readings by station

## Critical Unimplemented Functions

```rust
// src/alert/stalenesses.rs:47
unimplemented!("is_stale_at: parse datetime and compare against now")

// src/analysis/groupings.rs:34
unimplemented!("group_by_site: partition readings into per-site structs")
```

**Impact:** These 3 functions block alert logic and dashboard functionality. Estimated ~250 LOC to implement all three.

## Module Implementation Status

| Module | File | Status | Notes |
|--------|------|--------|-------|
| Core Models | `model.rs` | ✅ Complete | GaugeReading, FloodThresholds, NwisError |
| Site Registry | `stations.rs` | ✅ Complete | Loads from `stations.toml` |
| Database Utils | `db.rs` | ✅ Complete | Connection helpers |
| USGS Ingest | `ingest/usgs.rs` | ✅ Complete | NWIS IV API parser |
| USACE Ingest | `ingest/cwms.rs` | ✅ Complete | CWMS API integration |
| Peak Flow Ingest | `ingest/peak_flow.rs` | ✅ Complete | NWS historical events |
| Alert - Thresholds | `alert/thresholds.rs` | ✅ Complete | Simple threshold checking |
| Alert - Staleness | `alert/stalenesses.rs` | 🔴 Stub | Time-based checks needed |
| Analysis - Grouping | `analysis/groupings.rs` | 🔴 Stub | Site aggregation needed |
| Main Daemon | `main.rs` | 🟡 Skeleton | Runtime loop not implemented |

## Available Binaries (src/bin/)

| Binary | Status | Purpose |
|--------|--------|---------|
| `historical_ingest` | ✅ Ready | Backfill USGS IV data (120 days max) |
| `ingest_peak_flows` | ✅ Ready | Import historical flood events from USGS |
| `ingest_cwms_historical` | ✅ Ready | Import USACE lock/dam data |
| `detect_backwater` | ✅ Ready | Simple Mississippi backwater check |

**Note:** Complex analysis moved to Python (floml package)

## Data Sources

1. **USGS NWIS IV API** - 15-minute gauge readings (discharge, stage)
2. **USGS Peak Flow** - Historical flood events (RDB format)
3. **NWS AHPS** - Flood stage thresholds (manually configured)
4. **USACE CWMS** - Lock/dam operations, pool levels

## Key Configuration Files

- `stations.toml` - Site registry with coordinates, thresholds, travel times
- `.env` - Database credentials (gitignored)
- `Cargo.toml` - Dependencies: postgres, reqwest, chrono, rust_decimal, serde, toml

## Next Implementation Steps (Priority Order)

### Rust Daemon (Core Monitoring)

1. **Implement staleness detection** (`alert/stalenesses.rs`)
   - Parse ISO 8601 datetime with timezone
   - Compare against current time
   - Return boolean if reading exceeds threshold

2. **Implement threshold checking** (`alert/thresholds.rs`)
   - Compare reading value against NWS thresholds
   - Return AlertLevel enum (Normal/Action/Flood/Moderate/Major)

3. **Implement site grouping** (`analysis/groupings.rs`)
   - Group flat readings by site_code
   - Associate discharge (00060) and stage (00065) parameters
   - Return HashMap<String, SiteReadings>

4. **Build monitoring runtime** (`main.rs`)
   - 15-minute polling loop
   - Call USGS API for all sites
   - Store readings in database
   - Check thresholds and generate simple alerts
   - Track data staleness

### Python Analysis (Complex Statistics)

5. **Set up Python environment**
   ```bash
   mkdir python_analysis
   cd python_analysis
   python -m venv venv
   pip install pandas numpy scipy scikit-learn psycopg2-binary sqlalchemy
   ```

6. **Create first analysis script**
   - Connect to PostgreSQL
   - Read historical flood events
   - Perform precursor pattern detection
   - Write results to flood_analysis schema

7. **Implement regression models**
   - Upstream-downstream correlation
   - Stage-discharge relationships
   - Backwater influence scoring

See [docs/PYTHON_INTEGRATION.md](flomon_service/docs/PYTHON_INTEGRATION.md) for analysis architecture.
   - Compare reading value against NWS thresholds
   - Return AlertLevel enum (Normal/Action/Flood/Moderate/Major)
   - Fixes: 2 unit tests

3. **Implement site grouping** (`analysis/groupings.rs`)
   - Group flat readings by site_code
   - Associate discharge (00060) and stage (00065) parameters
   - Return HashMap<String, SiteReadings>
   - Fixes: 8 unit tests

4. **Historical data backfill**
   ```bash
   cargo run --bin historical_ingest
   cargo run --bin ingest_peak_flows
   ```

5. **Build monitoring runtime**
   - 15-minute polling loop
   - Call USGS API for all sites
   - Store readings in database
   - Check thresholds and generate alerts
   - Refresh materialized views

## Common Commands

```bash
# Database validation
cd flomon_service && ./scripts/validate_db_setup.sh

# Run all tests
cargo test --lib --no-fail-fast
cargo test --test peak_flow_integration

# Check unimplemented functions
grep -r "unimplemented!" src/ --include="*.rs"

# Connect to database
PGPASSWORD=flopro_dev_2026 psql -h localhost -U flopro_admin -d flopro_db

# Run specific binary
cargo run --bin historical_ingest
```

## Architecture Notes

**Multi-schema design:**
- `usgs_raw` - Raw USGS data (never deleted)
- `nws` - NWS forecasts/thresholds (updated periodically)
- `usace` - USACE operational data
- `flood_analysis` - Derived analytics
- `public` - Unified views for dashboard

**Materialized view:** `public.latest_readings` - Most recent reading per site/parameter (refresh every 15 min)

**Authentication:** Uses TCP/IP (`-h localhost`) not Unix sockets to avoid peer auth issues

## Known Issues / Decisions

1. **Edition 2024** in Cargo.toml - May need adjustment if building on older Rust
2. **Peoria pool threshold test failing** - Assertion in `config.rs:159` about Peoria pool not having thresholds (intentional)
3. **Unused imports** - Some warnings in compilation (non-critical)
4. **No runtime scheduler** - Main service doesn't poll yet
5. **No alert delivery** - Alert logic exists but not wired to notification system

## Documentation

- `QUICKSTART_DB.md` - Quick database setup
- `docs/DATABASE_SETUP.md` - Complete database guide
- `docs/SCHEMA_EXTENSIBILITY.md` - Schema design principles
- `docs/CWMS_INTEGRATION.md` - USACE integration details
- `docs/PYTHON_INTEGRATION.md` - Python analysis architecture (**NEW**)
- `docs/REFACTORING_PLAN.md` - Rust-Python separation plan (**NEW**)
- `docs/FLOOD_ANALYSIS.md` - Event analysis framework (archived - moved to Python)
- `scripts/README.md` - Validation script details

## Security

- `.env` excluded from git (contains DB password)
- `.gitignore` covers credentials, logs, state files, backups
- Database uses password auth over TCP/IP
- No hardcoded secrets in source

## Dependencies (Key)

- `postgres` 0.19 (with chrono support)
- `reqwest` 0.11 (with rustls-tls, no OpenSSL)
- `chrono` 0.4 (datetime handling)
- `rust_decimal` 1.33 (PostgreSQL NUMERIC support)
- `serde/serde_json` 1.0 (JSON parsing)
- `toml` 0.8 (config parsing)

## Project Maturity

| Layer | Status | Ready for Production |
|-------|--------|---------------------|
| Database | ✅ Complete | Yes |
| Data Ingestion | ✅ Complete | Yes |
| Configuration | ✅ Complete | Yes |
| Alert Logic | 🔴 Stubbed | No |
| Monitoring Loop | 🔴 Not Started | No |
| Dashboard | 🔴 Not Started | No |

**Estimated completion to MVP:** 20-30 hours (implement 3 stubs + runtime loop + basic alerting)

## Agent Quick Start Checklist

When resuming work on this project:

1. ✅ Verify database is running: `pg_isready -h localhost`
2. ✅ Check connection: `PGPASSWORD=flopro_dev_2026 psql -h localhost -U flopro_admin -d flopro_db -c "\dt usgs_raw.*"`
3. ✅ Run tests to see current state: `cargo test --lib 2>&1 | grep "test result"`
4. ⚠️ Remember 3 critical stubs block 19 tests
5. 📝 Check `plans.md` for user's current focus areas
6. 🔧 Common workflow: Implement stub → Run tests → Repeat

## Recent Changes (Feb 20, 2026)

**Refactoring - Rust-Python Separation:**
- ✅ Removed `src/bin/analyze_flood_events.rs` (complex analysis → Python)
- ✅ Removed `src/analysis/flood_events.rs` (statistical modeling → Python)
- ✅ Simplified `src/analysis/mod.rs` (focus on data grouping only)
- ✅ Updated `main.rs` with daemon architecture outline
- ✅ Created `docs/REFACTORING_PLAN.md` - migration strategy
- ✅ Created `docs/PYTHON_INTEGRATION.md` - analysis architecture
- ✅ Updated README.md and PROJECT_STATUS.md

**Database Setup:**
- ✅ Fixed validation script for TCP/IP authentication
- ✅ Enhanced .gitignore for security (credentials, logs, state files)
- ✅ Updated DATABASE_SETUP.md with TCP/IP guidance
- ✅ Created QUICKSTART_DB.md for new users
- ✅ All 5 database migrations applied successfully
- ✅ Permissions granted to flopro_admin user
- ✅ 8 USGS sites seeded in database
