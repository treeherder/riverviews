# Flood Event Analysis System

## Overview

The flood event analysis system creates comprehensive, relational flood event records by:

1. **Analyzing precursor patterns** - Identifies when significant river rise began before each historical flood
2. **Correlating multi-source data** - Links USGS gauge data, USACE CWMS data, and future sources to each event
3. **Computing metrics** - Rise rate, duration, peak statistics, backwater contribution
4. **Detecting precursors** - Rapid rise events, backwater onset, sustained increases

This transforms isolated flood event records into rich, queryable flood event windows with all contextual data.

## Database Schema (`flood_analysis`)

### Core Tables

**`events`** - Enhanced flood events with analysis metadata
- Links to original `nws.flood_events` records
- Precursor window (when significant rise began)
- Rise metrics (rate, duration, total rise)
- Data availability flags (backwater, discharge, dam operations)

**`event_observations`** - USGS gauge readings during event
- Links each observation to an event
- Phase classification (precursor, rising, peak, falling, post)
- Time series metrics (hours before peak, 24h change)

**`event_cwms_data`** - USACE CWMS data during event
- Mississippi River stages for backwater analysis
- Lock/dam operations
- Time-aligned with event window

**`event_precursors`** - Detected leading indicators
- Rapid rise events (>X ft/day)
- Backwater onset detection
- Sustained rise patterns
- Severity scoring and confidence levels

**`event_metrics`** - Computed aggregate metrics
- Rise phase: duration, rate, total rise
- Peak phase: stage, discharge, hours above flood stage
- Fall phase: duration, recession rate
- Comparative: percentile rank, exceedance above threshold
- Contributing factors: backwater estimate, upstream influence

**`analysis_config`** - Analysis parameters
- Precursor lookback window (default: 14 days)
- Rise thresholds and rates
- Data quality requirements

## Analysis Process

### 1. Precursor Window Detection

For each flood event, the analysis looks backward from the peak to find when significant rise began:

```
Day -14: Stage 15.0 ft  ← Precursor lookback starts
Day -10: Stage 15.2 ft
Day -7:  Stage 15.8 ft  
Day -5:  Stage 17.2 ft  ← Significant rise detected (>2.0 ft total)
Day -3:  Stage 19.1 ft
Day  0:  Stage 21.5 ft (PEAK)
```

**Precursor window:** Day -5 to Day 0 (when rise exceeded 2.0 ft threshold)

**Metrics computed:**
- Total rise: 21.5 - 17.2 = 4.3 ft
- Duration: 5 days = 120 hours
- Average rise rate: 4.3 ft / 5 days = 0.86 ft/day
- Max rise rate: Maximum single-day increase

### 2. Multi-Source Data Correlation

For the detected event window (precursor start through post-peak), the analysis collects:

**USGS Data:**
- Stage (00065) observations every 15 minutes
- Discharge (00060) if available
- Classified by phase (precursor/rising/peak/falling/post)

**USACE CWMS Data:**
- Mississippi River stage at Grafton, Alton, LD24, LD25
- Illinois River pool levels at LaGrange, Peoria, Starved Rock
- Backwater differential (Mississippi - Illinois)

**Future Sources:**
- Weather data (precipitation, forecasts)
- Dam operation changes
- Upstream tributary flows

### 3. Metric Computation

**Rise Metrics:**
- Total rise from precursor start to peak
- Rise duration in hours
- Average and maximum rise rates (ft/day)

**Peak Metrics:**
- Peak stage and timestamp
- Peak discharge (if available)
- Hours above flood stage
- Exceedance above flood thresholds (minor/moderate/major)

**Backwater Analysis:**
- Mississippi-Illinois stage differential during event
- Estimated backwater contribution to Illinois River stage
- Backwater onset timing

**Comparative Metrics:**
- Percentile rank among all floods for this site
- Severity relative to historical record

### 4. Precursor Detection

The system identifies specific precursor conditions:

**Rapid Rise** - Rise rate exceeding threshold for sustained period
```json
{
  "precursor_type": "rapid_rise",
  "detected_at": "2019-05-08 06:00:00Z",
  "description": "Rapid rise of 2.3 ft/day detected",
  "severity_score": 7.5,
  "hours_before_peak": 48,
  "metrics": {
    "rise_rate_ft_per_day": 2.3,
    "duration_hours": 36
  }
}
```

**Backwater Onset** - Mississippi stage rises above Illinois + threshold
```json
{
  "precursor_type": "backwater_onset",
  "detected_at": "2019-05-09 12:00:00Z",
  "description": "Backwater conditions detected: Miss 38.2 ft, IL 20.1 ft",
  "severity_score": 9.0,
  "hours_before_peak": 24,
  "metrics": {
    "mississippi_stage_ft": 38.2,
    "illinois_stage_ft": 20.1,
    "differential_ft": 18.1
  }
}
```

