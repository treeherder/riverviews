# Riverviews 

This document consolidates all implementation notes, planned features, and future enhancements across the Riverviews flood monitoring project.

---

### NOAA Precipitation Forecasts
**Priority:** High  
**Source:** NOAA National Digital Forecast Database (NDFD) + Multi-Radar Multi-Sensor (MRMS)

**Data Types:**
- Precipitation forecasts (predicted rainfall, 1-7 day outlook)
- Radar-estimated observed rainfall
- Gridded precipitation products

**Use Case:** Forecast future discharge based on predicted precipitation in upstream basins

**Status:** 📋 Schema designed (`SCHEMA_EXTENSIBILITY.md`), not implemented  
**Migration:** `sql/005_precipitation.sql` (planned)

**Implementation Tasks:**
- [ ] Create precipitation schema and tables
- [ ] Implement NOAA NDFD API client
- [ ] Implement MRMS data ingestion
- [ ] Add precipitation monitoring to daemon
- [ ] Create precipitation analysis endpoints
- [ ] Integrate with backwater correlation analysis

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
**Migration:** `sql/006_soil_moisture.sql` (planned)

**Implementation Tasks:**
- [ ] Create soil moisture schema
- [ ] Implement NRCS SNOTEL API client
- [ ] Implement NOAA CPC data ingestion
- [ ] Add basin saturation calculation
- [ ] Create soil saturation analysis module
- [ ] Integrate with flood risk scoring

---

### NWS Live Alerts and Warnings
**Priority:** High  
**Source:** NWS EMWIN, NWS CAP feeds, NOAA Weather Wire Service

**Data Types:**
- Active flood watches, warnings, advisories
- NWS AHPS stage predictions and forecasts
- River hydrograph forecasts

**Use Case:** Official government flood warnings for authoritative alerts

**Status:** NWS flood events and thresholds implemented, live alerts not yet integrated  
**Migration:** `sql/007_nws_forecasts.sql` (extends existing NWS schema)

**Implementation Tasks:**
- [ ] Implement NWS CAP feed parser
- [ ] Add flood warning ingestion to daemon
- [ ] Create active alerts endpoint
- [ ] Implement alert notification system
- [ ] Add forecast vs. observed comparison
- [ ] Create alert history tracking

---

### IEM ASOS Full Integration
**Priority:** Medium  
**Status:** ⏸️ Schema and TOML configuration complete, ingestion code not implemented

**Implementation Tasks:**
- [ ] Complete ASOS data ingestion (code marked as TODO in TOML_CONFIGURATION.md)
- [ ] Automated precipitation summary computation (scheduled job)
- [ ] Alert generation based on threshold exceedances
- [ ] Backwater correlation (ASOS precip + LaGrange hydraulic control loss)
- [ ] IEMRE gridded precipitation integration
- [ ] MRMS radar-based QPE verification

---

## 🦀 Rust Service Improvements

### Refactoring (In Progress)
**Reference:** `flomon_service/docs/REFACTORING_PLAN.md`

**Phase 1: Remove Complex Analysis**
- [ ] Remove `src/bin/analyze_flood_events.rs`
- [ ] Remove `src/analysis/flood_events.rs`
- [ ] Simplify `src/analysis/mod.rs` and `groupings.rs`
- [ ] Update dependencies in `Cargo.toml` (remove analysis-only deps)
- [ ] Clean up documentation references

**Phase 2: Enhance Core Monitoring**
- [ ] Ensure `main.rs` is structured as daemon
- [ ] Add simple threshold monitoring
- [ ] Add staleness alerting
- [ ] Add basic rate-of-rise detection (simple ft/hour calculations)
- [ ] Document API surface for Python scripts

**Phase 3: Python Integration Interface**
- [ ] Create Python directory structure
- [ ] Document database access patterns for Python
- [ ] Create example Python script that reads from DB
- [ ] Define data contract (Rust curates, Python analyzes)
- [ ] Add configuration for Python paths/environments

**Phase 4: Documentation Updates**
- [ ] Update README with new architecture
- [ ] Update analysis documentation to reflect Python migration
- [ ] Create Python development guide
- [ ] Document deployment as daemon service

---

### Daemon Lifecycle Features
**Reference:** `flomon_service/tests/daemon_lifecycle.rs`

**Backfill and Gap Detection:**
- [ ] Implement `daemon.backfill_station()` function
  - Detect empty database (no readings for station)
  - Fetch historical data (instantaneous values for last 120 days)
  - Insert into `usgs_raw.gauge_readings`
  - Update `monitoring.station_state`

