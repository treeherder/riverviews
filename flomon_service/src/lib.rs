/// flomon_service: Peoria Illinois River flood risk monitoring service.
///
/// # Module structure
///
/// ```text
/// flomon_service
/// +-- model       - shared data types (GaugeReading, FloodThresholds, NwisError, ...)
/// +-- config      - station registry configuration loader (stations.toml)
/// +-- stations    - USGS site code registry with NWS flood stage thresholds
/// +-- zones       - Hydrological zone-based sensor grouping (zones.toml)
/// +-- usace_locations - USACE/CWMS location registry (usace_iem.toml)
/// +-- asos_locations - ASOS station registry (iem_asos.toml)
/// +-- daemon      - main daemon loop (startup, backfill, polling, warehousing)
/// +-- endpoint    - Zone-based HTTP API for flood monitoring
/// +-- ingest
/// |   +-- usgs    - USGS NWIS IV API: URL construction + JSON parsing
/// |   +-- cwms    - USACE CWMS API: timeseries data retrieval
/// |   +-- iem     - IEM/ASOS weather data API client
/// |   +-- fixtures (test only) - representative API response payloads
/// +-- monitor     - real-time staleness tracking (hybrid DB + in-memory)
/// +-- alert
/// |   +-- thresholds - flood stage severity evaluation
/// |   +-- staleness  - gauge reading freshness checking
/// +-- analysis
///     +-- grouping   - organizes flat readings into per-site or per-zone structs
/// ```

/// Public modules
pub mod alert;
pub mod analysis;
pub mod asos_locations;
pub mod config;
pub mod daemon;
pub mod db;
pub mod endpoint;
pub mod ingest;
pub mod logging;
pub mod model;
pub mod monitor;
pub mod stations;
pub mod usace_locations;
pub mod zones;
