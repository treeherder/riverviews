-- Flood Event Analysis Schema
-- Migration 005: Comprehensive flood event analysis with multi-source correlation
--
-- Purpose: Link historical flood events to all relevant observational data
--          across USGS, USACE, and future data sources. Enables analysis of
--          flood precursor patterns and contributing factors.
--
-- Author: Riverviews Team
-- Date: 2026-02-20

\set ON_ERROR_STOP on

BEGIN;

-- Create analysis schema
CREATE SCHEMA IF NOT EXISTS flood_analysis;

-- =============================================================================
-- 1. Enhanced Flood Events
-- =============================================================================
-- Extends nws.flood_events with analysis metadata and computed metrics

CREATE TABLE flood_analysis.events (
    id SERIAL PRIMARY KEY,
    
    -- Link to original event record
    source_event_id INTEGER REFERENCES nws.flood_events(id),
    
    -- Event identification
    site_code VARCHAR(15) NOT NULL,
    event_start TIMESTAMPTZ NOT NULL,
    event_peak TIMESTAMPTZ NOT NULL,
    event_end TIMESTAMPTZ,
    
    -- Severity classification
    severity VARCHAR(20) NOT NULL CHECK (severity IN ('minor', 'moderate', 'major', 'extreme')),
    peak_stage_ft NUMERIC(10,2),
    peak_discharge_cfs NUMERIC(12,0),
    flood_stage_ft NUMERIC(10,2),
    
    -- Precursor analysis window
    -- "When did we first see significant rise leading to this flood?"
    precursor_window_start TIMESTAMPTZ,
    precursor_window_end TIMESTAMPTZ,
    
    -- Rise pattern metrics
    total_rise_ft NUMERIC(10,2), -- Total rise from start of precursor to peak
    rise_duration_hours INTEGER,
    average_rise_rate_ft_per_day NUMERIC(10,4),
    max_rise_rate_ft_per_day NUMERIC(10,4),
    
    -- Contributing factors (flags for presence of data)
    has_backwater_data BOOLEAN DEFAULT FALSE,
    has_discharge_data BOOLEAN DEFAULT FALSE,
    has_dam_operation_data BOOLEAN DEFAULT FALSE,
    
    -- Analysis metadata
    analysis_version VARCHAR(20),
    analyzed_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(site_code, event_peak)
);

CREATE INDEX idx_events_site_code ON flood_analysis.events(site_code);
CREATE INDEX idx_events_peak_time ON flood_analysis.events(event_peak);
CREATE INDEX idx_events_severity ON flood_analysis.events(severity);
CREATE INDEX idx_events_precursor_window ON flood_analysis.events(precursor_window_start, precursor_window_end);

COMMENT ON TABLE flood_analysis.events IS 
'Comprehensive flood events with analysis metadata and precursor windows. Links to all observational data sources.';

-- =============================================================================
-- 2. Event Observations (USGS Gauge Data)
-- =============================================================================
-- Links flood events to USGS gauge readings during event window

CREATE TABLE flood_analysis.event_observations (
    observation_id SERIAL PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES flood_analysis.events(id) ON DELETE CASCADE,
    
    -- Observation metadata
    site_code VARCHAR(15) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    
    -- Observation phase within event
    phase VARCHAR(20) CHECK (phase IN ('precursor', 'rising', 'peak', 'falling', 'post')),
    
    -- USGS parameters (from usgs_raw.gauge_readings)
    stage_ft NUMERIC(10,2),
    discharge_cfs NUMERIC(12,0),
    
    -- Time series metrics
    hours_before_peak INTEGER, -- Negative = after peak
    stage_change_24h_ft NUMERIC(10,2), -- Change from 24 hours prior
    
    UNIQUE(event_id, timestamp, site_code)
);

CREATE INDEX idx_observations_event ON flood_analysis.event_observations(event_id);
CREATE INDEX idx_observations_phase ON flood_analysis.event_observations(event_id, phase);
CREATE INDEX idx_observations_timestamp ON flood_analysis.event_observations(timestamp);

