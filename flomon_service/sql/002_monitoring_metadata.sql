-- Monitoring Service State Tracking
-- Tracks when we last polled each station and what we received

CREATE TABLE usgs_raw.monitoring_state (
    site_code VARCHAR(8) PRIMARY KEY,
    parameter_code VARCHAR(5) NOT NULL,
    
    -- Polling metadata
    last_poll_attempted TIMESTAMPTZ,           -- When we last tried to fetch data
    last_poll_succeeded TIMESTAMPTZ,           -- Last successful API response
    last_data_received TIMESTAMPTZ,            -- Last time we got fresh readings
    
    -- Most recent reading metadata
    latest_reading_time TIMESTAMPTZ,           -- Timestamp of newest reading in DB
    latest_reading_value NUMERIC(12, 4),       -- Value of newest reading
    
    -- Station health
    consecutive_failures INTEGER DEFAULT 0,     -- Failed polls in a row
    status VARCHAR(20) DEFAULT 'active',       -- active, degraded, offline, unknown
    status_since TIMESTAMPTZ DEFAULT NOW(),    -- When status last changed
    
    -- Staleness tracking
    is_stale BOOLEAN DEFAULT false,            -- Current staleness state
    stale_since TIMESTAMPTZ,                   -- When it became stale
    staleness_threshold_minutes INTEGER DEFAULT 60,  -- Threshold for this station
    
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_monitoring_state_status ON usgs_raw.monitoring_state(status);
CREATE INDEX idx_monitoring_state_stale ON usgs_raw.monitoring_state(is_stale) WHERE is_stale = true;

COMMENT ON TABLE usgs_raw.monitoring_state IS 'Real-time monitoring service state per station';
COMMENT ON COLUMN usgs_raw.monitoring_state.last_poll_attempted IS 'Last time we queried USGS API (success or failure)';
COMMENT ON COLUMN usgs_raw.monitoring_state.last_data_received IS 'Last time API returned fresh readings';
COMMENT ON COLUMN usgs_raw.monitoring_state.consecutive_failures IS 'Reset to 0 on successful poll';

-- Initialize monitoring state for all active sites
INSERT INTO usgs_raw.monitoring_state (site_code, parameter_code, staleness_threshold_minutes)
SELECT 
    s.site_code,
    '00060' as parameter_code,  -- discharge
    CASE 
        WHEN s.site_code IN ('05568500', '05567500', '05568000') THEN 20  -- Critical sites: 20 min
        ELSE 60  -- Normal sites: 60 min
    END as staleness_threshold_minutes
FROM usgs_raw.sites s
WHERE s.active = true
ON CONFLICT (site_code) DO NOTHING;

-- Also track stage for all sites
INSERT INTO usgs_raw.monitoring_state (site_code, parameter_code, staleness_threshold_minutes)
SELECT 
    s.site_code,
    '00065' as parameter_code,  -- stage
    CASE 
        WHEN s.site_code IN ('05568500', '05567500', '05568000') THEN 20
        ELSE 60
    END as staleness_threshold_minutes
FROM usgs_raw.sites s
WHERE s.active = true
ON CONFLICT (site_code) DO NOTHING;

-- Function to update monitoring state after each poll
CREATE OR REPLACE FUNCTION usgs_raw.update_monitoring_state(
    p_site_code VARCHAR(8),
    p_parameter_code VARCHAR(5),
    p_poll_succeeded BOOLEAN,
    p_readings_count INTEGER,
    p_latest_reading_time TIMESTAMPTZ DEFAULT NULL,
    p_latest_reading_value NUMERIC DEFAULT NULL
)
RETURNS void AS $$
DECLARE
    v_threshold_minutes INTEGER;
    v_reading_age_minutes INTEGER;
    v_is_stale BOOLEAN;
    v_old_status VARCHAR(20);
BEGIN
    -- Get current state
    SELECT staleness_threshold_minutes, status
    INTO v_threshold_minutes, v_old_status
    FROM usgs_raw.monitoring_state
    WHERE site_code = p_site_code AND parameter_code = p_parameter_code;
    
    -- Calculate staleness if we have a reading
    IF p_latest_reading_time IS NOT NULL THEN
        v_reading_age_minutes := EXTRACT(EPOCH FROM (NOW() - p_latest_reading_time)) / 60;
        v_is_stale := v_reading_age_minutes > v_threshold_minutes;
    ELSE
        v_is_stale := true;
    END IF;
    
    -- Update state
    UPDATE usgs_raw.monitoring_state
    SET
        last_poll_attempted = NOW(),
        last_poll_succeeded = CASE WHEN p_poll_succeeded THEN NOW() ELSE last_poll_succeeded END,
        last_data_received = CASE WHEN p_readings_count > 0 THEN NOW() ELSE last_data_received END,
        latest_reading_time = COALESCE(p_latest_reading_time, latest_reading_time),
        latest_reading_value = COALESCE(p_latest_reading_value, latest_reading_value),
        consecutive_failures = CASE 
            WHEN p_poll_succeeded AND p_readings_count > 0 THEN 0
            WHEN p_poll_succeeded AND p_readings_count = 0 THEN consecutive_failures + 1
            ELSE consecutive_failures + 1
        END,
        status = CASE
            WHEN NOT p_poll_succeeded THEN 'offline'
            WHEN p_readings_count = 0 THEN 'offline'
            WHEN v_is_stale THEN 'degraded'
            ELSE 'active'
        END,
        status_since = CASE
            WHEN v_old_status IS DISTINCT FROM (
                CASE
                    WHEN NOT p_poll_succeeded THEN 'offline'
                    WHEN p_readings_count = 0 THEN 'offline'
                    WHEN v_is_stale THEN 'degraded'
                    ELSE 'active'
                END
            ) THEN NOW()
            ELSE status_since
        END,
        is_stale = v_is_stale,
        stale_since = CASE
            WHEN v_is_stale AND NOT COALESCE(is_stale, false) THEN NOW()
            WHEN NOT v_is_stale THEN NULL
            ELSE stale_since
        END,
        updated_at = NOW()
    WHERE site_code = p_site_code AND parameter_code = p_parameter_code;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION usgs_raw.update_monitoring_state IS 'Update monitoring state after each polling cycle';

-- View for dashboard/alerting
CREATE OR REPLACE VIEW usgs_raw.station_health AS
SELECT 
    ms.site_code,
    s.site_name,
    ms.parameter_code,
    ms.status,
    ms.status_since,
    ms.is_stale,
    ms.stale_since,
    ms.latest_reading_time,
    ms.latest_reading_value,
    EXTRACT(EPOCH FROM (NOW() - ms.latest_reading_time)) / 60 AS age_minutes,
    ms.staleness_threshold_minutes,
    ms.last_poll_attempted,
    ms.last_poll_succeeded,
    ms.consecutive_failures
FROM usgs_raw.monitoring_state ms
JOIN usgs_raw.sites s ON ms.site_code = s.site_code
WHERE s.active = true
ORDER BY 
    CASE ms.status
        WHEN 'offline' THEN 1
        WHEN 'degraded' THEN 2
        WHEN 'active' THEN 3
        ELSE 4
    END,
    s.site_code;

COMMENT ON VIEW usgs_raw.station_health IS 'Current health status of all monitored stations';
