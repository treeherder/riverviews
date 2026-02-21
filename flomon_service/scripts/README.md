# FloPro Utility Scripts

This directory contains scripts for database setup, diagnostics, historical data analysis,
and flood event characterization.

## Diagnostic and Analysis Scripts

### `test_usgs_services.py` - USGS API Services Diagnostic

**Purpose:** Test all three USGS data services (IV, DV, Peak) to verify availability and data quality.

**Usage:**
```bash
python3 scripts/test_usgs_services.py
```

**Tests performed:**
- IV Service (Instantaneous Values) - last 3 hours
- DV Service (Daily Values) - last 30 days  
- Peak Service (Annual Peaks) - full historical record

---

### `check_db_status.py` - Database Status Check

**Purpose:** Quick database connectivity and data availability check.

**Usage:**
```bash
python3 scripts/check_db_status.py
```

**Checks:**
- Database connectivity
- Available readings per station
- Latest reading timestamps
- Data freshness

---

### `analyze_historical_data.py` - Historical Data Analysis

**Purpose:** Analyze historical data patterns and availability in the database.

**Usage:**
```bash
python3 scripts/analyze_historical_data.py
```

---

## Database Setup Scripts

### `validate_db_setup.sh` - Database Validation

**Purpose:** Comprehensive database setup validation and diagnostic tool.

**What it checks:**
- PostgreSQL installation and version
- PostgreSQL service status
- Database existence (`flopro_db`)
- User account existence (`flopro_admin`)
- Database connection privileges
- Schema existence (`usgs_raw`, `nws`, `usace`)
- Schema access permissions
- Critical tables existence
- Environment configuration (`.env` file and `DATABASE_URL`)

**Usage:**
```bash
cd flomon_service
./scripts/validate_db_setup.sh
```

**Exit codes:**
- `0` - All validation checks passed, database ready to use
- `1` - Validation failed, setup steps required (printed to output)

**Example output on success:**
```
=== Flopro Database Setup Validation ===

[1/8] Checking PostgreSQL installation...
✓ PostgreSQL 14 installed
[2/8] Checking PostgreSQL service status...
✓ PostgreSQL service is running
...
✓ All checks passed! Database is properly configured.
```

**Example output on failure:**
```
=== Validation Summary ===
✗ Validation failed. Setup steps required:

Required Setup Steps:
────────────────────
1. Create database: psql -U postgres -c 'CREATE DATABASE flopro_db;'
2. Create user: psql -U postgres -c "CREATE USER flopro_admin WITH PASSWORD 'your_password';"
...
```

**When to run:**
- Initial setup before first use
- After PostgreSQL configuration changes
- When troubleshooting database connection issues
- Before running integration tests
- As part of CI/CD pipeline

---

### `grant_permissions.sql`

**Purpose:** Grant all necessary database permissions to the `flopro_admin` user.

**What it does:**
- Verifies `flopro_admin` user exists (errors if not)
- Grants connection privileges on `flopro_db`
- Grants USAGE on all application schemas
- Grants ALL PRIVILEGES on tables, sequences, and functions in each schema
- Sets default privileges for future objects
- Verifies permissions were granted successfully

**Usage:**
```bash
psql -U postgres -d flopro_db -f scripts/grant_permissions.sql
```

**Requirements:**
- Must run as PostgreSQL superuser (e.g., `postgres`)
- Database `flopro_db` must exist
- User `flopro_admin` must exist
- Schemas must already be created (by running migrations)

**Schemas handled:**
- `usgs_raw` - USGS gauge data
- `nws` - NWS flood events and thresholds
- `usace` - USACE CWMS data and backwater detection
- `public` - System schema

**Idempotent:** Safe to run multiple times without side effects.

**Example output:**
```
BEGIN
...
COMMIT

Permission grants completed successfully!

Verifying schema access:
 schema   | usage_privilege 
----------+-----------------
 nws      | GRANTED
 usace    | GRANTED
 usgs_raw | GRANTED
```

**When to run:**
- After creating database and user
- After running schema migrations
- When permission errors occur
- After creating new schemas

---

### `generate_flood_zone_snapshots.py` - Historical Flood Analysis

**Purpose:** Generate zone-based snapshots for all historical flood events to support 
regression analysis and ML model training.

**What it does:**
- Queries historical flood events from `nws.flood_events`
- For each event, fetches sensor readings ±6 hours from crest time
- Organizes readings by hydrological zone (7 zones from zones.toml)
- Classifies event type: TOP_DOWN, BOTTOM_UP, LOCAL_TRIBUTARY, or COMPOUND
- Detects backwater conditions (Mississippi River influence)
- Identifies upstream flood pulses
- Generates comprehensive markdown report for analysis

**Usage:**
```bash
cd flomon_service/scripts
python3 generate_flood_zone_snapshots.py [options]
```

**Options:**
- `--db-url URL` - Database connection (default: from .env)
- `--zones-config PATH` - Path to zones.toml (default: ../zones.toml)
- `--output FILE` - Output markdown file (default: ../../PEAK_FLOW_SUMMARY.md)

