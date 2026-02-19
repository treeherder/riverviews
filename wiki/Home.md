# FloPro: Peoria Flood Monitoring Service

Welcome to the FloPro (Flood Monitoring for Peoria) technical documentation wiki.

## What is FloPro?

FloPro is a real-time flood monitoring and early warning system for the Peoria, IL region on the Illinois River. The service continuously monitors USGS river gauge stations throughout the Illinois River basin to provide early flood warnings with 24-48 hour lead time from upstream monitoring points.

## System Overview

```
┌─────────────────┐
│   USGS NWIS     │  Data Source: Federal gauge network
│   API Endpoints │  (Instantaneous Values + Daily Values)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Data Ingestion │  Rust client with resilience
│   & Parsing     │  (src/ingest/usgs.rs)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   PostgreSQL    │  Multi-schema warehouse
│    Database     │  (87 years historical + real-time)
└────────┬────────┘
         │
         ├──────────────────┬────────────────┐
         ▼                  ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  Staleness   │  │    Flood     │  │  Dashboard   │
│  Monitoring  │  │   Alerting   │  │  & Reports   │
└──────────────┘  └──────────────┘  └──────────────┘
```

## Quick Navigation

### Core Documentation
- **[[Data Sources]]** - USGS gauge network and monitoring stations
- **[[Database Architecture]]** - PostgreSQL schema and design decisions
- **[[Staleness Tracking]]** - Ensuring data freshness and quality
- **[[Station Registry]]** - 8 monitored gauge sites on Illinois River system

### Technical Details
- **[[Technology Stack]]** - Why Rust, PostgreSQL, and our architecture choices
- **[[Historical Data Ingestion]]** - 87-year backfill strategy (DV + IV)
- **[[Real-Time Monitoring]]** - Continuous polling and alert system
- **[[Station Resilience]]** - Handling offline gauges gracefully

### Operational Guides
- **[[Database Setup]]** - PostgreSQL installation and schema deployment
- **[[Running Ingest]]** - Historical backfill and incremental updates
- **[[Monitoring Service]]** - Real-time polling service configuration

## Current Status (February 2026)

### ✅ Completed
- USGS API client (IV + DV endpoints)
- PostgreSQL database with 87 years of history
- Station parameter tracking and validation
- Integration tests for live API verification
- Hybrid staleness tracking (database + in-memory)
- Graceful handling of offline stations

### 🚧 In Progress
- Real-time monitoring service (main.rs)
- Alert dispatch system
- Web dashboard

### 📊 Station Health
**6 of 8 stations operational** (verified Feb 19, 2026)
- ✅ Kingston Mines (05568500) - Primary gauge
- ✅ Peoria (05567500)
- ✅ Chillicothe (05568000)
- ✅ Spoon River (05570000)
- ✅ Marseilles (05552500)
- ✅ Chicago Canal (05536890)
- ❌ Henry (05557000) - Offline
- ❌ Mackinaw River (05568580) - Offline

## Key Features

### Early Warning System
- **24-48 hour lead time** from upstream gauges
- **Multi-site correlation** across Illinois Waterway
- **Historical context** from 87 years of data (1939-present)

### Data Quality
- **Automatic staleness detection** (20-60 minute thresholds)
- **Station health monitoring** with degraded/offline states
- **Valid measurements only** - no placeholder records
- **Sentinel value filtering** - strips USGS -999999 markers

### Operational Resilience
- **Graceful degradation** - continues with available stations
- **Resumable ingestion** - year-by-year state tracking
- **Integration tests** - verify API before deployment
- **Database constraints** - prevent duplicate readings

## Repository Structure

```
flomon_service/
├── src/
│   ├── bin/
│   │   └── historical_ingest.rs  # DV+IV backfill
│   ├── ingest/
│   │   └── usgs.rs               # API client
│   ├── monitor/
│   │   └── mod.rs                # Staleness tracking
│   ├── alert/
│   │   ├── thresholds.rs         # Flood stage detection
│   │   └── stalenesses.rs        # Data age checking
│   ├── analysis/
│   │   └── groupings.rs          # Multi-site analysis
│   ├── stations.rs               # Station registry
│   └── model.rs                  # Core data structures
├── sql/
│   ├── 001_initial_schema.sql    # Database schema
│   └── 002_monitoring_metadata.sql # Staleness tables
├── docs/
│   ├── STATION_RESILIENCE.md     # Offline handling
│   └── DATA_STORAGE_STRATEGY.md  # Storage design
└── tests/                        # Integration tests
```

## Getting Started

1. **Setup Database**: See [[Database Setup]]
2. **Run Historical Ingest**: See [[Running Ingest]]
3. **Verify Stations**: `cargo test --ignored station_api_verify_all`
4. **Start Monitoring**: See [[Monitoring Service]] (coming soon)

## External Resources

**Data Sources:**
- [USGS NWIS](https://waterservices.usgs.gov/) - National Water Information System
- [NWS AHPS](https://water.weather.gov/ahps/) - Advanced Hydrologic Prediction Service

**Development:**
- [GitHub Repository](https://github.com/treeherder/illinois_river_flood_warning)
- [Rust Edition 2024](https://doc.rust-lang.org/)
- [PostgreSQL 14+](https://www.postgresql.org/)

---

**Last Updated:** February 19, 2026  
**Project Status:** Active Development  
**License:** TBD