**Sustained Rise** - Continuous increase over multiple days
```json
{
  "precursor_type": "sustained_rise",
  "detected_at": "2019-05-05 00:00:00Z",
  "description": "Sustained rise over 5 days",
  "severity_score": 6.0,
  "hours_before_peak": 120,
  "metrics": {
    "total_rise_ft": 4.3,
    "days": 5,
    "avg_rate_ft_per_day": 0.86
  }
}
```

## Usage

### Running the Analysis

**Analyze all unanalyzed events:**
```bash
cargo run --bin analyze_flood_events
```

**Analyze specific site:**
```bash
cargo run --bin analyze_flood_events -- --site-code 05567500
```

**Re-analyze all events (clears and rebuilds):**
```bash
cargo run --bin analyze_flood_events -- --reanalyze
```

### Example Output

```
🌊 Flood Event Analysis
=======================

📊 Connecting to database...
✓ Connected

⚙️  Loading analysis configuration...
✓ Configuration loaded:
  - Precursor lookback: 14 days
  - Rise threshold: 2.00 ft
  - Rise rate threshold: 0.50 ft/day
  - Post-peak window: 7 days

📋 Loading historical flood events...
✓ Found 118 events to analyze

🔍 Analyzing flood events...

  Analyzing 05567500 - 2019-05-10
    ✓ Inserted event 1 with 672 observations
  Analyzing 05567500 - 2013-04-20
    ✓ Inserted event 2 with 448 observations
  [...]

🔗 Correlating USACE CWMS data...
✓ Linked 2,841 CWMS observations to events

📈 Computing event metrics...
✓ Computed metrics for 118 events

==================================================
Summary:
  Successfully analyzed: 118
  Errors: 0
==================================================
```

## Querying the Data

### All Analyzed Events

```sql
SELECT * FROM flood_analysis.event_summary
ORDER BY peak_stage_ft DESC
LIMIT 10;
```

### Events with Backwater Influence

```sql
SELECT 
    site_code,
    event_peak,
    severity,
    peak_stage_ft,
    avg_stage_differential_ft,
    backwater_observation_count
FROM flood_analysis.backwater_influenced_events
ORDER BY avg_stage_differential_ft DESC;
```

### Observations for Specific Event

```sql
-- Get all observations for May 2019 flood at Peoria
SELECT 
    timestamp,
    phase,
    stage_ft,
    discharge_cfs,
    hours_before_peak
FROM flood_analysis.event_observations
WHERE event_id = 1
ORDER BY timestamp;
```

### Precursor Conditions

```sql
-- Find all rapid rise precursors
SELECT 
    e.site_code,
    e.event_peak,
    p.detected_at,
    p.description,
    p.severity_score,
    p.hours_before_peak,
    p.metrics
FROM flood_analysis.event_precursors p
JOIN flood_analysis.events e ON p.event_id = e.event_id
WHERE p.precursor_type = 'rapid_rise'
ORDER BY p.severity_score DESC;
```

### CWMS Data During Event

```sql
-- Mississippi and Illinois stages during an event
SELECT 
    timestamp,
    location_name,
    river_name,
    value as stage_ft,
    hours_before_peak
FROM flood_analysis.event_cwms_data
WHERE event_id = 1
  AND parameter_type = 'stage'
  AND river_name IN ('Mississippi River', 'Illinois River')
ORDER BY timestamp, river_name;
```

### Rise Rate Analysis

```sql
-- Compare rise rates across all events
SELECT 
    site_code,
    event_peak,
    severity,
    total_rise_ft,
    rise_duration_hours,
    average_rise_rate_ft_per_day,
    max_rise_rate_ft_per_day
FROM flood_analysis.events
WHERE total_rise_ft IS NOT NULL
ORDER BY average_rise_rate_ft_per_day DESC
LIMIT 20;
```

### Event Metrics with Backwater

```sql
-- Events with backwater contribution estimates
SELECT 
    e.site_code,
    e.event_peak,
    e.severity,
    e.peak_stage_ft,
    m.backwater_contribution_ft,
    m.exceedance_above_flood_stage_ft,
    e.has_backwater_data
FROM flood_analysis.events e
JOIN flood_analysis.event_metrics m ON e.event_id = m.event_id
WHERE m.backwater_contribution_ft IS NOT NULL
ORDER BY m.backwater_contribution_ft DESC;
```

## Configuration

Modify analysis parameters:

```sql
-- Update default configuration
UPDATE flood_analysis.analysis_config
SET 
    precursor_lookback_days = 21,  -- Look back 3 weeks
    significant_rise_threshold_ft = 3.0,  -- Higher threshold
    rise_rate_threshold_ft_per_day = 1.0  -- Only flag rapid rises
WHERE config_name = 'default';

-- Create custom configuration
INSERT INTO flood_analysis.analysis_config 
    (config_name, precursor_lookback_days, significant_rise_threshold_ft, description)
VALUES 
    ('sensitive', 30, 1.0, 'Longer lookback with lower threshold for early detection');
```

