# Integration Tests

## Overview

Integration tests verify the end-to-end peak flow ingestion pipeline:
- Database schema compatibility
- RDB parsing
- Flood event detection
- Data insertion and querying

## Quick Setup

**Automated validation (recommended):**
```bash
cd flomon_service
./scripts/validate_db_setup.sh
```

This script checks your PostgreSQL setup and provides specific remediation steps.  
See [scripts/README.md](../scripts/README.md) for details.

**Complete manual setup:** See [docs/DATABASE_SETUP.md](../docs/DATABASE_SETUP.md)

## Prerequisites

### 1. PostgreSQL Database

The database must be running with these migrations applied:

```bash
# Assuming PostgreSQL is installed and running
psql -U postgres

CREATE DATABASE flopro_db;
CREATE USER flopro_admin WITH PASSWORD 'flopro_password';
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;
\c flopro_db
CREATE SCHEMA usgs_raw;
CREATE SCHEMA nws;
GRANT ALL ON SCHEMA usgs_raw TO flopro_admin;
GRANT ALL ON SCHEMA nws TO flopro_admin;
\q
```

### 2. Apply SQL Migrations

```bash
# From flomon_service directory
psql -U flopro_admin -d flopro_db -f sql/001_initial_schema.sql
psql -U flopro_admin -d flopro_db -f sql/002_monitoring_state.sql
psql -U flopro_admin -d flopro_db -f sql/003_flood_metadata.sql
```

### 3. Configure Environment

Ensure `.env` file exists with database connection:

```dotenv
DATABASE_URL=postgresql://flopro_admin:flopro_password@localhost/flopro_db
```

## Running Tests

### All Integration Tests

```bash
cargo test --test peak_flow_integration -- --test-threads=1 --nocapture
```

**Note:** Use `--test-threads=1` to prevent database conflicts between concurrent tests.

### Individual Tests

```bash
# Non-database tests (always safe to run)
cargo test test_parse_rdb_produces_valid_records --test peak_flow_integration
cargo test test_identify_flood_events_with_peoria_thresholds --test peak_flow_integration
cargo test test_stations_toml_thresholds_match_database_expectations --test peak_flow_integration

# Database tests (require PostgreSQL running)
cargo test test_database_schema_exists --test peak_flow_integration
cargo test test_full_pipeline_rdb_to_database --test peak_flow_integration
```

## Test Categories

### Schema Validation Tests
- `test_database_schema_exists` - Verifies `nws.flood_events` table exists
- `test_database_schema_has_required_columns` - Checks column names and types

### Parser Tests
- `test_parse_rdb_produces_valid_records` - RDB format parsing
- `test_identify_flood_events_with_peoria_thresholds` - Flood classification logic

### Database Integration Tests
- `test_insert_flood_event_into_database` - Single event insertion
- `test_full_pipeline_rdb_to_database` - Complete RDB → database pipeline
- `test_severity_enum_values_accepted_by_database` - Severity enum validation
- `test_duplicate_prevention` - Duplicate event detection

### Configuration Tests
- `test_stations_toml_thresholds_match_database_expectations` - TOML config validation

## Troubleshooting

### "DATABASE_URL must be set"
- Ensure `.env` file exists in `flomon_service/` directory
- Verify DATABASE_URL is correctly formatted

### "table does not exist"
- Apply SQL migrations (see Prerequisites step 2)
- Verify schemas exist: `\dn` in psql

### "permission denied"
- Grant privileges to flopro_admin user
- Check PostgreSQL authentication (pg_hba.conf)

### Test failures with "already exists"
- Tests clean up after themselves
- If interrupted, manually clean test data:
  ```sql
  DELETE FROM nws.flood_events WHERE data_source LIKE '%TEST%';
  ```

## CI/CD Integration

For automated testing, set up a test database:

```bash
# Example GitHub Actions setup
services:
  postgres:
    image: postgres:14
    env:
      POSTGRES_DB: flopro_db_test
      POSTGRES_USER: test_user
      POSTGRES_PASSWORD: test_password
    options: >-
      --health-cmd pg_isready
      --health-interval 10s
      --health-timeout 5s
      --health-retries 5
```

Then apply migrations and run tests:

```bash
psql -U test_user -d flopro_db_test -f sql/*.sql
DATABASE_URL=postgresql://test_user:test_password@localhost/flopro_db_test \
  cargo test --test peak_flow_integration -- --test-threads=1
```
