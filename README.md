# Riverviews — Flood Monitoring System

Real-time flood monitoring with two components: **flomon_service** (Rust daemon) and **floml** (Python analysis). The reference implementation monitors the Illinois River basin (Peoria, IL) with 7 hydrological zones, 24 sensors, and lead times from 0–120 hours.

For official flood warnings always consult the [National Weather Service](https://www.weather.gov) and local emergency management. This is a personal project, not a certified warning system.

---

## Project Structure

```
flomon_service/   — Rust ingest daemon + HTTP API (port 8080)
floml/            — Python analysis and terminal dashboard
riverviews.wiki/  — Architecture and implementation docs
```

## Zones — Illinois River Reference Implementation

| Zone | Name | Lead Time | Sensors | Role |
|------|------|-----------|---------|------|
| **0** | Mississippi River | 12h–5d | Grafton, Alton, Hannibal (USACE) | Backwater source |
| **1** | Lower Illinois | 6–24h | LaGrange L&D pool/tailwater, Spoon River, KSPI precip | Backwater interface |
| **2** | Upper Peoria Lake | 0–6h | Peoria L&D, Kingston Mines, KPIA precip, gridded precip | **Property zone** |
| **3** | Local Tributaries | 6–18h | Mackinaw River, KBMI precip, gridded precip | Tributary monitoring |
| **4** | Mid Illinois | 18–48h | Starved Rock L&D, Marseilles, Henry, Vermilion River | Upstream propagation |
| **5** | Upper Illinois | 36–72h | Dresden Island L&D, Kankakee, Des Plaines | Confluence monitoring |
| **6** | Chicago CAWS | 3–5d | Lockport, Brandon Road, CSSC, KORD/KPWK precip | Lake Michigan drainage |

Flood types: **top-down** (zones 4–6 elevated), **bottom-up** (zone 0 backwater), **local tributary** (zone 3), **compound** (multiple zones).

---

## HTTP API

| Endpoint | Description |
|----------|-------------|
| `GET /zones` | All zones with metadata |
| `GET /zone/{id}` | Zone detail with sensor readings |
| `GET /status` | Overall basin status, backwater risk, upstream pulse |
| `GET /backwater` | Grafton stage, LaGrange differential, pool-loss detection |
| `GET /health` | Service health check |

See [riverviews.wiki/Zone-Based-API.md](riverviews.wiki/Zone-Based-API.md) for response schemas.

---

## Build & Run

### flomon_service (Rust, Edition 2024)

```bash
cd flomon_service
cargo build --release
cargo run --release -- --endpoint 8080    # start daemon
cargo run --release -- verify             # verify data sources
```

TOML config files must be present in cwd. A `.env` with `DATABASE_URL` is required when running outside `cargo run`.

### floml (Python 3.8+)

```bash
cd floml
source venv/bin/activate
pip install -r requirements.txt
python floml/db.py                        # verify DB connection
python scripts/zone_dashboard.py          # terminal dashboard
python scripts/zone_dashboard.py --api http://HOST:8080  # remote
```

### Docker (recommended for deployment)

```bash
cp .env.example .env          # fill in passwords
docker compose up -d
docker compose logs -f flomon_service
```

### Database Migrations

```bash
psql -U postgres -d flopro_db -f flomon_service/sql/001_initial_schema.sql
# ... through 006_iem_asos.sql
```

---

## Documentation

| File | Topic |
|------|-------|
| [riverviews.wiki/Zone-Based-API.md](riverviews.wiki/Zone-Based-API.md) | HTTP API endpoints and response schemas |
| [riverviews.wiki/Data-Sources.md](riverviews.wiki/Data-Sources.md) | USGS, USACE CWMS, IEM/ASOS — what each provides |
| [riverviews.wiki/Illinois-River-Implementation.md](riverviews.wiki/Illinois-River-Implementation.md) | Configured stations, operational status, thresholds |
| [riverviews.wiki/Database-Architecture.md](riverviews.wiki/Database-Architecture.md) | Schema design, multi-schema layout |
| [riverviews.wiki/Staleness-Tracking.md](riverviews.wiki/Staleness-Tracking.md) | Data freshness monitoring |
| [riverviews.wiki/Peak-Flow-Analysis.md](riverviews.wiki/Peak-Flow-Analysis.md) | Historical flood event characterization |
| [flomon_service/docs/TOML_CONFIGURATION.md](flomon_service/docs/TOML_CONFIGURATION.md) | Adding stations and zones |
| [flomon_service/docs/ASOS_IMPLEMENTATION.md](flomon_service/docs/ASOS_IMPLEMENTATION.md) | Precipitation monitoring and thresholds |
| [flomon_service/docs/CWMS_INTEGRATION_SUMMARY.md](flomon_service/docs/CWMS_INTEGRATION_SUMMARY.md) | USACE CWMS catalog discovery and status |
| [flomon_service/docs/STATION_RESILIENCE.md](flomon_service/docs/STATION_RESILIENCE.md) | Failure handling, backfill strategy |
| [flomon_service/docs/LOGGING_AND_ERROR_HANDLING.md](flomon_service/docs/LOGGING_AND_ERROR_HANDLING.md) | Log levels, error strategy |
| [flomon_service/docs/DATA_SOURCE_VERIFICATION.md](flomon_service/docs/DATA_SOURCE_VERIFICATION.md) | Verification test suite |
| [floml/QUICKSTART.md](floml/QUICKSTART.md) | FloML setup and first analysis |
| [TODO.md](TODO.md) | Open work items |
