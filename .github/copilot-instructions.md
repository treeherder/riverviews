# Riverviews — Flood Monitoring System

Generalized flood monitoring with two components: **flomon_service** (Rust daemon) and **floml** (Python analysis).

## Architecture

```
flomon_service/   — Rust ingest daemon + HTTP API
floml/            — Python ML analysis package
riverviews.wiki/  — Detailed architecture docs
```

**flomon_service** polls USGS NWIS IV, USACE CWMS, and IEM/ASOS APIs on 15-minute intervals, stores readings in PostgreSQL, and exposes a zone-based HTTP API on port 8080 (`/zones`, `/zone/{id}`, `/status`, `/backwater`, `/health`).

**floml** connects to the same PostgreSQL database for historical analysis (segmented regression, cross-station correlation, precursor detection). Scripts also consume the flomon_service HTTP endpoint for live data.

See [README.md](../README.md) for full project structure. Architecture details in [flomon_service/docs/](../flomon_service/docs/).

## Build & Test

### flomon_service (Rust, Edition 2024)

```bash
# Run from flomon_service/
cargo build --release
cargo run --release -- --endpoint 8080          # Start daemon
cargo run --release -- verify                   # Verify data sources

# Tests require DB + network (hit live APIs)
cargo test --test data_source_verification
cargo test --test daemon_lifecycle
./scripts/validate_db_setup.sh                  # Verify DB prerequisites
```

**TOML config files must be in cwd** when running — missing files panic intentionally.

### floml (Python 3.8+)

```bash
# Run from floml/
source venv/bin/activate
pip install -r requirements.txt
python floml/db.py                              # Verify DB connection
python scripts/analyze_events.py --site-code 05568500
```

### Database Setup (required by both)

```bash
# Apply migrations in order
psql -U flopro_admin -d flopro_db -f flomon_service/sql/001_initial_schema.sql
# ... through 006_iem_asos.sql
```

`.env` file required in each component directory:
```
DATABASE_URL=postgresql://flopro_admin:<password>@localhost/flopro_db
```

**Note:** The compiled binary does NOT auto-load `.env` — pass `DATABASE_URL` as an env var explicitly when running outside `cargo run`.

## Conventions

### Rust (flomon_service)

- **No async** — blocking `reqwest`, `std::thread`, `tiny_http`. If one API call hangs, the entire polling loop stalls.
- **Error handling**: custom `DbConfigError` enum in `db.rs` with remediation recipes; `NwisError` enum in `model.rs`; `Box<dyn Error>` at daemon recovery points. Missing config files panic intentionally.
- **Logging**: custom `logging.rs` (not `tracing`/`log` crates). Use `LogLevel` and `DataSource` enums; logs to both console and `./flomon_service.log`.
- **Zones**: hardcoded `zone_0`…`zone_6` in `zones.rs`. Adding `zone_7+` requires code changes to deserialize dynamically.
- **CWMS datum**: USGS uses NAVD88; CWMS uses NGVD29; ASOS uses local datum. Always document datum when setting thresholds.
- **Priority keywords** in TOML `relevance` fields control poll intervals: `PRIMARY`/`CRITICAL` → 15 min, `HIGH`/`UPSTREAM` → 60 min, `EXTENDED`/`CONFLUENCE` → 6 hr, default → daily. See [TOML_CONFIGURATION.md](../flomon_service/docs/TOML_CONFIGURATION.md).
- Prefer soft failures (log + continue) for API errors; hard panics only for config loading.

### Python (floml)

- Type hints used throughout; dataclass result objects (`RegressionResult`, `CorrelationResult`, `PrecursorEvent`).
- `DATABASE_URL` via `.env` loaded with `python-dotenv`; raises `ValueError` if missing.
- Call `verify_schemas()` on startup to assert required schemas exist.
- Correlation analysis requires ≥10 overlapping data points; precursor detection requires hourly frequency.

## Key Pitfalls

- **Integration tests hit live APIs** (USGS, CWMS, IEM) — can flap when external services degrade.
- **CWMS timeseries discovery fails silently** — if a location name doesn't match the CWMS catalog, polling is skipped without error. Verify manually:  
  `curl "https://cwms-data.usace.army.mil/cwms-data/catalog/TIMESERIES?office=MVR&like=Peoria.*&format=json"`
- **Wicket dam loss of pool control**: When Mississippi floods cause pool loss at Peoria (pool ≈ tailwater), pool elevation becomes meaningless — tailwater is the primary indicator. Special detection logic in `ingest/cwms.rs`.
- **USGS sentinel values**: `-999999` readings are treated as "no data"; don't store or alert on them.
- **floml scripts require flomon_service running** on `http://localhost:8080` for live visualizations (`demo_correlation.py`, `visualize_zones.py`, `zone_dashboard.py`).

## Documentation Index

| Reference | Topic |
|-----------|-------|
| [ASOS_IMPLEMENTATION.md](../flomon_service/docs/ASOS_IMPLEMENTATION.md) | IEM/ASOS integration, precipitation thresholds, lag times |
| [CWMS_INTEGRATION_SUMMARY.md](../flomon_service/docs/CWMS_INTEGRATION_SUMMARY.md) | USACE pool/tailwater retrieval, backwater detection |
| [DATA_SOURCE_ORGANIZATION.md](../flomon_service/docs/DATA_SOURCE_ORGANIZATION.md) | Multi-schema layout, data lineage, freshness tracking |
| [DATA_SOURCE_VERIFICATION.md](../flomon_service/docs/DATA_SOURCE_VERIFICATION.md) | Verification suite and connectivity testing |
| [LOGGING_AND_ERROR_HANDLING.md](../flomon_service/docs/LOGGING_AND_ERROR_HANDLING.md) | Error strategy, staleness thresholds, remediation recipes |
| [STATION_RESILIENCE.md](../flomon_service/docs/STATION_RESILIENCE.md) | Redundancy patterns, backfill strategy, graceful degradation |
| [TOML_CONFIGURATION.md](../flomon_service/docs/TOML_CONFIGURATION.md) | Adding stations/zones, priority keywords, runtime discovery |
| [floml/QUICKSTART.md](../floml/QUICKSTART.md) | FloML setup and first analysis walkthrough |
| [flomon_service/tests/README.md](../flomon_service/tests/README.md) | Integration test prerequisites and migration order |
| [riverviews.wiki/](../riverviews.wiki/) | Peak flow analysis, staleness tracking, DB architecture |
