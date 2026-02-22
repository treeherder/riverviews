# ASOS Weather Monitoring Implementation

## Overview

ASOS (Automated Surface Observing System) weather station integration for precipitation monitoring in support of tributary flood forecasting in the Illinois River basin.

## Data Source

**IEM (Iowa Environmental Mesonet)**
- Base URL: https://mesonet.agron.iastate.edu
- 1-minute precipitation endpoint: `/cgi-bin/request/asos1min.py`
- Current observations: `/json/current.py`
- Coverage: 2000-present with high-resolution precipitation data

## Configured Stations

| Station | Name | Basin | Priority | Poll Interval | Purpose |
|---------|------|-------|----------|---------------|---------|
| **KPIA** | Peoria | Illinois River | CRITICAL | 15 min | Primary local precipitation station |
| **KBMI** | Bloomington | Mackinaw River | HIGH | 60 min | Tributary basin (lag: 12 hrs) |
| **KSPI** | Springfield | Sangamon River | HIGH | 60 min | Tributary basin (lag: 24 hrs) |
| **KGBG** | Galesburg | Spoon River | HIGH | 60 min | Tributary basin (lag: 18 hrs) |
| **KORD** | O'Hare | Des Plaines River | MEDIUM | 6 hrs | Extended coverage (lag: 6 hrs) |
| **KPWK** | Wheeling | Des Plaines River | MEDIUM | 6 hrs | Extended coverage |

## Basin Precipitation Thresholds

These thresholds trigger flood watch/warning assessments based on accumulated precipitation:

| Basin | 6-Hour Watch | 6-Hour Warning | 24-Hour Watch | 24-Hour Warning | Lag Time |
|-------|--------------|----------------|---------------|-----------------|----------|
| **Illinois River** | 1.5" | 2.5" | 3.0" | 5.0" | 48 hours |
| **Mackinaw River** | 1.0" | 2.0" | 2.5" | 4.0" | 12 hours |
| **Spoon River** | 1.2" | 2.5" | 3.0" | 5.0" | 18 hours |
| **Sangamon River** | 1.5" | 3.0" | 3.5" | 5.5" | 24 hours |
| **Des Plaines River** | 1.0" | 2.0" | 2.5" | 4.5" | 6 hours |

### Lag Time Interpretation

**Lag time** is the approximate duration from when significant precipitation falls in a basin to when peak streamflow occurs at the downstream USGS gauge:

- **Short lag (6 hrs)**: Urban basins with limited storage (Des Plaines)
- **Medium lag (12-18 hrs)**: Small agricultural basins (Mackinaw, Spoon)
- **Long lag (24-48 hrs)**: Larger basins with storage/wetlands (Sangamon, Illinois mainstem)

## Database Schema

### Tables

#### `asos_stations`
Station metadata registry (populated from `iem_asos.toml`):
- `station_id`: ASOS identifier (e.g., "KPIA")
- `name`: Human-readable name
- `lat/lon/elevation_ft`: Georeferencing
- `basin`: Associated tributary basin
- `upstream_gauge`: USGS gauge ID that responds to precip
- `priority`: CRITICAL, HIGH, MEDIUM, LOW
- `poll_interval_minutes`: How often to poll IEM API

#### `asos_observations`
High-resolution weather observations (1-minute to hourly):
- `station_id`, `observation_time`: Primary identifier
- `temp_f`, `dewpoint_f`, `relative_humidity`: Temperature fields
- `wind_direction_deg`, `wind_speed_knots`, `wind_gust_knots`: Wind
- **`precip_1hr_in`**: 1-hour precipitation (CRITICAL for flood risk)
- `pressure_mb`, `visibility_mi`: Atmospheric conditions
- `sky_condition`, `weather_codes`: Qualitative observations
- `data_source`: 'IEM_CURRENT' or 'IEM_1MIN'