**Prerequisites:**
- Python 3.7+ with `psycopg2-binary` and `toml` packages
- Database populated with flood events (run `ingest_peak_flows` first)
- zones.toml configuration file
- Optional: Historical gauge readings for complete sensor data

**Output:**
- Markdown report with zone status at each flood crest
- Event type classification
- Backwater/upstream pulse indicators
- Sensor readings organized by zone
- Ready for regression analysis

**Example:**
```bash
# Install dependencies
pip install psycopg2-binary toml

# Generate report
python3 generate_flood_zone_snapshots.py
```

**See:** [README_ZONE_SNAPSHOTS.md](README_ZONE_SNAPSHOTS.md) for complete documentation

---

## Typical Setup Workflow

### First-Time Setup

1. **Create database and user** (as postgres superuser):
   ```bash
   psql -U postgres << SQL
   CREATE DATABASE flopro_db;
   CREATE USER flopro_admin WITH PASSWORD 'your_secure_password';
   GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;
   \c flopro_db
   GRANT ALL ON SCHEMA public TO flopro_admin;
   SQL
   ```

2. **Run schema migrations** (as flopro_admin):
   ```bash
   cd flomon_service
   psql -U flopro_admin -d flopro_db -f sql/001_initial_schema.sql
   psql -U flopro_admin -d flopro_db -f sql/002_monitoring_state.sql
   psql -U flopro_admin -d flopro_db -f sql/003_flood_metadata.sql
   psql -U flopro_admin -d flopro_db -f sql/004_usace_cwms.sql
   ```

3. **Grant permissions** (as postgres superuser):
   ```bash
   psql -U postgres -d flopro_db -f scripts/grant_permissions.sql
   ```

4. **Configure environment**:
   ```bash
   cp .env.example .env
   # Edit .env and set DATABASE_URL with your password
   ```

5. **Validate setup**:
   ```bash
   ./scripts/validate_db_setup.sh
   ```

### Troubleshooting

**Problem:** Validation script fails with permission errors

**Solution:**
```bash
psql -U postgres -d flopro_db -f scripts/grant_permissions.sql
```

---

**Problem:** "Database does not exist" error

**Solution:**
```bash
psql -U postgres -c "CREATE DATABASE flopro_db;"
```

---

**Problem:** "Role does not exist" error

**Solution:**
```bash
psql -U postgres << SQL
CREATE USER flopro_admin WITH PASSWORD 'your_password';
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;
SQL
```

---

**Problem:** "Schema does not exist" error

**Solution:** Run migrations in order (see step 2 above)

---

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_DB: flopro_db
          POSTGRES_USER: postgres
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    
    steps:
      - uses: actions/checkout@v2
      
      - name: Setup database
        run: |
          psql -h localhost -U postgres -c "CREATE USER flopro_admin WITH PASSWORD 'test_password';"
          psql -h localhost -U postgres -c "GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;"
          psql -h localhost -U flopro_admin -d flopro_db -f sql/*.sql
          psql -h localhost -U postgres -d flopro_db -f scripts/grant_permissions.sql
        env:
          PGPASSWORD: postgres
      
      - name: Validate setup
        run: |
          cd flomon_service
          ./scripts/validate_db_setup.sh
      
      - name: Run tests
        run: cargo test --all
        env:
          DATABASE_URL: postgresql://flopro_admin:test_password@localhost/flopro_db
```

### Docker Compose Example

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:14
    environment:
      POSTGRES_DB: flopro_db
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
    ports:
      - "5432:5432"
    volumes:
      - ./sql:/docker-entrypoint-initdb.d/migrations
      - ./scripts/grant_permissions.sql:/docker-entrypoint-initdb.d/zzz_permissions.sql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5

  app:
    build: .
    depends_on:
      postgres:
        condition: service_healthy
    environment:
      DATABASE_URL: postgresql://flopro_admin:postgres@postgres/flopro_db
    command: ./scripts/validate_db_setup.sh && cargo run
```

## Additional Resources

- **Complete setup guide:** [docs/DATABASE_SETUP.md](../docs/DATABASE_SETUP.md)
- **Schema documentation:** [docs/SCHEMA_EXTENSIBILITY.md](../docs/SCHEMA_EXTENSIBILITY.md)
- **Validation system details:** [docs/VALIDATION_SYSTEM.md](../docs/VALIDATION_SYSTEM.md)
- **Test setup:** [tests/README.md](../tests/README.md)

## Security Notes

1. **Never commit credentials** to version control
2. Use **strong passwords** for database users (16+ chars, mixed case, numbers, symbols)
3. **Restrict network access** via `pg_hba.conf` in production
4. **Rotate passwords** periodically
5. **Use TLS** for remote database connections in production
6. **Backup regularly** - see DATABASE_SETUP.md for backup procedures

## Support

If validation fails and the provided steps don't resolve the issue:

1. Check PostgreSQL logs: `/var/log/postgresql/postgresql-*-main.log`
2. Verify PostgreSQL is running: `systemctl status postgresql`
3. Test connection manually: `psql -U postgres -d flopro_db`
4. Review `pg_hba.conf` authentication settings
5. Consult [docs/DATABASE_SETUP.md](../docs/DATABASE_SETUP.md) troubleshooting section
