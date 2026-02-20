# FloPro Project Status

**Last Updated:** February 20, 2026  
**Project:** Flood Monitoring Service (flomon_service) - Peoria Illinois River  
**Language:** Rust | **Database:** PostgreSQL 18.2 | **LOC:** ~26,253

## Quick Context

Flood monitoring system for Peoria, IL area. Ingests USGS gauge data, NWS flood thresholds, and USACE lock/dam operations to provide early flood warnings.

## Database Status: ✅ PRODUCTION READY

**Connection:** TCP/IP authentication (not peer auth)
```bash
PGPASSWORD=flopro_dev_2026 psql -h localhost -U flopro_admin -d flopro_db
```

**DATABASE_URL:** `postgresql://flopro_admin:flopro_dev_2026@localhost/flopro_db` (in `.env`)

**Schemas:** usgs_raw, nws, usace, flood_analysis, public  
**Migrations:** 5 files in `sql/` (001-005)  
**Validation:** `./scripts/validate_db_setup.sh`

**Seeded Data:**
- 8 USGS monitoring sites
- 4 NWS flood thresholds
- Ready for historical backfill

## Test Status

**Integration Tests:** ✅ 9/9 passing (100%) - `cargo test --test peak_flow_integration`  
**Unit Tests:** ⚠️ 45/64 passing (70%) - `cargo test --lib`

**19 Failing Tests** (intentional - unimplemented stubs):
- 9 failures: `src/alert/stalenesses.rs` - staleness detection not implemented
- 9 failures: `src/analysis/groupings.rs` - site grouping not implemented  
- 1 failure: `src/analysis/flood_events.rs` - precursor detection incomplete

## Critical Unimplemented Functions

```rust
// src/alert/stalenesses.rs:47
unimplemented!("is_stale_at: parse datetime and compare against now")

// src/alert/thresholds.rs:36  
unimplemented!("check_flood_stage: compare reading.value against threshold levels")

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
| Alert - Thresholds | `alert/thresholds.rs` | 🔴 Stub | Comparison logic needed |
| Alert - Staleness | `alert/stalenesses.rs` | 🔴 Stub | Time-based checks needed |
| Analysis - Grouping | `analysis/groupings.rs` | 🔴 Stub | Site aggregation needed |
| Analysis - Events | `analysis/flood_events.rs` | 🟡 Partial | Schema exists, logic incomplete |
| Monitor | `monitor/mod.rs` | 🔴 Minimal | Polling loop not implemented |
| Main Service | `main.rs` | 🔴 Placeholder | Runtime not implemented |

## Available Binaries (src/bin/)

| Binary | Status | Purpose |
|--------|--------|---------|
| `historical_ingest` | ✅ Ready | Backfill USGS IV data (120 days max) |
| `ingest_peak_flows` | ✅ Ready | Import historical flood events from USGS |
| `ingest_cwms_historical` | ✅ Ready | Import USACE lock/dam data |
| `analyze_flood_events` | 📝 Planned | Event correlation analysis |
| `detect_backwater` | 📝 Planned | Mississippi backwater detection |

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

1. **Implement staleness detection** (`alert/stalenesses.rs`)
   - Parse ISO 8601 datetime with timezone
   - Compare against current time
   - Return boolean if reading exceeds threshold
   - Fixes: 9 unit tests

2. **Implement threshold checking** (`alert/thresholds.rs`)
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
- `docs/FLOOD_ANALYSIS.md` - Event analysis framework
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

- ✅ Fixed validation script for TCP/IP authentication
- ✅ Enhanced .gitignore for security (credentials, logs, state files)
- ✅ Updated DATABASE_SETUP.md with TCP/IP guidance
- ✅ Created QUICKSTART_DB.md for new users
- ✅ All 5 database migrations applied successfully
- ✅ Permissions granted to flopro_admin user
- ✅ 8 USGS sites seeded in database
