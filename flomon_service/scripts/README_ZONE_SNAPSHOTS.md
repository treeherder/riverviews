# Zone Snapshot Generator for Historical Floods

## Purpose

This script generates "moment in time" zone-based snapshots for each historical flood event, 
showing the status of all 7 hydrological zones at the time of each flood crest.

This data is used for:
- **Regression analysis** to identify flood precursors
- **Event classification** (top-down, bottom-up, compound, local tributary)
- **ML model training** to predict flood arrival times
- **Pattern recognition** across historical events

## Prerequisites

### 1. Database Setup

Flood events must be populated in the database:

```bash
# First, run peak flow ingestion to populate nws.flood_events
cd flomon_service
cargo run --bin ingest_peak_flows

# This will:
# - Fetch peak flow data from USGS for all monitored stations
# - Parse RDB format
# - Identify flood events (stage >= flood_stage_ft)
# - Insert into nws.flood_events table
```

### 2. Historical Gauge Readings

For accurate zone snapshots, the database should contain historical gauge readings:

```bash
# Backfill historical USGS data (up to 120 days)
cargo run --bin historical_ingest

# For older data, you'll need to:
# 1. Download historical USGS data manually
# 2. Import CWMS historical lock/dam data
# 3. Import ASOS weather observations
```

**Note:** Without historical readings, the script will still generate event classifications
based on flood location and timing, but sensor readings will show "NO DATA" for events
outside the available data window.

### 3. Python Dependencies

```bash
pip install psycopg2-binary toml
```

## Usage

### Basic Usage

```bash
cd flomon_service/scripts
python3 generate_flood_zone_snapshots.py
```

This will:
1. Connect to the database
2. Load zones.toml configuration
3. Fetch all historical flood events from nws.flood_events
4. For each event, query sensor readings ±6 hours from crest time
5. Generate zone snapshots and classify event types
6. Write report to `PEAK_FLOW_SUMMARY.md`

### Custom Database

```bash
python3 generate_flood_zone_snapshots.py \
  --db-url postgresql://user:pass@host/dbname
```

### Custom Output File

```bash
python3 generate_flood_zone_snapshots.py \
  --output my_flood_analysis.md
```

### Custom Zones Configuration

```bash
python3 generate_flood_zone_snapshots.py \
  --zones-config /path/to/zones.toml
```

## Output Format

The script generates a markdown report with:

### 1. Event Type Distribution

Summary statistics showing how many floods were:
- **COMPOUND**: Backwater + upstream pulse (most dangerous)
- **BOTTOM_UP**: Mississippi backwater dominant
- **TOP_DOWN**: Chicago/upper basin precipitation
- **LOCAL_TRIBUTARY**: Mackinaw River flash flooding

### 2. Individual Flood Event Analysis

For each flood:

```markdown
### 05568500 – 2013-04-21 12:00

**Severity:** MAJOR  
**Peak Stage:** 24.62 ft  
**Event Type:** COMPOUND  

#### Zone Status at Crest

| Zone | Name | Status | Active Sensors | Data Coverage |
|------|------|--------|----------------|---------------|
| 0 | Mississippi River... | CRITICAL | 3/3 | 100% |
| 1 | LaGrange Lock... | CRITICAL | 4/5 | 80% |
| 2 | Property Zone... | CRITICAL | 6/6 | 100% |
...

#### Event Characteristics
- **Backwater Active:** Yes
- **Upstream Pulse:** Yes
- **Local Tributary:** No

#### Critical Sensor Readings
**Zone 2 (Property Zone):**
- 05568500: 24.62 ft (12:00)
- IL07P: 18.79 ft (11:45)
- KPIA: 0.8 in precip (12:00)
```

### 3. Regression Analysis Dataset

The structured data can be exported as CSV/JSON for regression analysis:

```python
# Example feature extraction for ML
for event in flood_events:
    features = {
        # Zone 0 (Mississippi backwater indicators)
        'grafton_stage_t_minus_24hr': zone_0_readings['Grafton'][-24h],
        'alton_stage_t_minus_24hr': zone_0_readings['Alton'][-24h],
        
        # Zone 1 (backwater interface)
        'lagrange_differential_t_minus_24hr': pool - tailwater,
        
        # Zone 6 (upstream precipitation)
        'chicago_precip_6hr_t_minus_72hr': zone_6_readings['KORD'][-72h],
        
        # Target
        'property_peak_stage': zone_2_readings['Kingston Mines'][crest_time],
        'event_type': classify_event(zones)
    }
```

## Event Classification Logic

### COMPOUND Event
```python
if zone_0_active and any(zone in [4,5,6] for zone in active_zones):
    event_type = "COMPOUND"
```

**Mechanism:** Property zone trapped between:
- South: Mississippi backwater blocking drainage through LaGrange
- North: Upper basin runoff continuing to arrive

**Historical Examples:** 2013-04-21, 2015-12-29

### BOTTOM_UP Event
```python
if zone_0_active and not any(zone >= 4 for zone in active_zones):
    event_type = "BOTTOM_UP"
```

**Mechanism:** Mississippi River blocks Illinois River outflow

**Counterintuitive Signature:** Property floods while upstream zones (4-6) are quiet

