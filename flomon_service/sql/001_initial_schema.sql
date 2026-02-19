-- FloPro Database Schema - Initial Migration
-- Version: 001
-- Description: USGS gauge reading tables and initial site registry
-- 
-- This creates the foundation for storing historical USGS NWIS data
-- and establishes the multi-schema architecture for future data sources.

-- ============================================================================
-- SCHEMAS
-- ============================================================================

-- Raw USGS data from NWIS IV API
CREATE SCHEMA IF NOT EXISTS usgs_raw;

-- NWS flood forecasts and thresholds (future)
CREATE SCHEMA IF NOT EXISTS nws;

-- NOAA weather data (future)
CREATE SCHEMA IF NOT EXISTS noaa;

-- USACE lock/dam operations (future)
CREATE SCHEMA IF NOT EXISTS usace;

-- public schema contains unified/processed data
-- (already exists by default)

COMMENT ON SCHEMA usgs_raw IS 'Raw USGS NWIS instantaneous values (15-min gauge readings)';
COMMENT ON SCHEMA nws IS 'NWS Advanced Hydrologic Prediction Service data';
COMMENT ON SCHEMA noaa IS 'NOAA precipitation and weather radar data';
COMMENT ON SCHEMA usace IS 'US Army Corps of Engineers lock and dam operations';

-- ============================================================================
-- USGS RAW DATA TABLES
-- ============================================================================

-- -----------------------------------------------------------------------------
-- Site Metadata
-- -----------------------------------------------------------------------------
CREATE TABLE usgs_raw.sites (
    site_code VARCHAR(8) PRIMARY KEY,           -- 8-digit USGS site number
    site_name TEXT NOT NULL,                    -- Official USGS name
    latitude NUMERIC(10, 7) NOT NULL,           -- WGS84 latitude
    longitude NUMERIC(11, 7) NOT NULL,          -- WGS84 longitude
    description TEXT,                           -- Human-readable description
    active BOOLEAN NOT NULL DEFAULT true,       -- Currently monitored?
    
    -- Metadata
    first_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT valid_latitude CHECK (latitude BETWEEN -90 AND 90),
    CONSTRAINT valid_longitude CHECK (longitude BETWEEN -180 AND 180)
);

CREATE INDEX idx_usgs_sites_active ON usgs_raw.sites(active) WHERE active = true;

COMMENT ON TABLE usgs_raw.sites IS 'USGS gauge station metadata and locations';
COMMENT ON COLUMN usgs_raw.sites.site_code IS 'USGS 8-digit site identifier (e.g., 05568500)';

-- -----------------------------------------------------------------------------
-- Gauge Readings (Time Series Data)
-- -----------------------------------------------------------------------------
CREATE TABLE usgs_raw.gauge_readings (
    id BIGSERIAL PRIMARY KEY,
    
    -- Identifiers
    site_code VARCHAR(8) NOT NULL,              -- References usgs_raw.sites
    parameter_code VARCHAR(5) NOT NULL,         -- 00060=discharge, 00065=stage
    
    -- Measurement
    value NUMERIC(12, 4) NOT NULL,              -- Measured value
    unit VARCHAR(10) NOT NULL,                  -- ft, ft3/s, etc.
    qualifier VARCHAR(1) NOT NULL DEFAULT 'P',  -- P=provisional, A=approved
    
    -- Timestamp
    reading_time TIMESTAMPTZ NOT NULL,          -- Measurement timestamp (with TZ)
    
    -- Metadata
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),  -- When we stored this
    
    -- Prevent duplicate readings
    CONSTRAINT unique_reading UNIQUE (site_code, parameter_code, reading_time)
);

-- Performance indexes
CREATE INDEX idx_gauge_readings_site_time 
    ON usgs_raw.gauge_readings(site_code, reading_time DESC);

CREATE INDEX idx_gauge_readings_site_param_time 
    ON usgs_raw.gauge_readings(site_code, parameter_code, reading_time DESC);

CREATE INDEX idx_gauge_readings_time 
    ON usgs_raw.gauge_readings(reading_time DESC);

-- Partial index for recent data (most queries)
CREATE INDEX idx_gauge_readings_recent 
    ON usgs_raw.gauge_readings(site_code, parameter_code, reading_time DESC)
    WHERE reading_time > NOW() - INTERVAL '30 days';

COMMENT ON TABLE usgs_raw.gauge_readings IS 'Historical USGS gauge readings at 15-minute intervals';
COMMENT ON COLUMN usgs_raw.gauge_readings.parameter_code IS '00060=discharge, 00065=gage height (stage)';
COMMENT ON COLUMN usgs_raw.gauge_readings.qualifier IS 'P=Provisional (subject to revision), A=Approved';
COMMENT ON COLUMN usgs_raw.gauge_readings.reading_time IS 'Observation timestamp in Central Time (stored as UTC)';

-- ============================================================================
-- PUBLIC SCHEMA - UNIFIED VIEWS
-- ============================================================================

