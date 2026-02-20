# Quick Reference: Database Setup for New Users

This guide provides the essentials for setting up the FloPro PostgreSQL database.

## Prerequisites

- PostgreSQL 12+ installed and running
- `sudo` access for postgres user

## Setup Commands

From the `flomon_service` directory, run:

```bash
# 1. Create database and user
sudo -u postgres psql << 'SQL'
CREATE DATABASE flopro_db;
CREATE USER flopro_admin WITH PASSWORD 'your_secure_password';
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;
\c flopro_db
GRANT ALL ON SCHEMA public TO flopro_admin;
SQL

# 2. Run migrations
sudo -u postgres psql -d flopro_db -f sql/001_initial_schema.sql
sudo -u postgres psql -d flopro_db -f sql/002_monitoring_metadata.sql
sudo -u postgres psql -d flopro_db -f sql/003_flood_metadata.sql
sudo -u postgres psql -d flopro_db -f sql/004_usace_cwms.sql
sudo -u postgres psql -d flopro_db -f sql/005_flood_analysis.sql

# 3. Grant permissions
sudo -u postgres psql -d flopro_db -f scripts/grant_permissions.sql

# 4. Configure environment
cp .env.example .env
# Edit .env and set: DATABASE_URL=postgresql://flopro_admin:your_password@localhost/flopro_db

# 5. Validate
./scripts/validate_db_setup.sh
```

## Connection Method

FloPro uses **TCP/IP authentication** (not Unix sockets):

```bash
# Correct way to connect
PGPASSWORD=your_password psql -h localhost -U flopro_admin -d flopro_db

# This will fail (peer auth)
psql -U flopro_admin -d flopro_db
```

## Security Notes

- `.env` file is excluded from git (contains credentials)
- Never commit database passwords
- Use strong passwords (16+ chars, mixed case, numbers, symbols)
- `DATABASE_URL` format: `postgresql://username:password@localhost/database`

## Troubleshooting

### "Peer authentication failed"
**Solution:** Add `-h localhost` to use TCP/IP:
```bash
PGPASSWORD=your_password psql -h localhost -U flopro_admin -d flopro_db
```

### "Password authentication failed"
**Solution:** Verify password in `.env` matches what you set:
```bash
sudo -u postgres psql -c "ALTER USER flopro_admin WITH PASSWORD 'new_password';"
# Then update .env
```

### "Permission denied for schema"
**Solution:** Re-run permissions script:
```bash
sudo -u postgres psql -d flopro_db -f scripts/grant_permissions.sql
```

## Full Documentation

See `docs/DATABASE_SETUP.md` for complete details including:
- Authentication configuration
- Production deployment
- Backup and recovery
- Schema overview
- Security best practices
