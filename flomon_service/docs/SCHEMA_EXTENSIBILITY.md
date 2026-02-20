# Schema Extensibility and Future Data Sources

**Created:** February 19, 2026  
**Status:** Design documentation for multi-source flood prediction system

## Overview

The FloPro database schema is designed for incremental expansion. New data sources can be added through numbered migrations without disrupting existing functionality. The multi-schema architecture (`usgs_raw`, `nws`, `noaa`, `usace`) provides natural separation of concerns.

## Migration Timeline

### Current State (Applied)
- ✅ `001_initial_schema.sql` - USGS gauge readings and site metadata
- ✅ `002_monitoring_metadata.sql` - Staleness tracking and health monitoring

### Ready to Apply (Data Populated Later)
- 🔄 `003_flood_metadata.sql` - NWS flood thresholds and historical events
  - Can apply schema now, populate data after historical ingestion
  - Threshold table pre-initialized from stations.toml
  - Event detection runs after gauge reading backfill

### Future Expansions (Planned)
- 📋 `004_precipitation.sql` - NOAA rainfall observations and forecasts
- 📋 `005_soil_moisture.sql` - NRCS/SNOTEL soil saturation data
- 📋 `006_nws_forecasts.sql` - AHPS stage predictions and alerts
- 📋 `007_usace_operations.sql` - Lock/dam release schedules

## Extensibility Principles

### 1. Schema-per-Source
Each data provider gets its own schema namespace:
```sql
usgs_raw.*   -- USGS gauge readings
nws.*        -- National Weather Service alerts/forecasts
noaa.*       -- NOAA precipitation/weather
usace.*      -- US Army Corps of Engineers operations
soil.*       -- Soil moisture/saturation (NRCS, NOAA CPC)
```

### 2. Non-Destructive Migrations
Each migration:
- Uses `CREATE TABLE IF NOT EXISTS` for idempotency
- Adds new tables without modifying existing structures
- Includes rollback procedures in comments
- Documents data source and update frequency

### 3. Incremental Data Population
Apply schema before data is available:
- Empty tables establish structure
- Application code handles missing data gracefully
- Backfill historical data separately from schema creation
- Real-time ingestion begins when source is configured

## Future Data Source Extensions

### Soil Moisture / Ground Saturation

**Use Case:** Saturated ground amplifies runoff from rainfall, increasing flood risk.

**Data Sources:**
- USDA NRCS SNOTEL stations (snowpack + soil moisture)
- NOAA Climate Prediction Center (CPC) soil moisture grids
- NASA SMAP satellite observations

**Schema Design:**

```sql
-- Migration: sql/005_soil_moisture.sql

CREATE SCHEMA IF NOT EXISTS soil;

-- Point observations from SNOTEL/field stations
CREATE TABLE soil.moisture_observations (
    id BIGSERIAL PRIMARY KEY,
    
    -- Location
    station_id TEXT NOT NULL,
    station_name TEXT,
    location GEOGRAPHY(POINT),
    basin_id TEXT,                    -- 'upper_illinois', 'peoria', 'lower_illinois'
    
    -- Measurement
    depth_inches INT NOT NULL,        -- Soil depth (2, 4, 8, 20, 40 inches typical)
    moisture_percent NUMERIC(5, 2),   -- Volumetric water content (0-100%)
    observation_time TIMESTAMPTZ NOT NULL,
    
    -- Metadata
    source TEXT NOT NULL,             -- 'NRCS_SNOTEL', 'NOAA_CPC', 'NASA_SMAP'
    data_quality TEXT,                -- 'provisional', 'verified'
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT unique_moisture_obs UNIQUE (station_id, depth_inches, observation_time)
);

CREATE INDEX idx_moisture_obs_time ON soil.moisture_observations(observation_time DESC);
CREATE INDEX idx_moisture_obs_basin ON soil.moisture_observations(basin_id, observation_time DESC);

-- Aggregated saturation by basin area
CREATE TABLE soil.basin_saturation (
    id SERIAL PRIMARY KEY,
    basin_id TEXT NOT NULL,
    
    -- Saturation metrics
    saturation_index NUMERIC(5, 2) NOT NULL,  -- 0-100 scale
    saturation_depth_inches NUMERIC(6, 2),    -- Effective depth of saturation
    field_capacity_pct NUMERIC(5, 2),         -- % of field capacity
    
    -- Time
    valid_time TIMESTAMPTZ NOT NULL,
    forecast_hours INT,                       -- NULL for current, hours ahead for forecast
    
    -- Classification
    saturation_category TEXT,                 -- 'dry', 'normal', 'wet', 'saturated'
    contributing_to_flood BOOLEAN,            -- True when critical for flood risk
    
    -- Source
    source TEXT,
    last_updated TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT unique_basin_saturation UNIQUE (basin_id, valid_time, forecast_hours)
);

CREATE INDEX idx_basin_saturation_current 
    ON soil.basin_saturation(basin_id, valid_time DESC) 
    WHERE forecast_hours IS NULL;

COMMENT ON TABLE soil.moisture_observations IS 
    'Point measurements of soil moisture from NRCS SNOTEL and other field stations';
COMMENT ON TABLE soil.basin_saturation IS 
    'Aggregated soil saturation index by Illinois River basin sub-regions';
COMMENT ON COLUMN soil.basin_saturation.saturation_index IS 
    'Composite metric: 0=completely dry, 100=fully saturated. Critical flood risk typically >75';
```