COMMENT ON TABLE flood_analysis.event_observations IS 
'USGS gauge readings linked to flood events, with phase classification and time series metrics.';

-- =============================================================================
-- 3. Event CWMS/USACE Data
-- =============================================================================
-- Links flood events to USACE CWMS data (Mississippi River, lock/dam operations)

CREATE TABLE flood_analysis.event_cwms_data (
    cwms_data_id SERIAL PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES flood_analysis.events(id) ON DELETE CASCADE,
    
    -- CWMS location
    location_id VARCHAR(255) NOT NULL,
    location_name VARCHAR(255),
    river_name VARCHAR(100),
    
    -- Observation
    timestamp TIMESTAMPTZ NOT NULL,
    parameter_type VARCHAR(50), -- 'stage', 'flow', 'elevation', 'pool_level', 'gate_position'
    value NUMERIC(15,4),
    unit VARCHAR(20),
    
    -- Backwater analysis (if applicable)
    is_backwater_location BOOLEAN DEFAULT FALSE,
    mississippi_stage_ft NUMERIC(10,2),
    illinois_stage_ft NUMERIC(10,2),
    stage_differential_ft NUMERIC(10,2), -- Mississippi - Illinois
    backwater_detected BOOLEAN,
    
    -- Time context
    hours_before_peak INTEGER,
    
    UNIQUE(event_id, location_id, timestamp, parameter_type)
);

CREATE INDEX idx_cwms_event ON flood_analysis.event_cwms_data(event_id);
CREATE INDEX idx_cwms_location ON flood_analysis.event_cwms_data(location_id);
CREATE INDEX idx_cwms_backwater ON flood_analysis.event_cwms_data(event_id) WHERE backwater_detected = true;

COMMENT ON TABLE flood_analysis.event_cwms_data IS 
'USACE CWMS observations linked to flood events, including backwater analysis.';

-- =============================================================================
-- 4. Event Precursor Conditions
-- =============================================================================
-- Stores identified leading indicators and precursor patterns

CREATE TABLE flood_analysis.event_precursors (
    precursor_id SERIAL PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES flood_analysis.events(id) ON DELETE CASCADE,
    
    -- Precursor metadata
    precursor_type VARCHAR(50) NOT NULL, -- 'rapid_rise', 'sustained_rise', 'backwater_onset', 'high_discharge', 'dam_operations'
    detected_at TIMESTAMPTZ NOT NULL,
    
    -- Precursor description
    description TEXT,
    
    -- Metrics
    severity_score NUMERIC(5,2), -- 0.0 to 10.0 scale
    confidence NUMERIC(5,2), -- 0.0 to 1.0
    
    -- Time before peak
    hours_before_peak INTEGER,
    
    -- Supporting data (JSONB for flexibility)
    metrics JSONB,
    
    -- Examples of metrics JSONB content:
    -- Rapid rise: {"rise_rate_ft_per_day": 2.5, "duration_hours": 36}
    -- Backwater: {"miss_stage_ft": 38.2, "il_stage_ft": 20.1, "differential_ft": 18.1}
    -- High discharge: {"discharge_cfs": 85000, "normal_discharge_cfs": 25000}
    
    UNIQUE(event_id, precursor_type, detected_at)
);

CREATE INDEX idx_precursors_event ON flood_analysis.event_precursors(event_id);
CREATE INDEX idx_precursors_type ON flood_analysis.event_precursors(precursor_type);
CREATE INDEX idx_precursors_severity ON flood_analysis.event_precursors(severity_score DESC);

COMMENT ON TABLE flood_analysis.event_precursors IS 
'Identified leading indicators and precursor patterns for each flood event.';

-- =============================================================================
-- 5. Event Metrics Summary
-- =============================================================================
-- Computed aggregate metrics for each event

