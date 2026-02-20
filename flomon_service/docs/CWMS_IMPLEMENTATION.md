# CWMS Data Integration - Implementation Summary

**Date:** February 20, 2026  
**Status:** ✅ Ready for historical ingestion

## What Was Built

### 1. Database Schema (`sql/004_usace_cwms.sql`)

Created 5 tables in `usace` schema:

- **`cwms_locations`** (7 pre-configured monitoring points)
  - 4 Mississippi River locations for backwater detection
  - 3 Illinois River lock/dam locations
  
- **`cwms_timeseries`** (timeseries data storage)
  - Stage, flow, elevation readings
  - Quality codes and metadata
  
- **`lock_operations`** (dam operational data)
  - Pool levels, gate positions
  - Flow rates, operational modes
  
- **`backwater_events`** (backwater flood tracking)
  - Mississippi-Illinois gradient analysis
  - Severity classification
  - Impact tracking
  
- **`cwms_ingestion_log`** (audit trail)
  - API call tracking
  - Error logging

**Plus:**
- 2 helper views for quick analysis
- 1 backwater detection function
- All permissions granted to `flopro_admin`

### 2. Rust API Client (`src/ingest/cwms.rs`)

CWMS Data API integration:
- `fetch_timeseries()` - Retrieve data for date range
- `fetch_recent()` - Get last N hours
- `fetch_historical()` - Backfill historical data
- `detect_backwater()` - Compare river stages
- `classify_backwater_severity()` - Severity levels

**API Endpoint:** `https://cwms-data.usace.army.mil/cwms-data/`

### 3. Historical Ingestion Binary (`src/bin/ingest_cwms_historical.rs`)

Complete historical backfill tool:
- Fetches all data since 2015 (when CWMS went public)
- Processes all 7 pre-configured locations
- Duplicate prevention (safe to re-run)
- Rate limiting (2 sec between requests)
- Progress logging and error handling

**Usage:**
```bash
cd flomon_service
cargo run --bin ingest_cwms_historical
```

### 4. Backwater Detection Tool (`src/bin/detect_backwater.rs`)

Real-time backwater analysis:
- Current Mississippi River conditions
- Illinois River near-confluence conditions
- Stage differential calculation
- Severity classification
- Active event summary

**Usage:**
```bash
cargo run --bin detect_backwater
```

### 5. Documentation

- **`docs/CWMS_INTEGRATION.md`** - Complete CWMS guide
- **`docs/SCHEMA_EXTENSIBILITY.md`** - Updated with CWMS status
- **SQL comments** - Inline documentation in schema

## Pre-Configured Monitoring Locations

### Mississippi River (Backwater Detection)

| Location | River Mile | Purpose |
|----------|------------|---------|
| Grafton, IL | 218.0 | Primary backwater detection (at IL confluence) |
| Mel Price L&D (Alton) | 200.8 | Below confluence validation |
| Lock & Dam 24 | 273.4 | Upstream reference |
| Lock & Dam 25 | 241.4 | Upstream reference |

### Illinois River (Lock/Dam Operations)

| Location | River Mile | Purpose |
|----------|------------|---------|
| LaGrange Lock & Dam | 80.2 | Downstream operations |
| Peoria Lock & Dam | 157.6 | Mid-river operations |
| Starved Rock Lock & Dam | 231.0 | Upstream operations |

## Why This Matters

### Bottom-Up Floods Explained

Normal conditions:
```
Mississippi River (Grafton): 430 ft elevation
Illinois River (Grafton):    432 ft elevation
Flow: Illinois → Mississippi (normal downstream)
```

Backwater conditions:
```
Mississippi River (Grafton): 440 ft elevation  ⚠️ HIGH
Illinois River (Grafton):    432 ft elevation
Flow: Mississippi → Illinois (REVERSED!)
Result: Water backs up Illinois River, flooding Peoria
```

**Real Example - May 2019:**
- Mississippi at 38+ ft flooded Grafton
- Blocked Illinois River drainage
- Peoria experienced severe flooding despite moderate Illinois flows
- Backwater extended 100+ miles upstream

## Testing & Verification

All binaries compile successfully:
```bash
✓ ingest_cwms_historical
✓ detect_backwater  
✓ ingest_peak_flows (existing)
```