### NWS Live Alerts and Warnings

**Use Case:** Official government flood warnings provide authoritative alerts to relay to users.

**Data Sources:**
- NWS EMWIN (Emergency Managers Weather Information Network)
- NWS CAP (Common Alerting Protocol) feeds
- NOAA Weather Wire Service

**Schema Design:**

```sql
-- Migration: sql/006_nws_forecasts.sql (extends NWS schema)

-- Active flood watches, warnings, and advisories
CREATE TABLE nws.flood_warnings (
    id SERIAL PRIMARY KEY,
    
    -- Location
    site_code VARCHAR(8) REFERENCES usgs_raw.sites(site_code),
    forecast_point_id TEXT,           -- NWS AHPS ID (e.g., 'KINI2' for Kingston Mines)
    affected_counties TEXT[],         -- ['Tazewell County', 'Peoria County']
    
    -- Alert classification
    alert_type TEXT NOT NULL,         -- 'WATCH', 'WARNING', 'ADVISORY', 'STATEMENT'
    severity TEXT NOT NULL,           -- 'MINOR', 'MODERATE', 'MAJOR'
    urgency TEXT,                     -- 'IMMEDIATE', 'EXPECTED', 'FUTURE'
    certainty TEXT,                   -- 'OBSERVED', 'LIKELY', 'POSSIBLE'
    
    -- Timing
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ,
    onset_time TIMESTAMPTZ,           -- When flooding is expected to begin
    
    -- Content
    headline TEXT NOT NULL,           -- "Flood Warning for Illinois River at Peoria"
    alert_text TEXT,                  -- Full NWS text
    instructions TEXT,                -- What to do
    
    -- Tracking
    status TEXT DEFAULT 'active',     -- 'active', 'extended', 'canceled', 'expired'
    canceled_at TIMESTAMPTZ,
    superseded_by INT REFERENCES nws.flood_warnings(id),
    
    -- Source
    source_url TEXT,                  -- Link to official NWS page
    cap_alert_id TEXT UNIQUE,         -- CAP identifier for deduplication
    
    -- Metadata
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT valid_alert_type CHECK (
        alert_type IN ('WATCH', 'WARNING', 'ADVISORY', 'STATEMENT', 'OUTLOOK')
    ),
    CONSTRAINT valid_severity CHECK (
        severity IN ('MINOR', 'MODERATE', 'MAJOR', 'EXTREME', 'UNKNOWN')
    )
);

CREATE INDEX idx_flood_warnings_active 
    ON nws.flood_warnings(site_code, issued_at DESC) 
    WHERE status = 'active';

CREATE INDEX idx_flood_warnings_cap ON nws.flood_warnings(cap_alert_id);

-- NWS forecasted stage hydrographs
CREATE TABLE nws.stage_forecasts (
    id BIGSERIAL PRIMARY KEY,
    
    -- Location
    site_code VARCHAR(8) NOT NULL REFERENCES usgs_raw.sites(site_code),
    forecast_point_id TEXT,
    
    -- Forecast
    forecast_time TIMESTAMPTZ NOT NULL,       -- When this stage is predicted
    predicted_stage_ft NUMERIC(6, 2) NOT NULL,
    predicted_flow_cfs NUMERIC(10, 2),
    
    -- Uncertainty
    confidence_level TEXT,                    -- 'low', 'medium', 'high'
    prediction_interval_lower NUMERIC(6, 2),  -- 90% confidence bounds
    prediction_interval_upper NUMERIC(6, 2),
    
    -- Model metadata
    forecast_issued TIMESTAMPTZ NOT NULL,     -- When NWS published this forecast run
    model_run TEXT,                           -- e.g., 'Feb-19-2026-06Z'
    model_name TEXT,                          -- 'RFC_FLDWAV', 'RFC_CHPS'
    
    -- Verification (populated later)
    actual_stage_ft NUMERIC(6, 2),
    forecast_error_ft NUMERIC(6, 2),
    
    -- Source
    source TEXT DEFAULT 'NWS AHPS',
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT unique_stage_forecast UNIQUE (site_code, forecast_time, forecast_issued)
);

CREATE INDEX idx_stage_forecasts_upcoming 
    ON nws.stage_forecasts(site_code, forecast_time) 
    WHERE forecast_time > NOW() AND forecast_time < NOW() + INTERVAL '7 days';

CREATE INDEX idx_stage_forecasts_verification 
    ON nws.stage_forecasts(site_code, forecast_time) 
    WHERE actual_stage_ft IS NOT NULL;

COMMENT ON TABLE nws.flood_warnings IS 
    'Official NWS flood watches, warnings, and advisories from CAP/EMWIN feeds';
COMMENT ON TABLE nws.stage_forecasts IS 
    'NWS AHPS predicted future river stages (hydrograph forecasts)';
COMMENT ON COLUMN nws.stage_forecasts.forecast_error_ft IS 
    'Computed as (actual_stage_ft - predicted_stage_ft) after event occurs. Used for model accuracy assessment';
```

