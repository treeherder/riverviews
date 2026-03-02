# TOML-Based CWMS Configuration Guide

## Overview

The flood monitoring system loads USACE/CWMS location metadata from `usace_stations.toml` and discovers actual timeseries IDs at runtime via the CWMS catalog API. This allows you to add new locations, update monitoring priorities, and adjust configuration without code changes or database migrations.

## Key Features

### 1. Runtime Timeseries Discovery

The system **does not hardcode** CWMS timeseries IDs. Instead, at startup:

1. Loads location metadata from `usace_stations.toml`
2. Queries CWMS catalog API: `https://cwms-data.usace.army.mil/cwms-data/catalog/TIMESERIES`
3. Discovers actual available timeseries for each location
4. Uses discovered timeseries IDs for all subsequent polling

This solves the "version suffix varies by office" problem - the catalog returns the exact timeseries IDs that actually exist.

### 2. Configuration File Structure

```toml
[[usace_stations]]
shef_id         = "IL07"              # Legacy SHEF ID
cwms_location   = "Peoria-Pool"       # CWMS location name
office          = "MVR"               # USACE district office
name            = "Illinois River at Peoria Lock and Dam"
river_mile      = 157.6
pool_elevation_target_ft_ngvd29 = 447.0
data_types      = ["pool_elevation", "tailwater_elevation", "lockage"]
relevance       = "PRIMARY — directly controls Upper Peoria Lake level. ..."
flood_note      = "Wicket dam operation: when wickets are laid down..."
```

### 3. Monitoring Priorities (Auto-Detected)

The system determines polling frequency from the `relevance` text (case-insensitive):

- **CRITICAL** (15-min polling): Contains "PRIMARY" or "CRITICAL"
- **HIGH** (60-min polling): Contains "HIGH" or "UPSTREAM WARNING"  
- **MEDIUM** (360-min / 6-hour polling): Contains "EXTENDED" or "CONFLUENCE MONITOR"
- **LOW** (1440-min / daily polling): Everything else

Change priority by editing keywords in the `relevance` field in the TOML file.

## Startup Sequence

```
🌊 Flood Monitoring Service
============================

📊 Initializing daemon...
🔍 Discovering CWMS timeseries IDs from catalog...
   Illinois River at Peoria Lock and Dam ... ✓
   Illinois River at LaGrange Lock and Dam ... ✓
   Mississippi River at Grafton, IL ... ✓
   ...
   
   Discovered timeseries for 13/13 locations

✓ Daemon initialized
```

Each location is queried against the CWMS catalog. If discovery fails, you'll see:
```
   Illinois River at Havana Lock and Dam ... ✗ No timeseries found
      Warning: Will skip polling for Illinois River at Havana Lock and Dam
```

## LaGrange Backwater Detection

The code includes special logic for detecting when the Mississippi River takes control:

```rust
// In src/ingest/cwms.rs
pub fn detect_hydraulic_control_loss(
    pool_elevation_ft: f64,
    tailwater_elevation_ft: f64,
    margin_ft: f64,
) -> bool {
    (tailwater_elevation_ft + margin_ft) >= pool_elevation_ft
}
```

When `LaGrange tailwater >= LaGrange pool - 0.5ft`:
- Dam has lost hydraulic control
- Mississippi backwater is dominant
- Peoria readings become lagging indicators
- Your property is flooding from "the bottom up"

## Catalog API Examples

### Query all Peoria timeseries
```bash
curl "https://cwms-data.usace.army.mil/cwms-data/catalog/TIMESERIES?office=MVR&like=Peoria.*&format=json"
```

Returns:
```json
{
  "entries": [
    {"name": "Peoria-Pool.Elev.Inst.~1Hour.0.CBT-RAW", "office": "MVR"},
    {"name": "Peoria-TW.Elev.Inst.~1Hour.0.CBT-RAW", "office": "MVR"},
    ...
  ]
}
```

### Query all LaGrange timeseries
```bash
curl "https://cwms-data.usace.army.mil/cwms-data/catalog/TIMESERIES?office=MVR&like=LaGrange.*&format=json"
```

### Query Mississippi River gauges (MVS district)
```bash
curl "https://cwms-data.usace.army.mil/cwms-data/catalog/TIMESERIES?office=MVS&like=Grafton.*&format=json"
```

