# PostgreSQL Setup for FloPro

## Local Development Setup

### Install PostgreSQL

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install postgresql postgresql-contrib
sudo systemctl start postgresql
sudo systemctl enable postgresql
```

**Check installation:**
```bash
psql --version
# Should show: psql (PostgreSQL) 14.x or higher
```

### Create Database and User

**Switch to postgres user:**
```bash
sudo -u postgres psql
```

**In PostgreSQL console:**
```sql
-- Create database
CREATE DATABASE flopro_db;

-- Create user with password
CREATE USER flopro_admin WITH ENCRYPTED PASSWORD 'your_secure_password_here';

-- Grant privileges
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;

-- Connect to the database
\c flopro_db

-- Grant schema creation permissions
GRANT ALL ON SCHEMA public TO flopro_admin;

-- Exit
\q
```

### Test Connection

```bash
psql -U flopro_admin -d flopro_db -h localhost
# Enter password when prompted
```

### Environment Variables

Create `.env` file in `flomon_service/`:

```bash
# .env
DATABASE_URL=postgresql://flopro_admin:your_secure_password_here@localhost/flopro_db
```

**For production:**
```bash
DATABASE_URL=postgresql://flopro_admin:password@your-vps-ip:5432/flopro_db
```

---

## Database Schema

### Schema Organization

```
flopro_db/
├── usgs_raw.*        -- Raw USGS data (append-only)
├── nws.*             -- NWS forecasts & thresholds
├── noaa.*            -- Weather data (future)
├── usace.*           -- Lock/dam operations (future)
└── public.*          -- Unified views & processed data
```

### Initial Schema (USGS only)

Run the migration file: `sql/001_initial_schema.sql`

---

## Testing Before Production

### 1. Local Test
```bash
# Run initial migration
psql -U flopro_admin -d flopro_db -f sql/001_initial_schema.sql

# Test historical ingest
cargo run --bin historical_ingest

# Verify data
psql -U flopro_admin -d flopro_db -c "SELECT COUNT(*) FROM usgs_raw.gauge_readings;"
```

### 2. Check Data Quality
```sql
-- How many readings per site?
SELECT site_code, COUNT(*) as reading_count
FROM usgs_raw.gauge_readings
GROUP BY site_code
ORDER BY reading_count DESC;

-- Check date range
SELECT 
  MIN(reading_time) as earliest,
  MAX(reading_time) as latest
FROM usgs_raw.gauge_readings;

-- Look for gaps
SELECT site_code, parameter_code, 
       COUNT(*) as readings_per_day,
       DATE(reading_time) as day
FROM usgs_raw.gauge_readings
WHERE reading_time > NOW() - INTERVAL '7 days'
GROUP BY site_code, parameter_code, DATE(reading_time)
ORDER BY site_code, parameter_code, day;
```

### 3. Performance Check
```sql
-- Ensure indexes are used
EXPLAIN ANALYZE
SELECT * FROM usgs_raw.gauge_readings
WHERE site_code = '05568500'
  AND reading_time > NOW() - INTERVAL '24 hours'
ORDER BY reading_time DESC;
```

---

## Production Deployment Checklist

- [ ] Backup local database: `pg_dump flopro_db > flopro_backup.sql`
- [ ] Set up PostgreSQL on VPS
- [ ] Configure firewall (port 5432)
- [ ] Use SSL connections (set `sslmode=require` in DATABASE_URL)
- [ ] Set up automated backups (pg_dump cron job)
- [ ] Monitor disk space (gauge data grows ~500MB/year)
- [ ] Set up connection pooling (PgBouncer recommended)

---

## Monitoring & Maintenance

### Vacuum and Analyze
```sql
-- Run weekly
VACUUM ANALYZE usgs_raw.gauge_readings;
```

### Check Database Size
```sql
SELECT 
  pg_size_pretty(pg_database_size('flopro_db')) as total_size,
  pg_size_pretty(pg_relation_size('usgs_raw.gauge_readings')) as readings_table_size;
```

### Index Health
```sql
SELECT schemaname, tablename, indexname, idx_scan
FROM pg_stat_user_indexes
WHERE schemaname = 'usgs_raw'
ORDER BY idx_scan;
-- If idx_scan is 0, consider dropping unused indexes
```
