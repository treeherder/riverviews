/// Data organization utilities for the flood monitoring service.
///
/// This module provides basic data grouping and organization helpers.
/// Complex statistical analysis, regression, and pattern detection
/// are handled by external Python scripts that read from the curated
/// database.
///
/// Submodules:
/// - `groupings` — organizes flat ingest output into per-site structures.

pub mod groupings;
