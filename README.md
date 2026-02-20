# FloPro - Flood Monitoring Service

**Status:** Early development - infrastructure and testing framework in place

## Vision

FloPro is a generalized flood monitoring system designed to work with any river or waterway. The software ingests data from multiple sources (USGS gauges, NOAA weather, NWS forecasts, soil moisture, dam operations), builds predictive models from historical patterns, and provides early warning alerts for downstream communities.

**Key Goals:**
- **Waterway-agnostic**: Configure for any river system via TOML configuration
- **Multi-source intelligence**: Combine gauge data, weather forecasts, soil saturation, and dam operations
- **Historical learning**: Build flood prediction models from decades of archived data
- **Upstream lead time**: Detect flood conditions hours or days before they arrive downstream
- **Open architecture**: Extensible database schema for new data sources

**Reference Implementation:** Illinois River Basin (Peoria, IL) - 8 USGS monitoring stations with 87 years of historical data


## Project Structure

```
flomon_service/
├── src/
│   ├── bin/
│   │   └── historical_ingest.rs  # Historical data backfill
│   ├── alert/                     # Flood threshold and staleness checking
│   ├── analysis/                  # Data grouping and trend analysis
│   ├── ingest/                    # USGS API client and data parsing
│   ├── model.rs                   # Core data structures
│   ├── stations.rs                # Station registry and metadata
│   ├── lib.rs                     # Shared library
│   └── main.rs                    # Real-time monitoring service (TBD)
├── tests/                         # Integration tests
└── Cargo.toml
```

## Current Status

**What Works:**
- ✅ **Configuration system** - TOML-based station metadata with flood thresholds, travel times, and distances
- ✅ **USGS data clients** - API integration for both instantaneous (15-min) and daily values (1939-present)
- ✅ **Historical ingestion** - Dual-tier backfill system with resumable state tracking
- ✅ **PostgreSQL schema** - Multi-schema architecture (usgs_raw, nws, noaa, usace, soil) ready for extensibility
- ✅ **Testing framework** - 21 unit tests passing, 17 TDD placeholders (expected failures), 4 integration tests
- ✅ **Database migrations** - Schema for gauge readings, monitoring state, and flood thresholds

**In Development:**
- 🔄 **Staleness detection** - Identify offline stations and data gaps (function stubs in place, 9 tests waiting)
- 🔄 **Data grouping** - Multi-site analysis and comparison (function stubs, 8 tests waiting)
- 🔄 **Flood threshold checking** - Detect when readings exceed NWS action/flood stages (2 tests waiting)

**Not Yet Started:**
- ⏸️ Real-time monitoring daemon (main.rs)
- ⏸️ Alert dispatch system
- ⏸️ Web dashboard
- ⏸️ Additional data source ingestion (NWS forecasts, NOAA precipitation, soil moisture, USACE operations)

## Multi-Source Data Architecture

FloPro is designed to integrate multiple data sources for comprehensive flood prediction:

### Currently Implemented: USGS Stream Gauges

**Source:** U.S. Geological Survey National Water Information System (NWIS)

| API | Coverage | Resolution | Use Case |
|-----|----------|------------|----------|
| **Instantaneous Values (IV)** | Last 120 days | 15 minutes | Real-time monitoring, recent flood detail |
| **Daily Values (DV)** | 1939-present | Daily means | Historical analysis, long-term trends, model training |

**Measurements:**
- Discharge (parameter 00060): Streamflow in cubic feet per second
- Gage height (parameter 00065): River stage in feet

**Status:** ✅ Fully implemented - API clients, database storage, historical backfill

### Planned: NWS Forecasts & Alerts

**Source:** National Weather Service Advanced Hydrologic Prediction Service (AHPS)

**Data Types:**
- Stage forecasts (predicted future river levels)
- Flood warnings and watches (CAP alerts)
- Official flood stage thresholds (action/minor/moderate/major)

**Use Case:** Authoritative government alerts, predicted future conditions

**Status:** 📋 Schema ready (nws.flood_warnings, nws.stage_forecasts), ingestion not implemented

### Planned: NOAA Precipitation

