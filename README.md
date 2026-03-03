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
└── riverviews.wiki/  # Technical documentation
```

## Data Sources

The system integrates multiple data sources for comprehensive flood monitoring. All operational sources have automated verification testing ([DATA_SOURCE_VERIFICATION.md](flomon_service/docs/DATA_SOURCE_VERIFICATION.md)).

### 1. USGS Stream Gauges (Primary Hydrological Data)

**Source:** U.S. Geological Survey National Water Information System (NWIS)  
**API Endpoint:** `https://waterservices.usgs.gov/nwis/`

**Data Types:**

| API Type | Coverage | Resolution | Parameters | Use Case |
|----------|----------|------------|------------|----------|
| **Instantaneous Values (IV)** | Last 120 days | 15 minutes | 00060 (discharge), 00065 (stage) | Real-time monitoring, flood event tracking |
| **Daily Values (DV)** | 1939-present | Daily means | Same as IV | Historical analysis, long-term trends, model training |
| **Peak Flow Records** | 1941-2025 | Annual peaks | Stage and discharge | Flood frequency analysis, threshold validation |

**Configuration:** 8 stations spanning the Illinois River basin (Kingston Mines, Peoria pool, Chillicothe, Henry, Marseilles) and key tributaries (Mackinaw, Spoon, Des Plaines)

**Operational Status:** ✅ 5 of 8 stations operational (62.5%)
- Working: Kingston Mines (05568500), Peoria pool (05567500), Chillicothe (05568000), Marseilles (05552500), Spoon River (05570000)
- Offline: Henry (05557000), Mackinaw River (05568580), Chicago Sanitary Canal (05536890) - USGS equipment issues

**Implementation Status:**
- ✅ API clients for IV, DV, and peak flow data
- ✅ PostgreSQL schema `usgs_raw` with 87 years of historical data loaded
- ✅ Station metadata with NWS flood thresholds (action/minor/moderate/major stages)
- ✅ Resilience framework for graceful degradation when stations fail
- ✅ Integration tests with live API verification

**Documentation:** [STATION_RESILIENCE.md](flomon_service/docs/STATION_RESILIENCE.md), [TOML_CONFIGURATION.md](flomon_service/docs/TOML_CONFIGURATION.md)

---

### 2. NWS Peak Flow Events (Reference Data)

**Source:** National Weather Service Advanced Hydrologic Prediction Service (AHPS)  
**Coverage:** USGS peak flow database + NWS AHPS flood history

**Data Types:**
- Historical flood events with crest times and peak stages
- Official flood stage thresholds (action/minor/moderate/major) by site
- Annual peak flow records with discharge measurements
- Flood frequency statistics

**Use Case:** Historical flood analysis, regression model training, threshold validation, event detection algorithm testing

**Status:** ✅ Fully implemented
- 118 historical floods ingested (1993-2026) for Illinois River basin
- Database-driven threshold system (replaces hardcoded values)
- Integration with USGS data for stage-discharge relationship modeling
- 8 integration tests for peak flow processing

**Schema:** `nws.flood_events`, `nws.flood_thresholds` (see [sql/005_flood_analysis.sql](flomon_service/sql/005_flood_analysis.sql))

---

### 3. ASOS Weather Stations (Precipitation Monitoring)

**Source:** Iowa Environmental Mesonet (IEM) ASOS/AWOS network  
**API Endpoints:** 
- Current observations: `https://mesonet.agron.iastate.edu/json/current.py`
- High-resolution precipitation: `https://mesonet.agron.iastate.edu/cgi-bin/request/asos1min.py`

**Data Types:**
- **Precipitation:** 1-minute interval rainfall accumulation (2000-present) and 1-hour accumulations
- **Temperature & Humidity:** Air temperature, dewpoint, relative humidity
- **Wind:** Speed (knots), direction (degrees), gusts
- **Atmospheric:** Pressure (mb), visibility (miles)
- **Conditions:** Sky condition codes, present weather phenomena

**Configured Stations (6):**

| Station | Location | Basin | Priority | Role |
|---------|----------|-------|----------|------|
| **KPIA** | Peoria | Illinois River | CRITICAL | Primary local precipitation (15-min polling) |
| **KBMI** | Bloomington | Mackinaw River | HIGH | Tributary basin monitoring (60-min) |
| **KSPI** | Springfield | Sangamon River | HIGH | Southern basin reference (60-min) |
| **KGBG** | Galesburg | Spoon River | HIGH | Western tributary basin (60-min) |
| **KORD** | Chicago O'Hare | Des Plaines River | MEDIUM | Northern basin context (6-hr) |
| **KPWK** | Wheeling | Des Plaines tributary | MEDIUM | Extended coverage (6-hr) |