CREATE TABLE flood_analysis.event_metrics (
    metric_id SERIAL PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES flood_analysis.events(id) ON DELETE CASCADE,
    
    -- Rise phase metrics
    initial_stage_ft NUMERIC(10,2),
    peak_stage_ft NUMERIC(10,2),
    total_rise_ft NUMERIC(10,2),
    rise_duration_hours INTEGER,
    avg_rise_rate_ft_per_day NUMERIC(10,4),
    max_single_day_rise_ft NUMERIC(10,4),
    
    -- Peak phase metrics
    peak_discharge_cfs NUMERIC(12,0),
    peak_stage_timestamp TIMESTAMPTZ,
    hours_above_flood_stage INTEGER,
    
    -- Fall phase metrics
    fall_duration_hours INTEGER,
    avg_fall_rate_ft_per_day NUMERIC(10,4),
    
    -- Comparative metrics
    exceedance_above_flood_stage_ft NUMERIC(10,2),
    percentile_rank INTEGER, -- Within all events for this site (0-100)
    
    -- Contributing factors summary
    backwater_contribution_ft NUMERIC(10,2), -- Estimated contribution from backwater
    upstream_influence BOOLEAN,
    dam_operations_active BOOLEAN,
    
    -- Data quality
    observation_count INTEGER,
    data_completeness_pct NUMERIC(5,2),
    
    UNIQUE(event_id)
);

CREATE INDEX idx_metrics_event ON flood_analysis.event_metrics(event_id);

COMMENT ON TABLE flood_analysis.event_metrics IS 
'Aggregate computed metrics for each flood event.';

-- =============================================================================
-- 6. Analysis Configuration
-- =============================================================================
-- Stores parameters used for flood event analysis

CREATE TABLE flood_analysis.analysis_config (
    config_id SERIAL PRIMARY KEY,
    config_name VARCHAR(100) UNIQUE NOT NULL,
    
    -- Precursor detection parameters
    precursor_lookback_days INTEGER DEFAULT 14, -- How many days before peak to analyze
    significant_rise_threshold_ft NUMERIC(10,2) DEFAULT 2.0, -- Minimum rise to consider "significant"
    rise_rate_threshold_ft_per_day NUMERIC(10,4) DEFAULT 0.5, -- Minimum rate of rise
    
    -- Event window parameters
    post_peak_window_days INTEGER DEFAULT 7, -- Days after peak to include
    
    -- Backwater detection parameters
    backwater_differential_threshold_ft NUMERIC(10,2) DEFAULT 2.0,
    
    -- Data quality thresholds
    min_observation_count INTEGER DEFAULT 10,
    min_data_completeness_pct NUMERIC(5,2) DEFAULT 70.0,
    
    -- Metadata
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    is_active BOOLEAN DEFAULT TRUE
);

-- Insert default configuration
INSERT INTO flood_analysis.analysis_config 
    (config_name, description) 
VALUES 
    ('default', 'Default flood event analysis configuration');

COMMENT ON TABLE flood_analysis.analysis_config IS 
'Parameters controlling flood event analysis and precursor detection.';

-- =============================================================================
-- Views for Common Queries
-- =============================================================================

-- View: Complete event summary with all metrics
CREATE VIEW flood_analysis.event_summary AS
SELECT 
    e.id,
    e.site_code,
    e.event_peak,
    e.severity,
    e.peak_stage_ft,
    e.peak_discharge_cfs,
    e.precursor_window_start,
    e.rise_duration_hours,
    e.average_rise_rate_ft_per_day,
    
    -- Metrics
    m.total_rise_ft,
    m.hours_above_flood_stage,
    m.exceedance_above_flood_stage_ft,
    m.percentile_rank,
    m.backwater_contribution_ft,
    
    -- Precursor counts
    (SELECT COUNT(*) FROM flood_analysis.event_precursors p 
     WHERE p.event_id = e.id) as precursor_count,
    
    -- Data availability
    e.has_backwater_data,
    e.has_discharge_data,
    e.has_dam_operation_data,
    
    -- Observation counts
    (SELECT COUNT(*) FROM flood_analysis.event_observations o 
     WHERE o.event_id = e.id) as usgs_observation_count,
    (SELECT COUNT(*) FROM flood_analysis.event_cwms_data c 
     WHERE c.event_id = e.id) as cwms_observation_count
     