**Source:** NOAA Multi-Radar Multi-Sensor (MRMS) + National Digital Forecast Database (NDFD)

**Data Types:**
- Observed rainfall (radar-estimated, last 1/3/6/24/48 hours)
- Precipitation forecasts (predicted rainfall, 1-7 day outlook)

**Use Case:** Rainfall is the primary driver of river flooding - predict future discharge based on forecasted precipitation

**Status:** 📋 Schema designed (noaa.observed_precipitation, noaa.precipitation_forecasts), not implemented

### Planned: Soil Moisture

**Source:** USDA NRCS SNOTEL + NOAA Climate Prediction Center

**Data Types:**
- Point observations from field stations (volumetric water content by depth)
- Basin-averaged saturation index (composite metric 0-100)

**Use Case:** Saturated ground amplifies runoff from rainfall - high soil moisture increases flood risk

**Status:** 📋 Schema designed (soil.moisture_observations, soil.basin_saturation), not implemented

### Planned: USACE Lock & Dam Operations

**Source:** U.S. Army Corps of Engineers Lock Performance Monitoring System

**Data Types:**
- Current pool levels and release rates
- Scheduled water releases (maintenance/flood control)
- Operational status (normal, flood control mode, emergency)

**Use Case:** Dam releases can cause sudden discharge spikes - scheduled releases provide advance warning

**Status:** 📋 Schema designed (usace.lock_operations, usace.scheduled_releases), not implemented

---

## Reference Implementation: Illinois River Basin

**Geographic Focus:** Peoria, Illinois and Upper Peoria Lake

**Why Illinois River?**
- Developer's local area (stakeholder knowledge)
- 87 years of USGS data available (1939-present)
- Major tributary system (Mississippi River basin)
- Managed waterway with locks/dams (USACE operations)
- Documented flood history (1993, 2013, 2019)

**Monitoring Stations:** 8 USGS gauges configured in `stations.toml`
- Upstream early warning: Marseilles (80 mi, 36 hr), Chillicothe (10 mi, 9 hr)
- Primary monitoring: Kingston Mines (pool gauge), Peoria (discharge)
- Downstream verification: Havana (30 mi)
- Tributaries: Spoon River, Mackinaw River
- Water source: Chicago Sanitary & Ship Canal

See [stations.toml](flomon_service/stations.toml) for complete configuration including flood thresholds, distances, and travel times.

---

## Getting Started

### Prerequisites

- Rust 1.70+ (Edition 2024)
- PostgreSQL 14+
- USGS NWIS API access (no key required, rate-limited)

### Database Setup

```bash
# Create database
createdb flomon

# Apply migrations in order
psql -d flomon -f flomon_service/sql/001_initial_schema.sql
psql -d flomon -f flomon_service/sql/002_monitoring_metadata.sql
psql -d flomon -f flomon_service/sql/003_flood_metadata.sql

# Configure connection
echo "DATABASE_URL=postgresql://localhost/flomon" > flomon_service/.env
```

### Historical Data Ingestion

Load 87 years of USGS data (Illinois River reference implementation):

```bash
cd flomon_service
cargo run --bin historical_ingest
```

**What this does:**
- **Phase 1 (DV):** Fetch daily values from Oct 1939 to ~125 days ago (~31,755 days × 8 sites)
- **Phase 2 (IV):** Fetch 15-minute values for last 120 days (~11,520 readings × 8 sites)
- **Total:** ~692,400 gauge readings loaded in ~4-5 minutes

**Subsequent runs:** Incrementally update IV data only (DV historical data is static)

State tracking in `historical_ingest_state.json` - delete to re-run full backfillAHPS)

**Data Types:**
- Stage forecasts (predicted future river levels)
- Flood warnings and watches (CAP alerts)
- Official flood stage thresholds (action/minor/moderate/major)

**Use Case:** Authoritative government alerts, predicted future conditions

**Status:** 📋 Schema ready (nws.flood_warnings, nws.stage_forecasts), ingestion not implemented

### Planned: NOAA Precipitation

**Source:** NOAA Multi-Radar Multi-Sensor (MRMS) + National Digital Forecast Database (NDFD)

