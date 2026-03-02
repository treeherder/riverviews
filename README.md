# Riverviews - Flood Monitoring Service

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

Riverviews is a generalized flood monitoring system designed to work with any river or waterway. Right now, the system has two major elements: firstly, there is a daemon service that is intended to run full-time as an online service.  flomon_service  is responsible for injesting data from the various data sources and storing it in postgres, validating and curating - checking that it is current, and ensuring that the data in general is as complete and well-labled as possible. flomon_service also provides an endpoint data stream for live data, and a simple alerting script based on rate-of-change and predetermined thresholds.  FloML is a package that holds mainly python-based scripts for analysis of the data provided by flomon_service, interacting with the endpoint and, eventually dynamically configuring the thresholds set in flomon_service. FloML additionally provides a dashboard view for use in human-monitoring live conditions and eventually querying views on past events.


## Project Structure

```
riverviews/
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
│   │   ├── generate_flood_zone_snapshots.py  # Zone regression testing
│   │   └── README.md             # Scripts documentation
│   ├── docs/                     # Architecture documentation
│   ├── sql/                      # Database migrations (001-006)
│   ├── zones.toml                # Zone definitions
│   └── Cargo.toml
├── floml/                        # Python analysis package
│   ├── floml/                    # Core library (regression, correlation, db)
│   ├── scripts/                  # Visualization and analysis tools
│   │   ├── zone_dashboard.py    # Live monitoring dashboard
│   │   ├── visualize_zones.py   # Zone detail viewer
│   │   ├── demo_correlation.py  # Correlation analysis
│   │   └── README.md            # Tool documentation
│   └── notebooks/                # Jupyter analysis notebooks
└── riverviews.wiki/  # Technical documentation
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
- Hourly precipitation observations (1-hour accumulation)
- Temperature, dewpoint, humidity
- Wind speed, direction, and gusts
- Atmospheric pressure and visibility
- Sky conditions and present weather codes

**Use Case:** Precipitation is the primary driver of tributary flooding - monitor basin rainfall for flood prediction. Temperature and pressure support severe weather detection.

**Stations:** 
- KPIA (Peoria) - Primary local precipitation (15-min polling)
- KBMI (Bloomington) - Mackinaw River basin (60-min)
- KSPI (Springfield) - Sangamon River basin (60-min)
- KGBG (Galesburg) - Spoon River basin (60-min)
- KORD (O'Hare) - Des Plaines River basin (6-hr)
- KPWK (Wheeling) - Des Plaines River tributary (6-hr)

**Status:** ✅ Fully operational - API client implemented, schema deployed (sql/006_iem_asos.sql), 6/6 stations verified, 16 integration tests passing

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
The system organizes sensors into 7 hydrological zones from the Mississippi River (backwater source) through the Illinois River basin to the Chicago area. Each zone has defined lead times (0-120 hours) for flood prediction at the Peoria property zone. Sensors are organized into geographic zones representing the flood propagation path:


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

See [flomon_service/zones.toml](flomon_service/zones.toml) for complete zone definitions and [riverviews.wiki/ZONE_ENDPOINT_MIGRATION.md](riverviews.wiki/ZONE_ENDPOINT_MIGRATION.md) for API documentation.

---

## Documentation

- **[floml/README.md](floml/README.md)** - Python analysis package (regression, correlation, ML)
- **[floml/scripts/README.md](floml/scripts/README.md)** - Visualization tools and live monitoring
- **[floml/QUICKSTART.md](floml/QUICKSTART.md)** - Get started with analysis

- **[flomon_service/docs/README.md](flomon_service/docs/README.md)** - Full docs index for Rust daemon
  - Database setup and configuration
  - Data source integration (USGS, CWMS, ASOS)
  - Architecture and design patterns
  - Operational procedures

### Analysis & Regression Testing

- **[flomon_service/scripts/README_ZONE_SNAPSHOTS.md](flomon_service/scripts/README_ZONE_SNAPSHOTS.md)** - Zone snapshot regression
- **[flomon_service/scripts/generate_flood_zone_snapshots.py](flomon_service/scripts/generate_flood_zone_snapshots.py)** - Snapshot generation

### Project Wiki

Technical notes and design decisions: [riverviews.wiki/](riverviews.wiki/)

### Testing

**Integration test suites:**
- `tests/asos_integration.rs` - ASOS weather station data collection and storage (16 tests)
- `tests/data_source_verification.rs` - Live API verification for USGS/CWMS/ASOS (4 tests)
- `tests/daemon_lifecycle.rs` - Daemon startup and monitoring behavior (13 tests)
- `tests/peak_flow_integration.rs` - Historical flood analysis integration (8 tests)

**Test coverage:**
- 78 unit tests (library code)
- 41 integration tests (end-to-end workflows)
- All ASOS stations verified operational (6/6)
- 5/8 USGS stations verified (3 decommissioned)

---

## Disclaimer

This is a persona project for education and experimentation, not yet a reliable technology. **For official flood warnings and emergency information, always consult the National Weather Service and local emergency management authorities.**