#### `asos_precip_summary`
Pre-computed precipitation summaries for specified time windows (6hr, 12hr, 24hr, 48hr):
- `station_id`, `period_start`, `period_type`: Identifier
- `precip_total_in`: Total accumulation
- `precip_max_1hr_in`: Peak 1-hour intensity
- `exceeds_watch_threshold`: Boolean flag (>= basin watch threshold)
- `exceeds_warning_threshold`: Boolean flag (>= basin warning threshold)

#### `basin_precip_thresholds`
Reference table with flood risk thresholds by basin (pre-populated).

### Views

- **`asos_latest`**: Most recent observation per station
- **`asos_active_precip`**: 6-hour precipitation totals (flood watch threshold)

### Data Retention

- **1-minute data**: Keep for 90 days (detailed event analysis)
- **Hourly summaries**: Keep indefinitely
- **Archive**: Move to cold storage after 1 year
- Cleanup function: `cleanup_asos_observations()` (deletes old 1-minute data)

## Implementation Details

### Module Structure

```
flomon_service/
├── src/
│   ├── asos_locations.rs      — TOML loader, priority detection, basin thresholds
│   ├── ingest/
│   │   └── iem.rs             — IEM API client (fetch_current, fetch_recent_precip)
│   └── daemon.rs              — Polling integration
├── iem_asos.toml              — Station configuration
└── sql/
    └── 006_iem_asos.sql       — Database schema
```

### API Client Functions

**`iem::fetch_current(client, station_id)`**
- Fetches latest observation from IEM current endpoint
- Returns single `AsosObservation`
- Used for real-time status checks

**`iem::fetch_recent_precip(client, station_id, hours)`**
- Fetches 1-minute precipitation data for last N hours
- Returns `Vec<AsosObservation>`
- Used for polling (hours=4) and backfill (hours=days*24)

**`iem::calculate_cumulative_precip(observations)`**
- Sums precipitation over observation set
- Returns total accumulation in inches

**`iem::detect_rainfall_event(observations, threshold_in)`**
- Checks if cumulative precip exceeds threshold
- Returns boolean

### Location Loading

**`asos_locations::load_locations(path)`**
- Parses `iem_asos.toml`
- Determines monitoring priority from relevance text
- Returns `Vec<AsosLocation>`

**Priority Detection Keywords:**
- **CRITICAL**: "primary", "critical"
- **HIGH**: "high", "tributary"
- **MEDIUM**: "medium", "extended"
- **LOW**: default

### Daemon Integration

**Startup (in `daemon.initialize()`)**:
```rust
let asos_locs = asos_locations::load_locations("iem_asos.toml")?;
self.asos_locations = asos_locs;
```

**Polling (in `daemon.poll_all_stations()`)**:
```rust
for location in &self.asos_locations.clone() {
    let observations = self.poll_asos_station(&location.station_id)?;
    let inserted = self.warehouse_asos_observations(&observations)?;
}
```

**Warehousing**:
- `warehouse_asos_observations()` — Idempotent INSERT with ON CONFLICT DO NOTHING
- Converts all numeric fields to PostgreSQL NUMERIC (Decimal)
- Sets `data_source` based on endpoint used

## Usage Examples

### Setup Database Schema

```bash
# Apply ASOS schema
psql -U flopro_user -d flopro_db -f sql/006_iem_asos.sql

# Verify tables
psql -U flopro_user -d flopro_db -c "\dt"
```

### Query Recent Precipitation

```sql
-- Last 6 hours of precipitation by station
SELECT * FROM asos_active_precip 
WHERE precip_6hr_in > 0.5 
ORDER BY precip_6hr_in DESC;

-- Latest observation per station
SELECT * FROM asos_latest;

-- Check if any basin exceeds flood watch
SELECT 
    st.station_id,
    st.basin,
    SUM(obs.precip_1hr_in) AS precip_6hr,
    thresh.watch_6hr_in AS watch_threshold,
    CASE WHEN SUM(obs.precip_1hr_in) >= thresh.watch_6hr_in 
         THEN 'WATCH' 
         ELSE 'NORMAL' 
    END AS status
FROM asos_stations st
JOIN asos_observations obs ON st.station_id = obs.station_id
JOIN basin_precip_thresholds thresh ON st.basin = thresh.basin
WHERE obs.observation_time >= NOW() - INTERVAL '6 hours'
GROUP BY st.station_id, st.basin, thresh.watch_6hr_in
ORDER BY precip_6hr DESC;
```

