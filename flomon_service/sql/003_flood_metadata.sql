-- FloPro Database Schema - Flood Metadata
-- Version: 003
-- Description: NWS flood thresholds and historical flood event tracking
-- 
-- This migration adds relational data for:
--   1. Official NWS flood stage thresholds (currently hardcoded in Rust)
--   2. Historical flood events (time windows when flooding occurred)
--   3. Discharge correlation data (flow rates associated with flood stages)
--
-- WHY NOW: We're about to load 87 years of historical data (1939-2026).
-- Adding this schema first allows us to:
--   - Identify and tag historical flood events automatically during ingestion
--   - Validate threshold accuracy against known flood occurrences
--   - Build training datasets for predictive models
--   - Understand lag times between upstream/downstream stations

-- ============================================================================
-- NWS FLOOD THRESHOLDS
-- ============================================================================

-- Official NWS AHPS flood stage categories for each monitored station.
-- Source: https://water.weather.gov/ahps/
CREATE TABLE nws.flood_thresholds (
    site_code VARCHAR(8) PRIMARY KEY REFERENCES usgs_raw.sites(site_code),
    
    -- NWS flood stage categories (in feet above gauge datum)
    action_stage_ft NUMERIC(6, 2),          -- Preparation stage - monitor closely
    flood_stage_ft NUMERIC(6, 2),           -- Minor flooding begins (NWS "flood")
    moderate_flood_stage_ft NUMERIC(6, 2),  -- Significant property damage
    major_flood_stage_ft NUMERIC(6, 2),     -- Severe widespread flooding
    
    -- Metadata
    source TEXT NOT NULL DEFAULT 'NWS AHPS',  -- Data source
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,  -- When these thresholds became official
    last_verified TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    notes TEXT,  -- e.g., "Updated after 2013 flood re-survey"
    
    -- Data validation
    CONSTRAINT thresholds_ascending CHECK (
        action_stage_ft < flood_stage_ft AND
        flood_stage_ft < moderate_flood_stage_ft AND
        moderate_flood_stage_ft < major_flood_stage_ft
    ),
    CONSTRAINT thresholds_reasonable CHECK (
        action_stage_ft > 0 AND major_flood_stage_ft < 100  -- sanity check
    )
);

COMMENT ON TABLE nws.flood_thresholds IS 
    'Official NWS Advanced Hydrologic Prediction Service flood stage categories';
COMMENT ON COLUMN nws.flood_thresholds.action_stage_ft IS 
    'Stage at which preparation should begin (lowest alert level)';
COMMENT ON COLUMN nws.flood_thresholds.flood_stage_ft IS 
    'Minor flooding begins - typically roadway/agricultural flooding';
COMMENT ON COLUMN nws.flood_thresholds.moderate_flood_stage_ft IS 
    'Moderate flooding - structures threatened, evacuations may begin';
COMMENT ON COLUMN nws.flood_thresholds.major_flood_stage_ft IS 
    'Major flooding - widespread severe inundation, life-threatening';

-- Optional: Track threshold changes over time (if NWS revises after flood studies)
CREATE TABLE nws.flood_threshold_history (
    id SERIAL PRIMARY KEY,
    site_code VARCHAR(8) NOT NULL REFERENCES usgs_raw.sites(site_code),
    
    action_stage_ft NUMERIC(6, 2),
    flood_stage_ft NUMERIC(6, 2),
    moderate_flood_stage_ft NUMERIC(6, 2),
    major_flood_stage_ft NUMERIC(6, 2),
    
    effective_date DATE NOT NULL,
    superseded_date DATE,  -- When these thresholds were replaced
    change_reason TEXT,    -- e.g., "Post-2013 flood bathymetric survey"
    
    CONSTRAINT history_valid_date_range CHECK (
        superseded_date IS NULL OR superseded_date > effective_date
    )
);

CREATE INDEX idx_threshold_history_site ON nws.flood_threshold_history(site_code, effective_date);

-- ============================================================================
-- HISTORICAL FLOOD EVENTS
-- ============================================================================

