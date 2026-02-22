# Database Setup Guide

This guide provides complete instructions for setting up the PostgreSQL database required by the Riverviews flood monitoring service.

## Prerequisites

- PostgreSQL 12 or later (tested with PostgreSQL 18.2)
- `psql` command-line tool
- Sufficient privileges to create databases and users

## Authentication Methods

Riverviews uses **TCP/IP authentication** (password-based) instead of Unix socket peer authentication for better compatibility across environments.

**TCP/IP Connection (Recommended):**
```bash
# Connects via localhost with password authentication
PGPASSWORD=your_password psql -h localhost -U flopro_admin -d flopro_db
```
- Works from any user account
- Required for containerized deployments
- More flexible for CI/CD pipelines
- The validation script automatically uses this method

**Unix Socket (Admin tasks only):**
```bash
# Requires running as postgres system user
sudo -u postgres psql -d flopro_db
```
- Used for initial setup and migrations
- No password required (peer auth)
- Superuser privileges

## Quick Validation

Before starting, verify your current setup:

```bash
cd flomon_service
./scripts/validate_db_setup.sh
```

This script will:
- Automatically read credentials from `.env` file
- Connect via TCP/IP (`-h localhost`)
- Check PostgreSQL installation and version
- Verify the database exists
- Confirm user accounts and permissions
- Test actual database connectivity with your credentials
- List any missing schemas or tables
- Provide specific remediation steps

## Initial Setup (Fresh Installation)

### 1. Create Database and User

```bash
sudo -u postgres psql << 'SQL'
-- Create database
CREATE DATABASE flopro_db;

-- Create application user
CREATE USER flopro_admin WITH PASSWORD 'your_secure_password_here';

-- Grant database privileges
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;

-- Connect to new database
\c flopro_db

-- Grant schema privileges
GRANT ALL ON SCHEMA public TO flopro_admin;
SQL
```

**Important:** Replace `your_secure_password_here` with a strong password.

### 2. Run Schema Migrations

Apply all database migrations in order:

```bash
cd flomon_service

# Migration 001: Core USGS and NWS schemas
sudo -u postgres psql -d flopro_db -f sql/001_initial_schema.sql

# Migration 002: Monitoring state tracking
sudo -u postgres psql -d flopro_db -f sql/002_monitoring_metadata.sql

# Migration 003: Flood metadata and peak flows
sudo -u postgres psql -d flopro_db -f sql/003_flood_metadata.sql

# Migration 004: USACE CWMS integration
sudo -u postgres psql -d flopro_db -f sql/004_usace_cwms.sql

# Migration 005: Flood event analysis
sudo -u postgres psql -d flopro_db -f sql/005_flood_analysis.sql
```

**Note:** Migrations may show some warnings about index predicates requiring immutable functions. These are non-critical and can be safely ignored.

### 3. Grant Schema Permissions

The migrations create custom schemas that need explicit permissions. Use the automated script:

```bash
sudo -u postgres psql -d flopro_db -f scripts/grant_permissions.sql
```

This grants `flopro_admin` full access to all application schemas (`usgs_raw`, `nws`, `usace`, `flood_analysis`).

### 4. Configure Environment

```bash
# Copy example environment file
cp .env.example .env

# Edit .env and update DATABASE_URL
nano .env  # or your preferred editor
```

Set the `DATABASE_URL` with your password:

```env
DATABASE_URL=postgresql://flopro_admin:your_secure_password_here@localhost/flopro_db
```

### 5. Verify Setup

Run the validation script to confirm everything is configured correctly:

```bash
./scripts/validate_db_setup.sh
```

If all checks pass, you'll see:
```
✓ All checks passed! Database is properly configured.
```

## Testing the Setup

### Run Integration Tests

```bash
cargo test --test peak_flow_integration
```

All 9 tests should pass:
```
test test_parse_peak_flow_multiple_valid ... ok
test test_parse_peak_flow_empty ... ok
test test_insert_flood_event ... ok
test test_find_or_create_threshold_existing ... ok
...
```

### Test Data Ingestion

