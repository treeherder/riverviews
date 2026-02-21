-- ============================================================================
-- 006_iem_asos.sql
--
-- ASOS (Automated Surface Observing System) Station Data Schema
--
-- Purpose:
--   Store weather observations from IEM (Iowa Environmental Mesonet) ASOS
--   stations for precipitation and meteorological monitoring relevant to
--   tributary flood forecasting in the Illinois River basin.
--
-- Data Sources:
--   - IEM ASOS 1-minute precipitation: https://mesonet.agron.iastate.edu/cgi-bin/request/asos1min.py
--   - IEM Current Observations: https://mesonet.agron.iastate.edu/json/current.py
--
-- Key Stations:
--   - KPIA: Peoria (primary local precip station)
--   - KBMI: Bloomington (Mackinaw River basin)
--   - KSPI: Springfield (Sangamon River basin)
--   - KGBG: Galesburg (Spoon River basin)
--   - KORD: Chicago O'Hare (Des Plaines River basin)
--   - KPWK: Wheeling (Des Plaines River tributary)
--
-- Retention Policy:
--   - Keep 1-minute precipitation data for 90 days (critical for event analysis)
--   - Keep hourly summary data indefinitely
--   - Archive to cold storage after 1 year
--
-- Monitoring Priorities:
--   - CRITICAL (15 min): KPIA (primary local)
--   - HIGH (60 min): KBMI, KSPI, KGBG (tributary basins)
--   - MEDIUM (6 hr): KORD, KPWK (extended coverage)
--
-- ============================================================================

-- ============================================================================
-- ASOS Station Metadata Registry
-- ============================================================================