### NOAA Precipitation Data

**Use Case:** Heavy rainfall is the primary driver of river flooding.

**Data Sources:**
- NOAA Multi-Radar Multi-Sensor (MRMS) - observed rainfall
- NOAA National Digital Forecast Database (NDFD) - predicted rainfall
- NOAA Weather Prediction Center (WPC) - QPF (Quantitative Precipitation Forecast)

**Schema Design:**

```sql
-- Migration: sql/004_precipitation.sql

-- Observed precipitation (radar-based)
CREATE TABLE noaa.observed_precipitation (
    id BIGSERIAL PRIMARY KEY,
    
    -- Location
    basin_area TEXT NOT NULL,         -- 'upper_illinois', 'peoria', 'mackinaw', 'spoon'
    location GEOGRAPHY(POINT),        -- Centroid or representative point
    
    -- Precipitation totals
    rainfall_1hr_inches NUMERIC(5, 2),
    rainfall_3hr_inches NUMERIC(5, 2),
    rainfall_6hr_inches NUMERIC(5, 2),
    rainfall_24hr_inches NUMERIC(5, 2),
    rainfall_48hr_inches NUMERIC(5, 2),
    rainfall_72hr_inches NUMERIC(5, 2),
    
    -- Timing
    observation_time TIMESTAMPTZ NOT NULL,
    accumulation_period INTERVAL,     -- Duration of accumulation
    
    -- Source
    source TEXT NOT NULL DEFAULT 'NOAA_MRMS',  -- 'NOAA_MRMS', 'CoCoRaHS', 'COOP'
    data_quality TEXT,                         -- 'real_time', 'quality_controlled'
    
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT unique_observed_precip UNIQUE (basin_area, observation_time, source)
);

CREATE INDEX idx_observed_precip_time ON noaa.observed_precipitation(observation_time DESC);
CREATE INDEX idx_observed_precip_basin ON noaa.observed_precipitation(basin_area, observation_time DESC);

-- Forecasted precipitation
CREATE TABLE noaa.precipitation_forecasts (
    id BIGSERIAL PRIMARY KEY,
    
    -- Location
    basin_area TEXT NOT NULL,
    location GEOGRAPHY(POINT),
    
    -- Forecast period
    forecast_period_start TIMESTAMPTZ NOT NULL,
    forecast_period_end TIMESTAMPTZ NOT NULL,
    
    -- Predicted rainfall
    predicted_rainfall_inches NUMERIC(5, 2) NOT NULL,
    probability_of_precip INT,        -- 0-100% (PoP)
    confidence TEXT,                  -- 'low', 'medium', 'high'
    
    -- Forecasted max rate
    max_hourly_rate_inches NUMERIC(4, 2),
    
    -- Model metadata
    forecast_issued TIMESTAMPTZ NOT NULL,
    forecast_model TEXT,              -- 'WPC_QPF', 'NDFD', 'NAM', 'GFS'
    forecast_hours_ahead INT,
    
    -- Verification
    actual_rainfall_inches NUMERIC(5, 2),
    forecast_error_inches NUMERIC(5, 2),
    
    -- Source
    source TEXT DEFAULT 'NOAA_WPC',
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT unique_precip_forecast UNIQUE (basin_area, forecast_period_start, forecast_issued)
);

CREATE INDEX idx_precip_forecasts_upcoming 
    ON noaa.precipitation_forecasts(basin_area, forecast_period_start) 
    WHERE forecast_period_start > NOW();

COMMENT ON TABLE noaa.observed_precipitation IS 
    'Radar-estimated and gauge-measured rainfall accumulations by basin area';
COMMENT ON TABLE noaa.precipitation_forecasts IS 
    'NOAA QPF (Quantitative Precipitation Forecast) predictions';
```

