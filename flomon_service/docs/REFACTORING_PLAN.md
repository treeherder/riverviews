# Refactoring Plan: Rust-Python Separation

**Date:** February 20, 2026  
**Objective:** Separate concerns between Rust monitoring daemon and Python analysis scripts

## Architecture Vision

### Rust Service: Core Monitoring Daemon
**Purpose:** Server-side data curation and reliable alerting

**Responsibilities:**
- **Data Ingestion**
  - USGS real-time gauge data
  - USACE CWMS timeseries data
  - NWS flood event records
  - Historical peak flow ingestion

- **Data Curation**
  - Validation and quality checks
  - Staleness tracking
  - Data normalization
  - Database storage and indexing

- **Simple Monitoring**
  - Threshold-based alerts (flood stage, action stage)
  - Rate-of-rise detection (simple ft/hour calculations)
  - Staleness alerts (data gaps)
  - Multi-station comparison (simple differentials)

- **Data Service**
  - Clean, reliable data access for external scripts
  - Database views for common queries
  - Simple REST API (future consideration)
  - Event logging and audit trails

### Python Scripts: Complex Analysis
**Purpose:** Statistical analysis, regression, and ML tasks

**Responsibilities:**
- **Regression Analysis**
  - Stage-discharge relationships
  - Upstream-downstream correlations
  - Multi-variate flood prediction models

- **Pattern Recognition**
  - Precursor pattern detection
  - Flood event classification
  - Backwater influence modeling

- **Statistical Computing**
  - Time series analysis
  - Anomaly detection
  - Confidence intervals and uncertainty quantification

- **Visualization**
  - Charts, plots, dashboards
  - Report generation

## Components to Remove/Simplify

### Remove Complex Analysis Binaries
1. **`analyze_flood_events`** - Complex precursor detection, metrics computation
   - Move to Python: Precursor window detection, rise rate analysis, event classification
   
2. **`detect_backwater`** - Currently just reporting, but planned for complex correlation
   - Keep simple reporting in Rust (current state differential)
   - Move to Python: Statistical correlation, backwater influence scoring

### Simplify Analysis Module
1. **`src/analysis/flood_events.rs`** - Remove entirely
   - All precursor detection logic → Python
   - All statistical metric computation → Python
   - Event correlation logic → Python

2. **`src/analysis/groupings.rs`** - Keep simplified
   - Basic data grouping by site/time
   - Simple aggregation helpers
   - No statistical analysis

3. **`src/analysis/mod.rs`** - Update documentation
   - Remove references to complex analysis
   - Focus on data organization utilities

### Database Schema Considerations

**Keep in Rust service:**
- `usgs_raw.*` - Raw USGS data ingestion
- `usace.*` - USACE CWMS data
- `nws.*` - NWS flood events and thresholds
- `monitoring.*` - Staleness, alerts, simple metrics

**Available for Python analysis:**
- `flood_analysis.*` - Python writes analysis results here
- New views/tables for Python output
- Analysis configuration tables (Python can read thresholds, configs)

## Migration Steps

### Phase 1: Remove Complex Analysis (Rust)
- [x] Document refactoring plan
- [ ] Remove `src/bin/analyze_flood_events.rs`
- [ ] Remove `src/analysis/flood_events.rs`
- [ ] Simplify `src/analysis/mod.rs` and `groupings.rs`
- [ ] Update dependencies in `Cargo.toml` if any are analysis-only
- [ ] Clean up documentation references

### Phase 2: Enhance Core Monitoring (Rust)
- [ ] Ensure `main.rs` is structured as a daemon
- [ ] Add simple threshold monitoring
- [ ] Add staleness alerting
- [ ] Add basic rate-of-rise detection (simple)
- [ ] Document API surface for Python scripts

### Phase 3: Python Integration Interface
- [ ] Create Python directory structure
- [ ] Document database access patterns for Python
- [ ] Create example Python script that reads from DB
- [ ] Define data contract (what Rust curates, what Python analyzes)
- [ ] Add configuration for Python paths/environments

### Phase 4: Documentation Updates
- [ ] Update README with new architecture
- [ ] Update analysis documentation to reflect Python migration
- [ ] Create Python development guide
- [ ] Document deployment as daemon service

## Data Flow After Refactoring

```
┌─────────────────────────────────────────────────────┐
│                  External Data Sources               │
│          (USGS, USACE, NWS, Historical Files)       │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│             Rust Monitoring Daemon                   │
│  ┌───────────────────────────────────────────────┐  │
│  │  Ingestion: Fetch, Parse, Validate           │  │
│  └─────────────────────┬─────────────────────────┘  │
│  ┌───────────────────────────────────────────────┐  │
│  │  Curation: Store, Index, Track Staleness     │  │
│  └─────────────────────┬─────────────────────────┘  │
│  ┌───────────────────────────────────────────────┐  │
│  │  Simple Alerts: Thresholds, Gaps, Rate       │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────┬───────────────────────────┘
                          │
                          ▼
          ┌───────────────────────────────┐
          │    PostgreSQL Database        │
          │  (Clean, Validated Data)      │
          └───────┬───────────────────────┘
                  │
      ┌───────────┴───────────┐
      ▼                       ▼
┌─────────────┐    ┌──────────────────────┐
│  Python     │    │  Other Scripts/      │
│  Analysis   │    │  Applications        │
│  Scripts    │    │                      │
└─────────────┘    └──────────────────────┘
```

## Benefits

1. **Separation of Concerns**
   - Rust focuses on reliability, performance, uptime
   - Python focuses on analysis flexibility, statistical libraries

2. **Development Velocity**
   - Faster iteration on analysis algorithms in Python
   - Stable, reliable data platform in Rust

3. **Right Tool for Job**
   - Rust: Systems programming, daemon, data integrity
   - Python: NumPy, SciPy, Pandas, scikit-learn, matplotlib

4. **Maintainability**
   - Simpler Rust codebase
   - Analysis code in familiar Python ecosystem
   - Clear interfaces between components

## Next Steps

1. Complete Phase 1 (remove complex Rust analysis)
2. Test that simple monitoring still works
3. Set up Python environment and directory
4. Write first Python analysis script as proof of concept
5. Document integration patterns