## Integration with Forecasting

The flood event analysis provides critical data for flood forecasting:

### Pattern Recognition

```sql
-- Find events with similar precursor patterns to current conditions
WITH current_conditions AS (
    SELECT 
        site_code,
        -- Current rise rate calculation
        2.1 as current_rise_rate_ft_per_day
    FROM ...
)
SELECT 
    e.event_id,
    e.event_peak,
    e.severity,
    e.peak_stage_ft,
    e.average_rise_rate_ft_per_day,
    ABS(e.average_rise_rate_ft_per_day - cc.current_rise_rate_ft_per_day) as rate_difference
FROM flood_analysis.events e
CROSS JOIN current_conditions cc
WHERE e.site_code = cc.site_code
  AND e.average_rise_rate_ft_per_day IS NOT NULL
ORDER BY rate_difference
LIMIT 10;
```

### Backwater Risk Assessment

```sql
-- Current backwater conditions vs historical events
SELECT 
    AVG(peak_stage_ft) as avg_peak_when_backwater,
    MIN(peak_stage_ft) as min_peak_when_backwater,
    MAX(peak_stage_ft) as max_peak_when_backwater,
    AVG(hours_above_flood_stage) as avg_duration_hours
FROM flood_analysis.events e
JOIN flood_analysis.event_metrics m ON e.event_id = m.event_id
WHERE e.has_backwater_data = true
  AND e.site_code = '05567500';
```

### Lead Time Analysis

```sql
-- How much lead time did we have before flood stage?
SELECT 
    e.site_code,
    e.event_peak,
    e.flood_stage_ft,
    e.precursor_window_start,
    EXTRACT(EPOCH FROM (e.event_peak - e.precursor_window_start)) / 3600.0 as lead_time_hours,
    e.average_rise_rate_ft_per_day
FROM flood_analysis.events e
WHERE e.severity IN ('moderate', 'major')
ORDER BY lead_time_hours;
```

## Extending the System

### Adding New Data Sources

1. **Create correlation function** in `src/bin/analyze_flood_events.rs`:
```rust
fn correlate_weather_data(client: &mut Client) -> Result<i32, Box<dyn std::error::Error>> {
    // Link weather observations to event windows
}
```

2. **Add table for new source**:
```sql
CREATE TABLE flood_analysis.event_weather_data (
    weather_id SERIAL PRIMARY KEY,
    event_id INTEGER REFERENCES flood_analysis.events(event_id),
    timestamp TIMESTAMPTZ,
    precipitation_in NUMERIC(10,2),
    -- ...
);
```

3. **Update event flags**:
```sql
ALTER TABLE flood_analysis.events 
ADD COLUMN has_weather_data BOOLEAN DEFAULT FALSE;
```

### Adding New Precursor Types

Implement detection logic in `src/analysis/flood_events.rs`:

```rust
// Detect upstream tributary surge
if upstream_discharge > threshold {
    precursors.push(PrecursorCondition {
        precursor_type: "upstream_surge".to_string(),
        detected_at: timestamp,
        description: format!("Upstream tributary discharge: {} cfs", upstream_discharge),
        severity_score: calculate_severity(upstream_discharge),
        confidence: 0.80,
        hours_before_peak: hours_before,
        metrics: serde_json::json!({
            "discharge_cfs": upstream_discharge,
            "normal_discharge_cfs": normal_discharge
        }),
    });
}
```

## Benefits

1. **Comprehensive Event Records** - All data sources linked to each flood
2. **Pattern Recognition** - Identify common precursor patterns across events
3. **Forecast Improvement** - Use historical precursors to predict future floods
4. **Risk Assessment** - Quantify backwater contribution and other factors
5. **Data Quality** - Identify gaps in observational coverage
6. **Research** - Enable analysis of flood characteristics and trends

## Future Enhancements

- [ ] Automated precursor pattern classification (ML)
- [ ] Real-time event tracking (match current conditions to historical patterns)
- [ ] Weather data integration (precipitation, soil moisture)
- [ ] Upstream tributary correlation
- [ ] Dam operation change detection
- [ ] Event similarity scoring
- [ ] Forecast accuracy tracking (compare predicted vs actual using precursors)

## See Also

- [SCHEMA_EXTENSIBILITY.md](SCHEMA_EXTENSIBILITY.md) - Database schema design
- [CWMS_INTEGRATION.md](CWMS_INTEGRATION.md) - USACE CWMS data source
- Migration: [sql/005_flood_analysis.sql](../sql/005_flood_analysis.sql)
- Analysis module: [src/analysis/flood_events.rs](../src/analysis/flood_events.rs)
- Binary: [src/bin/analyze_flood_events.rs](../src/bin/analyze_flood_events.rs)
