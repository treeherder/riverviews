/// Data analysis for the Peoria flood monitoring service.
///
/// Submodules:
/// - `groupings` — organizes flat ingest output into per-site structures.
///
/// Future additions: rate-of-rise detection, upstream correlation,
/// trend analysis, basin-wide flood potential scoring.

pub mod groupings;
pub mod flood_events;
