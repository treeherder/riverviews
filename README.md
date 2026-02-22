# FloPro - Flood Monitoring Service

Real-time flood monitoring system using zone-based hydrological modeling to track river conditions across multiple data sources. Combines Rust for reliable data curation with Python for statistical analysis and regression testing.

**Status:** Zone-based architecture implemented | Historical data ingestion complete | Analysis infrastructure operational

## Table of Contents

- [Vision](#vision)
- [Project Structure](#project-structure)
- [Data Sources](#data-sources)
- [Reference Implementation](#reference-implementation-illinois-river-basin)
- [Zone-Based Architecture](#zone-based-architecture)
- [Getting Started](#getting-started)
- [Configuration](#configuration)
- [Documentation](#documentation)

## Vision

FloPro is a generalized flood monitoring system designed to work with any river or waterway. The system consists of:

1. **Rust Monitoring Daemon** - Reliable, server-side data curation and simple alerting
2. **Python Analysis Scripts** - Complex statistical analysis, regression, and ML modeling

The daemon ingests data from multiple sources (USGS gauges, USACE CWMS, ASOS weather stations, NWS events), validates and curates it in PostgreSQL, and provides zone-based monitoring through an HTTP API. Python scripts perform complex analysis on the curated data including historical flood characterization and regression testing.

## Project Structure

```
flopro/
├── flomon_service/               # Rust monitoring daemon
│   ├── src/
│   │   ├── bin/
│   │   │   ├── historical_ingest.rs      # USGS historical backfill
│   │   │   ├── ingest_cwms_historical.rs # CWMS historical backfill
│   │   │   ├── ingest_peak_flows.rs      # NWS peak flow ingestion
│   │   │   ├── analyze_flood_events.rs   # Event analysis (deprecated)
│   │   │   └── detect_backwater.rs       # Backwater detection
│   │   ├── alert/                # Threshold and staleness monitoring
│   │   ├── analysis/             # Zone-based groupings
│   │   ├── ingest/               # Multi-source API clients
│   │   ├── model.rs              # Core data structures
│   │   ├── stations.rs           # Station and zone registry
│   │   ├── lib.rs                # Shared library
│   │   └── main.rs               # HTTP API server
│   ├── scripts/
│   │   └── generate_flood_zone_snapshots.py  # Regression analysis
│   ├── docs/                     # Architecture documentation
│   ├── sql/                      # Database migrations (001-005)
│   ├── zones.toml                # Zone definitions
│   └── Cargo.toml
├── illinois_river_flood_warning.wiki/  # Technical documentation
└── floml/                        # Python ML analysis package
```

## Data Sources

### Currently Implemented: USGS Stream Gauges

**Source:** U.S. Geological Survey National Water Information System (NWIS)

| API | Coverage | Resolution | Use Case |
|-----|----------|------------|----------|
| **Instantaneous Values (IV)** | Last 120 days | 15 minutes | Real-time monitoring, recent flood detail |
| **Daily Values (DV)** | 1939-present | Daily means | Historical analysis, long-term trends, model training |

**Measurements:**
- Discharge (parameter 00060): Streamflow in cubic feet per second
- Gage height (parameter 00065): River stage in feet

**Status:** ✅ Fully implemented - API clients, database storage, 87 years of historical data loaded

### Implemented: NWS Peak Flow Events

**Source:** National Weather Service Advanced Hydrologic Prediction Service (AHPS)

**Data Types:**
- Historical flood events with crest times and peak stages
- Official flood stage thresholds (action/minor/moderate/major)
- Annual peak flow records

**Use Case:** Historical flood analysis, regression testing, threshold validation

**Status:** ✅ Implemented - 118 historical floods ingested (1993-2026), thresholds database-driven

### Implemented: ASOS Weather Stations

**Source:** Iowa Environmental Mesonet (IEM) ASOS/AWOS network

**Data Types:**
- 1-minute precipitation observations
- Accumulated rainfall (6hr, 24hr windows)
- Station metadata and quality codes

**Use Case:** Precipitation is the primary driver of tributary flooding - monitor basin rainfall for flood prediction

**Stations:** KPIA (Peoria), KBMI (Bloomington), KSPI (Springfield), KGBG (Galesburg), KORD (O'Hare), KPWK (Wheeling)

**Status:** ✅ Schema implemented (sql/006_asos_weather.sql), 6 stations configured with basin-specific thresholds

### Implemented: USACE Corps Water Management System

**Source:** U.S. Army Corps of Engineers CWMS Data API

**Data Types:**
- Mississippi River stage readings (backwater source monitoring)
- Illinois River lock/dam pool levels and tailwater stages
- Lock operations and gate positions

**Use Case:** Backwater flood detection - when high Mississippi River levels block Illinois River drainage, causing bottom-up flooding

**Locations:** Grafton, Alton, Hannibal (Mississippi); LaGrange, Peoria, Starved Rock (Illinois locks)

**Status:** ✅ Implemented - Historical CWMS data ingestion, backwater event detection and severity classification

### Planned: NOAA Precipitation Forecasts

**Source:** NOAA National Digital Forecast Database (NDFD) + Multi-Radar Multi-Sensor (MRMS)

**Data Types:**
- Precipitation forecasts (predicted rainfall, 1-7 day outlook)
- Radar-estimated observed rainfall

**Use Case:** Forecast future discharge based on predicted precipitation in upstream basins

**Status:** 📋 Schema designed, not implemented

### Planned: Soil Moisture

**Source:** USDA NRCS SNOTEL + NOAA Climate Prediction Center

**Data Types:**
- Point observations from field stations (volumetric water content)
- Basin-averaged saturation index

**Use Case:** Saturated ground amplifies runoff - high soil moisture increases flood risk from precipitation

**Status:** 📋 Schema designed, not implemented

---

## Reference Implementation: Illinois River Basin

**Geographic Focus:** Peoria, Illinois and Upper Peoria Lake

**Why Illinois River?**
- Developer's local area (stakeholder knowledge)
- Major tributary system (Mississippi River basin)
- Managed waterway with many sensors under FOIA
- Documented flood history (1993, 2013, 2019)

**Zone-Based Monitoring:**
The system organizes sensors into 7 hydrological zones from the Mississippi River (backwater source) through the Illinois River basin to the Chicago area. Each zone has defined lead times (0-120 hours) for flood prediction at the Peoria property zone. See [Zone-Based Architecture](#zone-based-architecture) below.

---

## Zone-Based Architecture

FloPro uses a **zone-based hydrological model** rather than individual site monitoring. Sensors are organized into 7 geographic zones representing the flood propagation path to Peoria, IL:

| Zone | Name | Lead Time | Primary Sensors | Role |
|------|------|-----------|-----------------|------|
| **0** | Mississippi River | 12h-5 days | Grafton, Alton, Hannibal | Backwater source detection |
| **1** | Lower Illinois | 6-24 hours | LaGrange, Havana, Spoon River | Backwater interface |
| **2** | Upper Peoria Lake | 0-6 hours | Peoria pool, Kingston Mines | **Property zone** (primary) |
| **3** | Local Tributaries | 6-18 hours | Mackinaw, Spoon, KBMI | Tributary flood monitoring |
| **4** | Mid Illinois | 18-48 hours | Henry, Starved Rock | Upstream flood propagation |
| **5** | Upper Illinois | 36-72 hours | Dresden, Kankakee, Des Plaines | Confluence monitoring |
| **6** | Chicago CAWS | 3-5 days | Lockport, CSSC, KORD weather | Lake Michigan drainage |

**Flood Type Classification:**
- **Top-down:** Upstream zones (4-6) elevated, flows downstream to property
- **Bottom-up:** Mississippi backwater (Zone 0) blocks Illinois drainage
- **Local tributary:** Zone 3 precipitation causing tributary flooding
- **Compound:** Multiple zones active simultaneously (worst case)

**API Endpoints:**
- `GET /zones` - List all zones with metadata
- `GET /zone/{id}` - All sensors in a zone with current readings
- `GET /status` - Overall basin flood status across all zones
- `GET /backwater` - Backwater flood risk analysis

See [flomon_service/zones.toml](flomon_service/zones.toml) for complete zone definitions and [illinois_river_flood_warning.wiki/ZONE_ENDPOINT_MIGRATION.md](illinois_river_flood_warning.wiki/ZONE_ENDPOINT_MIGRATION.md) for API documentation.

---

## Getting Started

### Prerequisites

- Rust 1.70+ (Edition 2024)
- PostgreSQL 14+
- USGS NWIS API access (no key required, rate-limited)

### Database Setup

See [flomon_service/docs/DATABASE_SETUP.md](flomon_service/docs/DATABASE_SETUP.md) for complete instructions.

**Quick start:**
```bash
# Create database and user
sudo -u postgres psql <<SQL
CREATE DATABASE flopro_db;
CREATE USER flopro_admin WITH PASSWORD 'your_password';
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;
SQL

# Apply migrations
for f in flomon_service/sql/*.sql; do
  sudo -u postgres psql -d flopro_db -f "$f"
done

# Grant permissions
sudo -u postgres psql -d flopro_db -f flomon_service/scripts/grant_permissions.sql

# Configure connection
echo "DATABASE_URL=postgresql://flopro_admin:your_password@localhost/flopro_db" > flomon_service/.env

# Validate setup
./flomon_service/scripts/validate_db_setup.sh
```

### Historical Data Ingestion

**USGS gauge data (1939-present):**
```bash
cd flomon_service
cargo run --bin historical_ingest
```
- Phase 1: Daily values from Oct 1939 to ~125 days ago
- Phase 2: 15-minute values for last 120 days
- ~692,000 readings loaded in 4-5 minutes
- State tracked in `historical_ingest_state.json`

**CWMS backwater monitoring:**
```bash
cargo run --bin ingest_cwms_historical
```
- Mississippi River stages (Grafton, Alton, Hannibal)
- Illinois River lock/dam data (LaGrange, Peoria, Starved Rock)

**NWS peak flow events:**
```bash
cargo run --bin ingest_peak_flows
```
- 118 historical flood events (1993-2026)
- Annual peak discharge records

### Run HTTP API Server

```bash
cd flomon_service
cargo run --release -- --endpoint 8080
```

**Available endpoints:**
- `http://localhost:8080/zones` - List all monitoring zones
- `http://localhost:8080/zone/2` - Peoria property zone status
- `http://localhost:8080/status` - Basin-wide flood status
- `http://localhost:8080/backwater` - Backwater risk analysis
- `http://localhost:8080/health` - Service health check

---

## Configuration

### Zone Definitions

Sensor zones are defined in [flomon_service/zones.toml](flomon_service/zones.toml). Each zone includes:
- Geographic extent and hydrological role
- Lead time to Peoria property
- Sensor list with roles (direct, boundary, precip, proxy)
- Alert conditions

**Example zone entry:**
```toml
[[zone]]
id = 2
name = "Upper Peoria Lake — Property Zone (Primary)"
lead_time_hours_min = 0
lead_time_hours_max = 6
primary_alert_condition = "Peoria pool > 447.5 ft / Kingston Mines stage > 14 ft"

[[zone.sensor]]
site_code = "05567500-PEORIA-POOL"
role = "direct"
relevance = "MOST IMPORTANT SINGLE READING for property elevation"
```

### Station Registry

USGS stations are registered in [flomon_service/usgs_stations.toml](flomon_service/usgs_stations.toml). Legacy site-based configuration; zones are now primary.

### Database Schema

**Multi-schema architecture** for data source separation:

```
usgs_raw.*   -- USGS gauge readings and site metadata
nws.*        -- NWS flood thresholds, forecasts, alerts (planned)
noaa.*       -- Precipitation observations and forecasts (planned)
usace.*      -- Lock/dam operations and releases (planned)
soil.*       -- Soil moisture and saturation (planned)
```

**Applied migrations:**
- `001_initial_schema.sql` - USGS gauge readings and site metadata
- `002_monitoring_metadata.sql` - Staleness tracking and health monitoring
- `003_flood_metadata.sql` - NWS flood thresholds and historical events
- `004_usace_cwms.sql` - CWMS locations, timeseries, backwater event detection
- `005_flood_analysis.sql` - Flood analysis tables and zone views

**Key tables:**
- `usgs_raw.gauge_readings` - Time-series discharge and stage (87 years × 15min resolution)
- `nws.flood_events` - Historical floods with crest times and peak stages (118 events)
- `usace.cwms_timeseries` - Mississippi/Illinois lock data for backwater detection
- `usace.backwater_events` - Detected backwater floods with severity classification
- `flood_analysis.zone_snapshots` - Zone status at historical flood crests

See [flomon_service/docs/SCHEMA_EXTENSIBILITY.md](flomon_service/docs/SCHEMA_EXTENSIBILITY.md) for schema design patterns.

## Documentation

### Technical Documentation (flomon_service/docs/)

**Core Infrastructure:**
- [DATABASE_SETUP.md](flomon_service/docs/DATABASE_SETUP.md) - Complete database setup guide
- [VALIDATION_SYSTEM.md](flomon_service/docs/VALIDATION_SYSTEM.md) - Database validation and permission scripts
- [DATA_STORAGE_STRATEGY.md](flomon_service/docs/DATA_STORAGE_STRATEGY.md) - Data storage architectural principles
- [SCHEMA_EXTENSIBILITY.md](flomon_service/docs/SCHEMA_EXTENSIBILITY.md) - Multi-source schema design patterns
- [EXTENSIBLE_ARCHITECTURE.md](flomon_service/docs/EXTENSIBLE_ARCHITECTURE.md) - Data source integration patterns

**Data Source Integration:**
- [ASOS_IMPLEMENTATION.md](flomon_service/docs/ASOS_IMPLEMENTATION.md) - Weather station precipitation monitoring
- [CWMS_IMPLEMENTATION.md](flomon_service/docs/CWMS_IMPLEMENTATION.md) - CWMS integration implementation
- [CWMS_INTEGRATION.md](flomon_service/docs/CWMS_INTEGRATION.md) - CWMS backwater monitoring rationale
- [TOML_CONFIGURATION.md](flomon_service/docs/TOML_CONFIGURATION.md) - CWMS TOML configuration guide

**Analysis & Strategy:**
- [PYTHON_INTEGRATION.md](flomon_service/docs/PYTHON_INTEGRATION.md) - Python database access for analysis
- [REFACTORING_PLAN.md](flomon_service/docs/REFACTORING_PLAN.md) - Rust-Python separation context
- [THRESHOLD_STRATEGY.md](flomon_service/docs/THRESHOLD_STRATEGY.md) - Threshold management approach
- [PRE_INGESTION_STRATEGY.md](flomon_service/docs/PRE_INGESTION_STRATEGY.md) - Pre-ingestion database strategy
- [STATION_RESILIENCE.md](flomon_service/docs/STATION_RESILIENCE.md) - Offline sensor handling

### Wiki Documentation (illinois_river_flood_warning.wiki/)

- [Home.md](illinois_river_flood_warning.wiki/Home.md) - Project overview and structure
- [Technology-Stack.md](illinois_river_flood_warning.wiki/Technology-Stack.md) - Technology choices and rationale
- [Data-Sources.md](illinois_river_flood_warning.wiki/Data-Sources.md) - USGS NWIS API integration
- [Database-Architecture.md](illinois_river_flood_warning.wiki/Database-Architecture.md) - PostgreSQL design decisions
- [Staleness-Tracking.md](illinois_river_flood_warning.wiki/Staleness-Tracking.md) - Data freshness monitoring
- [ZONE_ENDPOINT_MIGRATION.md](illinois_river_flood_warning.wiki/ZONE_ENDPOINT_MIGRATION.md) - Zone-based API design

### Project Status

- [PROJECT_STATUS.md](illinois_river_flood_warning.wiki/PROJECT_STATUS.md) - Current implementation status
- [ARCHITECTURE_COMPARISON.md](illinois_river_flood_warning.wiki/ARCHITECTURE_COMPARISON.md) - Zone refactoring comparison
- [DOCUMENTATION_AUDIT.md](illinois_river_flood_warning.wiki/DOCUMENTATION_AUDIT.md) - Documentation cleanup record
- [PEAK_FLOW_SUMMARY.md](PEAK_FLOW_SUMMARY.md) - Historical flood analysis with zone framework

### Analysis Scripts

- [scripts/generate_flood_zone_snapshots.py](flomon_service/scripts/generate_flood_zone_snapshots.py) - Zone snapshot regression analysis
- [scripts/README_ZONE_SNAPSHOTS.md](flomon_service/scripts/README_ZONE_SNAPSHOTS.md) - Zone snapshot documentation

---

## Disclaimer

This is a personal flood monitoring project for portfolio demonstration. **For official flood warnings and emergency information, always consult the National Weather Service and local emergency management authorities.**