**Data Types:**
- Observed rainfall (radar-estimated, last 1/3/6/24/48 hours)
- Precipitation forecasts (predicted rainfall, 1-7 day outlook)

**Use Case:** Rainfall is the primary driver of river flooding - predict future discharge based on forecasted precipitation

**Status:** 📋 Schema designed (noaa.observed_precipitation, noaa.precipitation_forecasts), not implemented

### Planned: Soil Moisture

**Source:** USDA NRCS SNOTEL + NOAA Climate Prediction Center

**Data Types:**
- Point observations from field stations (volumetric water content by depth)
- Basin-averaged saturation index (composite metric 0-100)

**Use Case:** Saturated ground amplifies runoff from rainfall - high soil moisture increases flood risk

**Status:** 📋 Schema designed (soil.moisture_observations, soil.basin_saturation), not implemented

### Planned: USACE Lock & Dam Operations

**Source:** U.S. Army Corps of Engineers Lock Performance Monitoring System

**Data Types:**
- Current pool levels and release rates
- Scheduled water releases (maintenance/flood control)
- Operational status (normal, flood control mode, emergency)

**Use Case:** Dam releases can cause sudden discharge spikes - scheduled releases provide advance warning

**Status:** 📋 Schema designed (usace.lock_operations, usace.scheduled_releases), not implemented

---

## Configuration

### Station Configuration (stations.toml)

All station metadata, flood thresholds, and geographic relationships are defined in `stations.toml`:

```toml
[[station]]
site_code = "05568500"
name = "Illinois River at Kingston Mines, IL"
description = "Pool gauge for Peoria region (zero lag)"
latitude = 40.550556
longitude = -89.761111

distance_from_peoria_miles = 0
distance_direction = "upstream"
travel_time_to_peoria_hours = 0

expected_parameters = ["00060", "00065"]

[station.thresholds]
action_stage_ft = 15.0
flood_stage_ft = 18.0
moderate_flood_stage_ft = 21.0
major_flood_stage_ft = 23.0
```

**Station Reliability:** 6 of 8 configured stations operational (verified Feb 2026)
- 2 stations offline/decommissioned (Henry, Mackinaw Green Valley)
- System continues operating with degraded station set
- See [docs/STATION_RESILIENCE.md](flomon_service/docs/STATION_RESILIENCE.md)

### Database Schema

**Multi-schema architecture** for data source separation:

```
usgs_raw.*   -- USGS gauge readings and site metadata
nws.*        -- NWS flood thresholds, forecasts, alerts (planned)
noaa.*       -- Precipitation observations and forecasts (planned)
usace.*      -- Lock/dam operations and releases (planned)
soil.*       -- Soil moisture and saturation (planned)
```

**Current tables:**
- `usgs_raw.gauge_readings` - Time-series discharge and stage measurements
- `usgs_raw.sites` - Station metadata and coordinates
- `usgs_raw.monitoring_state` - Polling status and staleness tracking
- `nws.flood_thresholds` - Official NWS action/flood/moderate/major stages

**Planned tables:** See [docs/SCHEMA_EXTENSIBILITY.md](flomon_service/docs/SCHEMA_EXTENSIBILITY.md) for future data sources

### Upstream Early Warning

**Flood wave travel times** (Illinois River reference):

| Station | Distance | Travel Time | Use Case |
|---------|----------|-------------|----------|
| Marseilles | 80 mi upstream | 36 hours | Long-range warning |
| Starved Rock | 60 mi upstream | 24-48 hours | Medium-range forecast |
| Chillicothe | 10 mi upstream | 9 hours | Short-term alert |
| Kingston Mines | Pool gauge | 0 hours | Current conditions |

Times vary with flow velocity, precipitation, and USACE dam operations.

---

**Hybrid Database + In-Memory Cache:**
- Database (`monitoring_state` table): Source of truth, survives restarts
- In-memory cache: Fast staleness checks without database queries
- Refreshed on startup, updated each poll cycle
- See [sql/002_monitoring_metadata.sql](flomon_service/sql/002_monitoring_metadata.sql)

**Station Resilience:**
- Graceful degradation when stations go offline
- Per-parameter validation (discharge and stage tracked independently)
- Database constraints prevent duplicate readings (`ON CONFLICT DO NOTHING`)
- System continues with partial station set