CREATE TABLE IF NOT EXISTS asos_stations (
    station_id TEXT PRIMARY KEY,                   -- ASOS station ID (e.g., "KPIA")
    name TEXT NOT NULL,                            -- Human-readable name
    latitude DOUBLE PRECISION NOT NULL,            -- Decimal degrees
    longitude DOUBLE PRECISION NOT NULL,           -- Decimal degrees
    elevation_ft DOUBLE PRECISION NOT NULL,        -- Elevation in feet MSL
    basin TEXT NOT NULL,                           -- Associated river basin
    upstream_gauge TEXT,                           -- Associated USGS gauge
    priority TEXT NOT NULL,                        -- CRITICAL, HIGH, MEDIUM, LOW
    poll_interval_minutes INTEGER NOT NULL,        -- How often to poll IEM API
    data_types TEXT[] NOT NULL,                    -- Data types to monitor
    relevance TEXT NOT NULL,                       -- Why we monitor this station
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

COMMENT ON TABLE asos_stations IS 
'ASOS station metadata from iem_asos.toml configuration';

COMMENT ON COLUMN asos_stations.station_id IS 
'ASOS station identifier (e.g., KPIA for Peoria)';

COMMENT ON COLUMN asos_stations.basin IS 
'Associated tributary basin (Mackinaw, Spoon, Sangamon, Des Plaines, Illinois)';

COMMENT ON COLUMN asos_stations.upstream_gauge IS 
'USGS gauge ID that responds to precipitation at this station';

-- ============================================================================
-- ASOS Observations (1-minute to hourly resolution)
-- ============================================================================

CREATE TABLE IF NOT EXISTS asos_observations (
    id BIGSERIAL PRIMARY KEY,
    station_id TEXT NOT NULL REFERENCES asos_stations(station_id),
    observation_time TIMESTAMP WITH TIME ZONE NOT NULL,
    
    -- Temperature and Humidity
    temp_f DOUBLE PRECISION,                       -- Air temperature (°F)
    dewpoint_f DOUBLE PRECISION,                   -- Dewpoint (°F)
    relative_humidity DOUBLE PRECISION,            -- Relative humidity (%)
    
    -- Wind
    wind_direction_deg DOUBLE PRECISION,           -- Wind direction (degrees)
    wind_speed_knots DOUBLE PRECISION,             -- Wind speed (knots)
    wind_gust_knots DOUBLE PRECISION,              -- Wind gust (knots)
    
    -- Precipitation (CRITICAL for flood forecasting)
    precip_1hr_in DOUBLE PRECISION,                -- 1-hour precipitation (inches)
    precip_1min_in DOUBLE PRECISION,               -- 1-minute precipitation (inches) - if available
    
    -- Pressure and Visibility
    pressure_mb DOUBLE PRECISION,                  -- Sea level pressure (millibars)
    visibility_mi DOUBLE PRECISION,                -- Visibility (statute miles)
    
    -- Sky Condition and Weather
    sky_condition TEXT,                            -- Sky cover (CLR, FEW, SCT, BKN, OVC)
    weather_codes TEXT,                            -- Present weather (RA, SN, FG, etc.)
    
    -- Metadata
    data_source TEXT NOT NULL,                     -- 'IEM_CURRENT' or 'IEM_1MIN'
    ingested_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    UNIQUE(station_id, observation_time)           -- Prevent duplicates
);

COMMENT ON TABLE asos_observations IS 
'Weather observations from ASOS stations via Iowa Environmental Mesonet API';

COMMENT ON COLUMN asos_observations.precip_1hr_in IS 
'1-hour precipitation accumulation - CRITICAL for tributary flood risk assessment';

COMMENT ON COLUMN asos_observations.weather_codes IS 
'Present weather codes (RA=rain, SN=snow, TSRA=thunderstorm, etc.)';

-- ============================================================================
-- Precipitation Aggregations (for faster queries)
-- ============================================================================

CREATE TABLE IF NOT EXISTS asos_precip_summary (
    id BIGSERIAL PRIMARY KEY,
    station_id TEXT NOT NULL REFERENCES asos_stations(station_id),
    period_start TIMESTAMP WITH TIME ZONE NOT NULL,
    period_end TIMESTAMP WITH TIME ZONE NOT NULL,
    
    -- Precipitation Totals
    precip_total_in DOUBLE PRECISION NOT NULL,     -- Total precipitation in period
    precip_max_1hr_in DOUBLE PRECISION,            -- Maximum 1-hour intensity
    hours_with_precip INTEGER,                     -- Number of hours with measurable precip
    
    -- Basin-specific Flood Risk Indicators
    exceeds_watch_threshold BOOLEAN DEFAULT FALSE, -- >= basin watch threshold
    exceeds_warning_threshold BOOLEAN DEFAULT FALSE, -- >= basin warning threshold
    
    -- Summary Period Type
    period_type TEXT NOT NULL,                     -- '6HR', '12HR', '24HR', '48HR'
    
    -- Metadata
    computed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    UNIQUE(station_id, period_start, period_type)
);

COMMENT ON TABLE asos_precip_summary IS 
'Pre-computed precipitation summaries for flood risk thresholds';

COMMENT ON COLUMN asos_precip_summary.exceeds_watch_threshold IS 
'Precipitation exceeds basin-specific flood watch threshold';

COMMENT ON COLUMN asos_precip_summary.exceeds_warning_threshold IS 
'Precipitation exceeds basin-specific flood warning threshold';

-- ============================================================================
-- Indexes for Query Performance
-- ============================================================================

-- Primary time-series queries
CREATE INDEX IF NOT EXISTS idx_asos_obs_station_time 
    ON asos_observations(station_id, observation_time DESC);

-- Precipitation analysis
CREATE INDEX IF NOT EXISTS idx_asos_obs_precip 
    ON asos_observations(station_id, observation_time DESC) 
    WHERE precip_1hr_in IS NOT NULL;

-- Recent observations (last 24 hours)
CREATE INDEX IF NOT EXISTS idx_asos_obs_recent 
    ON asos_observations(observation_time DESC) 
    WHERE observation_time >= NOW() - INTERVAL '24 hours';

-- Precipitation summary queries
CREATE INDEX IF NOT EXISTS idx_asos_summary_station_period 
    ON asos_precip_summary(station_id, period_start DESC, period_type);

-- Active flood watches/warnings
CREATE INDEX IF NOT EXISTS idx_asos_summary_flood_risk 
    ON asos_precip_summary(station_id, period_start DESC) 
    WHERE exceeds_watch_threshold = TRUE OR exceeds_warning_threshold = TRUE;

-- ============================================================================
-- Data Retention Policies (cleanup old data)
-- ============================================================================

-- Partition asos_observations by month for efficient cleanup
-- (Optional: can be enabled later for production scale)

-- Function to clean up old 1-minute data
CREATE OR REPLACE FUNCTION cleanup_asos_observations() 
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM asos_observations
    WHERE observation_time < NOW() - INTERVAL '90 days'
      AND data_source = 'IEM_1MIN';
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION cleanup_asos_observations() IS 
'Delete 1-minute ASOS data older than 90 days (keep hourly data indefinitely)';

-- ============================================================================
-- Helper Views
-- ============================================================================

-- Latest observations per station
CREATE OR REPLACE VIEW asos_latest AS
SELECT DISTINCT ON (station_id)
    station_id,
    observation_time,
    temp_f,
    dewpoint_f,
    wind_speed_knots,
    precip_1hr_in,
    weather_codes,
    ingested_at
FROM asos_observations
ORDER BY station_id, observation_time DESC;

COMMENT ON VIEW asos_latest IS 
'Most recent observation for each ASOS station';

-- Active precipitation (last 6 hours)
CREATE OR REPLACE VIEW asos_active_precip AS
SELECT 
    st.station_id,
    st.name,
    st.basin,
    SUM(obs.precip_1hr_in) AS precip_6hr_in,
    MAX(obs.observation_time) AS latest_observation
FROM asos_stations st
LEFT JOIN asos_observations obs 
    ON st.station_id = obs.station_id
    AND obs.observation_time >= NOW() - INTERVAL '6 hours'
WHERE obs.precip_1hr_in IS NOT NULL
GROUP BY st.station_id, st.name, st.basin
ORDER BY precip_6hr_in DESC NULLS LAST;

COMMENT ON VIEW asos_active_precip IS 
'6-hour precipitation totals for all ASOS stations (flood watch threshold)';

-- ============================================================================
-- Basin Precipitation Thresholds (reference data)
-- ============================================================================

CREATE TABLE IF NOT EXISTS basin_precip_thresholds (
    basin TEXT PRIMARY KEY,
    watch_6hr_in DOUBLE PRECISION NOT NULL,        -- Flood watch threshold
    warning_6hr_in DOUBLE PRECISION NOT NULL,      -- Flood warning threshold
    watch_24hr_in DOUBLE PRECISION NOT NULL,
    warning_24hr_in DOUBLE PRECISION NOT NULL,
    lag_hours INTEGER NOT NULL,                    -- Precip to stream response lag
    notes TEXT
);

COMMENT ON TABLE basin_precip_thresholds IS 
'Precipitation thresholds for flood watch/warning by tributary basin';

INSERT INTO basin_precip_thresholds (basin, watch_6hr_in, warning_6hr_in, watch_24hr_in, warning_24hr_in, lag_hours, notes)
VALUES
    ('Illinois River', 1.5, 2.5, 3.0, 5.0, 48, 'Mainstem - slow response'),
    ('Mackinaw River', 1.0, 2.0, 2.5, 4.0, 12, 'Bloomington to Green Valley'),
    ('Spoon River', 1.2, 2.5, 3.0, 5.0, 18, 'Galesburg to Seville'),
    ('Sangamon River', 1.5, 3.0, 3.5, 5.5, 24, 'Springfield to Oakford'),
    ('Des Plaines River', 1.0, 2.0, 2.5, 4.5, 6, 'Chicago metro - fast response')
ON CONFLICT (basin) DO NOTHING;

-- ============================================================================
-- Permissions
-- ============================================================================

GRANT SELECT, INSERT, UPDATE ON asos_stations TO flomon_user;
GRANT SELECT, INSERT, UPDATE ON asos_observations TO flomon_user;
GRANT SELECT, INSERT, UPDATE ON asos_precip_summary TO flomon_user;
GRANT SELECT ON asos_latest TO flomon_user;
GRANT SELECT ON asos_active_precip TO flomon_user;
GRANT SELECT ON basin_precip_thresholds TO flomon_user;
GRANT USAGE, SELECT ON SEQUENCE asos_observations_id_seq TO flomon_user;
GRANT USAGE, SELECT ON SEQUENCE asos_precip_summary_id_seq TO flomon_user;

-- ============================================================================
-- Verification Queries
-- ============================================================================

-- Uncomment to verify schema after applying:

-- SELECT * FROM asos_stations ORDER BY priority;
-- SELECT * FROM basin_precip_thresholds ORDER BY lag_hours;
-- SELECT * FROM asos_latest;
-- SELECT * FROM asos_active_precip WHERE precip_6hr_in > 0.5;