FROM flood_analysis.events e
LEFT JOIN flood_analysis.event_metrics m ON e.id = m.event_id
ORDER BY e.event_peak DESC;

COMMENT ON VIEW flood_analysis.event_summary IS 
'Complete summary of all flood events with metrics and data availability.';

-- View: Events with backwater influence
CREATE VIEW flood_analysis.backwater_influenced_events AS
SELECT 
    e.*,
    COUNT(DISTINCT c.cwms_data_id) as backwater_observation_count,
    AVG(c.stage_differential_ft) as avg_stage_differential_ft,
    MAX(c.stage_differential_ft) as max_stage_differential_ft
FROM flood_analysis.events e
INNER JOIN flood_analysis.event_cwms_data c ON e.id = c.event_id
WHERE c.backwater_detected = true
GROUP BY e.id
ORDER BY e.event_peak DESC;

COMMENT ON VIEW flood_analysis.backwater_influenced_events IS 
'Flood events where Mississippi River backwater was detected.';

-- =============================================================================
-- Helper Functions
-- =============================================================================

-- Function: Calculate rise rate between two timestamps
CREATE OR REPLACE FUNCTION flood_analysis.calculate_rise_rate(
    p_site_code VARCHAR,
    p_start_time TIMESTAMPTZ,
    p_end_time TIMESTAMPTZ
) RETURNS NUMERIC AS $$
DECLARE
    v_start_stage NUMERIC;
    v_end_stage NUMERIC;
    v_hours NUMERIC;
BEGIN
    -- Get stage at start
    SELECT stage_ft INTO v_start_stage
    FROM flood_analysis.event_observations
    WHERE site_code = p_site_code 
      AND timestamp <= p_start_time
    ORDER BY ABS(EXTRACT(EPOCH FROM (timestamp - p_start_time)))
    LIMIT 1;
    
    -- Get stage at end
    SELECT stage_ft INTO v_end_stage
    FROM flood_analysis.event_observations
    WHERE site_code = p_site_code 
      AND timestamp <= p_end_time
    ORDER BY ABS(EXTRACT(EPOCH FROM (timestamp - p_end_time)))
    LIMIT 1;
    
    IF v_start_stage IS NULL OR v_end_stage IS NULL THEN
        RETURN NULL;
    END IF;
    
    v_hours := EXTRACT(EPOCH FROM (p_end_time - p_start_time)) / 3600.0;
    
    IF v_hours <= 0 THEN
        RETURN NULL;
    END IF;
    
    -- Return rise rate in feet per day
    RETURN ((v_end_stage - v_start_stage) / v_hours) * 24.0;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION flood_analysis.calculate_rise_rate IS 
'Calculate rise rate in feet per day between two timestamps for a gauge site.';

-- =============================================================================
-- Permissions
-- =============================================================================

GRANT USAGE ON SCHEMA flood_analysis TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA flood_analysis TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA flood_analysis TO flopro_admin;
GRANT ALL PRIVILEGES ON ALL FUNCTIONS IN SCHEMA flood_analysis TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA flood_analysis GRANT ALL ON TABLES TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA flood_analysis GRANT ALL ON SEQUENCES TO flopro_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA flood_analysis GRANT ALL ON FUNCTIONS TO flopro_admin;

COMMIT;

-- Verification
\echo ''
\echo 'Flood Event Analysis Schema Created Successfully!'
\echo ''
\echo 'Schema: flood_analysis'
\echo 'Tables:'
SELECT schemaname, tablename 
FROM pg_tables 
WHERE schemaname = 'flood_analysis'
ORDER BY tablename;
