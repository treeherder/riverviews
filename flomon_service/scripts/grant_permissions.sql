-- Grant Schema Permissions Script
-- Run as postgres superuser to grant necessary permissions to flopro_admin
--
-- Usage: psql -U postgres -d flopro_db -f scripts/grant_permissions.sql

\set ON_ERROR_STOP on

BEGIN;

-- Verify flopro_admin user exists
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'flopro_admin') THEN
        RAISE EXCEPTION 'User flopro_admin does not exist. Create it first with: CREATE USER flopro_admin WITH PASSWORD ''your_password'';';
    END IF;
END
$$;

-- Grant database-level privileges
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;

-- USGS Raw Data Schema
GRANT USAGE ON SCHEMA usgs_raw TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA usgs_raw TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA usgs_raw TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL FUNCTIONS IN SCHEMA usgs_raw TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA usgs_raw GRANT ALL ON TABLES TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA usgs_raw GRANT ALL ON SEQUENCES TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA usgs_raw GRANT ALL ON FUNCTIONS TO flopro_admin;

-- NWS Flood Events Schema
GRANT USAGE ON SCHEMA nws TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA nws TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA nws TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL FUNCTIONS IN SCHEMA nws TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA nws GRANT ALL ON TABLES TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA nws GRANT ALL ON SEQUENCES TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA nws GRANT ALL ON FUNCTIONS TO flopro_admin;

-- USACE CWMS Schema
GRANT USAGE ON SCHEMA usace TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA usace TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA usace TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL FUNCTIONS IN SCHEMA usace TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA usace GRANT ALL ON TABLES TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA usace GRANT ALL ON SEQUENCES TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA usace GRANT ALL ON FUNCTIONS TO flopro_admin;

-- Public schema (for extensions, if needed)
GRANT ALL ON SCHEMA public TO flopro_admin;

COMMIT;

-- Verify permissions
\echo ''
\echo 'Permission grants completed successfully!'
\echo ''
\echo 'Verifying schema access:'

SELECT 
    nspname AS schema,
    CASE WHEN has_schema_privilege('flopro_admin', nspname, 'USAGE') 
        THEN 'GRANTED' 
        ELSE 'DENIED' 
    END AS usage_privilege
FROM pg_namespace
WHERE nspname IN ('usgs_raw', 'nws', 'usace')
ORDER BY nspname;