### USACE Lock and Dam Operations

**Use Case:** Dam releases can spike downstream flows; advance warning enables better flood prediction.

**Data Sources:**
- USACE Lock Performance Monitoring System (LPMS)
- USACE Water Control Data System
- Public lock/dam status pages

**Schema Design:**

```sql
-- Migration: sql/007_usace_operations.sql

-- Historical and current lock/dam operations
CREATE TABLE usace.lock_operations (
    id BIGSERIAL PRIMARY KEY,
    
    -- Facility
    facility_code TEXT NOT NULL,      -- 'PEO' (Peoria), 'SRO' (Starved Rock)
    facility_name TEXT NOT NULL,
    river_mile NUMERIC(6, 2),         -- Location on Illinois River
    
    -- Operation
    operation_time TIMESTAMPTZ NOT NULL,
    pool_elevation_ft NUMERIC(6, 2),
    pool_target_ft NUMERIC(6, 2),
    release_rate_cfs NUMERIC(10, 2),
    inflow_rate_cfs NUMERIC(10, 2),
    
    -- Status
    operational_mode TEXT,            -- 'normal', 'flood_control', 'low_water', 'maintenance'
    gates_open INT,
    gates_total INT,
    
    -- Metadata
    notes TEXT,
    data_quality TEXT,
    source TEXT DEFAULT 'USACE_LPMS',
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT valid_operational_mode CHECK (
        operational_mode IN ('normal', 'flood_control', 'low_water', 'maintenance', 'emergency')
    )
);

CREATE INDEX idx_lock_ops_facility ON usace.lock_operations(facility_code, operation_time DESC);
CREATE INDEX idx_lock_ops_flood_control 
    ON usace.lock_operations(facility_code, operation_time DESC) 
    WHERE operational_mode = 'flood_control';

-- Scheduled releases (for predictive purposes)
CREATE TABLE usace.scheduled_releases (
    id SERIAL PRIMARY KEY,
    
    -- Facility
    facility_code TEXT NOT NULL,
    facility_name TEXT NOT NULL,
    
    -- Schedule
    scheduled_time TIMESTAMPTZ NOT NULL,
    expected_rate_cfs NUMERIC(10, 2) NOT NULL,
    duration_hours INT,
    ramp_up_hours INT,                -- Time to reach full rate
    ramp_down_hours INT,
    
    -- Purpose
    reason TEXT,                      -- 'flood_control', 'maintenance', 'navigation', 'emergency'
    impact_downstream BOOLEAN,
    estimated_arrival_time TIMESTAMPTZ,  -- When pulse reaches downstream gauge
    
    -- Status
    status TEXT DEFAULT 'scheduled',  -- 'scheduled', 'in_progress', 'completed', 'canceled'
    actual_start TIMESTAMPTZ,
    actual_end TIMESTAMPTZ,
    
    -- Metadata
    published_at TIMESTAMPTZ DEFAULT NOW(),
    notes TEXT,
    
    CONSTRAINT valid_release_reason CHECK (
        reason IN ('flood_control', 'maintenance', 'navigation', 'emergency', 'other')
    )
);

CREATE INDEX idx_scheduled_releases_upcoming 
    ON usace.scheduled_releases(facility_code, scheduled_time) 
    WHERE status = 'scheduled' AND scheduled_time > NOW();

COMMENT ON TABLE usace.lock_operations IS 
    'Historical lock and dam operations including pool levels and releases';
COMMENT ON TABLE usace.scheduled_releases IS 
    'Planned dam releases that may impact downstream flood risk';
```