**Test-Driven Development:**
- Function stubs with failing tests for unimplemented features
- 17 TDD placeholders (staleness detection, data grouping, flood thresholds)
- Tests pass as functions are implemented
GitHub wikis are separate git repositories. To publish these pages:

### 1. Clone the Wiki Repository

```bash
# Clone your main repo's wiki
git clone https://github.com/treeherder/illinois_river_flood_warning.wiki.git
cd illinois_river_flood_warning.wiki
```---

## Technology Stack

- **Language:** Rust (Edition 2024)
- **Database:** PostgreSQL 14+ (multi-schema design)
- **Configuration:** TOML
- **Dependencies:**
  - `postgres` 0.19 - Database client
  - `reqwest` 0.11 - HTTP client for USGS/NOAA/NWS APIs
  - `serde` / `serde_json` - JSON parsing (WaterML format)
  - `chrono` - Timestamp handling
  - `toml` 0.8 - Configuration file parsing

## Project Roadmap

### Phase 1: Foundation (Current)
- ✅ Configuration system (TOML-based station metadata)
- ✅ USGS data clients (IV and DV APIs)
- ✅ Historical ingestion (87-year backfill with resumable state)
- ✅ PostgreSQL schema (multi-schema for extensibility)
- ✅ Testing framework (21 passing, 17 TDD stubs, 4 integration)

### Phase 2: Core Monitoring (In Progress)
- 🔄 Implement staleness detection (function stubs in place)
- 🔄 Implement data grouping (function stubs in place)
- 🔄 Implement flood threshold checking (function stubs in place)
- ⏸️ Real-time monitoring daemon (main.rs)
- ⏸️ Alert dispatch system

### Phase 3: Multi-Source Integration (Planned)
- 📋 NWS forecast ingestion (AHPS hydrographs)
- 📋 NWS alert ingestion (CAP feeds for watches/warnings)
- 📋 NOAA precipitation (MRMS observed + NDFD forecasts)
- 📋 USACE lock/dam operations
- 📋 Soil moisture data (NRCS SNOTEL)

### Phase 4: Intelligence (Future)
- 📋 Flood prediction models (regression analysis on historical data)
- 📋 Multi-source risk scoring (combine gauge + weather + soil + dam releases)
- 📋 Automated flood event detection
- 📋 Return interval calculations

### Phase 5: User Interface (Future)
- 📋 Web dashboard (real-time visualization)
- 📋 Historical charts and comparisons
- 📋 Alert subscription system
- 📋 Mobile notifications

---

## Documentation

- [PRE_INGESTION_STRATEGY.md](flomon_service/docs/PRE_INGESTION_STRATEGY.md) - Why to apply database schema before loading historical data
- [SCHEMA_EXTENSIBILITY.md](flomon_service/docs/SCHEMA_EXTENSIBILITY.md) - How to add new data sources (NWS, NOAA, USACE, soil)
- [STATION_RESILIENCE.md](flomon_service/docs/STATION_RESILIENCE.md) - Handling offline stations and degraded data
- [DATA_STORAGE_STRATEGY.md](flomon_service/docs/DATA_STORAGE_STRATEGY.md) - Database design for missing data tracking
- [stations.toml](flomon_service/stations.toml) - Station configuration reference

## Contributing

This project is in early development. The architecture is designed to be waterway-agnostic - contributions for new river systems, data sources, or prediction models are welcome.

**To adapt for your waterway:**
1. Create `stations.toml` with your USGS gauge stations
2. Configure flood thresholds (get from NWS AHPS for your area)
3. Set travel times and distances for upstream early warning
4. Run historical ingestion to populate database
5. (Future) Train prediction models on your historical flood events

---

## License

TBD

---

## Disclaimer

This service provides informational flood monitoring only. **For official flood warnings and emergency information, consult the National Weather Service and local emergency management authorities.**

**Data Sources:**
- USGS NWIS: https://waterservices.usgs.gov/
- NWS AHPS: https://water.noaa.gov/
- NOAA: https://www.noaa.gov/
- USACE: https://www.usace.army.mil/