**Historical Examples:** 1982-12-04 (record 20.21 ft at Peoria Pool)

### TOP_DOWN Event
```python
if any(zone in [4,5,6] for zone in active_zones) and not zone_0_active:
    event_type = "TOP_DOWN"
```

**Mechanism:** Chicago/upper basin precipitation progresses downstream

**Classic Pattern:** Zone 6 → 5 → 4 → 3 → 2 progression over days

**Historical Examples:** Many spring floods with upper basin snowmelt

### LOCAL_TRIBUTARY Event
```python
if zone_3_active and not (zone_0_active or any(zone >= 4 for zone in active_zones)):
    event_type = "LOCAL_TRIBUTARY"
```

**Mechanism:** Mackinaw River rapid response to intense local rainfall

**Shortest Warning Time:** 6-18 hours (vs. 24-72 hours for top-down)

**Key Indicator:** Mackinaw River rate-of-rise > 1 ft/hr

## Data Availability Windows

| Data Source | Typical Availability | Impact on Snapshots |
|-------------|---------------------|---------------------|
| **USGS IV** | 120 days (via API) | Recent floods only without backfill |
| **USGS Peak Flow** | 1915-present | Event dates/stages always available |
| **CWMS** | ~1990-present | Lock/dam data limited for pre-1990s floods |
| **ASOS** | ~2000-present | Weather data limited for older events |

**Recommendation:** For complete historical analysis:
- Download USGS historical daily values (1915-present)
- Focus detailed sensor analysis on recent floods (2000-present)
- Use peak flow database for older event classification (dates/stages only)

## Example Workflow

### Step 1: Populate Database with Flood Events

```bash
cd flomon_service

# Ingest peak flow data (creates flood event records)
cargo run --bin ingest_peak_flows

# Verify events
PGPASSWORD=flopro_dev_2026 psql -h localhost -U flopro_admin -d flopro_db \
  -c "SELECT COUNT(*) FROM nws.flood_events;"
```

### Step 2: Backfill Historical Data (Optional)

```bash
# Recent data (last 120 days)
cargo run --bin historical_ingest

# Older data requires manual download/import
# See docs/DATABASE_SETUP.md for details
```

### Step 3: Generate Zone Snapshots

```bash
cd scripts
python3 generate_flood_zone_snapshots.py
```

### Step 4: Review Output

```bash
cat PEAK_FLOW_SUMMARY.md
```

### Step 5: Extract for Regression Analysis

```python
# In your analysis script:
import re
import pandas as pd

# Parse markdown output
events = parse_flood_summary_markdown('PEAK_FLOW_SUMMARY.md')

# Convert to DataFrame
df = pd.DataFrame(events)

# Feature engineering
df['backwater_severity'] = df['zone_0_status'].apply(
    lambda x: 3 if x=='CRITICAL' else 2 if x=='WARNING' else 1
)

# Train model
from sklearn.ensemble import RandomForestRegressor
model = RandomForestRegressor()
model.fit(df[feature_cols], df['peak_stage_ft'])
```

## Maintenance

### Adding New Zones

Edit `zones.toml` and add sensors to new zone. Script will automatically include
the new zone in snapshots.

### Custom Event Classification

Edit the `classify_flood_event()` method in the script to implement custom logic.

### Performance

- **Small datasets** (<100 events): ~1-2 minutes
- **Large datasets** (>500 events): ~10-15 minutes
- **Bottleneck:** Database queries for each sensor at each event time

**Optimization:** Add database indexes on timestamp columns:

```sql
CREATE INDEX idx_usgs_readings_site_time 
  ON usgs_raw.gauge_readings(site_code, reading_time);

CREATE INDEX idx_cwms_location_time 
  ON usace.cwms_timeseries(location_id, timestamp);
```

## Troubleshooting

### No Flood Events Found

```
Found 0 historical flood events
```

**Solution:** Run peak flow ingestion first:
```bash
cargo run --bin ingest_peak_flows
```

### No Sensor Readings

```
Zone 2 (Property Zone):
- 05568500: NO DATA
- IL07P: NO DATA
```

**Cause:** No historical gauge readings in database for event date

**Solutions:**
1. **Recent events**: Run `cargo run --bin historical_ingest`
2. **Older events**: Download historical daily values from USGS
3. **Accept limitation**: Event classification still works from peak flow metadata

### Database Connection Error

```
Error: could not connect to server
```

**Check:**
1. PostgreSQL is running: `pg_isready -h localhost`
2. Database exists: `psql -l | grep flopro_db`
3. Credentials correct in `--db-url` or default

### Missing zones.toml

```
Error: Failed to load zones.toml
```

**Solution:** Run from `flomon_service/scripts/` directory or specify path:
```bash
python3 generate_flood_zone_snapshots.py --zones-config ../zones.toml
```

## See Also

- [PEAK_FLOW_SUMMARY.md](../../PEAK_FLOW_SUMMARY.md) - Output report
- [zones.toml](../zones.toml) - Zone configuration
- [ZONE_ENDPOINT_MIGRATION.md](../ZONE_ENDPOINT_MIGRATION.md) - Zone-based API docs
- [ARCHITECTURE_COMPARISON.md](../ARCHITECTURE_COMPARISON.md) - Zone architecture design
