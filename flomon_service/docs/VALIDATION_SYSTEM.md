# Database Setup & Validation Summary

## What Was Created

I've created a comprehensive database validation system to prevent silent PostgreSQL failures and provide clear setup guidance:

### 1. **Validation Script** (`scripts/validate_db_setup.sh`)
   - Checks PostgreSQL installation and service status
   - Verifies database and user existence
   - Validates schema permissions
   - Checks environment configuration (.env)
   - Provides step-by-step remediation instructions

   **Usage:**
   ```bash
   cd flomon_service
   ./scripts/validate_db_setup.sh
   ```

### 2. **Permission Grant Script** (`scripts/grant_permissions.sql`)
   - Grants all necessary permissions to `flopro_admin` user
   - Works idempotently (safe to re-run)
   - Verifies permissions after granting

   **Usage:**
   ```bash
   psql -U postgres -d flopro_db -f scripts/grant_permissions.sql
   ```

### 3. **Database Setup Guide** (`docs/DATABASE_SETUP.md`)
   - Complete setup instructions from scratch
   - Troubleshooting section for common errors
   - Production deployment guidelines
   - Backup and recovery procedures
   - Migration management best practices

### 4. **Database Module** (`src/db.rs`)
   - Centralized database connection with validation
   - Clear, actionable error messages
   - Schema permission verification
   - Shared across all binaries and tests

## Key Improvements

### Before (Silent Failures)
```rust
let db_url = env::var("DATABASE_URL")
    .expect("DATABASE_URL must be set");  // ❌ Unclear error
Client::connect(&db_url, NoTls)
    .expect("Failed to connect");  // ❌ No guidance
```

### After (Helpful Errors)
```rust
flomon_service::db::connect_and_verify(&["usgs_raw", "nws"])
    .unwrap_or_else(|e| {
        eprintln!("\n{}\n", e);  // ✅ Clear error explanation
        eprintln!("Run setup validation: ./scripts/validate_db_setup.sh\n");
        std::process::exit(1);
    });
```

## Error Messages Now Include

- **Missing DATABASE_URL:**
  ```
  DATABASE_URL environment variable not set.

  Required Setup:
  1. Copy .env.example to .env: cp .env.example .env
  2. Edit .env and set DATABASE_URL=postgresql://flopro_admin:password@localhost/flopro_db
  3. Run setup validation: ./scripts/validate_db_setup.sh

  For complete setup instructions, see: docs/DATABASE_SETUP.md
  ```

- **Connection Failed:**
  ```
  Failed to connect to PostgreSQL database.

  Error: connection to server at "localhost", port 5432 failed

  Common causes:
  - PostgreSQL service not running (check: pg_isready)
  - Database 'flopro_db' does not exist
  - User 'flopro_admin' does not exist
  - Incorrect password in DATABASE_URL
  - pg_hba.conf does not allow local connections

  Run setup validation: ./scripts/validate_db_setup.sh
  See: docs/DATABASE_SETUP.md for complete setup instructions
  ```

- **Missing Schema:**
  ```
  Required database schema 'usace' does not exist.

  Run migration scripts in order:
  1. psql -U flopro_admin -d flopro_db -f sql/001_initial_schema.sql
  2. psql -U flopro_admin -d flopro_db -f sql/002_monitoring_state.sql
  3. psql -U flopro_admin -d flopro_db -f sql/003_flood_metadata.sql
  4. psql -U flopro_admin -d flopro_db -f sql/004_usace_cwms.sql

  See: docs/DATABASE_SETUP.md
  ```

- **Permission Denied:**
  ```
  Permission denied for schema 'nws'.

  Grant permissions:
  psql -U postgres -d flopro_db -f scripts/grant_permissions.sql

  Or manually:
  psql -U postgres -d flopro_db -c "GRANT USAGE ON SCHEMA nws TO flopro_admin;"
  psql -U postgres -d flopro_db -c "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA nws TO flopro_admin;"

  See: docs/DATABASE_SETUP.md
  ```

## Updated Components

### Binaries
- `ingest_peak_flows` - Now uses `db::connect_and_verify()`
- `ingest_cwms_historical` - Database validation on startup
- `detect_backwater` - Clear error messages

