# USACE CWMS Data Integration

**Created:** February 20, 2026  
**Status:** Schema deployed, ready for historical ingestion

## Overview

Integration with the US Army Corps of Engineers **Corps Water Management System (CWMS) Data API** to monitor:

1. **Mississippi River Backwater Flooding** - When high Mississippi levels back water up into the Illinois River
2. **Lock & Dam Operations** - Pool levels, gate positions, and releases on Illinois River navigation system
3. **Upstream Flow Conditions** - Mississippi River conditions that affect Illinois River flood risk

## Why CWMS Data Matters

### Bottom-Up Floods (Backwater)

The Illinois River flows into the Mississippi River at Grafton, IL (River Mile 218). When Mississippi River stages are exceptionally high (typically during spring floods), the Mississippi can:

- **Block normal Illinois River drainage** - Water can't flow out, backing up
- **Reverse the gradient** - Water actually flows UP the Illinois River
- **Amplify flood levels** - A minor Illinois River flood becomes major due to backwater

**Historical Example:** May 2019 - Mississippi River at 38+ ft blocked Illinois drainage, causing severe flooding upstream at Peoria despite moderate Illinois River flow.

### Lock & Dam Impact

The Illinois Waterway has 8 locks and dams that:
- Control pool levels between dams
- Can release large volumes during flood control operations
- Create flood pulses that propagate downstream
- Affect timing and magnitude of flood crests

## CWMS API

