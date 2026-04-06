# Riverviews TODO

This document tracks planned features and future enhancements for the Riverviews flood monitoring project.

---

## 🚀 Phase 2: Generic Multi-Waterway Architecture

**Goal:** Refactor from an Illinois River–specific implementation to a truly generic platform where each waterway runs as an independent, containerised deployment configured entirely through data files — no code changes required to add a new river.

### Guiding Principles

- **One container set per waterway.** Each deployment has its own PostgreSQL instance, flomon_service daemon, and sms_gateway. Configuration files (TOML) drive everything. A `waterway_id` label in all tables separates data when sharing infrastructure is ever desired.
- **Discovery-first ingest.** Before committing to polling a station, the system verifies it exists and has the expected parameters via the source API. This is the pattern already used for CWMS catalog discovery; extend it to USGS and ASOS.
- **Generic zone model.** Remove hardcoded `zone_0`…`zone_6` fields from `ZoneCollection` and replace with a `HashMap<String, Zone>` so any number of zones with any names can be loaded from TOML.
- **Pluggable data sources.** New source APIs (e.g. Environment Canada, GRDC) should be addable by implementing a common `DataSource` trait, adding a TOML file, and mounting it — no daemon code changes.

### Phase 2 Work Items

#### 2.1 — Configuration layer
- [ ] **`waterway.toml`** — New top-level config for a deployment: `waterway_id`, `display_name`, geographic bounding box, datum system, time zone. The Docker image is generic; `waterway.toml` is what makes it Illinois River vs. Missouri River.
- [ ] **Dynamic zone deserialization** — Replace `ZoneCollection { zone_0, … zone_6 }` with `HashMap<String, Zone>`. Update `get_all_zones()` and `get_zone()` accordingly.
- [ ] **Merged alerting config** — Move `recipients.numbers` to `waterway.toml` so alert routing is per-deployment without editing `alerting.toml`.

#### 2.2 — Discovery tooling
- [ ] **`flomon_service discover`** CLI subcommand — Given a geographic bounding box (from `waterway.toml`) or a list of USGS HUC codes, query USGS, CWMS catalog, and IEM APIs to enumerate all available stations and produce draft `usgs_stations.toml`, `usace_stations.toml`, and `iem_asos.toml` files for human review.
- [ ] **USGS parameter discovery** — For each station code, hit `https://waterservices.usgs.gov/nwis/iv/?sites=XXXXXX&format=json` and auto-populate `expected_parameters` and `thresholds` (from AHPS JSON if available).
- [ ] **NHD integration** — Use the USGS National Hydrography Dataset to infer upstream/downstream relationships and suggest zone groupings.

#### 2.3 — Docker / image generics
- [ ] **Single generic image** — Remove any Illinois-specific constants from the binary. The compiled `flomon_service` image should work for any waterway as long as the TOML mounts are provided.
- [ ] **`docker-compose.template.yml`** — Parameterised compose file where `WATERWAY_ID` is substituted at deploy time, enabling multiple waterways on one host with isolated networks and volumes (`pgdata_${WATERWAY_ID}`).
- [ ] **Automated schema namespacing** — Prefix all PostgreSQL schemas with `waterway_id` (e.g. `illinois_usgs_raw`) so a shared Postgres instance can host multiple deployments. (Optional: keep per-deployment Postgres for simplicity.)

#### 2.4 — Analysis generics (floml)
- [ ] **Remove Illinois River hardcoding** — Replace hardcoded site codes, zone IDs, and parameter tuning in `analyze_events.py`, `demo_correlation.py`, and `visualize_zones.py` with values read from `waterway.toml` or the flomon_service `/zones` endpoint.
- [ ] **Zone-agnostic dashboards** — `zone_dashboard.py` and `visualize_zones.py` should render whatever zones the connected service reports, not a fixed 7-zone layout.
- [ ] **Threshold optimisation integration** — Once `floml` discovers improved thresholds via segmented regression, provide a CLI to write them back to `usgs_stations.toml` for the next daemon restart.

