-- Migration: 004 - USACE Corps Water Management System (CWMS) Data
-- 
-- Data Source: USACE CWMS Data Dissemination API
-- Base URL: https://cwms-data.usace.army.mil/cwms-data/
-- Documentation: https://cwms-data.usace.army.mil/cwms-data/swagger-ui.html
--
-- Update Frequency: 15-60 minutes (varies by parameter)
-- Historical Data: Available since 2015 (some locations have earlier data)
--
-- Purpose:
--   1. Track Mississippi River levels for backwater flooding detection
--   2. Monitor lock/dam operations and pool levels on Illinois River
--   3. Detect "bottom-up" floods where Mississippi backs up into Illinois
--   4. Track dam releases that affect downstream flow
--
-- Applied: <date>
-- Rollback: DROP SCHEMA usace CASCADE;

CREATE SCHEMA IF NOT EXISTS usace;

COMMENT ON SCHEMA usace IS 
    'US Army Corps of Engineers data from CWMS (Corps Water Management System) API';

-- ============================================================================
-- CWMS Location Metadata
-- ============================================================================

CREATE TABLE usace.cwms_locations (
    id SERIAL PRIMARY KEY,
    
    -- CWMS identifiers
    location_id TEXT NOT NULL UNIQUE,     -- e.g., 'LD24.Stage.Inst.15Minutes.0.Ccp-Rev'
    office_id TEXT NOT NULL,              -- e.g., 'MVR' (Rock Island District), 'LRC' (Louisville)
    base_location TEXT NOT NULL,          -- e.g., 'LD24' (Lock & Dam 24)
    
    -- Location details
    location_name TEXT NOT NULL,
    river_name TEXT,                      -- 'Mississippi River', 'Illinois River'
    river_mile NUMERIC(6, 2),
    state_code VARCHAR(2),
    
    -- Geographic
    latitude NUMERIC(10, 7),
    longitude NUMERIC(11, 7),
    elevation_ft NUMERIC(8, 2),
    
    -- Station type
    location_type TEXT,                   -- 'lock_dam', 'gauge', 'project', 'pool'
    project_purpose TEXT[],               -- ['navigation', 'flood_control', 'power']
    
    -- Monitoring configuration
    monitored BOOLEAN DEFAULT false,
    monitoring_reason TEXT,               -- 'backwater_detection', 'dam_operations', 'flow_tracking'
    
    -- Illinois River relationship
    affects_illinois BOOLEAN DEFAULT false,
    upstream_of_confluence BOOLEAN,       -- True if on Mississippi above IL confluence
    
    -- Metadata
    active BOOLEAN DEFAULT true,
    notes TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_cwms_locations_monitored ON usace.cwms_locations(location_id) WHERE monitored = true;
CREATE INDEX idx_cwms_locations_office ON usace.cwms_locations(office_id);
CREATE INDEX idx_cwms_locations_affects_il ON usace.cwms_locations(location_id) WHERE affects_illinois = true;

COMMENT ON TABLE usace.cwms_locations IS 
    'CWMS location metadata for monitored USACE facilities and gauges';
COMMENT ON COLUMN usace.cwms_locations.location_id IS 
    'CWMS location identifier (unique within office, may include parameter)';
COMMENT ON COLUMN usace.cwms_locations.affects_illinois IS 
    'True if this location impacts Illinois River flooding (Mississippi backwater or upstream dams)';

-- ============================================================================
-- CWMS Timeseries Data
-- ============================================================================

CREATE TABLE usace.cwms_timeseries (
    id BIGSERIAL PRIMARY KEY,
    
    -- Location reference
    location_id TEXT NOT NULL REFERENCES usace.cwms_locations(location_id),
    
    -- CWMS timeseries identification
    timeseries_id TEXT NOT NULL,          -- Full ID: 'LD24.Stage.Inst.15Minutes.0.Ccp-Rev'
    parameter_id TEXT NOT NULL,           -- e.g., 'Stage', 'Flow', 'Elev'
    parameter_type TEXT NOT NULL,         -- e.g., 'Inst' (instantaneous), 'Ave', 'Total'
    interval TEXT NOT NULL,               -- e.g., '15Minutes', '1Hour', '1Day'
    duration TEXT,                        -- e.g., '0' (instantaneous)
    version TEXT,                         -- e.g., 'Ccp-Rev' (version/quality)
    
    -- Measurement
    timestamp TIMESTAMPTZ NOT NULL,
    value NUMERIC(12, 4) NOT NULL,
    unit TEXT NOT NULL,                   -- e.g., 'ft', 'cfs', 'ft3/s'
    quality_code INT,                     -- CWMS quality code (0=missing, etc.)
    
    -- Metadata
    data_source TEXT DEFAULT 'CWMS_API',
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT unique_cwms_reading UNIQUE (timeseries_id, timestamp)
);

CREATE INDEX idx_cwms_ts_location_time ON usace.cwms_timeseries(location_id, timestamp DESC);
CREATE INDEX idx_cwms_ts_param_time ON usace.cwms_timeseries(parameter_id, timestamp DESC) 
    WHERE parameter_id IN ('Stage', 'Flow', 'Elev');
CREATE INDEX idx_cwms_ts_recent ON usace.cwms_timeseries(location_id, timestamp DESC)
    WHERE timestamp > NOW() - INTERVAL '7 days';

COMMENT ON TABLE usace.cwms_timeseries IS 
    'CWMS timeseries observations (stage, flow, elevation, releases)';
COMMENT ON COLUMN usace.cwms_timeseries.quality_code IS 
    'CWMS quality codes: 0=missing, 1=okay, 2=questionable, 3=reject, etc.';

-- ============================================================================
-- Lock & Dam Operations
-- ============================================================================

CREATE TABLE usace.lock_operations (
    id BIGSERIAL PRIMARY KEY,
    
    -- Facility
    location_id TEXT NOT NULL REFERENCES usace.cwms_locations(location_id),
    facility_name TEXT NOT NULL,
    
    -- Pool levels
    observation_time TIMESTAMPTZ NOT NULL,
    pool_elevation_ft NUMERIC(8, 3),      -- Current pool level
    tailwater_elevation_ft NUMERIC(8, 3), -- Downstream level
    pool_target_ft NUMERIC(8, 3),         -- Target pool elevation
    
    -- Flow
    inflow_cfs NUMERIC(12, 2),
    outflow_cfs NUMERIC(12, 2),
    spillway_flow_cfs NUMERIC(12, 2),
    powerhouse_flow_cfs NUMERIC(12, 2),
    
    -- Gate status
    gates_open INT,
    gates_total INT,
    gate_opening_ft NUMERIC(6, 2),        -- Average gate opening height
    
    -- Operational mode
    operation_mode TEXT,                  -- 'normal', 'flood_control', 'low_water', 'maintenance'
    flood_control_active BOOLEAN DEFAULT false,
    
    -- Notes
    operator_notes TEXT,
    
    -- Metadata
    data_quality TEXT,
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT unique_lock_operation UNIQUE (location_id, observation_time),
    CONSTRAINT valid_operation_mode CHECK (
        operation_mode IS NULL OR 
        operation_mode IN ('normal', 'flood_control', 'low_water', 'maintenance', 'emergency')
    )
);

CREATE INDEX idx_lock_ops_time ON usace.lock_operations(location_id, observation_time DESC);
CREATE INDEX idx_lock_ops_flood_control ON usace.lock_operations(location_id, observation_time DESC)
    WHERE flood_control_active = true;

COMMENT ON TABLE usace.lock_operations IS 
    'Lock and dam operational data including pool levels, flows, and gate positions';
COMMENT ON COLUMN usace.lock_operations.flood_control_active IS 
    'True when dam is in flood control operations (high releases, gates fully open)';

-- ============================================================================
-- Backwater Event Detection
-- ============================================================================

CREATE TABLE usace.backwater_events (
    id SERIAL PRIMARY KEY,
    
    -- Event identification
    event_start TIMESTAMPTZ NOT NULL,
    event_end TIMESTAMPTZ,                -- NULL if ongoing
    
    -- Mississippi River level
    mississippi_location_id TEXT NOT NULL REFERENCES usace.cwms_locations(location_id),
    mississippi_peak_ft NUMERIC(8, 3) NOT NULL,
    mississippi_normal_ft NUMERIC(8, 3),  -- Typical elevation for comparison
    elevation_above_normal_ft NUMERIC(6, 2),
    
    -- Illinois River impact
    illinois_site_code VARCHAR(8) REFERENCES usgs_raw.sites(site_code),
    illinois_affected BOOLEAN DEFAULT false,
    gradient_reversal BOOLEAN DEFAULT false,  -- True if water flowing UP Illinois River
    
    -- Severity
    backwater_severity TEXT,              -- 'minor', 'moderate', 'major'
    affected_river_miles NUMERIC(6, 1),   -- How far upstream backwater extends
    
    -- Detection
    detection_method TEXT,                -- 'gradient_analysis', 'stage_comparison', 'flow_reversal'
    confidence TEXT,                      -- 'low', 'medium', 'high'
    
    -- Event metadata
    event_name TEXT,                      -- e.g., "May 2019 Mississippi Backwater"
    notes TEXT,
    verified BOOLEAN DEFAULT false,
    
    -- Tracking
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT valid_backwater_severity CHECK (
        backwater_severity IS NULL OR 
        backwater_severity IN ('minor', 'moderate', 'major', 'extreme')
    )
);

CREATE INDEX idx_backwater_events_active ON usace.backwater_events(event_start DESC)
    WHERE event_end IS NULL;
CREATE INDEX idx_backwater_events_illinois ON usace.backwater_events(illinois_site_code, event_start DESC)
    WHERE illinois_affected = true;

COMMENT ON TABLE usace.backwater_events IS 
    'Detected backwater flooding events where Mississippi River backs up into Illinois River';
COMMENT ON COLUMN usace.backwater_events.gradient_reversal IS 
    'When Mississippi level exceeds Illinois River level, water can flow backwards up the Illinois';

-- ============================================================================
-- CWMS Data Quality Tracking
-- ============================================================================

CREATE TABLE usace.cwms_ingestion_log (
    id BIGSERIAL PRIMARY KEY,
    
    -- Ingestion metadata
    location_id TEXT NOT NULL,
    timeseries_id TEXT,
    
    -- Time range
    query_start TIMESTAMPTZ NOT NULL,
    query_end TIMESTAMPTZ NOT NULL,
    
    -- Results
    records_retrieved INT NOT NULL DEFAULT 0,
    records_inserted INT NOT NULL DEFAULT 0,
    records_updated INT NOT NULL DEFAULT 0,
    records_skipped INT NOT NULL DEFAULT 0,
    
    -- Status
    status TEXT NOT NULL,                 -- 'success', 'partial', 'failed'
    error_message TEXT,
    api_response_code INT,
    
    -- Performance
    duration_ms INT,
    
    -- Tracking
    ingested_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_cwms_log_location ON usace.cwms_ingestion_log(location_id, ingested_at DESC);
CREATE INDEX idx_cwms_log_status ON usace.cwms_ingestion_log(status, ingested_at DESC)
    WHERE status != 'success';

COMMENT ON TABLE usace.cwms_ingestion_log IS 
    'Audit log for CWMS API data ingestion runs';

-- ============================================================================
-- Helper Views
-- ============================================================================

-- Current Mississippi River conditions (backwater risk assessment)
CREATE VIEW usace.mississippi_current_conditions AS
SELECT 
    l.location_id,
    l.location_name,
    l.river_mile,
    l.upstream_of_confluence,
    latest.timestamp as observation_time,
    latest.value as current_stage_ft,
    latest.unit,
    AGE(NOW(), latest.timestamp) as data_age,
    CASE 
        WHEN AGE(NOW(), latest.timestamp) > INTERVAL '2 hours' THEN 'stale'
        WHEN AGE(NOW(), latest.timestamp) > INTERVAL '1 hour' THEN 'aging'
        ELSE 'fresh'
    END as freshness
FROM usace.cwms_locations l
JOIN LATERAL (
    SELECT timestamp, value, unit
    FROM usace.cwms_timeseries t
    WHERE t.location_id = l.location_id
      AND t.parameter_id = 'Stage'
    ORDER BY timestamp DESC
    LIMIT 1
) latest ON true
WHERE l.river_name = 'Mississippi River'
  AND l.monitored = true
ORDER BY l.river_mile DESC;

COMMENT ON VIEW usace.mississippi_current_conditions IS 
    'Latest Mississippi River stage readings for backwater flood risk assessment';

-- Active lock operations summary
CREATE VIEW usace.active_lock_operations AS
SELECT 
    l.location_name,
    l.river_name,
    l.river_mile,
    ops.observation_time,
    ops.pool_elevation_ft,
    ops.pool_target_ft,
    (ops.pool_elevation_ft - ops.pool_target_ft) as pool_deviation_ft,
    ops.outflow_cfs,
    ops.gates_open,
    ops.gates_total,
    ROUND(100.0 * ops.gates_open / NULLIF(ops.gates_total, 0), 1) as gates_open_pct,
    ops.operation_mode,
    ops.flood_control_active,
    AGE(NOW(), ops.observation_time) as data_age
FROM usace.lock_operations ops
JOIN usace.cwms_locations l ON ops.location_id = l.location_id
WHERE ops.observation_time > NOW() - INTERVAL '6 hours'
  AND l.river_name = 'Illinois River'
ORDER BY l.river_mile DESC;

COMMENT ON VIEW usace.active_lock_operations IS 
    'Recent lock and dam operations on Illinois River system';

-- ============================================================================
-- Backwater Detection Function
-- ============================================================================

CREATE OR REPLACE FUNCTION usace.detect_backwater_conditions(
    p_mississippi_location TEXT,
    p_illinois_site_code VARCHAR(8),
    p_threshold_ft NUMERIC DEFAULT 2.0
) RETURNS TABLE (
    backwater_detected BOOLEAN,
    mississippi_stage_ft NUMERIC,
    illinois_stage_ft NUMERIC,
    stage_differential_ft NUMERIC,
    gradient_reversed BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    WITH miss_current AS (
        SELECT value as stage_ft
        FROM usace.cwms_timeseries
        WHERE location_id = p_mississippi_location
          AND parameter_id = 'Stage'
        ORDER BY timestamp DESC
        LIMIT 1
    ),
    il_current AS (
        SELECT value as stage_ft
        FROM usgs_raw.gauge_readings
        WHERE site_code = p_illinois_site_code
          AND parameter_code = '00065'  -- stage
        ORDER BY reading_time DESC
        LIMIT 1
    )
    SELECT 
        (m.stage_ft - i.stage_ft) > p_threshold_ft,
        m.stage_ft,
        i.stage_ft,
        (m.stage_ft - i.stage_ft),
        (m.stage_ft > i.stage_ft)
    FROM miss_current m, il_current i;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION usace.detect_backwater_conditions IS 
    'Compare Mississippi and Illinois River stages to detect backwater flooding conditions';

-- ============================================================================
-- Initial Configuration
-- ============================================================================

-- Pre-populate key Mississippi River locations for backwater monitoring
INSERT INTO usace.cwms_locations (
    location_id, office_id, base_location, location_name, river_name, 
    river_mile, monitored, monitoring_reason, affects_illinois, upstream_of_confluence
) VALUES
    -- Grafton, IL - Just above IL River confluence
    ('Grafton-Mississippi.Stage.Inst.15Minutes.0.Ccp-Rev', 'MVS', 'Grafton-Mississippi', 
     'Mississippi River at Grafton, IL', 'Mississippi River', 
     218.0, true, 'backwater_detection', true, false),
    
    -- Alton, IL - Below confluence
    ('Mel Price TW-Mississippi.Stage.Inst.15Minutes.0.Ccp-Rev', 'MVS', 'Mel Price TW', 
     'Mississippi River at Mel Price L&D (Alton)', 'Mississippi River', 
     200.8, true, 'backwater_detection', true, false),
    
    -- Lock & Dam 24 - Clarksville, MO
    ('LD 24 Pool-Mississippi.Stage.Inst.15Minutes.0.Ccp-Rev', 'MVS', 'LD 24 Pool', 
     'Mississippi River LD 24 Pool', 'Mississippi River', 
     273.4, true, 'dam_operations', true, true),
    
    -- Lock & Dam 25 - Winfield, MO  
    ('LD 25 TW-Mississippi.Stage.Inst.15Minutes.0.Ccp-Rev', 'MVS', 'LD 25 TW', 
     'Mississippi River LD 25 Tailwater', 'Mississippi River', 
     241.4, true, 'backwater_detection', true, true)
ON CONFLICT (location_id) DO NOTHING;

-- Pre-populate Illinois River lock locations
INSERT INTO usace.cwms_locations (
    location_id, office_id, base_location, location_name, river_name,
    river_mile, monitored, monitoring_reason, affects_illinois, location_type
) VALUES
    -- LaGrange Lock & Dam
    ('LaGrange LD-Pool.Stage.Inst.15Minutes.0.Ccp-Rev', 'MVR', 'LaGrange LD-Pool',
     'Illinois River at LaGrange Lock & Dam Pool', 'Illinois River',
     80.2, true, 'dam_operations', true, 'lock_dam'),
    
    -- Peoria Lock & Dam
    ('Peoria LD-Pool.Stage.Inst.15Minutes.0.Ccp-Rev', 'MVR', 'Peoria LD-Pool',
     'Illinois River at Peoria Lock & Dam Pool', 'Illinois River',
     157.6, true, 'dam_operations', true, 'lock_dam'),
    
    -- Starved Rock Lock & Dam
    ('Starved Rock LD-Pool.Stage.Inst.15Minutes.0.Ccp-Rev', 'MVR', 'Starved Rock LD-Pool',
     'Illinois River at Starved Rock Lock & Dam Pool', 'Illinois River',
     231.0, true, 'dam_operations', true, 'lock_dam')
ON CONFLICT (location_id) DO NOTHING;

-- Grant permissions
GRANT USAGE ON SCHEMA usace TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA usace TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA usace TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL FUNCTIONS IN SCHEMA usace TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA usace GRANT ALL ON TABLES TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA usace GRANT ALL ON SEQUENCES TO flopro_admin;
