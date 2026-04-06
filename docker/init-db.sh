#!/bin/bash
# init-db.sh — runs SQL migrations in order when the PostgreSQL container
# starts for the first time.  Placed in /docker-entrypoint-initdb.d/ so
# the official postgres image executes it automatically.
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    -- Create application roles if they do not already exist
    DO \$\$
    BEGIN
        IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'flopro_admin') THEN
            CREATE ROLE flopro_admin LOGIN PASSWORD '${FLOPRO_ADMIN_PASSWORD}';
        END IF;
        -- Read-only role referenced by migration GRANTs
        IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'flopro_user') THEN
            CREATE ROLE flopro_user NOLOGIN;
        END IF;
    END
    \$\$;
    GRANT ALL PRIVILEGES ON DATABASE $POSTGRES_DB TO flopro_admin;
EOSQL

for f in /migrations/*.sql; do
    echo "Applying $f ..."
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" -f "$f"
done

# Grant flopro_admin usage on all application schemas created by migrations
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    GRANT USAGE ON SCHEMA usgs_raw, nws, noaa, usace, flood_analysis TO flopro_admin;
    GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA usgs_raw, nws, noaa, usace, flood_analysis TO flopro_admin;
    GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA usgs_raw, nws, noaa, usace, flood_analysis TO flopro_admin;
    ALTER DEFAULT PRIVILEGES IN SCHEMA usgs_raw, nws, noaa, usace, flood_analysis
        GRANT ALL ON TABLES TO flopro_admin;
    ALTER DEFAULT PRIVILEGES IN SCHEMA usgs_raw, nws, noaa, usace, flood_analysis
        GRANT ALL ON SEQUENCES TO flopro_admin;
    -- public schema tables (ASOS stations, observations, etc.)
    GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO flopro_admin;
    GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO flopro_admin;
EOSQL

echo "All migrations applied."