#### 2.5 — Data source generics
- [ ] **`DataSource` trait in Rust** — Abstract over USGS/CWMS/IEM behind a common polling interface returning `Vec<GaugeReading>`. New sources implement the trait; the daemon loops over a `Vec<Box<dyn DataSource>>`.
- [ ] **USGS-compatible adapter** — Waterwatch/CDEC (California DWR) and other agencies use similar REST patterns; a thin adapter layer would let them appear as USGS sources.
- [ ] **International station support** — GRDC (Global Runoff Data Centre), Environment Canada HYDAT, and ANA (Brazil) all provide discharge data. Model their authentication and response formats as new `DataSource` impls.

### Migration Path (Illinois River → Generic)

1. **Step 1** — Add `waterway.toml` with `waterway_id = "illinois_river"` and copy existing constants in. No behaviour changes.
2. **Step 2** — Dynamic zones. Replace `ZoneCollection` struct. Vetted by existing integration tests.
3. **Step 3** — `discover` subcommand implemented and validated on Illinois River (should reproduce current TOML files from scratch).
4. **Step 4** — Deploy a second waterway (e.g. Missouri River) using the discover workflow and the same Docker image. Treat this as the acceptance test.
5. **Step 5** — Migrate floml scripts to be waterway-agnostic.

---



## 🌊 New Data Sources

### NOAA Precipitation Forecasts
**Priority:** High  
**Source:** NOAA National Digital Forecast Database (NDFD) + Multi-Radar Multi-Sensor (MRMS)

**Data Types:**
- Precipitation forecasts (predicted rainfall, 1-7 day outlook)
- Radar-estimated observed rainfall
- Gridded precipitation products

**Use Case:** Forecast future discharge based on predicted precipitation in upstream basins

**Status:** 📋 Schema designed, not implemented

**Implementation Tasks:**
- [ ] Create precipitation schema and tables
- [ ] Implement NOAA NDFD API client
- [ ] Implement MRMS data ingestion
- [ ] Add precipitation monitoring to daemon
- [ ] Create precipitation analysis endpoints
- [ ] Integrate with flood forecasting models

---

### Soil Moisture / Ground Saturation
**Priority:** Medium  
**Source:** USDA NRCS SNOTEL + NOAA Climate Prediction Center + NASA SMAP

**Data Types:**
- Point observations from field stations (volumetric water content)
- Basin-averaged saturation index
- Soil saturation forecasts

**Use Case:** Saturated ground amplifies runoff - high soil moisture increases flood risk from precipitation

**Status:** 📋 Schema designed, not implemented

**Implementation Tasks:**
- [ ] Create soil moisture schema
- [ ] Implement NRCS SNOTEL API client
- [ ] Implement NOAA CPC data ingestion
- [ ] Add basin saturation calculation
- [ ] Integrate with flood risk scoring

---

### NWS Live Alerts and Warnings
**Priority:** High  
**Source:** NWS CAP feeds, NOAA Weather Wire Service

**Data Types:**
- Active flood watches, warnings, advisories
- NWS AHPS stage predictions and forecasts
- River hydrograph forecasts

**Use Case:** Official government flood warnings for authoritative alerts

**Status:** NWS flood events and thresholds implemented, live alerts not yet integrated

**Implementation Tasks:**
- [ ] Implement NWS CAP feed parser
- [ ] Add flood warning ingestion to daemon
- [ ] Create active alerts endpoint
- [ ] Add forecast vs. observed comparison
- [ ] Create alert history tracking

---

## 🦀 Rust Service Enhancements

### Station Resilience
**Reference:** [STATION_RESILIENCE.md](flomon_service/docs/STATION_RESILIENCE.md)

- [ ] Station Health Dashboard - Real-time availability display
- [ ] Automatic Backfill - When station recovers, fetch missed data
- [ ] Redundant Stations - Define backup stations for critical locations
- [ ] Parameter Fallbacks - Estimate missing values from nearby stations
- [ ] USGS Status API - Query site status instead of waiting for failures