- [ ] Implement gap detection
  - Detect gaps in time series data
  - Log gap locations and durations
  - Prioritize critical gaps for backfill

- [ ] Implement gap filling
  - Backfill detected gaps from USGS API
  - Validate backfilled data
  - Update monitoring state

**Polling and Warehousing:**
- [ ] Implement daemon polling loop
  - Poll each station every 15 minutes
  - Handle station polling schedule
  - Manage polling rotation

- [ ] Implement `daemon.poll_and_warehouse()`
  - Parse and validate USGS API responses
  - Insert new readings (idempotent)
  - Update monitoring state
  - Prevent duplicate readings

- [ ] Implement monitoring state updates
  - Record latest reading timestamp
  - Record last poll attempt
  - Reset failure counter on success

**Error Handling and Resilience:**
- [ ] Implement comprehensive error handling
  - Log API failures
  - Increment failure counter
  - Continue polling other stations
  - Retry failed stations

- [ ] Implement staleness alerting
  - Alert when station hasn't reported for >60 minutes
  - Continue monitoring (don't crash)
  - Clear alert when fresh data arrives

---

### Station Resilience
**Reference:** `flomon_service/docs/STATION_RESILIENCE.md`

**Future Enhancements:**
- [ ] Station Health Dashboard - Real-time availability display
- [ ] Automatic Backfill - When station recovers, fetch missed data
- [ ] Redundant Stations - Define backup stations for critical locations
- [ ] Parameter Fallbacks - Estimate missing values from nearby stations
- [ ] USGS Status API - Query site status instead of waiting for failures
- [ ] Multiple daemon instances support

---

### Endpoint Enhancements
**Reference:** `riverviews.wiki/ZONE_ENDPOINT_MIGRATION.md`

- [ ] Add rate-of-rise calculations for Zone 3 (Mackinaw River)
- [ ] Implement forecast endpoint (currently stubbed)
- [ ] Add WebSocket support for real-time zone updates
- [ ] Calculate backwater risk based on Mississippi River elevations (see `endpoint.rs.old:640`)

---

### REST API (Future)
**Reference:** `flomon_service/docs/PYTHON_INTEGRATION.md`

- [ ] Design REST API for Rust daemon
- [ ] Implement HTTP server in daemon
- [ ] Create API endpoints for current conditions
- [ ] Add authentication/authorization
- [ ] Document API specifications
- [ ] Create Python client library

---

## 🐍 Python/FloML Analysis Features

### Testing
- [ ] Implement unit tests for FloML package (marked TODO in `floml/QUICKSTART.md`)
- [ ] Add integration tests for database connections
- [ ] Create test fixtures for analysis functions

### Advanced Analysis
**Reference:** `flomon_service/docs/REFACTORING_PLAN.md`

**These features are moving from Rust to Python:**
- [ ] Precursor pattern detection
- [ ] Flood event classification
- [ ] Backwater influence modeling
- [ ] Time series analysis
- [ ] Anomaly detection
- [ ] Confidence intervals and uncertainty quantification
- [ ] Advanced visualization (charts, plots, dashboards)
- [ ] Automated report generation

### Correlation and Regression
- [ ] Stage-discharge relationships
- [ ] Upstream-downstream correlations
- [ ] Multi-variate flood prediction models
- [ ] Segmented regression improvements

---

## 🗄️ Database and Schema

### Threshold Management
**Reference:** `flomon_service/docs/THRESHOLD_STRATEGY.md`

**ML-Discovered Thresholds:**
- [ ] Implement segmented regression for threshold discovery
  - Find channel → bankfull → floodplain transitions
  - Identify natural physical breakpoints

- [ ] Implement historical correlation analysis
  - Identify stages that correlate with actual impacts
  - Analyze road closures, property damage

- [ ] Implement multi-station threshold optimization
  - Optimize upstream thresholds based on downstream effects

- [ ] Create `flood_analysis.recommended_thresholds` table
- [ ] Implement threshold update mechanism
- [ ] Implement dual threshold comparison (static vs. ML-discovered)

---

### Schema Extensions
**Reference:** `flomon_service/docs/SCHEMA_EXTENSIBILITY.md`

**Planned Migrations:**
- [ ] `sql/005_precipitation.sql` - NOAA rainfall observations and forecasts
- [ ] `sql/006_soil_moisture.sql` - NRCS/SNOTEL soil saturation data
- [ ] `sql/007_nws_forecasts.sql` - AHPS stage predictions and alerts
- [ ] `sql/008_dam_operations.sql` - Planned dam releases impacting flood risk

**Future Schema Expansions:**
- [ ] Add dam operations tracking
- [ ] Add reservoir level monitoring
- [ ] Add ice jam detection data
- [ ] Add flood impact records (property damage, road closures)

---

## 🔧 Monitoring and Operations

### Prometheus Metrics (Future Enhancement)
**Reference:** `flomon_service/docs/LOGGING_AND_ERROR_HANDLING.md`

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

**Reference:** `riverviews.wiki/ZONE_ENDPOINT_MIGRATION.md`

- [ ] Update dashboards to consume zone-based endpoints
- [ ] Refactor Python analysis scripts to use `/status` endpoint
- [ ] Create real-time dashboard with WebSocket updates
- [ ] Add historical flood event visualization
- [ ] Create interactive basin map
- [ ] Add precipitation overlay visualization
- [ ] Create public-facing status page

---

## 🧪 Testing

### Integration Tests
**Reference:** `flomon_service/tests/`

- [ ] Expand ASOS integration tests
- [ ] Add CWMS availability tests
- [ ] Add end-to-end workflow tests
- [ ] Add performance benchmarks
- [ ] Create load testing suite

### Validation
- [ ] Add data quality validation rules
- [ ] Implement outlier detection
- [ ] Add cross-source validation (USGS vs. CWMS comparison)
- [ ] Create validation reporting dashboard

---

## 📚 Documentation

### Needed Documentation
- [ ] API reference documentation
- [ ] Deployment guide for production
- [ ] Configuration guide (all TOML options)
- [ ] Database backup and recovery procedures
- [ ] Monitoring and alerting setup guide
- [ ] Troubleshooting guide
- [ ] Data source integration guide (for adding new sources)
- [ ] Python FloML API documentation
- [ ] Example analysis workflows

### Wiki Updates
- [ ] Document threshold discovery methodology
- [ ] Add soil moisture integration guide
- [ ] Add precipitation analysis guide
- [ ] Document backwater correlation algorithm
- [ ] Add flood forecasting methodology

---

## 🏗️ Infrastructure

### Deployment
- [ ] Create Docker containerization
- [ ] Create docker-compose setup
- [ ] Add Kubernetes deployment manifests
- [ ] Create systemd service files
- [ ] Document production deployment process
- [ ] Add automated backup scripts

### CI/CD
- [ ] Set up continuous integration
- [ ] Add automated testing pipeline
- [ ] Add code coverage reporting
- [ ] Create automated release process
- [ ] Add database migration testing

### Performance
- [ ] Add database query optimization
- [ ] Implement caching for frequently accessed data
- [ ] Add connection pooling optimization
- [ ] Profile and optimize hot paths
- [ ] Add database partitioning for large tables

---

## 🎯 Priority Matrix

### Critical (Do First)
1. Complete Rust daemon polling and warehousing
2. Implement staleness alerting
3. Add NWS live alert integration
4. Complete ASOS precipitation integration
5. Update dashboards for zone endpoints

### High Priority
1. Add NOAA precipitation forecasts
2. Implement gap detection and backfill
3. Create REST API for daemon
4. Implement ML-discovered thresholds
5. Add forecast endpoint

### Medium Priority
1. Add soil moisture monitoring
2. Implement station health dashboard
3. Add WebSocket support
4. Create public status page
5. Implement Prometheus metrics

### Future/Nice-to-Have
1. Multiple daemon instances
2. Advanced visualization dashboards
3. Automated report generation
4. Mobile app integration
5. Machine learning flood prediction models

---

## 📝 Notes

### Known Limitations
- USGS IV API limited to 120 days of historical data
- System date in future will cause "no data available" errors (expected behavior)
- Some ASOS observations may have null values for precip, wind, or pressure
- CWMS data availability varies by location

### Design Decisions
- Rust handles data curation and reliability
- Python handles statistical analysis and ML
- PostgreSQL multi-schema architecture for data source separation
- TOML configuration for human-readable station definitions
- Zone-based API for geographic flood analysis

---

**Contributing:** When implementing features from this list, please:
1. Create a feature branch
2. Update this document to mark items as in-progress or complete
3. Reference this TODO in commit messages
4. Add appropriate tests
5. Update relevant documentation

**Questions?** See project wiki at [riverviews.wiki/](riverviews.wiki/)