## Multi-Source Integration Examples

### Comprehensive Flood Risk Assessment Query

```sql
-- Real-time flood risk dashboard for Kingston Mines
WITH current_conditions AS (
    SELECT 
        value as current_stage_ft,
        reading_time
    FROM usgs_raw.gauge_readings 
    WHERE site_code = '05568500' 
      AND parameter_code = '00065'  -- stage
    ORDER BY reading_time DESC 
    LIMIT 1
),
forecast_peak AS (
    SELECT 
        MAX(predicted_stage_ft) as peak_24hr,
        MAX(predicted_stage_ft) FILTER (WHERE forecast_time <= NOW() + INTERVAL '48 hours') as peak_48hr
    FROM nws.stage_forecasts 
    WHERE site_code = '05568500' 
      AND forecast_time BETWEEN NOW() AND NOW() + INTERVAL '7 days'
),
recent_rainfall AS (
    SELECT 
        rainfall_24hr_inches,
        rainfall_48hr_inches
    FROM noaa.observed_precipitation 
    WHERE basin_area = 'peoria'
    ORDER BY observation_time DESC 
    LIMIT 1
),
soil_status AS (
    SELECT 
        saturation_index,
        contributing_to_flood
    FROM soil.basin_saturation 
    WHERE basin_id = 'peoria'
      AND forecast_hours IS NULL
    ORDER BY valid_time DESC 
    LIMIT 1
),
active_alerts AS (
    SELECT 
        COUNT(*) as warning_count,
        MAX(severity) as max_severity
    FROM nws.flood_warnings 
    WHERE site_code = '05568500' 
      AND status = 'active'
),
upstream_releases AS (
    SELECT 
        SUM(expected_rate_cfs) as total_scheduled_cfs
    FROM usace.scheduled_releases 
    WHERE facility_code IN ('SRO', 'MSL')  -- Upstream of Peoria
      AND scheduled_time BETWEEN NOW() AND NOW() + INTERVAL '24 hours'
      AND status = 'scheduled'
),
thresholds AS (
    SELECT 
        action_stage_ft,
        flood_stage_ft,
        moderate_flood_stage_ft,
        major_flood_stage_ft
    FROM nws.flood_thresholds 
    WHERE site_code = '05568500'
)
SELECT 
    -- Current state
    cc.current_stage_ft,
    cc.reading_time as last_reading,
    (NOW() - cc.reading_time) as data_age,
    
    -- Forecasts
    fp.peak_24hr as forecast_peak_24hr,
    fp.peak_48hr as forecast_peak_48hr,
    
    -- Contributing factors
    rr.rainfall_24hr_inches,
    rr.rainfall_48hr_inches,
    ss.saturation_index as soil_saturation_pct,
    ss.contributing_to_flood as soil_critical,
    ur.total_scheduled_cfs as upstream_releases,
    
    -- Alerts
    aa.warning_count,
    aa.max_severity,
    
    -- Thresholds
    t.action_stage_ft,
    t.flood_stage_ft,
    t.moderate_flood_stage_ft,
    t.major_flood_stage_ft,
    
    -- Computed risk indicators
    CASE 
        WHEN cc.current_stage_ft >= t.major_flood_stage_ft THEN 'MAJOR_FLOODING'
        WHEN cc.current_stage_ft >= t.moderate_flood_stage_ft THEN 'MODERATE_FLOODING'
        WHEN cc.current_stage_ft >= t.flood_stage_ft THEN 'MINOR_FLOODING'
        WHEN cc.current_stage_ft >= t.action_stage_ft THEN 'ACTION_STAGE'
        WHEN fp.peak_24hr >= t.flood_stage_ft THEN 'FORECAST_FLOODING'
        WHEN ss.saturation_index > 75 AND rr.rainfall_24hr_inches > 1.0 THEN 'HIGH_RISK'
        ELSE 'NORMAL'
    END as flood_status,
    
    -- Days until forecast peak
    ROUND(EXTRACT(EPOCH FROM (
        (SELECT forecast_time FROM nws.stage_forecasts 
         WHERE site_code = '05568500' AND predicted_stage_ft = fp.peak_24hr 
         LIMIT 1) - NOW()
    )) / 3600) as hours_to_peak
    
FROM current_conditions cc
CROSS JOIN forecast_peak fp
CROSS JOIN recent_rainfall rr
CROSS JOIN soil_status ss
CROSS JOIN active_alerts aa
CROSS JOIN upstream_releases ur
CROSS JOIN thresholds t;
```