### Add New ASOS Station

Edit `iem_asos.toml`:

```toml
[[stations]]
station_id = "KMDW"
name = "Chicago Midway"
latitude = 41.786
longitude = -87.752
elevation_ft = 620.0
data_types = ["precipitation", "temperature"]
relevance = "Medium-priority extended coverage for Chicago metro rainfall"
basin = "Des Plaines River"
upstream_gauge = "05536995"  # USGS Chicago Sanitary & Ship Canal
```

Restart daemon to load new station.

### Manual Backfill

```rust
// In daemon or standalone script:
daemon.backfill_asos_station("KPIA", 30)?;  // Last 30 days
```

## Flood Forecasting Workflow

1. **Precipitation Detection**: ASOS stations detect significant rainfall
2. **Basin Aggregation**: Calculate 6hr, 12hr, 24hr totals per basin
3. **Threshold Comparison**: Check against basin-specific watch/warning levels
4. **Lag Time Application**: Add basin lag hours to precipitation timestamp
5. **Gauge Monitoring**: Monitor upstream USGS gauge at predicted peak time
6. **Alert Generation**: Issue watch/warning if both precip and gauge trends confirm

### Example: Mackinaw River Flood

```
1. KBMI (Bloomington) receives 2.5" in 6 hours → EXCEEDS WARNING (2.0")
2. Mackinaw River lag = 12 hours
3. Monitor USGS 05568000 (Mackinaw at Green Valley) starting 12 hours after rainfall
4. If stage rises to Action/Minor/Moderate, issue corresponding alert
```

## Implementation Status

✅ **Complete:**
- IEM API client (`src/ingest/iem.rs`)
- ASOS location loader (`src/asos_locations.rs`)
- Database schema (`sql/006_iem_asos.sql`)
- Daemon integration (polling, warehousing)
- Basin-specific thresholds
- Lag time calculations

⏳ **Future Enhancements:**
- Automated precipitation summary computation (scheduled job)
- Alert generation based on threshold exceedances
- Backwater correlation (ASOS precip + LaGrange hydraulic control loss)
- IEMRE gridded precipitation integration
- MRMS radar-based QPE verification
- API endpoint for current flood risk status

## Testing

```bash
# Build with full warnings
cargo build --release

# Run daemon with ASOS monitoring
./target/release/flomon_service

# Check for ASOS startup messages:
# 📡 Loaded 6 ASOS stations for precipitation monitoring
#    KPIA (Peoria) - Illinois River basin - Priority: Critical
#    ...

# Monitor logs for poll results:
# ✓ Poll complete: 342 new readings (8 USGS, 13 CWMS, 6 ASOS)
```

## References

- **IEM ASOS Documentation**: https://mesonet.agron.iastate.edu/request/download.phtml
- **ASOS Program**: https://www.weather.gov/asos/
- **Basin Thresholds**: Empirically derived from historical flood events + NWS guidelines
- **Lag Times**: Based on USGS StreamStats and historical hydrograph analysis

## Maintenance

### Monthly Tasks
- Review `asos_precip_summary` for flood events
- Validate threshold accuracy against actual flood occurrences
- Adjust poll intervals if needed

### Quarterly Tasks
- Run `cleanup_asos_observations()` to purge old 1-minute data
- Archive summaries to cold storage
- Review new ASOS station additions

### Annual Tasks
- Recalibrate basin thresholds based on previous year's events
- Update lag times if basin characteristics change
- Review upstream gauge associations

---

**Last Updated**: 2026-02-21  
**Implementation Version**: 1.0  
**Contact**: Flood Monitoring Service Team