Schema deployed:
```sql
✓ usace.cwms_locations (7 locations pre-configured)
✓ usace.cwms_timeseries (ready for data)
✓ usace.lock_operations (ready for data)
✓ usace.backwater_events (ready for detection)
✓ usace.cwms_ingestion_log (audit trail)
```

## Next Steps

1. **Run Historical Ingestion** (10-20 minutes):
   ```bash
   cd flomon_service
   cargo run --bin ingest_cwms_historical
   ```
   
   Expected: ~50,000-100,000 records (7 locations × ~10 years × 15-min intervals)

2. **Verify Data Loaded:**
   ```sql
   SELECT 
       location_id,
       COUNT(*) as readings,
       MIN(timestamp::date) as start_date,
       MAX(timestamp::date) as end_date
   FROM usace.cwms_timeseries
   GROUP BY location_id;
   ```

3. **Test Backwater Detection:**
   ```bash
   cargo run --bin detect_backwater
   ```

4. **Set Up Periodic Updates:**
   - Create cron job or systemd timer
   - Run every 15-30 minutes for real-time monitoring
   - Monitor `usace.cwms_ingestion_log` for failures

## Integration Examples

### Comprehensive Flood Risk Query

```sql
SELECT 
    -- Current Peoria stage
    (SELECT value FROM usgs_raw.gauge_readings 
     WHERE site_code = '05567500' 
       AND parameter_code = '00065' 
     ORDER BY reading_time DESC LIMIT 1) as peoria_stage_ft,
    
    -- Peoria flood threshold
    (SELECT flood_stage_ft FROM nws.flood_thresholds 
     WHERE site_code = '05567500') as peoria_flood_stage,
    
    -- Backwater status
    (SELECT backwater_detected 
     FROM usace.detect_backwater_conditions(
         'Grafton-Mississippi.Stage.Inst.15Minutes.0.Ccp-Rev',
         '05586100', 2.0
     )) as mississippi_backwater,
    
    -- Dam flood control status
    (SELECT flood_control_active 
     FROM usace.lock_operations 
     WHERE location_id LIKE '%Peoria%'
     ORDER BY observation_time DESC LIMIT 1) as peoria_dam_flood_mode;
```

### Historical Backwater Frequency

```sql
SELECT 
    EXTRACT(YEAR FROM event_start) as year,
    COUNT(*) as events,
    AVG(elevation_above_normal_ft) as avg_severity,
    MAX(backwater_severity) as max_severity
FROM usace.backwater_events
WHERE illinois_affected = true
GROUP BY year
ORDER BY year DESC;
```

## Files Created/Modified

**New Files:**
- `sql/004_usace_cwms.sql` - Database schema (520 lines)
- `src/ingest/cwms.rs` - API client (200 lines)
- `src/bin/ingest_cwms_historical.rs` - Historical ingest (170 lines)
- `src/bin/detect_backwater.rs` - Backwater detection tool (200 lines)
- `docs/CWMS_INTEGRATION.md` - Integration guide (330 lines)
- `docs/CWMS_IMPLEMENTATION.md` - This summary

**Modified Files:**
- `src/ingest/mod.rs` - Added cwms module
- `Cargo.toml` - Added urlencoding dependency, enabled json feature
- `docs/SCHEMA_EXTENSIBILITY.md` - Updated status

## Key Features

✅ **Backwater Detection** - Automatic Mississippi River backwater monitoring  
✅ **Lock/Dam Tracking** - Monitor USACE operations on Illinois River  
✅ **Historical Data** - Backfill from 2015-present  
✅ **Real-time Ready** - Infrastructure for live monitoring  
✅ **Data Quality** - Quality codes, ingestion logging  
✅ **Duplicate Prevention** - Safe to re-run ingestion  
✅ **Rate Limiting** - Respects API limits  
✅ **Error Handling** - Continues on individual failures  

## Success Criteria

- [x] Schema deployed to `flopro_db`
- [x] 7 monitoring locations configured
- [x] Binaries compile without errors
- [ ] Historical data ingested (run `ingest_cwms_historical`)
- [ ] Backwater detection tested (run `detect_backwater`)
- [ ] Periodic ingestion scheduled (next step)

---

**Ready for historical ingestion!** Run `cargo run --bin ingest_cwms_historical` to begin.