### Upstream Cascade Alert Query

```sql
-- Detect flood waves propagating downstream
SELECT 
    s.site_code,
    s.site_name,
    sc.travel_time_to_peoria_hours,
    g.value as current_stage_ft,
    t.flood_stage_ft,
    (g.value - t.flood_stage_ft) as feet_above_flood,
    g.reading_time,
    (g.reading_time + (sc.travel_time_to_peoria_hours || ' hours')::INTERVAL) as estimated_arrival_peoria
FROM usgs_raw.gauge_readings g
JOIN usgs_raw.sites s USING (site_code)
JOIN station_config sc ON s.site_code = sc.site_code  -- From application cache
LEFT JOIN nws.flood_thresholds t USING (site_code)
WHERE g.parameter_code = '00065'  -- stage
  AND g.reading_time > NOW() - INTERVAL '4 hours'
  AND sc.distance_direction = 'upstream'
  AND g.value > t.flood_stage_ft  -- Currently flooding upstream
ORDER BY sc.travel_time_to_peoria_hours;
```

## Migration Checklist

When adding a new data source:

- [ ] Create numbered migration file (`sql/00X_description.sql`)
- [ ] Document data source, update frequency, and API endpoints in header
- [ ] Use appropriate schema namespace (existing or new)
- [ ] Add indexes for common query patterns
- [ ] Include COMMENT ON statements for tables and columns
- [ ] Write example queries showing integration with existing data
- [ ] Update this documentation with new tables
- [ ] Create corresponding Rust structs in `src/model.rs`
- [ ] Implement ingest module in `src/ingest/`
- [ ] Add tests for new data types
- [ ] Update monitoring to track ingestion health

## Data Source Priority

**Immediate (Next 6 months):**
1. NWS flood forecasts (AHPS hydrographs) - most impactful for predictions
2. NOAA observed precipitation (MRMS) - explains current conditions
3. NWS flood warnings (CAP feeds) - authoritative alerts

**Medium-term (6-12 months):**
4. USACE lock operations - important for managed sections of river
5. NOAA precipitation forecasts - improves lead time

**Long-term (12+ months):**
6. Soil moisture data - advanced modeling
7. Snowpack data - spring flood forecasting (less critical for Illinois)

## Backward Compatibility

All new migrations must:
- Not modify existing tables
- Not change existing column types
- Not add new NOT NULL columns without defaults to existing tables
- Maintain existing query patterns
- Provide fallback behavior when new data is unavailable

This ensures the application continues working even if new data sources haven't been configured yet.

---

**Related Documentation:**
- [PRE_INGESTION_STRATEGY.md](./PRE_INGESTION_STRATEGY.md) - Strategy for flood metadata tables
- [DATA_STORAGE_STRATEGY.md](./DATA_STORAGE_STRATEGY.md) - Valid readings vs. absence tracking
- [sql/003_flood_metadata.sql](../sql/003_flood_metadata.sql) - Current flood threshold schema
