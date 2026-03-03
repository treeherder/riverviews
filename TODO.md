# Riverviews TODO

This document tracks planned features and future enhancements for the Riverviews flood monitoring project.

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
