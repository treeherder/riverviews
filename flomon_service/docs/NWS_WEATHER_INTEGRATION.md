# NOAA NWS API Integration Plan

## Current State: IEM/ASOS

**Issues:**
- Iowa Environmental Mesonet ASOS API frequently returns 503 errors
- Regional service (Iowa State University) not as reliable as federal sources
- No SLA or guaranteed uptime

**Usage:**
- Precipitation data for flood modeling
- Currently monitoring: PIA (Peoria), BMI (Bloomington), SPI (Springfield), ORD (Chicago), PWK (Wheeling), GBG (Galesburg)

## Proposed: NOAA NWS API

**Advantages:**
- Federal service with high uptime (>99.5%)
- Modern RESTful API with JSON responses
- No API key required (request User-Agent header only)
- Comprehensive weather data including precipitation, warnings, forecasts

**Base URL:** `https://api.weather.gov`

**Documentation:** https://www.weather.gov/documentation/services-web-api

### Relevant Endpoints

#### 1. Observation Stations by State/Zone
```
GET /stations?state=IL
```
Returns list of all observation stations in Illinois. Use to discover station IDs for our monitoring region.

#### 2. Latest Observation
```
GET /stations/{stationId}/observations/latest
```
Returns most recent observation including:
- `precipitationLastHour` (in meters, convert to inches)
- `temperature`
- `windSpeed`
- `timestamp`

Example for Peoria (KPIA):
```bash
curl -H "User-Agent: FloodMonitor/0.1 (flomon@example.com)" \
  https://api.weather.gov/stations/KPIA/observations/latest
```

Response excerpt:
```json
{
  "properties": {
    "timestamp": "2026-04-06T16:54:00+00:00",
    "temperature": {
      "value": 15.6,
      "unitCode": "wmoUnit:degC"
    },
    "precipitationLastHour": {
      "value": 0.0,
      "unitCode": "wmoUnit:m",
      "qualityControl": "qc:V"
    }
  }
}
```

#### 3. Observation History
```
GET /stations/{stationId}/observations?start={ISO8601}&end={ISO8601}
```
For backfilling gaps in precipitation data (limited to 7 days history).

### Implementation Plan

#### Phase 1: Add NWS Module (Parallel to ASOS)
- Create `flomon_service/src/ingest/nws.rs`
- Implement `fetch_nws_observation(station_id)` -> `WeatherObservation`
- Add to daemon polling loop alongside ASOS
- Compare data quality for 7 days before switching

#### Phase 2: Database Schema
```sql
-- Option A: Reuse existing asos_observations table (rename to weather_observations)
ALTER TABLE public.asos_observations RENAME TO weather_observations;
ALTER TABLE public.weather_observations ADD COLUMN source TEXT DEFAULT 'ASOS';
CREATE INDEX idx_weather_observations_source ON weather_observations(source);

-- Option B: New nws_observations table (if schema differs significantly)
CREATE TABLE public.nws_observations (
    id SERIAL PRIMARY KEY,
    station_id TEXT NOT NULL,
    observation_time TIMESTAMPTZ NOT NULL,
    temperature_c NUMERIC(5,2),
    precip_1hr_mm NUMERIC(6,2),  -- Store in mm, convert to inches in query
    wind_speed_ms NUMERIC(5,2),
    raw_json JSONB,  -- Store full response for debugging
    UNIQUE(station_id, observation_time)
);
```

#### Phase 3: Migrate Configuration
Update `iem_asos.toml` (or create `nws_stations.toml`):
```toml
[[stations]]
station_id = "KPIA"
location = "Peoria, IL"
relevance = "PRIMARY"
zone_assignments = [2]  # Upper Peoria Lake zone

[[stations]]
station_id = "KBMI"
location = "Bloomington, IL"
relevance = "UPSTREAM"
zone_assignments = [3]  # Mackinaw tributary zone
```

#### Phase 4: Update Endpoint Query
Modify `flomon_service/src/endpoint.rs` to query from new table/column:
```rust
let rows = client.query(
    "SELECT 
        CASE 
            WHEN precip_1hr_mm IS NOT NULL THEN precip_1hr_mm * 0.0393701  -- mm to inches
            ELSE precip_1hr_in  -- Fallback to old ASOS column
        END AS precip_1hr_in,
        observation_time
     FROM public.weather_observations
     WHERE station_id = $1
     ORDER BY observation_time DESC
     LIMIT 1",
    &[station_id]
)?;
```

