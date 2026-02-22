#!/usr/bin/env bash
#
# Database Setup Validation Script
# Checks PostgreSQL configuration and provides actionable setup instructions
#
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REQUIRED_PG_VERSION="12"
DB_NAME="flopro_db"
DB_USER="flopro_admin"
DB_HOST="${PGHOST:-localhost}"  # Use PGHOST env var or default to localhost
REQUIRED_SCHEMAS=("usgs_raw" "nws" "usace")
REQUIRED_TABLES=(
    "usgs_raw.gauge_readings"
    "nws.flood_thresholds"
    "nws.flood_events"
    "usace.cwms_locations"
    "usace.cwms_timeseries"
)

# Try to load password from .env file if it exists
DB_PASSWORD=""
if [ -f .env ]; then
    # Extract password from DATABASE_URL
    DB_URL=$(grep "^DATABASE_URL=" .env 2>/dev/null | cut -d= -f2- || echo "")
    if [[ "$DB_URL" =~ postgresql://[^:]+:([^@]+)@.* ]]; then
        DB_PASSWORD="${BASH_REMATCH[1]}"
        export PGPASSWORD="$DB_PASSWORD"
    fi
fi

# Helper function for psql with proper connection parameters
# Uses TCP/IP (-h localhost) instead of Unix sockets to avoid peer auth issues
psql_as_admin() {
    if [ -n "$DB_PASSWORD" ]; then
        PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -U "$DB_USER" "$@" 2>/dev/null
    else
        # Fallback: try without password (will prompt or fail)
        psql -h "$DB_HOST" -U "$DB_USER" "$@" 2>/dev/null
    fi
}

# Helper for superuser queries (only for checks that require it)
psql_as_postgres() {
    # Try sudo -u postgres first (Unix socket), then fall back to TCP/IP
    if sudo -u postgres psql "$@" 2>/dev/null; then
        return 0
    else
        # If sudo doesn't work, try TCP/IP as postgres user
        psql -h "$DB_HOST" -U postgres "$@" 2>/dev/null
    fi
}

echo -e "${BLUE}=== Riverviews Database Setup Validation ===${NC}\n"

# Track validation status
VALIDATION_PASSED=true
SETUP_STEPS=()

#
# 1. Check PostgreSQL is installed and running
#
echo -e "${BLUE}[1/8] Checking PostgreSQL installation...${NC}"
if command -v psql &> /dev/null; then
    PG_VERSION=$(psql --version | grep -oP '\d+' | head -1)
    echo -e "${GREEN}✓${NC} PostgreSQL $PG_VERSION installed"
    
    if [ "$PG_VERSION" -lt "$REQUIRED_PG_VERSION" ]; then
        echo -e "${YELLOW}⚠${NC} PostgreSQL $PG_VERSION found, version $REQUIRED_PG_VERSION+ recommended"
    fi
else
    echo -e "${RED}✗${NC} PostgreSQL not found"
    VALIDATION_PASSED=false
    SETUP_STEPS+=("Install PostgreSQL: https://www.postgresql.org/download/")
    echo -e "${YELLOW}→ Install PostgreSQL $REQUIRED_PG_VERSION or later${NC}"
fi

echo -e "${BLUE}[2/8] Checking PostgreSQL service status...${NC}"
if pg_isready -q -h "$DB_HOST"; then
    echo -e "${GREEN}✓${NC} PostgreSQL service is running on $DB_HOST"
else
    echo -e "${RED}✗${NC} PostgreSQL service is not responding on $DB_HOST"
    VALIDATION_PASSED=false
    SETUP_STEPS+=("Start PostgreSQL service: sudo systemctl start postgresql")
    echo -e "${YELLOW}→ Start PostgreSQL service${NC}"
fi

#
# 2. Check database exists
#
echo -e "${BLUE}[3/8] Checking database '$DB_NAME'...${NC}"
if psql_as_postgres -lqt | cut -d \| -f 1 | grep -qw "$DB_NAME"; then
    echo -e "${GREEN}✓${NC} Database '$DB_NAME' exists"
else
    echo -e "${RED}✗${NC} Database '$DB_NAME' not found"
    VALIDATION_PASSED=false
    SETUP_STEPS+=("Create database: sudo -u postgres psql -c 'CREATE DATABASE $DB_NAME;'")
    echo -e "${YELLOW}→ Create database${NC}"
fi

#
# 3. Check database user exists
#
echo -e "${BLUE}[4/8] Checking database user '$DB_USER'...${NC}"
if psql_as_postgres -d postgres -tAc "SELECT 1 FROM pg_roles WHERE rolname='$DB_USER'" | grep -q 1; then
    echo -e "${GREEN}✓${NC} User '$DB_USER' exists"
else
    echo -e "${RED}✗${NC} User '$DB_USER' not found"
    VALIDATION_PASSED=false
    SETUP_STEPS+=("Create user: sudo -u postgres psql -c \"CREATE USER $DB_USER WITH PASSWORD 'your_secure_password';\"")
    echo -e "${YELLOW}→ Create database user${NC}"
fi

#
# 4. Check user has necessary permissions and can connect
#
echo -e "${BLUE}[5/8] Checking user permissions and connectivity...${NC}"
# First check if user has CONNECT privilege
if psql_as_postgres -d "$DB_NAME" -tAc "SELECT has_database_privilege('$DB_USER', '$DB_NAME', 'CONNECT')" | grep -q t; then
    echo -e "${GREEN}✓${NC} User has CONNECT privilege"
    
    # Now test actual connection with the user credentials
    if [ -n "$DB_PASSWORD" ]; then
        if psql_as_admin -d "$DB_NAME" -c "\conninfo" &>/dev/null; then
            echo -e "${GREEN}✓${NC} User can successfully connect to database via TCP/IP"
        else
            echo -e "${RED}✗${NC} User cannot connect (check password in .env)"
            VALIDATION_PASSED=false
            SETUP_STEPS+=("Verify DATABASE_URL password in .env is correct")
        fi
    else
        echo -e "${YELLOW}⚠${NC} No password found in .env - cannot test connection"
        SETUP_STEPS+=("Set DATABASE_URL in .env with password")
    fi
else
    echo -e "${RED}✗${NC} User lacks CONNECT privilege"
    VALIDATION_PASSED=false
    SETUP_STEPS+=("Grant connect: sudo -u postgres psql -c \"GRANT ALL PRIVILEGES ON DATABASE $DB_NAME TO $DB_USER;\"")
    echo -e "${YELLOW}→ Grant database connection privileges${NC}"
fi

#
# 5. Check schemas exist
#
echo -e "${BLUE}[6/8] Checking database schemas...${NC}"
MISSING_SCHEMAS=()
for schema in "${REQUIRED_SCHEMAS[@]}"; do
    if psql_as_admin -d "$DB_NAME" -tAc "SELECT 1 FROM information_schema.schemata WHERE schema_name='$schema'" | grep -q 1; then
        echo -e "${GREEN}  ✓${NC} Schema '$schema' exists"
    else
        echo -e "${RED}  ✗${NC} Schema '$schema' missing"
        MISSING_SCHEMAS+=("$schema")
        VALIDATION_PASSED=false
    fi
done

if [ ${#MISSING_SCHEMAS[@]} -gt 0 ]; then
    SETUP_STEPS+=("Run migrations: cd flomon_service && sudo -u postgres psql -d $DB_NAME -f sql/001_initial_schema.sql")
    SETUP_STEPS+=("                 sudo -u postgres psql -d $DB_NAME -f sql/002_monitoring_metadata.sql")
    SETUP_STEPS+=("                 sudo -u postgres psql -d $DB_NAME -f sql/003_flood_metadata.sql")
    SETUP_STEPS+=("                 sudo -u postgres psql -d $DB_NAME -f sql/004_usace_cwms.sql")
    SETUP_STEPS+=("                 sudo -u postgres psql -d $DB_NAME -f sql/005_flood_analysis.sql")
fi

#
# 6. Check schema permissions
#
echo -e "${BLUE}[7/8] Checking schema permissions...${NC}"
PERMISSION_ISSUES=()
for schema in "${REQUIRED_SCHEMAS[@]}"; do
    # Check if schema exists first
    if psql_as_admin -d "$DB_NAME" -tAc "SELECT 1 FROM information_schema.schemata WHERE schema_name='$schema'" | grep -q 1; then
        if psql_as_admin -d "$DB_NAME" -tAc "SELECT has_schema_privilege('$DB_USER', '$schema', 'USAGE')" | grep -q t; then
            echo -e "${GREEN}  ✓${NC} User has access to schema '$schema'"
        else
            echo -e "${RED}  ✗${NC} User lacks access to schema '$schema'"
            PERMISSION_ISSUES+=("$schema")
            VALIDATION_PASSED=false
        fi
    fi
done

if [ ${#PERMISSION_ISSUES[@]} -gt 0 ]; then
    SETUP_STEPS+=("Grant schema permissions: cd flomon_service && sudo -u postgres psql -d $DB_NAME -f scripts/grant_permissions.sql")
fi

#
# 7. Check critical tables exist
#
echo -e "${BLUE}[8/8] Checking critical tables...${NC}"
MISSING_TABLES=()
for table in "${REQUIRED_TABLES[@]}"; do
    IFS='.' read -r schema_name table_name <<< "$table"
    if psql_as_admin -d "$DB_NAME" -tAc "SELECT 1 FROM information_schema.tables WHERE table_schema='$schema_name' AND table_name='$table_name'" | grep -q 1; then
        echo -e "${GREEN}  ✓${NC} Table '$table' exists"
    else
        echo -e "${RED}  ✗${NC} Table '$table' missing"
        MISSING_TABLES+=("$table")
        VALIDATION_PASSED=false
    fi
done

#
# 8. Check DATABASE_URL environment variable
#
echo -e "\n${BLUE}[ENV] Checking environment configuration...${NC}"
if [ -f .env ]; then
    echo -e "${GREEN}✓${NC} .env file exists"
    
    if grep -q "^DATABASE_URL=" .env; then
        DB_URL=$(grep "^DATABASE_URL=" .env | cut -d= -f2-)
        echo -e "${GREEN}✓${NC} DATABASE_URL is set"
        
        # Validate format
        if [[ "$DB_URL" =~ postgresql://.*@.*/.* ]]; then
            echo -e "${GREEN}✓${NC} DATABASE_URL format looks valid"
        else
            echo -e "${YELLOW}⚠${NC} DATABASE_URL format may be invalid"
            SETUP_STEPS+=("Update DATABASE_URL in .env: postgresql://$DB_USER:your_password@localhost/$DB_NAME")
        fi
    else
        echo -e "${RED}✗${NC} DATABASE_URL not set in .env"
        VALIDATION_PASSED=false
        SETUP_STEPS+=("Add to .env: DATABASE_URL=postgresql://$DB_USER:your_password@localhost/$DB_NAME")
    fi
else
    echo -e "${YELLOW}⚠${NC} .env file not found"
    SETUP_STEPS+=("Create .env file: cp .env.example .env")
    SETUP_STEPS+=("Update DATABASE_URL with your password")
    VALIDATION_PASSED=false
fi

#
# Summary and remediation steps
#
echo -e "\n${BLUE}=== Validation Summary ===${NC}"
if [ "$VALIDATION_PASSED" = true ]; then
    echo -e "${GREEN}✓ All checks passed! Database is properly configured.${NC}\n"
    echo "You can now run:"
    echo "  cargo test --test peak_flow_integration"
    echo "  cargo run --bin ingest_peak_flows"
    echo "  cargo run --bin ingest_cwms_historical"
    exit 0
else
    echo -e "${RED}✗ Validation failed. Setup steps required:${NC}\n"
    
    echo -e "${YELLOW}Required Setup Steps:${NC}"
    echo -e "${YELLOW}────────────────────${NC}"
    
    # Print numbered steps
    step_num=1
    for step in "${SETUP_STEPS[@]}"; do
        echo -e "${YELLOW}$step_num.${NC} $step"
        ((step_num++))
    done
    
    echo -e "\n${BLUE}Quick Setup (if starting fresh):${NC}"
    echo -e "${BLUE}──────────────────────────────${NC}"
    cat << 'EOF'
# 1. Create database and user
sudo -u postgres psql << SQL
CREATE DATABASE flopro_db;
CREATE USER flopro_admin WITH PASSWORD 'your_secure_password';
GRANT ALL PRIVILEGES ON DATABASE flopro_db TO flopro_admin;
\c flopro_db
GRANT ALL ON SCHEMA public TO flopro_admin;
SQL

# 2. Run migrations (creates schemas and tables)
cd flomon_service
sudo -u postgres psql -d flopro_db -f sql/001_initial_schema.sql
sudo -u postgres psql -d flopro_db -f sql/002_monitoring_metadata.sql
sudo -u postgres psql -d flopro_db -f sql/003_flood_metadata.sql
sudo -u postgres psql -d flopro_db -f sql/004_usace_cwms.sql
sudo -u postgres psql -d flopro_db -f sql/005_flood_analysis.sql

# 3. Grant schema permissions to flopro_admin
sudo -u postgres psql -d flopro_db -f scripts/grant_permissions.sql

# 4. Configure environment
cp .env.example .env
# Edit .env and set: DATABASE_URL=postgresql://flopro_admin:your_secure_password@localhost/flopro_db

# 5. Verify setup
./scripts/validate_db_setup.sh
EOF

    echo -e "\n${BLUE}For detailed setup instructions, see:${NC}"
    echo "  docs/DATABASE_SETUP.md"
    echo "  scripts/README.md"
    
    exit 1
fi