## Adding a New Location

1. Add to `usace_stations.toml`:
```toml
[[usace_stations]]
shef_id         = "IL09"
cwms_location   = "Havana-Pool"
office          = "MVR"
name            = "Illinois River at Havana Lock and Dam"
river_mile      = 119.7
data_types      = ["pool_elevation", "tailwater_elevation"]
relevance       = "HIGH UPSTREAM WARNING — between LaGrange and Peoria"
```

2. Restart daemon - it will automatically:
   - Query catalog for "Havana.*" timeseries
   - Discover available data streams
   - Start polling based on priority (HIGH = 60 minutes)

3. No SQL or code changes needed!

## Handling Missing Timeseries

If catalog returns no timeseries for a location:

```
   Illinois River at Havana Lock and Dam ... ✗ No timeseries found
      Warning: Will skip polling for Illinois River at Havana Lock and Dam
```

The system continues operating with other locations. Fix by:

1. Verifying CWMS location name (check catalog manually)
2. Updating `cwms_location` in TOML  
3. Restarting daemon

## Data Type Mapping

The `data_types` field tells the discovery system what to look for:

- `"pool_elevation"` → Searches for `*-Pool.Elev.*` (prioritizing `.Inst` instantaneous readings)
- `"tailwater_elevation"` → Searches for `*-TW.Elev.*`, `*-Tailwater.Elev.*`, or `TW-*.Elev.*`
- `"stage"` → Searches for `*.Stage.*` (for river gauges, not pools)
- `"discharge"` → (not yet implemented)
- `"lockage"` → (not yet implemented - metadata only)
- `"met"` → (not yet implemented - meteorological data)

## SHEF ID Notes

The TOML file includes SHEF IDs (IL02-IL08) which are the legacy identifiers from `rivergages.mvr.usace.army.mil`. These map directly to CWMS location names, though spelling may differ slightly:

- SHEF: `IL06` → CWMS: `Starved-Rock-Pool` (or `StarvedRock-Pool`)
- SHEF: `IL07` → CWMS: `Peoria-Pool`
- SHEF: `IL08` → CWMS: `LaGrange-Pool`

The catalog discovery handles these variations automatically.

## Extensibility

The same TOML-based configuration pattern applies to other data sources:

- **IEM ASOS stations** (`iem_asos.toml`) - Implemented for precipitation monitoring
- **USGS gauges** (`usgs_stations.toml`) - Station registry for USGS real-time data
- **NWS forecast points** - Not yet implemented

All follow the pattern: **TOML Configuration → Runtime Discovery/Registration → Polling**

## Troubleshooting

### "No timeseries found for location: X"

1. Check catalog manually:
   ```bash
   curl "https://cwms-data.usace.army.mil/cwms-data/catalog/TIMESERIES?office=MVR&like=X.*&format=json" | jq
   ```

2. Verify `cwms_location` spelling matches catalog results

3. Check `data_types` includes what you expect (pool_elevation vs stage)

### "CWMS API error: 404 Not Found"

The discovered timeseries ID exists in catalog but returns 404 when queried:
- Timeseries may be configured but not receiving data
- Office ID may be wrong (MVR vs MVS)
- Historical data may not be available (check with shorter time range)

### "Failed to discover pool elevation"

Location might not have pool elevation timeseries (e.g., river gauges use "stage" instead):
- Change `data_types = ["pool_elevation"]` to `["stage"]`
- Or add both: `["pool_elevation", "stage"]` and system will find what's available

## Best Practices

1. **Don't hardcode timeseries IDs** - Let discovery handle it
2. **Check catalog before adding locations** - Verify data exists
3. **Use descriptive `relevance` text** - Determines priority automatically
4. **Document flood-specific behavior** in `flood_note` - Helps future debugging
5. **Test new locations with manual curl first** - Before adding to production config

## Implementation Files

- `src/usace_locations.rs` - TOML parsing and discovery logic
- `src/ingest/cwms.rs` - Catalog queries and backwater detection
- `src/daemon.rs` - Polling and backfill using discovered IDs
- `src/main.rs` - Startup discovery sequence
- `usace_stations.toml` - Configuration (user-editable)
