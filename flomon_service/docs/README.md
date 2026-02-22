# Riverviews Monitoring Service Documentation

Technical architecture and implementation documentation for the Rust monitoring daemon.

## 📚 Documentation Index

### Getting Started

- [DATABASE_SETUP.md](DATABASE_SETUP.md) - PostgreSQL schema setup and configuration
- [TOML_CONFIGURATION.md](TOML_CONFIGURATION.md) - Zone and station configuration

### Data Sources

- [USGS_DATA_SERVICES.md](USGS_DATA_SERVICES.md) - USGS NWIS API integration
- [CWMS_INTEGRATION_SUMMARY.md](CWMS_INTEGRATION_SUMMARY.md) - USACE CWMS integration
- [ASOS_IMPLEMENTATION.md](ASOS_IMPLEMENTATION.md) - IEM ASOS weather station integration
- [DATA_SOURCE_VERIFICATION.md](DATA_SOURCE_VERIFICATION.md) - Data source testing and validation

### Architecture & Design

- [EXTENSIBLE_ARCHITECTURE.md](EXTENSIBLE_ARCHITECTURE.md) - System architecture overview
- [DATA_STORAGE_STRATEGY.md](DATA_STORAGE_STRATEGY.md) - Database design patterns
- [SCHEMA_EXTENSIBILITY.md](SCHEMA_EXTENSIBILITY.md) - Schema evolution strategy
- [PYTHON_INTEGRATION.md](PYTHON_INTEGRATION.md) - Rust-Python integration patterns

### Operational Patterns

- [PRE_INGESTION_STRATEGY.md](PRE_INGESTION_STRATEGY.md) - Historical data backfill
- [STATION_RESILIENCE.md](STATION_RESILIENCE.md) - Error handling and recovery
- [THRESHOLD_STRATEGY.md](THRESHOLD_STRATEGY.md) - Alert threshold configuration
- [VALIDATION_SYSTEM.md](VALIDATION_SYSTEM.md) - Data validation approach
- [LOGGING_AND_ERROR_HANDLING.md](LOGGING_AND_ERROR_HANDLING.md) - Logging patterns

### Deprecated

- [REFACTORING_PLAN.md](REFACTORING_PLAN.md) - Historical refactoring notes (archived)

## Quick Links

- Main README: [../README.md](../README.md)
- FloML Analysis Package: [../../floml/README.md](../../floml/README.md)
- Visualization Tools: [../../floml/scripts/README.md](../../floml/scripts/README.md)