-- Record of past flood events, used for:
--   - Validating alert system against known floods
--   - Training predictive models
--   - Understanding lead times between stations
--   - Correlating weather/flow patterns
CREATE TABLE nws.flood_events (
    id SERIAL PRIMARY KEY,
    site_code VARCHAR(8) NOT NULL REFERENCES usgs_raw.sites(site_code),
    
    -- Event timing
    event_start TIMESTAMPTZ NOT NULL,      -- When stage exceeded flood_stage_ft
    event_end TIMESTAMPTZ,                  -- When stage dropped below flood_stage_ft (NULL if ongoing)
    crest_time TIMESTAMPTZ,                 -- When peak stage occurred
    
    -- Event severity
    peak_stage_ft NUMERIC(6, 2) NOT NULL,   -- Maximum stage during event
    severity VARCHAR(20) NOT NULL,          -- 'flood', 'moderate', 'major'
    
    -- Optional metadata
    event_name TEXT,                        -- e.g., "Spring 2019 Flood", "May 2013 Historic Flood"
    notes TEXT,                             -- Narrative description, impacts, damage
    
    -- Data quality
    data_source TEXT NOT NULL DEFAULT 'USGS gauge readings',
    verified BOOLEAN NOT NULL DEFAULT false,  -- Cross-referenced with NWS narratives?
    
    CONSTRAINT valid_event_times CHECK (
        event_end IS NULL OR event_end > event_start
    ),
    CONSTRAINT valid_crest_time CHECK (
        crest_time IS NULL OR 
        (crest_time >= event_start AND (event_end IS NULL OR crest_time <= event_end))
    ),
    CONSTRAINT valid_severity CHECK (
        severity IN ('flood', 'moderate', 'major')
    )
);

CREATE INDEX idx_flood_events_site_time ON nws.flood_events(site_code, event_start);
CREATE INDEX idx_flood_events_severity ON nws.flood_events(severity);
CREATE INDEX idx_flood_events_ongoing ON nws.flood_events(site_code) WHERE event_end IS NULL;

COMMENT ON TABLE nws.flood_events IS 
    'Historical flood events identified from gauge readings or NWS records';
COMMENT ON COLUMN nws.flood_events.severity IS 
    'Corresponds to NWS categories: flood (minor), moderate, major';
COMMENT ON COLUMN nws.flood_events.verified IS 
    'True if cross-referenced with official NWS flood event database';

-- ============================================================================
-- DISCHARGE-STAGE CORRELATION (optional but valuable)
-- ============================================================================

-- Many locations have both stage and discharge thresholds. For example,
-- a certain flow rate (CFS) reliably produces a certain stage (ft).
-- This helps predict flooding when only discharge data is available.
CREATE TABLE nws.discharge_thresholds (
    site_code VARCHAR(8) PRIMARY KEY REFERENCES usgs_raw.sites(site_code),
    
    -- Discharge thresholds (cubic feet per second) corresponding to flood stages
    action_discharge_cfs NUMERIC(10, 2),
    flood_discharge_cfs NUMERIC(10, 2),
    moderate_flood_discharge_cfs NUMERIC(10, 2),
    major_flood_discharge_cfs NUMERIC(10, 2),
    
    -- Metadata
    source TEXT NOT NULL DEFAULT 'NWS AHPS / Rating Curve',
    last_verified TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    notes TEXT,  -- e.g., "Based on pre-dam rating curve"
    
    CONSTRAINT discharge_thresholds_ascending CHECK (
        action_discharge_cfs < flood_discharge_cfs AND
        flood_discharge_cfs < moderate_flood_discharge_cfs AND
        moderate_flood_discharge_cfs < major_flood_discharge_cfs
    )
);

COMMENT ON TABLE nws.discharge_thresholds IS 
    'Discharge (flow) thresholds corresponding to flood stage categories';
COMMENT ON COLUMN nws.discharge_thresholds.flood_discharge_cfs IS 
    'Flow rate typically producing minor flood stage at this location';

-- ============================================================================
-- MATERIALIZED VIEW: FLOOD EVENT SUMMARY
-- ============================================================================

