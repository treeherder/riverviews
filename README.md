# Riverviews - Flood Monitoring Service

Riverviews should be a generalized flood monitoring system designed to work with any river or waterway. Right now, the system has two major elements: mainly, there is a daemon that is intended to run full-time as an online service.  This daemon, flomon_service, is responsible for maintaining an injestion pipeline from the various data sources and then storing, validating and curating this data - with the goal of checking that it is current and accurate, and that it is as complete and well-labled as possible. flomon_service also provides two endpoints: a data stream for live data, and a simple alerting script based on real-time monitored rate-of-change against predetermined thresholds. To support the flomon_service daemon, the riverviews package also provides analysis executables through the FloML directory. These scripts are designed to leverage the data provided by flomon_service: performing complex analysis, displaying information, and eventually dynamically re-configuring the toleances of thresholds - or providing additional constraints to those already set by default in flomon_service. 



## Table of Contents

- [Vision](#vision)
- [Project Structure](#project-structure)
- [Data Sources](#data-sources)
- [Reference Implementation](#reference-implementation-illinois-river-basin)
- [Zone-Based Architecture](#zone-based-architecture)
- [Getting Started](#getting-started)
- [Configuration](#configuration)
- [Documentation](#documentation)



## Project Structure

```
riverviews/
├── flomon_service/               # Rust monitoring daemon
│   ├── src/
│   │   ├── main.rs              # HTTP API server entry point
│   │   ├── daemon.rs            # Core daemon loop and polling logic
│   │   ├── endpoint.rs          # Zone-based HTTP API endpoints
│   │   ├── db.rs                # Database connectivity
│   │   ├── stations.rs          # USGS station registry loader
│   │   ├── usace_locations.rs   # USACE/CWMS location registry
│   │   ├── asos_locations.rs    # ASOS weather station registry
│   │   ├── zones.rs             # Zone configuration loader
│   │   ├── model.rs             # Core data structures
│   │   ├── logging.rs           # Structured logging system
│   │   ├── verify.rs            # Data source verification
│   │   ├── alert/               # Threshold and staleness monitoring
│   │   ├── analysis/            # Zone-based groupings
│   │   ├── ingest/              # Multi-source API clients (usgs, cwms, iem)
│   │   └── monitor/             # Monitoring utilities
│   ├── tests/                   # Integration test suites
│   │   ├── asos_integration.rs        # ASOS weather data tests
│   │   ├── daemon_lifecycle.rs        # Daemon behavior tests
│   │   ├── data_source_verification.rs # API verification tests
│   │   └── peak_flow_integration.rs   # Peak flow analysis tests
│   ├── scripts/                 # Python utility scripts
│   │   ├── generate_flood_zone_snapshots.py  # Zone regression testing
│   │   ├── analyze_historical_data.py        # Historical data analysis
│   │   ├── check_db_status.py                # Database status checker
│   │   ├── test_usgs_services.py             # USGS API testing
│   │   └── README.md                         # Scripts documentation
│   ├── docs/                    # Architecture documentation
│   │   ├── ASOS_IMPLEMENTATION.md            # Weather station integration
│   │   ├── CWMS_INTEGRATION_SUMMARY.md       # USACE CWMS status
│   │   ├── DATA_SOURCE_ORGANIZATION.md       # Configuration patterns
│   │   ├── DATA_SOURCE_VERIFICATION.md       # Testing framework
│   │   ├── LOGGING_AND_ERROR_HANDLING.md     # Logging system
│   │   ├── STATION_RESILIENCE.md             # Failure handling
│   │   └── TOML_CONFIGURATION.md             # Configuration guide
│   ├── sql/                     # Database schema migrations
│   │   ├── 001_initial_schema.sql       # Core monitoring tables
│   │   ├── 002_monitoring_metadata.sql  # Station metadata
│   │   ├── 003_flood_metadata.sql       # Flood thresholds
│   │   ├── 004_usace_cwms.sql           # USACE data schema
│   │   ├── 005_flood_analysis.sql       # Analysis tables
│   │   └── 006_iem_asos.sql             # Weather station schema
│   ├── usgs_stations.toml       # USGS gauge configuration
│   ├── usace_stations.toml      # USACE lock/dam configuration
│   ├── iem_asos.toml            # ASOS weather station configuration
│   ├── zones.toml               # Zone definitions
│   └── Cargo.toml               # Rust dependencies
├── floml/                       # Python analysis package
│   ├── floml/                   # Core library modules
│   │   ├── __init__.py
│   │   ├── db.py               # Database connection utilities
│   │   ├── regression.py       # Segmented linear regression
│   │   ├── correlation.py      # Multi-station correlation
│   │   └── precursors.py       # Flood precursor detection
│   ├── scripts/                 # Analysis and visualization tools
│   │   ├── zone_dashboard.py   # Live ncurses monitoring dashboard
│   │   ├── visualize_zones.py  # Terminal zone visualization
│   │   ├── demo_correlation.py # Sensor correlation analysis
│   │   └── analyze_events.py   # Historical event analysis
│   ├── examples/                # Example usage scripts
│   │   └── query_endpoint.py   # Endpoint querying examples
│   ├── tests/                   # Python unit tests
│   ├── QUICKSTART.md            # Quick reference guide
│   └── requirements.txt         # Python dependencies
├── riverviews.wiki/             # Technical documentation wiki
│   ├── Home.md                  # Wiki home page
│   ├── Data-Sources.md          # Data source documentation
│   ├── Database-Architecture.md # Database design
│   ├── Peak-Flow-Analysis.md    # Peak flow methodology
│   └── Staleness-Tracking.md    # Data staleness monitoring
├── TODO.md                      # Future enhancements tracker
└── README.md                    # This file
```

## Data Sources

The Riverviews system is designed to integrate multiple types of hydrological and meteorological data sources through a flexible, configuration-driven architecture. Data sources are classified by type and managed through TOML configuration files, allowing adaptation to various differerent US-monitored waterways without code changes.

### Data Source Classification System

**Source Types:**
- **Primary Hydrological** - Stream gauges providing discharge and stage measurements
- **Reference Data** - Historical flood records and official thresholds
- **Meteorological** - Weather stations for precipitation and atmospheric conditions
- **Infrastructure** - Lock, dam, and reservoir operations
- **Forecast** - Predicted conditions (precipitation, discharge, stage)

**Configuration Pattern:**
Each data source type has a dedicated TOML configuration file defining:
- Station/location metadata (coordinates, identifiers, names)
- Monitoring priority levels (CRITICAL, HIGH, MEDIUM, LOW)
- Expected data parameters and update frequencies
- Basin associations and hydrological relationships
- Polling intervals and staleness thresholds

### 1. Stream Gauge Networks (Primary Hydrological Data)

**Capability:** Integration with any stream gauge network providing standardized APIs for water level and flow measurements.

**Supported Networks:**
- **USGS NWIS** (U.S. Geological Survey National Water Information System)
- Any network providing similar REST APIs with discharge and stage parameters

**Data Types Available:**
- **Real-time observations:** High-frequency measurements (typically 5-30 minute intervals)
- **Historical daily values:** Long-term records for trend analysis and model training
- **Peak flow records:** Annual maximum events for flood frequency analysis
- **Parameter types:** Discharge (flow rate), stage (water level), velocity, temperature

**System Features:**
- **Automatic station discovery:** Query available parameters for any site code
- **Resilience framework:** Graceful degradation when individual stations fail
- **Parameter validation:** Expected vs. actual parameter verification
- **Gap detection:** Identify and backfill missing data periods
- **Threshold management:** Configure flood stage levels per station

**Configuration:** Stations defined in `usgs_stations.toml` with metadata including:
- Site codes and official names
- Geographic coordinates
- Expected parameters (discharge, stage, etc.)
- Flood thresholds (action, minor, moderate, major)
- Upstream/downstream relationships
- Travel times between locations

**Documentation:** See [STATION_RESILIENCE.md](flomon_service/docs/STATION_RESILIENCE.md) for handling station failures and [TOML_CONFIGURATION.md](flomon_service/docs/TOML_CONFIGURATION.md) for configuration patterns.

---

### 2. Historical Flood Records (Reference Data)

**Capability:** Ingest official flood events and threshold definitions from government sources for validation and model training.

**Supported Sources:**
- **NWS AHPS** (National Weather Service Advanced Hydrologic Prediction Service)
- **USGS Peak Flow Database** - Annual maximum streamflow records
- Any source providing flood crest times, peak stages, and official thresholds

**Data Types Available:**
- Historical flood events with crest timestamps and peak measurements
- Official flood stage thresholds by severity level (action/minor/moderate/major)
- Flood frequency statistics and return period analysis
- Comparison data for validating real-time flood detection algorithms

**System Features:**
- **Event classification:** Categorize floods by magnitude and duration
- **Threshold database:** Store official definitions for automated alerting
- **Model validation:** Compare predictions against historical events
- **Regression testing:** Verify algorithm accuracy across documented floods

**Use Cases:**
- Training stage-discharge relationship models
- Validating flood detection algorithms
- Setting baseline thresholds for automated alerts
- Historical trend analysis and climate impact assessment

---

### 3. Weather Station Networks (Meteorological Data)

**Capability:** Monitor precipitation and atmospheric conditions from weather observation networks to forecast tributary flooding and basin-wide rainfall.

**Supported Networks:**
- **ASOS/AWOS via IEM** (Automated Surface Observing System via Iowa Environmental Mesonet)
- Any weather network providing precipitation, temperature, wind, and pressure data

**Data Types Available:**
- **Precipitation:** High-resolution rainfall accumulation (1-minute to hourly intervals)
- **Temperature:** Air temperature, dewpoint, relative humidity
- **Wind:** Speed, direction, gusts
- **Atmospheric:** Barometric pressure, visibility
- **Conditions:** Sky cover, present weather phenomena

**System Features:**
- **Priority-based polling:** Configure update frequency by station importance
- **Precipitation thresholds:** Basin-specific alert levels for watch/warning
- **Accumulation windows:** Calculate totals over configurable time periods (6hr, 12hr, 24hr, 48hr)
- **Basin assignment:** Link stations to upstream tributary basins
- **Lag time modeling:** Define precipitation-to-peak relationships

**Configuration:** Stations defined in `iem_asos.toml` with:
- Station identifiers and names
- Priority levels (CRITICAL/HIGH/MEDIUM/LOW determine polling frequency)
- Basin associations (which river/tributary does this precipitation affect)
- Upstream gauge relationships (where does this precipitation flow to)
- Precipitation thresholds by time window

**Use Cases:**
- Tributary flood forecasting (precipitation drives small-basin response)
- Basin-wide saturation monitoring
- Event attribution (was flooding caused by precipitation or backwater)
- Early warning for fast-response watersheds (6-48 hour lead times)

**Documentation:** See [ASOS_IMPLEMENTATION.md](flomon_service/docs/ASOS_IMPLEMENTATION.md) for precipitation monitoring and threshold configuration.

---

### 4. Water Infrastructure Networks (Lock & Dam Operations)

**Capability:** Monitor lock, dam, and reservoir operations to detect backwater effects and manage water levels in controlled river systems.

**Supported Networks:**
- **USACE CWMS** (U.S. Army Corps of Engineers Corps Water Management System)
- Any infrastructure network providing pool elevations, tailwater stages, and gate operations

**Data Types Available:**
- Pool elevations (upstream water levels behind dams)
- Tailwater stages (downstream water levels below dams)
- Lock operations and passage counts
- Gate positions and discharge releases
- Flow measurements at control structures

**System Features:**
- **Runtime catalog discovery:** Automatically find available timeseries from API
- **Backwater detection:** Identify when downstream conditions block upstream drainage
- **Hydraulic control analysis:** Determine which structure controls water levels
- **Infrastructure status:** Monitor operational vs. design conditions

**Configuration:** Locations defined in `usace_stations.toml` with:
- CWMS location identifiers
- District office assignments
- Pool target elevations
- River mile positions
- Data types (pool/tailwater/lockage)

**Use Cases:**
- **Backwater flood detection:** When high downstream levels prevent upstream drainage
- **Controlled release management:** Monitor planned dam operations affecting flood risk
- **Pool level monitoring:** Track managed waterway conditions
- **Infrastructure impact analysis:** Quantify effects of lock/dam operations on flood propagation

**Technical Note:** Public API availability varies by district. System includes catalog discovery to verify data availability before attempting ingestion.

**Documentation:** See [CWMS_INTEGRATION_SUMMARY.md](flomon_service/docs/CWMS_INTEGRATION_SUMMARY.md) for API integration details.

---

### 5. Forecast Networks (Planned)

**Capability:** Integrate precipitation forecasts and discharge predictions to provide forward-looking flood risk assessment.

**Planned Sources:**
- **NOAA NDFD** (National Digital Forecast Database) - Quantitative precipitation forecasts
- **NOAA MRMS** (Multi-Radar Multi-Sensor) - Radar-estimated rainfall
- **NWS River Forecasts** - Official discharge and stage predictions

**Planned Data Types:**
- Quantitative Precipitation Forecasts (QPF): 1-7 day rainfall predictions
- Gridded precipitation products for basin-averaged calculations
- River stage and discharge forecasts
- Flood probability predictions

**Status:** 📋 Schema designed, not yet implemented

---

### 6. Soil Moisture Networks (Planned)

**Capability:** Monitor ground saturation to improve runoff coefficient estimates and rainfall-to-flood modeling accuracy.

**Planned Sources:**
- **USDA NRCS SNOTEL** - Snow and soil moisture telemetry
- **NOAA CPC** - Climate Prediction Center soil moisture products
- **NASA SMAP** - Soil Moisture Active Passive satellite data

**Planned Data Types:**
- Point observations from field stations (volumetric water content)
- Basin-averaged saturation indices
- Snow water equivalent (for spring melt scenarios)
- Soil moisture forecasts

**Use Case:** Saturated ground increases runoff coefficient - the same rainfall produces more streamflow when soil is already wet. Critical for improving flood prediction accuracy.

**Status:** 📋 Schema designed, not yet implemented

---

### Data Verification Framework

The system includes automated verification testing for all configured data sources:

- **Station availability checks:** Verify APIs respond and stations exist
- **Parameter validation:** Confirm expected data types are available
- **Data quality tests:** Check for reasonable values and completeness
- **Integration test suites:** Automated testing for each source type

**Verification Tools:**
- CLI command: `flomon_service verify`
- Integration tests: `cargo test --test data_source_verification`
- Reports generated in JSON and Markdown formats

**Documentation:** See [DATA_SOURCE_VERIFICATION.md](flomon_service/docs/DATA_SOURCE_VERIFICATION.md)

---

### Configuration-Driven Adaptation

**Key Principle:** All data source specifics are externalized to TOML configuration files. Adapting the system to a different river basin requires editing configuration files, not modifying source code.

**Configuration Files:**
- `usgs_stations.toml` - Stream gauge stations
- `iem_asos.toml` - Weather stations
- `usace_stations.toml` - Lock/dam infrastructure
- `zones.toml` - Hydrological zone groupings

---

## Reference Implementation: Illinois River Basin

[Illinois-River-Implementation.md](riverviews.wiki/Illinois-River-Implementation.md)

**Geographic Focus:** Peoria, Illinois and Upper Peoria Lake

**Why Illinois River?**
- Developer's local area (stakeholder knowledge)
- Major tributary system (Mississippi River basin)
- Managed waterway with many sensors under FOIA
- Documented flood history (1993, 2013, 2019)

**Monitoring:**
The system organizes sensors into 7 hydrological zones from the Mississippi River (backwater source) through the Illinois River basin to the Chicago area. . This example implementation combines 8 USGS gauges, 6 ASOS stations, 10 USACE locations, and configures them into 7 hydrological/geographical zones covering the Illinois River basin from the Mississippi confluence to Chicago, representing the flood propagation path.  Each zone has defined lead times (0-120 hours) for flood prediction at the central, Upper Peoria Lake interest zone:


| Zone | Name | Lead Time | Data Sources | Role |
|------|------|-----------|--------------|------|
| **0** | Mississippi River | 12h-5 days | **3 sensors:** 2 USACE stage (Hannibal, Alton), 1 USACE+USGS stage/discharge (Grafton) | Backwater source detection |
| **1** | Lower Illinois | 6-24 hours | **4 sensors:** 2 USACE pool/tailwater (LaGrange L&D), 1 USGS stage/discharge (Spoon River), 1 ASOS precip (Springfield) | Backwater interface |
| **2** | Upper Peoria Lake | 0-6 hours | **6 sensors:** 2 USACE pool/tailwater (Peoria L&D), 2 USGS stage/discharge (Peoria, Kingston Mines), 1 ASOS precip/wind/temp (PIA), 1 IEM/IEMRE gridded precip | **Property zone** (primary) |
| **3** | Local Tributaries | 6-18 hours | **3 sensors:** 1 USGS stage/discharge (Mackinaw River), 1 ASOS precip (BMI), 1 IEM/IEMRE gridded precip (basin) | Tributary flood monitoring |
| **4** | Mid Illinois | 18-48 hours | **4 sensors:** 1 USACE pool (Starved Rock L&D), 3 USGS stage/discharge (Marseilles, Henry, Vermilion River) | Upstream flood propagation |
| **5** | Upper Illinois | 36-72 hours | **3 sensors:** 1 USACE pool (Dresden Island L&D), 2 USGS stage/discharge (Kankakee, Des Plaines) | Confluence monitoring |
| **6** | Chicago CAWS | 3-5 days | **5 sensors:** 2 USACE pool (Lockport, Brandon Road), 1 USGS stage/discharge (CSSC), 2 ASOS precip (ORD, PWK) | Lake Michigan drainage |

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