---

### Endpoint Enhancements
**Reference:** [ZONE_ENDPOINT_MIGRATION.md](riverviews.wiki/ZONE_ENDPOINT_MIGRATION.md)

- [ ] Implement forecast endpoint (currently stubbed)
- [ ] Add WebSocket support for real-time zone updates
- [ ] Calculate backwater risk based on Mississippi River elevations
- [ ] Add authentication/authorization for API endpoints
---

## 🐍 Python/FloML Analysis Features

### Advanced Analysis

**Moving from exploratory scripts to production features:**
- [ ] Precursor pattern detection algorithms
- [ ] Flood event classification (top-down, bottom-up, compound)
- [ ] Backwater influence modeling improvements
- [ ] Time series anomaly detection
- [ ] Confidence intervals and uncertainty quantification
- [ ] Automated report generation

---

### Correlation and Regression
- [ ] Enhanced stage-discharge relationship models
- [ ] Upstream-downstream correlation analysis
- [ ] Multi-variate flood prediction models
- [ ] Segmented regression threshold optimization

---

## 🗄️ Database and Schema

### Threshold Management

**ML-Discovered Thresholds:**
- [ ] Implement segmented regression for threshold discovery
  - Find channel → bankfull → floodplain transitions
  - Identify natural physical breakpoints

- [ ] Implement historical correlation analysis
  - Identify stages that correlate with actual impacts
  - Analyze road closures, property damage

- [ ] Implement multi-station threshold optimization
- [ ] Create `flood_analysis.recommended_thresholds` table
- [ ] Implement threshold update mechanism
- [ ] Implement dual threshold comparison (static vs. ML-discovered)

---

### Schema Extensions

**Future Schema Expansions:**
- [ ] Add dam operations tracking (reservoir releases)
- [ ] Add reservoir level monitoring
- [ ] Add ice jam detection data
- [ ] Add flood impact records (property damage, road closures)
- [ ] Add precipitation forecast data
- [ ] Add soil moisture data

---

## 🔧 Monitoring and Operations

### Prometheus Metrics
**Reference:** [LOGGING_AND_ERROR_HANDLING.md](flomon_service/docs/LOGGING_AND_ERROR_HANDLING.md)

- [ ] Export failure counters for Prometheus
  - USGS failures by station
  - CWMS failures by location
  - ASOS failures by station
- [ ] Add data freshness metrics
- [ ] Add API response time metrics
- [ ] Add database query performance metrics
- [ ] Create Grafana dashboards

---

### Alerting System
- [ ] Implement configurable alert thresholds
- [ ] Add email notification support
- [ ] Add SMS notification support
- [ ] Add webhook integration
- [ ] Create alert history and acknowledgment system
- [ ] Implement alert escalation rules

---

## 📊 Web Interface and Dashboards

**Reference:** [ZONE_ENDPOINT_MIGRATION.md](riverviews.wiki/ZONE_ENDPOINT_MIGRATION.md)

- [ ] Create real-time dashboard with WebSocket updates
- [ ] Add historical flood event visualization
- [ ] Create interactive basin map
- [ ] Add precipitation overlay visualization
- [ ] Create public-facing status page
- [ ] Add mobile-responsive interface

---

## 🧪 Testing and Validation

### Data Quality
- [ ] Add data quality validation rules
- [ ] Implement outlier detection
- [ ] Add cross-source validation (USGS vs. CWMS comparison)
- [ ] Create validation reporting dashboard
- [ ] Add performance benchmarks
- [ ] Create load testing suite

---

## 📚 Documentation

- [ ] Create deployment guide for production environments
- [ ] Document backup and recovery procedures
- [ ] Create troubleshooting guide
- [ ] Add architecture decision records (ADRs)
- [ ] Create user manual for dashboards
- [ ] Document API rate limits and best practices

---
