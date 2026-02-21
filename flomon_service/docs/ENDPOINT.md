# HTTP Endpoint Documentation

The flood monitoring daemon provides a REST API for querying site data.

## Starting the Endpoint

```bash
# Start daemon with HTTP endpoint on port 8080
cargo run --release -- --endpoint 8080

# Or use the compiled binary
./target/release/flomon_service --endpoint 8080
```

## Endpoints

### `GET /health`

Health check endpoint.

**Example:**
```bash
curl http://localhost:8080/health
```

**Response:**
```json
{
  "status": "ok",
  "service": "flomon_service",
  "version": "0.1.0"
}
```

### `GET /site/{site_code}`

Query all relational data for a specific monitoring station.

**Parameters:**
- `site_code` - USGS 8-digit site code (e.g., `05568500`)

**Example:**
```bash
curl http://localhost:8080/site/05568500 | jq
```

**Success Response (200):**
```json
{
  "site_code": "05568500",
  "site_name": "Illinois River at Kingston Mines, IL",
  "description": "Main monitoring point for Kingston Mines pool",
  "latitude": 40.5533333,
  "longitude": -89.7644444,
  "discharge": {
    "value": 42300.0,
    "unit": "ft3/s",
    "datetime": "2024-05-01T12:00:00.000-05:00",
    "qualifier": "P"
  },
  "stage": {
    "value": 15.5,
    "unit": "ft",
    "datetime": "2024-05-01T12:00:00.000-05:00",
    "qualifier": "P"
  },
  "thresholds": {
    "action_stage_ft": 14.0,
    "flood_stage_ft": 16.0,
    "moderate_flood_stage_ft": 20.0,
    "major_flood_stage_ft": 24.0
  },
  "monitoring_state": {
    "status": "active",
    "last_poll_attempted": "2024-05-01T12:15:00Z",
    "last_poll_succeeded": "2024-05-01T12:15:00Z",
    "consecutive_failures": 0,
    "is_stale": false
  },
  "last_updated": "2024-05-01T17:00:00Z",
  "staleness_minutes": 45
}
```

**Error Response (404):**
```json
{
  "error": "Site code INVALID not found in station registry",
  "site_code": "INVALID"
}
```

## Response Fields

### Site Metadata
- `site_code` - USGS 8-digit station identifier
- `site_name` - Official USGS station name
- `description` - Human-readable description of monitoring purpose
- `latitude` - WGS84 latitude (decimal degrees)
- `longitude` - WGS84 longitude (decimal degrees)

### Current Readings
- `discharge` - Streamflow measurement (parameter 00060)
  - `value` - Discharge in cubic feet per second
  - `unit` - Unit of measurement (ft3/s)
  - `datetime` - ISO 8601 timestamp with timezone
  - `qualifier` - Data quality (P=Provisional, A=Approved)
  
- `stage` - Gage height measurement (parameter 00065)
  - `value` - Stage in feet
  - `unit` - Unit of measurement (ft)
  - `datetime` - ISO 8601 timestamp with timezone
  - `qualifier` - Data quality (P=Provisional, A=Approved)

### Thresholds
NWS flood stage thresholds (if defined for this station):
- `action_stage_ft` - Action stage (prepare for flooding)
- `flood_stage_ft` - Minor flood stage
- `moderate_flood_stage_ft` - Moderate flood stage
- `major_flood_stage_ft` - Major flood stage

### Monitoring State
- `status` - Station status (active, degraded, offline)
- `last_poll_attempted` - Last API polling attempt
- `last_poll_succeeded` - Last successful data retrieval
- `consecutive_failures` - Number of failed polls in a row
- `is_stale` - Whether data exceeds staleness threshold

### Data Freshness
- `last_updated` - Timestamp of most recent reading
- `staleness_minutes` - Age of data in minutes

## Using with Python/FloML

```python
import requests

# Query site data
response = requests.get('http://localhost:8080/site/05568500')
data = response.json()

# Extract stage reading
if data['stage']:
    stage_ft = data['stage']['value']
    timestamp = data['stage']['datetime']
    print(f"Current stage: {stage_ft} ft at {timestamp}")

# Check flood status
if data['thresholds'] and data['stage']:
    stage = data['stage']['value']
    thresholds = data['thresholds']
    
    if stage >= thresholds['major_flood_stage_ft']:
        print("MAJOR FLOOD")
    elif stage >= thresholds['moderate_flood_stage_ft']:
        print("MODERATE FLOOD")
    elif stage >= thresholds['flood_stage_ft']:
        print("MINOR FLOOD")
    elif stage >= thresholds['action_stage_ft']:
        print("ACTION STAGE")
```

## Data Organization

The endpoint uses the `groupings` module to organize data by site:
- Queries latest readings from `usgs_raw.gauge_readings`
- Groups discharge (00060) and stage (00065) by site code
- Enriches with station metadata from `stations.toml`
- Adds monitoring state from `usgs_raw.monitoring_state`
- Returns complete relational view of site data

## Testing

Run the test script:
```bash
./scripts/test_endpoint.sh
```

This will test:
1. Health check
2. Valid site queries
3. Invalid site handling
4. Invalid endpoint handling