```bash
# Ingest USGS historical flood events
cargo run --bin ingest_peak_flows

# Expected output:
# Successfully inserted 118 flood events
```

## Troubleshooting

### Peer Authentication Failed

**Symptom:** `FATAL: Peer authentication failed for user "flopro_admin"`

**Cause:** Trying to use Unix socket authentication without proper system user.

**Solution:** Use TCP/IP connection with `-h localhost`:
```bash
PGPASSWORD=your_password psql -h localhost -U flopro_admin -d flopro_db
```

The validation script and Riverviews application automatically use TCP/IP, so this should not occur during normal operation.

### Password Authentication Failed

**Symptom:** `FATAL: password authentication failed for user "flopro_admin"`

**Solutions:**
1. Verify password in `.env` matches the user's password
2. Check DATABASE_URL format is correct:
   ```
   DATABASE_URL=postgresql://flopro_admin:your_password@localhost/flopro_db
   ```
3. Reset password if needed:
   ```bash
   sudo -u postgres psql -c "ALTER USER flopro_admin WITH PASSWORD 'new_password';"
   ```
4. Update `.env` with new password

### Permission Denied Errors

**Symptom:** `ERROR: permission denied for schema usgs_raw`

**Solution:** Re-run permission grants:
```bash
sudo -u postgres psql -d flopro_db -f scripts/grant_permissions.sql
```

### Connection Refused

**Symptom:** `connection to server at "localhost" (::1), port 5432 failed`

**Solutions:**
1. Check PostgreSQL is running: `pg_isready -h localhost`
2. Start PostgreSQL: `sudo systemctl start postgresql`
3. Enable on boot: `sudo systemctl enable postgresql`
4. Check PostgreSQL is listening on TCP/IP:
   ```bash
   sudo netstat -tlnp | grep 5432
   # Should show: tcp ... 127.0.0.1:5432 ... postgres
   ```

### Role Does Not Exist

**Symptom:** `FATAL: role "flopro_admin" does not exist`

**Solution:** Create the user:
```bash
sudo -u postgres psql << SQL
CREATE USER flopro_admin WITH PASSWORD 'your_secure_password';
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;
SQL
```

### Database Does Not Exist

**Symptom:** `FATAL: database "flopro_db" does not exist`

**Solution:** Create the database:
```bash
sudo -u postgres psql -c "CREATE DATABASE flopro_db;"
```

### Testing Database Connection

Verify you can connect with your credentials:

```bash
# Using environment variable
PGPASSWORD=your_password psql -h localhost -U flopro_admin -d flopro_db -c "SELECT version();"

# Or using DATABASE_URL from .env
source .env
psql "$DATABASE_URL" -c "SELECT COUNT(*) FROM usgs_raw.sites;"
```

### Type Compatibility Issues

**Symptom:** `FromSql` trait errors or precision mismatches

**Solution:** Ensure you're using:
- `rust_decimal::Decimal` with `db-postgres` feature for NUMERIC columns
- `chrono::DateTime<Utc>` for TIMESTAMPTZ columns
- Proper type conversions in Rust code

### Silent Failures / Alternate Buffer Issues

**Symptom:** Terminal output disappears, commands open pager unexpectedly

**Causes:**
1. PostgreSQL warnings/notices trigger pager
2. Automatic pager on large result sets
3. `.psqlrc` configuration

**Solutions:**
```bash
# Disable pager for single command
psql -U postgres -d flopro_db -P pager=off -c "\dt"

# Set environment variable
export PAGER=cat
psql -U postgres -d flopro_db -c "\dt"

# Redirect output
psql -U postgres -d flopro_db -c "\dt" 2>&1 | cat

# Check for .psqlrc issues
cat ~/.psqlrc
```

## Schema Overview

After setup, your database will have these schemas:

### `usgs_raw` - USGS Data
- `gauge_readings` - Real-time river gauge observations
- `ingest_log` - API ingestion tracking

### `nws` - Flood Events & Thresholds
- `flood_thresholds` - NWS flood stage definitions (action/minor/moderate/major)
- `flood_events` - Historical flood events from USGS Peak Streamflow