-- Quickly answer: "How many times has Kingston Mines flooded since 1939?"
CREATE MATERIALIZED VIEW nws.flood_event_summary AS
SELECT 
    s.site_code,
    s.site_name,
    COUNT(*) as total_events,
    COUNT(*) FILTER (WHERE severity = 'flood') as minor_floods,
    COUNT(*) FILTER (WHERE severity = 'moderate') as moderate_floods,
    COUNT(*) FILTER (WHERE severity = 'major') as major_floods,
    MAX(peak_stage_ft) as highest_crest_ft,
    MAX(crest_time) as most_recent_flood,
    AVG(EXTRACT(EPOCH FROM (event_end - event_start)) / 3600) as avg_duration_hours
FROM nws.flood_events e
JOIN usgs_raw.sites s USING (site_code)
WHERE event_end IS NOT NULL  -- Only completed events
GROUP BY s.site_code, s.site_name
ORDER BY total_events DESC;

CREATE UNIQUE INDEX idx_flood_summary_site ON nws.flood_event_summary(site_code);

COMMENT ON MATERIALIZED VIEW nws.flood_event_summary IS 
    'Aggregated flood statistics per station (refresh after ingestion)';

-- ============================================================================
-- DATA POPULATION (from hardcoded Rust thresholds)
-- ============================================================================

-- Initialize flood thresholds from the values currently in src/stations.rs
-- Source: NWS AHPS verified Feb 2026
INSERT INTO nws.flood_thresholds (site_code, action_stage_ft, flood_stage_ft, moderate_flood_stage_ft, major_flood_stage_ft, notes)
VALUES
    ('05568500', 14.0, 16.0, 20.0, 24.0, 'Kingston Mines - Primary Peoria flood reference'),
    ('05568000', 13.0, 15.0, 19.0, 23.0, 'Chillicothe - 6-12hr lead time for Peoria'),
    ('05557000', 13.0, 15.0, 19.0, 22.0, 'Henry - 12-24hr lead time for Peoria'),
    ('05552500', 12.0, 14.0, 18.0, 22.0, 'Marseilles - 24-48hr lead time for Peoria')
ON CONFLICT (site_code) DO NOTHING;

-- Note: Peoria (05567500), Mackinaw (05568580), Spoon River (05570000), 
-- and Chicago Canal (05536890) do not have NWS flood thresholds defined.
-- Peoria is a managed pool gauge; the others are tributaries without
-- official flood stage categories.

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Function to detect flood events from historical gauge readings
-- Run this AFTER loading historical data to auto-populate nws.flood_events
CREATE OR REPLACE FUNCTION nws.detect_flood_events(
    p_site_code VARCHAR(8),
    p_min_duration_hours INT DEFAULT 6
) RETURNS TABLE (
    event_start TIMESTAMPTZ,
    event_end TIMESTAMPTZ,
    crest_time TIMESTAMPTZ,
    peak_stage_ft NUMERIC,
    severity TEXT
) AS $$
BEGIN
    -- This is a placeholder - actual implementation would:
    -- 1. Query gauge_readings for this site_code, parameter 00065 (stage)
    -- 2. Join with flood_thresholds to get severity levels
    -- 3. Identify continuous periods above flood_stage_ft
    -- 4. Filter out events shorter than min_duration_hours
    -- 5. Return event windows with peak stage and severity
    
    RETURN QUERY
    SELECT 
        NULL::TIMESTAMPTZ as event_start,
        NULL::TIMESTAMPTZ as event_end, 
        NULL::TIMESTAMPTZ as crest_time,
        NULL::NUMERIC as peak_stage_ft,
        NULL::TEXT as severity
    LIMIT 0;  -- Return empty set for now
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION nws.detect_flood_events IS 
    'Analyzes gauge readings to identify historical flood events (run after data ingestion)';

-- ============================================================================
-- GRANTS (adjust based on your user setup)
-- ============================================================================

-- GRANT SELECT ON ALL TABLES IN SCHEMA nws TO readonly_user;
-- GRANT ALL ON ALL TABLES IN SCHEMA nws TO flomon_service;

