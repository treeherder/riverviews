# Riverviews — Open Work Items

---

## ✅ Completed

- Zone-based HTTP API (`/zones`, `/zone/{id}`, `/status`, `/backwater`, `/health`)
- USGS NWIS ingest (real-time IV every 15min + startup backfill up to 120 days IV)
- USACE CWMS ingest with runtime catalog discovery (pool, tailwater, stage)
- IEM/ASOS precipitation ingest with basin assignment (hourly, all weather fields)
- 7-zone hydrological model (Illinois River reference implementation)
- Flood type classification (top-down, bottom-up, local tributary, compound)
- Backwater detection (Grafton stage + LaGrange pool/tailwater differential)
- Upstream flood pulse detection
- Terminal ncurses dashboard (`floml/scripts/zone_dashboard.py`)
- Docker deployment stack (postgres + flomon_service)
- CI/CD pipeline (GitHub Actions + self-hosted runner on GCE)
- PostgreSQL multi-schema design (usgs_raw, usace, noaa, monitoring)
- Custom logging system with `LogLevel` and `DataSource` enums
- Data staleness tracking (hybrid DB + in-memory)
- Integration test suites (ASOS, USGS, CWMS, daemon lifecycle, peak flow)
- Dockerfile healthcheck (curl-based)

## Ingest Bugs & Known Gaps

- **ASOS: no startup backfill** — `backfill_asos_station()` is defined in `daemon.rs` but never called from `main.rs`; ASOS data only accumulates forward from first run; fix: add ASOS staleness check + backfill call in `main.rs` startup sequence alongside USGS/CWMS
- **USGS: DV history not fetched** — `backfill_days: 120` (default) only fetches IV data; to get years of daily history, increase `backfill_days > 120` or add a one-time historical ingest CLI subcommand; the "87 years" history claim requires a manual config change or separate tool
- **CWMS staleness check uses `location_id`** — `check_cwms_staleness()` queries all timeseries for a location in aggregate; if pool has fresh data but tailwater is empty, tailwater won't get backfilled on startup
- **`asos_precip_summary` table is never populated** — schema defines 6h/12h/24h/48h pre-computed rolling totals, but no code ever inserts into it; `/zone/{id}` now correctly sums raw `asos_observations` directly instead
- **`IemAsosMinute` (1-min precip) struct unused** — the 1-minute ASOS data structure exists in `ingest/iem.rs` but `fetch_recent_precip` only fetches hourly data; no 1-min poll path exists in the daemon
- **ASOS poll ignores priority intervals** — all ASOS stations (PRIMARY through EXTENDED) are polled every 15min; the `poll_interval_minutes` field from priority is stored in DB but the daemon doesn't respect it
- **USGS only fetches 2 parameters** — hardcoded `["00060", "00065"]` (discharge + stage); stations may also report temperature (00010), conductance (00095), pH (00400) — not currently ingested

---



- `waterway.toml` — top-level deployment config: `waterway_id`, display name, bounding box, datum, timezone
- Dynamic zone deserialization — replace hardcoded `ZoneCollection { zone_0…zone_6 }` with `HashMap<String, Zone>`
- `flomon_service discover` CLI subcommand — query USGS/CWMS/IEM by bounding box; emit draft TOML files
- USGS parameter auto-discovery — populate `expected_parameters` and thresholds from AHPS JSON
- NHD integration — infer upstream/downstream relationships from National Hydrography Dataset
- Single generic Docker image — no Illinois-specific constants in binary
- `docker-compose.template.yml` — parameterised on `WATERWAY_ID` for multi-waterway hosting
- Zone-agnostic dashboard — render zones from API, not fixed 7-zone layout
- Remove Illinois River hardcoding from floml scripts (`analyze_events.py`, `demo_correlation.py`)
- Threshold optimisation write-back — CLI to update `usgs_stations.toml` from floml regression results

---

## Data Sources

- **NOAA NDFD** — quantitative precipitation forecasts (1–7 day QPF)
- **NOAA MRMS** — radar-estimated observed rainfall
- **NWS CAP feed** — live flood watches, warnings, advisories; AHPS river forecasts
- **NRCS SNOTEL / NOAA CPC / NASA SMAP** — soil moisture and basin saturation
- NWS forecast vs. observed comparison endpoint

---

## Service Enhancements

- `GET /forecast` endpoint — currently stubbed; implement stage/discharge prediction from active zone pattern
- WebSocket support for real-time zone updates
- Station automatic backfill — when a station recovers, fetch missed readings
- Redundant station definitions — fallback sensors for critical locations
- USGS Status API integration — query site status proactively instead of waiting for failures
- CWMS alternative source — investigate `rivergages.mvr.usace.army.mil` for MVR district pool data
- Alert delivery — SMS (Twilio via Pub/Sub `sms_gateway`) and email notification
- Alert history, acknowledgment, and escalation

---

## Analysis (floml)

- Precursor pattern detection
- Automated flood event classification from historical data
- Segmented regression threshold optimisation
- Enhanced stage-discharge models with confidence intervals
- Multi-variate flood prediction
- Time series anomaly detection

---

## Infrastructure & Operations

- Prometheus metrics export (failures by station/source, freshness, API latency)
- Grafana dashboards
- Schema extension: dam release tracking, ice jam detection, flood impact records
- Web dashboard with WebSocket updates and interactive basin map
- Data quality validation rules and cross-source comparison (USGS vs. CWMS)
- Deployment guide, backup/recovery procedures, troubleshooting guide