### Tests
- `peak_flow_integration.rs` - Better error reporting with full context

### All Use Same Infrastructure
- Consistent error messages across the codebase
- No assumptions about existing users/databases
- Clear path to resolution for every error

## Validation Script Example Output

### ✅ Success Case
```
=== Riverviews Database Setup Validation ===

[1/8] Checking PostgreSQL installation...
✓ PostgreSQL 14 installed
[2/8] Checking PostgreSQL service status...
✓ PostgreSQL service is running
[3/8] Checking database 'flopro_db'...
✓ Database 'flopro_db' exists
[4/8] Checking database user 'flopro_admin'...
✓ User 'flopro_admin' exists
[5/8] Checking user permissions...
✓ User can connect to database
[6/8] Checking database schemas...
  ✓ Schema 'usgs_raw' exists
  ✓ Schema 'nws' exists
  ✓ Schema 'usace' exists
[7/8] Checking schema permissions...
  ✓ User has access to schema 'usgs_raw'
  ✓ User has access to schema 'nws'
  ✓ User has access to schema 'usace'
[8/8] Checking critical tables...
  ✓ Table 'usgs_raw.gauge_readings' exists
  ✓ Table 'nws.flood_thresholds' exists
  ✓ Table 'nws.flood_events' exists
  ✓ Table 'usace.cwms_locations' exists
  ✓ Table 'usace.cwms_timeseries' exists

[ENV] Checking environment configuration...
✓ .env file exists
✓ DATABASE_URL is set
✓ DATABASE_URL format looks valid

=== Validation Summary ===
✓ All checks passed! Database is properly configured.

You can now run:
  cargo test --test peak_flow_integration
  cargo run --bin ingest_peak_flows
  cargo run --bin ingest_cwms_historical
```

### ❌ Failure Case (with remediation steps)
```
=== Riverviews Database Setup Validation ===

[1/8] Checking PostgreSQL installation...
✓ PostgreSQL 14 installed
[2/8] Checking PostgreSQL service status...
✓ PostgreSQL service is running
[3/8] Checking database 'flopro_db'...
✓ Database 'flopro_db' exists
[4/8] Checking database user 'flopro_admin'...
✓ User 'flopro_admin' exists
[5/8] Checking user permissions...
✓ User can connect to database
[6/8] Checking database schemas...
  ✓ Schema 'usgs_raw' exists
  ✓ Schema 'nws' exists
  ✗ Schema 'usace' missing
[7/8] Checking schema permissions...
  ✓ User has access to schema 'usgs_raw'
  ✗ User lacks access to schema 'nws'

=== Validation Summary ===
✗ Validation failed. Setup steps required:

Required Setup Steps:
────────────────────
1. Run migrations: psql -U flopro_admin -d flopro_db -f sql/004_usace_cwms.sql
2. Grant schema access: psql -U postgres -d flopro_db -c "GRANT USAGE ON SCHEMA nws TO flopro_admin;"
3.                       psql -U postgres -d flopro_db -c "GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA nws TO flopro_admin;"

Quick Setup (if starting fresh):
──────────────────────────────
[... full setup script provided ...]
```

## Testing It

1. **Run validation:**
   ```bash
   cd flomon_service
   chmod +x scripts/validate_db_setup.sh
   ./scripts/validate_db_setup.sh
   ```

2. **Try running a binary without setup:**
   ```bash
   cargo run --bin ingest_peak_flows
   ```
   
   You'll get a clear error message pointing to the validation script.

3. **Check compile status:**
   ```bash
   cargo check --all-targets
   ```

## Addressing the Alternate Buffer Issue

The "alternate buffer" problem appears to be related to:

1. **PostgreSQL pager:** psql automatically uses a pager for large output
2. **Shell configuration:** Terminal settings or .psqlrc enabling pager  
3. **Silent warnings:** PostgreSQL notices/warnings triggering pager

The validation script works around this using proper error handling and redirects.

## Next Steps

1. Run the validation script to check your current setup
2. Follow any remediation steps it provides
3. Use the new `db::connect_and_verify()` in any future code
4. Refer to `docs/DATABASE_SETUP.md` for detailed documentation

All database access now goes through a single validation layer that provides clear, actionable error messages instead of silent failures.
