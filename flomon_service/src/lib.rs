/// flomon_service: Peoria Illinois River flood risk monitoring service.
///
/// # Module structure
///
/// ```
/// flomon_service
/// ├── model       — shared data types (GaugeReading, FloodThresholds, NwisError, …)
/// ├── config      — station registry configuration loader (stations.toml)
/// ├── stations    — USGS site code registry with NWS flood stage thresholds
/// ├── ingest
/// │   ├── usgs    — USGS NWIS IV API: URL construction + JSON parsing
/// │   └── fixtures (test only) — representative API response payloads
/// ├── monitor     — real-time staleness tracking (hybrid DB + in-memory)
/// ├── alert
/// │   ├── thresholds — flood stage severity evaluation
/// │   └── staleness  — gauge reading freshness checking
/// └── analysis
///     └── grouping   — organizes flat readings into per-site structs
/// ```

pub mod alert;
pub mod analysis;
pub mod config;
pub mod ingest;
pub mod model;
pub mod monitor;
pub mod stations;