#### Phase 5: Deprecate ASOS
- After 30 days of successful NWS operation, remove IEM/ASOS code
- Archive `iem_asos.toml` as `iem_asos.toml.deprecated`
- Update documentation to reflect NWS as primary weather source

### Code Skeleton

**`flomon_service/src/ingest/nws.rs`:**
```rust
use serde::Deserialize;
use chrono::{DateTime, Utc};

#[derive(Deserialize)]
struct NwsObservationResponse {
    properties: NwsProperties,
}

#[derive(Deserialize)]
struct NwsProperties {
    timestamp: String,
    #[serde(rename = "precipitationLastHour")]
    precipitation_last_hour: Option<NwsValue>,
    temperature: Option<NwsValue>,
}

#[derive(Deserialize)]
struct NwsValue {
    value: Option<f64>,
    #[serde(rename = "unitCode")]
    unit_code: String,
}

pub struct NwsObservation {
    pub station_id: String,
    pub observation_time: DateTime<Utc>,
    pub precip_1hr_mm: Option<f64>,
    pub temperature_c: Option<f64>,
}

pub fn fetch_nws_observation(station_id: &str) -> Result<NwsObservation, Box<dyn std::error::Error>> {
    let url = format!("https://api.weather.gov/stations/{}/observations/latest", station_id);
    
    let client = reqwest::blocking::Client::builder()
        .user_agent("FloodMonitor/0.1 (contact@example.com)")
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    
    let response = client.get(&url).send()?;
    
    if !response.status().is_success() {
        return Err(format!("NWS API returned status {}", response.status()).into());
    }
    
    let body = response.text()?;
    let data: NwsObservationResponse = serde_json::from_str(&body)?;
    
    let observation_time = DateTime::parse_from_rfc3339(&data.properties.timestamp)?
        .with_timezone(&Utc);
    
    let precip_1hr_mm = data.properties.precipitation_last_hour
        .and_then(|v| v.value)
        .map(|m| m * 1000.0);  // Convert meters to millimeters
    
    let temperature_c = data.properties.temperature
        .and_then(|v| v.value);
    
    Ok(NwsObservation {
        station_id: station_id.to_string(),
        observation_time,
        precip_1hr_mm,
        temperature_c,
    })
}
```

### Rate Limiting

NWS API guidelines:
- No hard rate limit enforced
- Recommended: <1000 requests/hour per IP
- Current load: 6 stations × 4 polls/hour = 24 requests/hour (well within limits)

### Error Handling

NWS API is highly reliable but can have temporary outages. Implement same retry strategy as USGS:
- 3 attempts with exponential backoff
- Gracefully degrade (skip precipitation sensors if unavailable)
- Alert if >24 hours without successful weather data

### Testing Plan

1. **Endpoint Validation**: Verify each station ID returns data
   ```bash
   for station in KPIA KBMI KSPI KORD KPWK KGBG; do
       echo "Testing $station..."
       curl -s -H "User-Agent: Test/1.0" \
           "https://api.weather.gov/stations/$station/observations/latest" \
           | jq '.properties.precipitationLastHour'
   done
   ```

2. **Integration Test**: Add to `tests/data_source_integration.rs`
   ```rust
   #[test]
   fn nws_api_returns_recent_observation() {
       let obs = fetch_nws_observation("KPIA").unwrap();
       assert!(obs.observation_time > Utc::now() - Duration::hours(1));
   }
   ```

3. **Comparison Test**: Run both ASOS and NWS for 7 days, compare:
   - Data freshness (latency)
   - Uptime (successful polls %)
   - Precipitation values (should be identical)

### Migration Timeline

- **Week 1**: Implement NWS module, add to daemon (parallel)
- **Week 2-3**: Monitor data quality, resolve any issues
- **Week 4**: Switch zones.toml to use NWS stations, keep ASOS as fallback
- **Week 5**: Remove ASOS polling if NWS stable
- **Week 6**: Clean up code, update documentation

### Rollback Plan

If NWS API proves unreliable:
1. Revert zones.toml to ASOS station_ids
2. Re-enable IEM ASOS polling in daemon
3. Consider hybrid approach (both NWS and ASOS, use whichever is fresher)

## References

- NWS API Docs: https://www.weather.gov/documentation/services-web-api
- NWS API Specification: https://api.weather.gov/openapi.json
- Station List: https://api.weather.gov/stations?state=IL
- Example Station: https://api.weather.gov/stations/KPIA/observations/latest
