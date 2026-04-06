-- Migration 007: Backfill Tracking Infrastructure
--
-- Purpose: Track data collection health and gaps that need backfilling
--
-- This migration adds:
-- 1. station_health table - tracks last successful poll per station/source
-- 2. backfill_queue table - tracks gaps that need to be filled
-- 3. backfill_history table - audit log of backfill operations
--
-- Usage:
--   psql -U flopro_admin -d flopro_db -f sql/007_backfill_tracking.sql

-- ============================================================================
-- Station Health Tracking
-- ============================================================================

CREATE TABLE IF NOT EXISTS public.station_health (
    source_type TEXT NOT NULL,              -- 'USGS', 'CWMS', 'ASOS'
    station_id TEXT NOT NULL,               -- site_code, location_id, or station_id
    last_successful_poll TIMESTAMPTZ,       -- Last time we got data from source API
    last_successful_warehouse TIMESTAMPTZ,  -- Last time we stored new data in DB
    last_reading_timestamp TIMESTAMPTZ,     -- Timestamp of the most recent reading in DB
    consecutive_failures INTEGER DEFAULT 0,  -- Count of failed poll attempts since last success
    last_error TEXT,                        -- Most recent error message
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    PRIMARY KEY (source_type, station_id)
);

CREATE INDEX idx_station_health_failures 
    ON public.station_health(consecutive_failures) 
    WHERE consecutive_failures > 0;

CREATE INDEX idx_station_health_stale 
    ON public.station_health(last_reading_timestamp)
    WHERE last_reading_timestamp < NOW() - INTERVAL '2 hours';

COMMENT ON TABLE public.station_health IS 
'Tracks collection health for each monitored station. Updated by daemon after each poll attempt.';

COMMENT ON COLUMN public.station_health.last_successful_poll IS 
'When we last successfully contacted the source API (even if response was empty)';

COMMENT ON COLUMN public.station_health.last_successful_warehouse IS 
'When we last inserted new data into the database (indicates data freshness)';

COMMENT ON COLUMN public.station_health.last_reading_timestamp IS 
'Timestamp of the most recent reading we have in storage for this station';

-- ============================================================================
-- Backfill Queue
-- ============================================================================

CREATE TABLE IF NOT EXISTS public.backfill_queue (
    id SERIAL PRIMARY KEY,
    source_type TEXT NOT NULL,              -- 'USGS', 'CWMS', 'ASOS'
    station_id TEXT NOT NULL,               -- site_code, location_id, or station_id
    gap_start TIMESTAMPTZ NOT NULL,         -- Beginning of gap to fill
    gap_end TIMESTAMPTZ NOT NULL,           -- End of gap to fill
    priority INTEGER DEFAULT 50,            -- Higher = more urgent (0-100)
    status TEXT DEFAULT 'pending',          -- 'pending', 'in_progress', 'completed', 'failed'
    attempts INTEGER DEFAULT 0,             -- Number of backfill attempts
    last_attempt_at TIMESTAMPTZ,           
    last_error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    
    CHECK (gap_end > gap_start),
    CHECK (priority BETWEEN 0 AND 100),
    CHECK (status IN ('pending', 'in_progress', 'completed', 'failed'))
);

CREATE INDEX idx_backfill_queue_status 
    ON public.backfill_queue(status, priority DESC)
    WHERE status IN ('pending', 'failed');

CREATE INDEX idx_backfill_queue_station 
    ON public.backfill_queue(source_type, station_id);

COMMENT ON TABLE public.backfill_queue IS 
'Queue of data gaps that need to be filled. Daemon backfill worker processes highest priority pending items.';

COMMENT ON COLUMN public.backfill_queue.priority IS 
'Priority 0-100. Use 90+ for primary property sensors, 50 for upstream, 10 for extended lead time stations.';

-- ============================================================================
-- Backfill History
-- ============================================================================

CREATE TABLE IF NOT EXISTS public.backfill_history (
    id SERIAL PRIMARY KEY,
    queue_id INTEGER REFERENCES public.backfill_queue(id),
    source_type TEXT NOT NULL,
    station_id TEXT NOT NULL,
    gap_start TIMESTAMPTZ NOT NULL,
    gap_end TIMESTAMPTZ NOT NULL,
    readings_inserted INTEGER DEFAULT 0,
    duration_seconds NUMERIC(10, 2),
    completed_at TIMESTAMPTZ DEFAULT NOW(),
    notes TEXT
);