-- -----------------------------------------------------------------------------
-- Master Site Registry (combines all data sources)
-- -----------------------------------------------------------------------------
CREATE TABLE public.sites (
    site_id SERIAL PRIMARY KEY,
    site_code VARCHAR(20) NOT NULL UNIQUE,      -- Could be USGS, NWS, or other ID
    site_name TEXT NOT NULL,
    source VARCHAR(20) NOT NULL,                -- 'USGS', 'NWS', 'NOAA', etc.
    latitude NUMERIC(10, 7),
    longitude NUMERIC(11, 7),
    description TEXT,
    monitoring_priority INTEGER DEFAULT 5,      -- 1=critical, 5=normal, 10=low
    active BOOLEAN NOT NULL DEFAULT true,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sites_source ON public.sites(source);
CREATE INDEX idx_sites_active ON public.sites(active) WHERE active = true;

COMMENT ON TABLE public.sites IS 'Master registry of all monitoring sites across data sources';

-- -----------------------------------------------------------------------------
-- Latest Readings View (for dashboard/alerts)
-- -----------------------------------------------------------------------------
-- This will be a materialized view refreshed every 15 minutes
CREATE MATERIALIZED VIEW public.latest_readings AS
WITH ranked_readings AS (
    SELECT 
        gr.site_code,
        gr.parameter_code,
        gr.value,
        gr.unit,
        gr.qualifier,
        gr.reading_time,
        s.site_name,
        s.latitude,
        s.longitude,
        ROW_NUMBER() OVER (
            PARTITION BY gr.site_code, gr.parameter_code 
            ORDER BY gr.reading_time DESC
        ) as rn
    FROM usgs_raw.gauge_readings gr
    INNER JOIN usgs_raw.sites s ON gr.site_code = s.site_code
    WHERE s.active = true
      AND gr.reading_time > NOW() - INTERVAL '6 hours'  -- Only recent data
)
SELECT 
    site_code,
    site_name,
    latitude,
    longitude,
    parameter_code,
    value,
    unit,
    qualifier,
    reading_time,
    NOW() - reading_time AS data_age
FROM ranked_readings
WHERE rn = 1;

CREATE UNIQUE INDEX idx_latest_readings_site_param 
    ON public.latest_readings(site_code, parameter_code);

COMMENT ON MATERIALIZED VIEW public.latest_readings IS 'Most recent reading per site/parameter (refresh every 15 min)';

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Function to refresh the latest_readings view
CREATE OR REPLACE FUNCTION public.refresh_latest_readings()
RETURNS void AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY public.latest_readings;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION public.refresh_latest_readings IS 'Refresh latest_readings materialized view (call every 15 min)';

-- ============================================================================
-- SEED DATA - Initial USGS Sites
-- ============================================================================

INSERT INTO usgs_raw.sites (site_code, site_name, latitude, longitude, description) VALUES
    ('05568500', 'Illinois River at Kingston Mines, IL', 40.556139, -89.778722, 'Primary downstream reference gauge just below Peoria'),
    ('05567500', 'Mackinaw River near Congerville, IL', 40.605694, -89.193861, 'Major tributary; drains agricultural basin east of Peoria'),
    ('05568000', 'Illinois River at Chillicothe, IL', 40.921389, -89.476111, 'Upstream from Peoria Lock & Dam'),
    ('05557000', 'Illinois River at Henry, IL', 41.111667, -89.356111, 'Upstream monitoring point'),
    ('05568580', 'Mackinaw River near Green Valley, IL', 40.405972, -89.648333, 'Tributary monitoring'),
    ('05570000', 'Spoon River at Seville, IL', 40.481667, -90.344167, 'Western tributary'),
    ('05552500', 'Illinois River at Marseilles, IL', 41.332222, -88.706944, 'Major upstream reference point'),
    ('05536890', 'Des Plaines River at Riverside, IL', 41.822222, -87.823333, 'Chicago area inflow tracking')
ON CONFLICT (site_code) DO NOTHING;

-- Copy USGS sites to master registry
INSERT INTO public.sites (site_code, site_name, source, latitude, longitude, description, monitoring_priority)
SELECT 
    site_code,
    site_name,
    'USGS' as source,
    latitude,
    longitude,
    description,
    CASE 
        WHEN site_code = '05568500' THEN 1  -- Kingston Mines = critical
        WHEN site_code IN ('05567500', '05568000') THEN 2  -- Major sites
        ELSE 3
    END as monitoring_priority
FROM usgs_raw.sites
ON CONFLICT (site_code) DO NOTHING;

-- ============================================================================
-- PERMISSIONS
-- ============================================================================

-- Grant read/write to application user (assumes user 'flopro_admin')
GRANT USAGE ON SCHEMA usgs_raw TO flopro_admin;
GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA usgs_raw TO flopro_admin;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA usgs_raw TO flopro_admin;

GRANT SELECT ON public.sites TO flopro_admin;
GRANT SELECT ON public.latest_readings TO flopro_admin;

-- ============================================================================
-- MAINTENANCE
-- ============================================================================

-- Auto-vacuum settings for high-volume gauge_readings table
ALTER TABLE usgs_raw.gauge_readings SET (
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_scale_factor = 0.02
);

-- ============================================================================
-- VERIFICATION
-- ============================================================================

-- Print summary
DO $$
DECLARE
    site_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO site_count FROM usgs_raw.sites;
    RAISE NOTICE '✓ Initial schema created successfully';
    RAISE NOTICE '✓ Inserted % USGS monitoring sites', site_count;
    RAISE NOTICE '✓ Created materialized view: public.latest_readings';
    RAISE NOTICE '';
    RAISE NOTICE 'Next steps:';
    RAISE NOTICE '  1. Run: cargo run --bin historical_ingest';
    RAISE NOTICE '  2. Query: SELECT COUNT(*) FROM usgs_raw.gauge_readings;';
END $$;
