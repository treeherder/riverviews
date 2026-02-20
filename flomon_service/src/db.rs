/// Database connection and validation utilities
///
/// Provides robust database connectivity with clear error messages
/// and configuration validation.

use postgres::{Client, NoTls, Error};
use std::env;

/// Database configuration validation error
#[derive(Debug)]
pub enum DbConfigError {
    /// DATABASE_URL environment variable not set
    MissingDatabaseUrl,
    /// Invalid DATABASE_URL format
    InvalidDatabaseUrl(String),
    /// Connection failed
    ConnectionFailed(Error),
    /// Required schema missing
    MissingSchema(String),
    /// Permission denied
    PermissionDenied(String),
}

impl std::fmt::Display for DbConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbConfigError::MissingDatabaseUrl => {
                write!(f, "DATABASE_URL environment variable not set.\n\n")?;
                write!(f, "  Required Setup:\n")?;
                write!(f, "  1. Copy .env.example to .env: cp .env.example .env\n")?;
                write!(f, "  2. Edit .env and set DATABASE_URL=postgresql://flopro_admin:password@localhost/flopro_db\n")?;
                write!(f, "  3. Run setup validation: ./scripts/validate_db_setup.sh\n\n")?;
                write!(f, "  For complete setup instructions, see: docs/DATABASE_SETUP.md")
            }
            DbConfigError::InvalidDatabaseUrl(url) => {
                write!(f, "Invalid DATABASE_URL format: {}\n\n", url)?;
                write!(f, "  Expected format: postgresql://user:password@host:port/database\n")?;
                write!(f, "  Example: postgresql://flopro_admin:password@localhost/flopro_db")
            }
            DbConfigError::ConnectionFailed(e) => {
                write!(f, "Failed to connect to PostgreSQL database.\n\n")?;
                write!(f, "  Error: {}\n\n", e)?;
                write!(f, "  Common causes:\n")?;
                write!(f, "  - PostgreSQL service not running (check: pg_isready)\n")?;
                write!(f, "  - Database 'flopro_db' does not exist\n")?;
                write!(f, "  - User 'flopro_admin' does not exist\n")?;
                write!(f, "  - Incorrect password in DATABASE_URL\n")?;
                write!(f, "  - pg_hba.conf does not allow local connections\n\n")?;
                write!(f, "  Run setup validation: ./scripts/validate_db_setup.sh\n")?;
                write!(f, "  See: docs/DATABASE_SETUP.md for complete setup instructions")
            }
            DbConfigError::MissingSchema(schema) => {
                write!(f, "Required database schema '{}' does not exist.\n\n", schema)?;
                write!(f, "  Run migration scripts in order:\n")?;
                write!(f, "  1. psql -U flopro_admin -d flopro_db -f sql/001_initial_schema.sql\n")?;
                write!(f, "  2. psql -U flopro_admin -d flopro_db -f sql/002_monitoring_state.sql\n")?;
                write!(f, "  3. psql -U flopro_admin -d flopro_db -f sql/003_flood_metadata.sql\n")?;
                write!(f, "  4. psql -U flopro_admin -d flopro_db -f sql/004_usace_cwms.sql\n\n")?;
                write!(f, "  See: docs/DATABASE_SETUP.md")
            }
            DbConfigError::PermissionDenied(schema) => {
                write!(f, "Permission denied for schema '{}'.\n\n", schema)?;
                write!(f, "  Grant permissions:\n")?;
                write!(f, "  psql -U postgres -d flopro_db -f scripts/grant_permissions.sql\n\n")?;
                write!(f, "  Or manually:\n")?;
                write!(f, "  psql -U postgres -d flopro_db -c \"GRANT USAGE ON SCHEMA {} TO flopro_admin;\"\n", schema)?;
                write!(f, "  psql -U postgres -d flopro_db -c \"GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA {} TO flopro_admin;\"\n\n", schema)?;
                write!(f, "  See: docs/DATABASE_SETUP.md")
            }
        }
    }
}

impl std::error::Error for DbConfigError {}

/// Connect to the database with full validation and helpful error messages
pub fn connect_with_validation() -> Result<Client, DbConfigError> {
    // Load .env file if present
    dotenv::dotenv().ok();

    // Check DATABASE_URL is set
    let db_url = env::var("DATABASE_URL")
        .map_err(|_| DbConfigError::MissingDatabaseUrl)?;

    // Validate URL format (basic check)
    if !db_url.starts_with("postgresql://") && !db_url.starts_with("postgres://") {
        return Err(DbConfigError::InvalidDatabaseUrl(db_url));
    }

    // Attempt connection
    let client = Client::connect(&db_url, NoTls)
        .map_err(DbConfigError::ConnectionFailed)?;

    Ok(client)
}

/// Verify required schema exists with proper permissions
pub fn verify_schema(client: &mut Client, schema_name: &str) -> Result<(), DbConfigError> {
    // Check if schema exists
    let row = client.query_one(
        "SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name = $1)",
        &[&schema_name],
    ).map_err(DbConfigError::ConnectionFailed)?;

    let exists: bool = row.get(0);
    if !exists {
        return Err(DbConfigError::MissingSchema(schema_name.to_string()));
    }

    // Check if current user has USAGE privilege
    let row = client.query_one(
        "SELECT has_schema_privilege(current_user, $1, 'USAGE')",
        &[&schema_name],
    ).map_err(DbConfigError::ConnectionFailed)?;

    let has_permission: bool = row.get(0);
    if !has_permission {
        return Err(DbConfigError::PermissionDenied(schema_name.to_string()));
    }

    Ok(())
}

/// Connect and validate all required schemas exist with proper permissions
pub fn connect_and_verify(required_schemas: &[&str]) -> Result<Client, DbConfigError> {
    let mut client = connect_with_validation()?;

    // Verify each required schema
    for schema in required_schemas {
        verify_schema(&mut client, schema)?;
    }

    Ok(client)
}

/// Quick connection for scripts that don't need full validation
/// (still provides helpful error messages on failure)
pub fn connect_simple() -> Result<Client, DbConfigError> {
    dotenv::dotenv().ok();
    
    let db_url = env::var("DATABASE_URL")
        .map_err(|_| DbConfigError::MissingDatabaseUrl)?;

    Client::connect(&db_url, NoTls)
        .map_err(DbConfigError::ConnectionFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_url_format_validation() {
        // Valid formats
        assert!(format_looks_valid("postgresql://user:pass@localhost/db"));
        assert!(format_looks_valid("postgres://user:pass@localhost/db"));

        // Invalid formats
        assert!(!format_looks_valid("mysql://user:pass@localhost/db"));
        assert!(!format_looks_valid("localhost/db"));
        assert!(!format_looks_valid(""));
    }

    fn format_looks_valid(url: &str) -> bool {
        url.starts_with("postgresql://") || url.starts_with("postgres://")
    }

    #[test]
    #[ignore] // Only run when database is available
    fn test_connect_and_verify() {
        let result = connect_and_verify(&["usgs_raw", "nws", "usace"]);
        assert!(result.is_ok(), "Database connection and schema validation failed: {:?}", result.err());
    }
}