CREATE INDEX idx_backfill_history_station 
    ON public.backfill_history(source_type, station_id, completed_at DESC);

COMMENT ON TABLE public.backfill_history IS 
'Audit log of completed backfill operations. Useful for debugging data quality issues.';

-- ============================================================================
-- Helper Functions
-- ============================================================================

-- Function to detect gaps in USGS data and queue backfill
CREATE OR REPLACE FUNCTION detect_usgs_gaps(
    p_site_code TEXT,
    p_lookback_days INTEGER DEFAULT 7
) RETURNS INTEGER AS $$
DECLARE
    v_gap_count INTEGER := 0;
    v_expected_interval INTERVAL := '15 minutes';
    v_gap_start TIMESTAMPTZ;
    v_gap_end TIMESTAMPTZ;
    v_priority INTEGER;
BEGIN
    -- Find gaps > 1 hour in the last N days
    FOR v_gap_start, v_gap_end IN
        SELECT 
            reading_time AS gap_start,
            LEAD(reading_time) OVER (ORDER BY reading_time) AS gap_end
        FROM usgs_raw.gauge_readings
        WHERE site_code = p_site_code
          AND reading_time >= NOW() - (p_lookback_days || ' days')::INTERVAL
          AND parameter_code = '00065'  -- Stage readings
        HAVING LEAD(reading_time) OVER (ORDER BY reading_time) - reading_time > INTERVAL '1 hour'
    LOOP
        -- Determine priority based on recency
        IF v_gap_end > NOW() - INTERVAL '24 hours' THEN
            v_priority := 90;  -- Recent gaps = high priority
        ELSIF v_gap_end > NOW() - INTERVAL '72 hours' THEN
            v_priority := 50;
        ELSE
            v_priority := 20;
        END IF;
        
        -- Queue backfill if not already queued
        INSERT INTO public.backfill_queue 
            (source_type, station_id, gap_start, gap_end, priority)
        VALUES 
            ('USGS', p_site_code, v_gap_start, v_gap_end, v_priority)
        ON CONFLICT DO NOTHING;
        
        v_gap_count := v_gap_count + 1;
    END LOOP;
    
    RETURN v_gap_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION detect_usgs_gaps IS 
'Scan USGS readings for a given site and queue backfill jobs for gaps > 1 hour. Returns number of gaps found.';

-- Grant permissions
GRANT SELECT, INSERT, UPDATE ON public.station_health TO flopro_admin;
GRANT SELECT, INSERT, UPDATE ON public.backfill_queue TO flopro_admin;
GRANT SELECT, INSERT ON public.backfill_history TO flopro_admin;
GRANT USAGE ON SEQUENCE public.backfill_queue_id_seq TO flopro_admin;
GRANT USAGE ON SEQUENCE public.backfill_history_id_seq TO flopro_admin;

-- Initialize station_health with existing monitored stations
INSERT INTO public.station_health (source_type, station_id, last_reading_timestamp)
SELECT 'USGS', site_code, MAX(reading_time)
FROM usgs_raw.gauge_readings
GROUP BY site_code
ON CONFLICT (source_type, station_id) DO NOTHING;

INSERT INTO public.station_health (source_type, station_id, last_reading_timestamp)
SELECT 'CWMS', location_id, MAX(timestamp)
FROM usace.cwms_timeseries
GROUP BY location_id
ON CONFLICT (source_type, station_id) DO NOTHING;

INSERT INTO public.station_health (source_type, station_id, last_reading_timestamp)
SELECT 'ASOS', station_id, MAX(observation_time)
FROM public.asos_observations
GROUP BY station_id
ON CONFLICT (source_type, station_id) DO NOTHING;

-- Initial gap detection for critical stations (run manually after migration)
-- SELECT detect_usgs_gaps('05568500', 7);  -- Kingston Mines
-- SELECT detect_usgs_gaps('05567500', 7);  -- Peoria