**Base URL:** `https://cwms-data.usace.army.mil/cwms-data/`  
**Documentation:** [Swagger UI](https://cwms-data.usace.army.mil/cwms-data/swagger-ui.html)  
**Data Availability:** Public since 2015

**Example Request:**
```
GET /timeseries?name=Grafton-Mississippi.Stage.Inst.15Minutes.0.Ccp-Rev&office=MVS&begin=2024-01-01T00:00:00&end=2024-01-31T23:59:59
```

**Response Format:** JSON with timeseries values

## Database Schema

Applied via `sql/004_usace_cwms.sql`:

### Tables

- **`usace.cwms_locations`** - Monitored CWMS sites (gauges, locks, dams)
- **`usace.cwms_timeseries`** - Stage, flow, and elevation readings
- **`usace.lock_operations`** - Lock/dam operational data (pool levels, gates, flows)
- **`usace.backwater_events`** - Detected backwater flooding incidents
- **`usace.cwms_ingestion_log`** - API ingestion tracking

### Views

- **`usace.mississippi_current_conditions`** - Latest Mississippi River stages
- **`usace.active_lock_operations`** - Recent lock/dam operations

### Functions

- **`usace.detect_backwater_conditions()`** - Compare Mississippi to Illinois stages

## Pre-Configured Locations

Schema includes key monitoring points:

### Mississippi River (Backwater Detection)
- **Grafton, IL** - Just above IL River confluence (Mile 218.0)
- **Mel Price L&D (Alton)** - Below confluence (Mile 200.8)
- **Lock & Dam 24** - Upstream reference (Mile 273.4)
- **Lock & Dam 25** - Upstream reference (Mile 241.4)

### Illinois River (Dam Operations)
- **LaGrange Lock & Dam** - Mile 80.2
- **Peoria Lock & Dam** - Mile 157.6
- **Starved Rock Lock & Dam** - Mile 231.0

## Ingestion Tools

### Historical Backfill

```bash
cargo run --bin ingest_cwms_historical
```

Fetches all available data since 2015 (when CWMS went public) for monitored locations.

**Features:**
- Automatic date range detection
- Duplicate prevention (safe to re-run)
- Rate limiting (2 seconds between requests)
- Progress logging
- Error handling (continues on failures)

**Expected Runtime:** ~10-20 minutes for all locations (2015-present)

### Backwater Detection

```bash
cargo run --bin detect_backwater
```

Analyzes current conditions to detect Mississippi River backwater flooding.

**Output:**
- Current Mississippi River stages at key points
- Illinois River stages near confluence
- Stage differential analysis
- Backwater severity classification
- Active backwater event summary

## Backwater Detection Logic

Located in `src/ingest/cwms.rs`:

```rust
pub fn detect_backwater(
    mississippi_stage_ft: f64,
    illinois_stage_ft: f64,
    threshold_ft: f64,
) -> bool {
    (mississippi_stage_ft - illinois_stage_ft) > threshold_ft
}
```

**Severity Classification:**
- **None:** < 0.5 ft differential (normal)
- **Minor:** 0.5 - 2.0 ft (slight backwater)
- **Moderate:** 2.0 - 5.0 ft (significant backwater)
- **Major:** 5.0 - 10.0 ft (severe backwater)
- **Extreme:** > 10.0 ft (historic backwater)

## Integration with Flood Prediction

### Query Examples

**Current backwater risk:**
```sql
SELECT * FROM usace.detect_backwater_conditions(
    'Grafton-Mississippi.Stage.Inst.15Minutes.0.Ccp-Rev',
    '05586100',  -- IL River at Grafton USGS site
    2.0          -- 2 ft threshold
);
```

**Comprehensive flood risk (USGS + CWMS + NWS):**
```sql
SELECT 
    -- Illinois River current
    (SELECT value FROM usgs_raw.gauge_readings 
     WHERE site_code = '05567500' AND parameter_code = '00065' 
     ORDER BY reading_time DESC LIMIT 1) as peoria_stage,
    
    -- NWS flood threshold
    (SELECT flood_stage_ft FROM nws.flood_thresholds 
     WHERE site_code = '05567500') as flood_stage,
    
    -- Mississippi backwater status
    (SELECT backwater_detected 
     FROM usace.detect_backwater_conditions(
         'Grafton-Mississippi.Stage.Inst.15Minutes.0.Ccp-Rev',
         '05586100', 2.0
     )) as backwater_active,
    
    -- Upstream dam status
    (SELECT flood_control_active 
     FROM usace.lock_operations 
     WHERE location_id LIKE '%Starved Rock%' 
     ORDER BY observation_time DESC LIMIT 1) as dam_flood_mode;
```

**Historical backwater frequency:**
```sql
SELECT 
    EXTRACT(YEAR FROM event_start) as year,
    COUNT(*) as backwater_events,
    AVG(elevation_above_normal_ft) as avg_severity_ft,
    MAX(backwater_severity) as max_severity
FROM usace.backwater_events
WHERE illinoisaffected = true
GROUP BY EXTRACT(YEAR FROM event_start)
ORDER BY year DESC;
```

## Data Quality Notes

### Known Issues

1. **CWMS API Rate Limits** - Be conservative with request frequency
2. **Data Gaps** - Some locations may have missing periods
3. **Quality Codes** - Check `quality_code` field (0=missing, 1=good, 2=questionable)
4. **Office Variations** - Different USACE districts (MVS, MVR) manage different sections

### Recommended Monitoring

- **Update Frequency:** 15-30 minutes for real-time monitoring
- **Historical Checks:** Verify `cwms_ingestion_log` for failed ingestions
- **Backwater Detection:** Run every hour during Mississippi flood events

## Next Steps

1. **Run Historical Ingestion:**
   ```bash
   cd flomon_service
   cargo run --bin ingest_cwms_historical
   ```

2. **Verify Data:**
   ```sql
   SELECT 
       location_id,
       COUNT(*) as reading_count,
       MIN(timestamp) as earliest,
       MAX(timestamp) as latest
   FROM usace.cwms_timeseries
   GROUP BY location_id
   ORDER BY location_id;
   ```

3. **Set Up Periodic Ingestion:**
   - Real-time: Every 15-30 minutes
   - Cron job or systemd timer
   - Monitor `usace.cwms_ingestion_log` for failures

4. **Integrate with Flood Alerts:**
   - Query backwater conditions in main monitoring loop
   - Trigger alerts when backwater + high IL River = extreme risk
   - Track historical correlation between backwater and Peoria flooding

## References

- [CWMS Data Dissemination](https://cwms-data.usace.army.mil/)
- [USACE Rock Island District](https://www.mvr.usace.army.mil/) - Illinois River management
- [USACE St. Louis District](https://www.mvs.usace.army.mil/) - Mississippi River management
- [Illinois Waterway System](https://www.mvr.usace.army.mil/Missions/Navigation/Illinois-Waterway/)

## Related Files

- **Schema:** `sql/004_usace_cwms.sql`
- **Ingestion Module:** `src/ingest/cwms.rs`
- **Historical Ingest:** `src/bin/ingest_cwms_historical.rs`
- **Backwater Detection:** `src/bin/detect_backwater.rs`
- **Documentation:** `docs/SCHEMA_EXTENSIBILITY.md`
