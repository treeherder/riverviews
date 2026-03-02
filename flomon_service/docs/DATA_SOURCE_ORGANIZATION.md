## Overview

This document describes the proper separation of data sources in flomon_service. Each external data source has its own dedicated TOML configuration file with clear responsibilities.

## Configuration Files

### Active Configuration Files

| File | Purpose | Loader Module | Status |
|------|---------|---------------|--------|
| `usgs_stations.toml` | USGS real-time gauge stations | `stations.rs` / `config.rs` | ✅ Active |
| `usace_stations.toml` | USACE/CWMS lock & dam locations | `usace_locations.rs` | ✅ Active |
| `iem_asos.toml` | IEM ASOS weather stations + IEM API endpoints | `asos_locations.rs` | ✅ Active |
| `zones.toml` | Hydrological zone groupings (not a data source) | `zones.rs` | ✅ Active |

## Data Source Pattern

All data sources follow this consistent pattern:

```
External API/Service
    ↓
TOML Configuration File (stations/locations + metadata)
    ↓
Loader Module (src/{source}_locations.rs or src/config.rs)
    ↓
Daemon Integration (src/daemon.rs)
    ↓
Ingest Module (src/ingest/{source}.rs)
    ↓
Database (PostgreSQL schemas)
```