**Basin Precipitation Thresholds:** Configured for 6 and 24-hour accumulations at watch and warning levels (see [ASOS_IMPLEMENTATION.md](flomon_service/docs/ASOS_IMPLEMENTATION.md#basin-precipitation-thresholds))

**Use Case:** Precipitation is the primary driver of tributary flooding. Monitor basin-wide rainfall for flood forecasting with lag times of 6-48 hours depending on basin size and characteristics.

**Status:** ✅ Fully operational (100% station availability)
- API client with 1-minute and hourly data fetching
- Schema deployed: `asos_stations`, `asos_observations`, `asos_precip_summary` ([sql/006_iem_asos.sql](flomon_service/sql/006_iem_asos.sql))
- All 6 stations verified operational with complete data types
- 16 integration tests passing
- Automatic precipitation accumulation and threshold detection

**Documentation:** [ASOS_IMPLEMENTATION.md](flomon_service/docs/ASOS_IMPLEMENTATION.md)

---

### 4. USACE CWMS (Lock & Dam Data)

**Source:** U.S. Army Corps of Engineers Corps Water Management System  
**API Endpoint:** `https://cwms-data.usace.army.mil/cwms-data/`

**Intended Data Types:**
- Mississippi River stage readings (backwater source)
- Illinois River lock/dam pool elevations and tailwater stages
- Lock operations and gate positions
- Flow measurements

**Configured Locations (10):** Grafton, Alton, Hannibal (Mississippi); Peoria, LaGrange, Starved Rock, Marseilles, Dresden Island, Brandon Road, Lockport (Illinois locks)

**Use Case:** Backwater flood detection - when high Mississippi River levels block Illinois River drainage, causing "bottom-up" flooding at the confluence

**Status:** ⚠️ Infrastructure implemented, limited data availability
- ✅ Runtime catalog discovery via CWMS API
- ✅ Database schema deployed ([sql/004_usace_cwms.sql](flomon_service/sql/004_usace_cwms.sql))
- ✅ Historical data ingestion tools operational
- ✅ Backwater event detection algorithms implemented
- ❌ Illinois River lock/dam data NOT available in public CWMS API (MVR District)
- ⚠️ Mississippi River (Grafton) catalog found but limited real-time data
- 🔍 Alternative data source investigation ongoing: `https://rivergages.mvr.usace.army.mil/`

**Technical Note:** CWMS API catalog discovery works correctly, but most Illinois River infrastructure data is either not published publicly or uses different identifiers. Historical backfill from other sources remains functional.

**Documentation:** [CWMS_INTEGRATION_SUMMARY.md](flomon_service/docs/CWMS_INTEGRATION_SUMMARY.md)

---

### 5. NOAA Precipitation Forecasts (Planned)

**Source:** NOAA National Digital Forecast Database (NDFD) + Multi-Radar Multi-Sensor (MRMS)

**Planned Data Types:**
- Quantitative Precipitation Forecasts (QPF): 1-7 day rainfall predictions
- Radar-estimated observed rainfall (MRMS)
- Gridded precipitation data for basin-averaged calculations

**Use Case:** Forecast future discharge based on predicted precipitation in upstream basins, combined with soil moisture and current base flow

**Status:** 📋 Schema designed, not yet implemented

---

### 6. Soil Moisture Data (Planned)

**Source:** USDA NRCS SNOTEL + NOAA Climate Prediction Center

**Planned Data Types:**
- Point observations from field stations (volumetric water content %)
- Basin-averaged soil saturation index
- Snow water equivalent (for spring melt scenarios)

**Use Case:** Saturated ground amplifies runoff coefficient - high soil moisture increases flood risk from precipitation. Critical for improving rainfall-runoff model accuracy.

**Status:** 📋 Schema designed, not yet implemented

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

### Architecture Documentation

Technical implementation details in [flomon_service/docs/](flomon_service/docs/):

- **[ASOS_IMPLEMENTATION.md](flomon_service/docs/ASOS_IMPLEMENTATION.md)** - ASOS weather station integration for precipitation monitoring, including IEM data source configuration, basin-specific stations, and precipitation thresholds for flood forecasting

- **[CWMS_INTEGRATION_SUMMARY.md](flomon_service/docs/CWMS_INTEGRATION_SUMMARY.md)** - USACE Corps Water Management System (CWMS) integration status, runtime catalog discovery implementation, and current data availability findings

- **[DATA_SOURCE_ORGANIZATION.md](flomon_service/docs/DATA_SOURCE_ORGANIZATION.md)** - Organization pattern for data source configuration files, including TOML file structure and loader module responsibilities for USGS, USACE, and ASOS sources

- **[DATA_SOURCE_VERIFICATION.md](flomon_service/docs/DATA_SOURCE_VERIFICATION.md)** - Automated verification framework for testing all configured data sources, generating operational status reports, and identifying working vs. non-working stations

- **[LOGGING_AND_ERROR_HANDLING.md](flomon_service/docs/LOGGING_AND_ERROR_HANDLING.md)** - Structured logging system with failure classification (expected vs. unexpected errors), log levels, and diagnostic output for monitoring daemon operations

- **[STATION_RESILIENCE.md](flomon_service/docs/STATION_RESILIENCE.md)** - Resilience strategies for handling station failures, including metadata-driven parameter expectations, graceful degradation, and maintaining operational continuity when gauges go offline

- **[TOML_CONFIGURATION.md](flomon_service/docs/TOML_CONFIGURATION.md)** - TOML-based configuration guide for CWMS locations with runtime timeseries discovery via catalog API, eliminating hardcoded timeseries IDs

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