### `usace` - USACE CWMS Data
- `cwms_locations` - Monitored lock/dam sites
- `cwms_timeseries` - Stage/flow/elevation readings
- `lock_operations` - Dam operational data
- `backwater_events` - Mississippi River backwater floods
- `cwms_ingestion_log` - API call tracking

## Security Considerations

### Password Management

- Never commit `.env` file to version control
- Use strong passwords (16+ characters, mixed case, numbers, symbols)
- Consider using PostgreSQL's `.pgpass` file for password-less local access
- Rotate passwords periodically

### Network Access

For production deployments:

1. **Restrict `pg_hba.conf`:**
   ```
   # Only allow local connections
   local   flopro_db    flopro_admin    md5
   host    flopro_db    flopro_admin    127.0.0.1/32    md5
   ```

2. **Use TLS for remote connections:**
   ```rust
   // In production, use TLS instead of NoTls
   use postgres::tls::Connector;
   ```

3. **Principle of least privilege:**
   - Application user (`flopro_admin`) has full access to application schemas
   - No superuser privileges required
   - Read-only users for reporting/analytics

## Production Deployment

### Automated Setup Script

For CI/CD or automated deployments:

```bash
#!/bin/bash
set -euo pipefail

# Source database credentials from secure vault
DB_PASSWORD="${FLOPRO_DB_PASSWORD}"

# Create database
sudo -u postgres psql -c "CREATE DATABASE flopro_db;"

# Create user
sudo -u postgres psql << SQL
CREATE USER flopro_admin WITH PASSWORD '$DB_PASSWORD';
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;
SQL

# Run migrations
sudo -u postgres psql -d flopro_db \
    -f sql/001_initial_schema.sql \
    -f sql/002_monitoring_metadata.sql \
    -f sql/003_flood_metadata.sql \
    -f sql/004_usace_cwms.sql \
    -f sql/005_flood_analysis.sql

# Grant permissions
sudo -u postgres psql -d flopro_db -f scripts/grant_permissions.sql

# Configure environment
echo "DATABASE_URL=postgresql://flopro_admin:${DB_PASSWORD}@localhost/flopro_db" > .env

# Validate
./scripts/validate_db_setup.sh
```

### Backup and Recovery

```bash
# Backup database (using TCP/IP connection)
PGPASSWORD=your_password pg_dump -h localhost -U flopro_admin -d flopro_db -F c -f flopro_db_backup.dump

# Restore database
PGPASSWORD=your_password pg_restore -h localhost -U flopro_admin -d flopro_db -c flopro_db_backup.dump

# Backup only schema
PGPASSWORD=your_password pg_dump -h localhost -U flopro_admin -d flopro_db --schema-only > schema_backup.sql

# Backup only data
PGPASSWORD=your_password pg_dump -h localhost -U flopro_admin -d flopro_db --data-only > data_backup.sql
```

## Migration Management

Future schema changes should:

1. Create new migration file: `sql/00X_description.sql`
2. Include rollback script: `sql/rollback/00X_description.sql`
3. Update `docs/SCHEMA_EXTENSIBILITY.md`
4. Test on development database first
5. Run validation after migration

**Best practices:**
- Never modify existing migrations
- Use transactions for atomic changes
- Include comments explaining the "why"
- Test both upgrade and rollback paths

## Support

If you encounter issues not covered here:

1. Run diagnostic script: `./scripts/validate_db_setup.sh`
2. Check PostgreSQL logs: `/var/log/postgresql/postgresql-XX-main.log`
3. Verify environment: `echo $DATABASE_URL`
4. Test connection: `psql "$DATABASE_URL" -c "SELECT version();"`
5. Review tests/README.md for test-specific setup

## References

- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
- [Rust postgres Crate](https://docs.rs/postgres/latest/postgres/)
- [SCHEMA_EXTENSIBILITY.md](SCHEMA_EXTENSIBILITY.md) - Schema design principles
- [CWMS_INTEGRATION.md](CWMS_INTEGRATION.md) - USACE CWMS setup
