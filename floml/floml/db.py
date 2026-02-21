"""Database connection utilities for FloML.

Provides connections to the PostgreSQL database curated by the Rust daemon.
"""

import os
import logging
from typing import Optional
from sqlalchemy import create_engine, text
from sqlalchemy.engine import Engine
import psycopg2
from dotenv import load_dotenv

logger = logging.getLogger(__name__)

# Load environment variables
load_dotenv()


def get_database_url() -> str:
    """Get database URL from environment.
    
    Returns:
        Database connection string
        
    Raises:
        ValueError: If DATABASE_URL not set
    """
    url = os.getenv("DATABASE_URL")
    if not url:
        raise ValueError(
            "DATABASE_URL not set. Create .env file with:\n"
            "DATABASE_URL=postgresql://flopro_admin:password@localhost/flopro_db"
        )
    return url


def get_engine(echo: bool = False) -> Engine:
    """Create SQLAlchemy engine for database connections.
    
    Args:
        echo: If True, log all SQL statements
        
    Returns:
        SQLAlchemy engine
    """
    url = get_database_url()
    engine = create_engine(url, echo=echo)
    logger.info("Created database engine")
    return engine


def get_connection():
    """Get raw psycopg2 connection.
    
    Returns:
        psycopg2 connection object
    """
    url = get_database_url()
    conn = psycopg2.connect(url)
    logger.info("Created database connection")
    return conn


def verify_schemas(engine: Optional[Engine] = None) -> bool:
    """Verify required database schemas exist.
    
    Args:
        engine: SQLAlchemy engine (creates new one if None)
        
    Returns:
        True if all required schemas exist
        
    Raises:
        RuntimeError: If required schemas are missing
    """
    if engine is None:
        engine = get_engine()
    
    required_schemas = ["usgs_raw", "nws", "usace", "flood_analysis"]
    
    with engine.connect() as conn:
        result = conn.execute(text(
            "SELECT schema_name FROM information_schema.schemata "
            "WHERE schema_name = ANY(:schemas)"
        ), {"schemas": required_schemas})
        
        existing = {row[0] for row in result}
    
    missing = set(required_schemas) - existing
    
    if missing:
        raise RuntimeError(
            f"Missing required database schemas: {', '.join(missing)}\n"
            "Run migrations in ../flomon_service/sql/"
        )
    
    logger.info(f"Verified schemas: {', '.join(required_schemas)}")
    return True


if __name__ == "__main__":
    # Test database connection
    logging.basicConfig(level=logging.INFO)
    
    try:
        engine = get_engine()
        verify_schemas(engine)
        
        with engine.connect() as conn:
            result = conn.execute(text("SELECT COUNT(*) FROM usgs_raw.sites"))
            count = result.scalar()
            print(f"✓ Database connected - {count} USGS sites configured")
            
    except Exception as e:
        print(f"✗ Database connection failed: {e}")
        exit(1)
