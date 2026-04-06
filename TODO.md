# Riverviews — Open Work Items

---

## ✅ Completed

- Zone-based HTTP API (`/zones`, `/zone/{id}`, `/status`, `/backwater`, `/health`)
- USGS NWIS ingest (real-time IV + 87 years DV historical backfill)
- USACE CWMS ingest with runtime catalog discovery
- IEM/ASOS precipitation ingest with basin assignment
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

---

## Generic Multi-Waterway Architecture

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
