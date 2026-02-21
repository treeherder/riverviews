# Extensible Data Source Architecture

## Overview

The flood monitoring service now has a proven, extensible architecture for integrating multiple data sources. The system currently supports **USGS gauges** and **CWMS locations**, with clear patterns for adding future sources like NWS forecasts, NOAA precipitation, etc.

## Architecture Pattern

### 1. Data Source Components

Each data source follows this structure:

```
src/ingest/{source}.rs     - API client & data fetching
src/daemon.rs              - Polling, backfill, warehousing
sql/00X_{source}.sql       - Database schema
src/endpoint.rs            - Endpoint data integration
```

### 2. Implementation Checklist

For each new data source, implement:

- [ ] **API Client** (`src/ingest/{source}.rs`)
  - HTTP client functions
  - Response parsing (JSON/XML/CSV)
  - Error handling
  - Data model structs

- [ ] **Database Schema** (`sql/00X_{source}.sql`)
  - Raw data table (timeseries/readings)
  - Metadata table (locations/stations)
  - Ingestion tracking/logging
  - Indexes for performance

- [ ] **Daemon Integration** (`src/daemon.rs`)
  - Location/station loading
  - Staleness checking
  - Polling (recent data, every 15min)
  - Backfill (historical data)
  - Warehousing (INSERT with ON CONFLICT DO NOTHING)

- [ ] **Startup Integration** (`src/main.rs`)
  - Load locations/stations
  - Check staleness
  - Backfill if needed
  - Add to main polling loop

- [ ] **Endpoint Integration** (`src/endpoint.rs`)
  - Response data structures
  - Database queries
  - Include in site response

## Current Implementation

### USGS Integration ✅ Complete

**Components:**
- `src/ingest/usgs.rs` - IV/DV API clients
- `usgs_raw.gauge_readings` table (430,074 records)
- `usgs_raw.stations` metadata
- 8 monitored stations
- 15-minute polling interval
- 120-day IV backfill + historical DV

**Pattern:**
```rust
// Daemon methods
fn poll_station(&mut self, site_code: &str) -> Result<Vec<GaugeReading>, Error>
fn backfill_station(&mut self, site_code: &str) -> Result<usize, Error>
fn warehouse_readings(&mut self, readings: &[GaugeReading]) -> Result<usize, Error>
fn check_staleness(&mut self, site_code: &str) -> Result<Option<Duration>, Error>
```

### CWMS Integration ✅ Complete

**Components:**
- `src/ingest/cwms.rs` - CWMS API client
- `usace.cwms_timeseries` table (0 records - pending valid data)
- `usace.cwms_locations` metadata (7 locations)
- 7 monitored locations (4 Mississippi, 3 Illinois)
- 15-minute polling interval
- 120-day backfill

**Pattern:**
```rust
// Daemon methods
fn poll_cwms_location(&mut self, location: &CwmsLocation) -> Result<usize, Error>
fn backfill_cwms_location(&mut self, location: &CwmsLocation) -> Result<usize, Error>
fn warehouse_cwms_timeseries(&mut self, timeseries: &[CwmsTimeseries]) -> Result<usize, Error>
fn check_cwms_staleness(&mut self, location_id: &str) -> Result<Option<Duration>, Error>
```

**Endpoint Integration:**
```json
{
  "cwms_context": {
    "mississippi_river_locations": [...],
    "illinois_river_locations": [...],
    "backwater_risk": null
  }
}
```

## Adding New Data Sources

### Example: NWS Flood Forecasts

**Step 1: API Client** (`src/ingest/nws.rs`)
```rust
pub struct NwsForecast {
    pub location_id: String,
    pub forecast_time: DateTime<Utc>,
    pub valid_time: DateTime<Utc>,
    pub stage_ft: f64,
    pub severity: String,
}

pub fn fetch_forecasts(
    client: &reqwest::blocking::Client,
    location_id: &str,
) -> Result<Vec<NwsForecast>, Box<dyn Error>> {
    // Implement NWS API calls
}
```

**Step 2: Database Schema** (`sql/006_nws_forecasts.sql`)
```sql
CREATE SCHEMA IF NOT EXISTS nws;

CREATE TABLE nws.forecast_locations (
    location_id TEXT PRIMARY KEY,
    location_name TEXT NOT NULL,
    river_name TEXT NOT NULL,
    monitored BOOLEAN DEFAULT true
);

CREATE TABLE nws.forecasts (
    location_id TEXT NOT NULL,
    forecast_time TIMESTAMPTZ NOT NULL,
    valid_time TIMESTAMPTZ NOT NULL,
    stage_ft NUMERIC(10,2),
    severity TEXT,
    PRIMARY KEY (location_id, forecast_time, valid_time)
);

CREATE INDEX ON nws.forecasts (location_id, valid_time);
```

**Step 3: Daemon Integration** (`src/daemon.rs`)
```rust
#[derive(Debug, Clone)]
pub struct NwsLocation {
    pub location_id: String,
    pub location_name: String,
    pub river_name: String,
}

pub struct Daemon {
    // ... existing fields
    nws_locations: Vec<NwsLocation>,
}

impl Daemon {
    fn load_nws_locations(&mut self) -> Result<(), Box<dyn Error>> {
        let rows = self.client.as_mut().unwrap().query(
            "SELECT location_id, location_name, river_name 
             FROM nws.forecast_locations WHERE monitored = true",
            &[]
        )?;
        
        self.nws_locations = rows.iter().map(|row| NwsLocation {
            location_id: row.get(0),
            location_name: row.get(1),
            river_name: row.get(2),
        }).collect();
        
        Ok(())
    }
    
    pub fn poll_nws_location(&mut self, location: &NwsLocation) -> Result<usize, Error> {
        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        
        let forecasts = nws::fetch_forecasts(&http_client, &location.location_id)?;
        self.warehouse_nws_forecasts(&forecasts)
    }
    
    fn warehouse_nws_forecasts(&mut self, forecasts: &[NwsForecast]) -> Result<usize, Error> {
        let client = self.client.as_mut().ok_or("Not initialized")?;
        let mut inserted = 0;
        
        for forecast in forecasts {
            let rows = client.execute(
                "INSERT INTO nws.forecasts 
                 (location_id, forecast_time, valid_time, stage_ft, severity)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (location_id, forecast_time, valid_time) DO NOTHING",
                &[&forecast.location_id, &forecast.forecast_time, 
                  &forecast.valid_time, &forecast.stage_ft, &forecast.severity]
            )?;
            inserted += rows as usize;
        }
        
        Ok(inserted)
    }
}
```

**Step 4: Main Loop Integration** (`src/main.rs`)
```rust
// Check NWS forecast freshness
println!("📋 Checking NWS forecast data...");
let nws_locations = daemon.get_nws_locations().to_vec();

for location in &nws_locations {
    match daemon.check_nws_staleness(&location.location_id) {
        Ok(None) => {
            println!("   {} - No forecasts (fetching)", location.location_name);
            daemon.poll_nws_location(location)?;
        }
        Ok(Some(staleness)) if staleness.num_hours() > 6 => {
            println!("   {} - Stale forecasts (refreshing)", location.location_name);
            daemon.poll_nws_location(location)?;
        }
        _ => {}
    }
}
```

**Step 5: Polling Loop** (`src/daemon.rs::poll_all_stations`)
```rust
// Poll NWS forecasts (every 6 hours, not every 15min)
if (Utc::now().timestamp() % (6 * 3600)) < 900 {  // First 15min of 6-hour period
    for location in &self.nws_locations.clone() {
        match self.poll_nws_location(&location) {
            Ok(inserted) => {
                results.insert(format!("NWS:{}", location.location_name), inserted);
            }
            Err(e) => {
                eprintln!("Failed to poll NWS {}: {}", location.location_name, e);
                results.insert(format!("NWS:{}", location.location_name), 0);
            }
        }
    }
}
```

**Step 6: Endpoint Integration** (`src/endpoint.rs`)
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SiteDataResponse {
    // ... existing fields
    pub nws_forecasts: Option<Vec<NwsForecastData>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NwsForecastData {
    pub valid_time: String,
    pub stage_ft: f64,
    pub severity: String,
}

fn fetch_nws_forecasts(client: &mut Client, site_code: &str) -> Result<Vec<NwsForecastData>, String> {
    let rows = client.query(
        "SELECT valid_time, stage_ft, severity
         FROM nws.forecasts
         WHERE location_id = (
             SELECT nws_location_id FROM usgs_raw.stations WHERE site_code = $1
         )
         AND valid_time > NOW()
         ORDER BY valid_time",
        &[&site_code]
    ).map_err(|e| format!("NWS query failed: {}", e))?;
    
    Ok(rows.iter().map(|row| NwsForecastData {
        valid_time: row.get::<_, DateTime<Utc>>(0).to_rfc3339(),
        stage_ft: row.get::<_, rust_decimal::Decimal>(1).to_string().parse().unwrap_or(0.0),
        severity: row.get(2),
    }).collect())
}
```

## Design Principles

### 1. Idempotent Operations
All warehousing uses `ON CONFLICT DO NOTHING` to safely handle:
- Service restarts
- Overlapping time windows
- Backfill and polling the same data
- Multiple daemon instances (future)

### 2. Graceful Degradation
- Data source failures don't crash the daemon
- Failed polls retry on next interval
- Missing data returns empty results, not errors
- Endpoint continues serving available data

### 3. Staleness Detection
Every data source tracks latest timestamp:
```sql
SELECT MAX(timestamp) FROM {source}.{table} WHERE location_id = $1
```

If stale (>2 hours), automatic backfill triggers.

### 4. Separate Schemas
Each data source gets its own PostgreSQL schema:
- `usgs_raw` - USGS gauge readings
- `nws` - NWS flood forecasts
- `usace` - CWMS lock/dam data
- `noaa` - NOAA precipitation (future)
- `flood_analysis` - Cross-source analytics

### 5. Configuration-Driven
Locations/stations defined in database, not code:
```sql
UPDATE usgs_raw.stations SET monitored = false WHERE site_code = '12345678';
INSERT INTO nws.forecast_locations (...) VALUES (...);
```

No code changes needed to add/remove monitoring.

## Data Flow Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     External APIs                        │
├──────────┬──────────┬──────────┬──────────┬─────────────┤
│   USGS   │   CWMS   │   NWS    │   NOAA   │  (Future)   │
│   APIs   │   API    │   API    │   API    │   Sources   │
└─────┬────┴────┬─────┴────┬─────┴────┬─────┴─────┬───────┘
      │         │          │          │           │
      │ Poll    │ Poll     │ Poll     │ Poll      │ Poll
      │ 15min   │ 15min    │ 6hr      │ 1hr       │ ...
      │         │          │          │           │
┌─────▼─────────▼──────────▼──────────▼───────────▼───────┐
│                   Daemon (Rust)                          │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐        │
│  │ Staleness  │  │   Backfill │  │  Warehouse │        │
│  │  Detection │  │   Missing  │  │   INSERT   │        │
│  └────────────┘  └────────────┘  └────────────┘        │
└───────────────────────┬──────────────────────────────────┘
                        │
                        │ Write
                        ▼
            ┌───────────────────────┐
            │   PostgreSQL          │
            ├───────────────────────┤
            │ usgs_raw schema       │
            │ usace schema          │
            │ nws schema            │
            │ noaa schema           │
            │ flood_analysis schema │
            └───────┬───────────────┘
                    │
      ┌─────────────┼─────────────┐
      │ Read        │ Read        │ Read
      ▼             ▼             ▼
┌──────────┐  ┌──────────┐  ┌──────────┐
│ HTTP     │  │ Python   │  │ Analysis │
│ Endpoint │  │ Scripts  │  │ Tools    │
│ (8080)   │  │          │  │          │
└──────────┘  └──────────┘  └──────────┘
```

## Current Status

### Implemented ✅
- USGS: 8 stations, 430K+ readings, full backfill, continuous polling
- CWMS: 7 locations, infrastructure complete, pending valid API data
- HTTP endpoint: Comprehensive site data with CWMS context
- Staleness detection: Both USGS and CWMS
- Auto-backfill: Startup checks + gap detection
- Idempotent warehousing: Safe restarts/overlaps
- Error handling: Graceful degradation per source
- Extensible patterns: Clear template for new sources

### Next Steps 🔄
- Validate CWMS timeseries names with actual API
- Add NWS flood forecast ingestion
- Add NOAA precipitation data
- Implement backwater risk calculation
- Add cross-source flood event correlation
- Multi-source analytics in `flood_analysis` schema

## Performance Considerations

### Polling Intervals
- **USGS IV**: 15 minutes (API updates every 15-60min)
- **CWMS**: 15 minutes (API updates every 15-60min)
- **NWS forecasts**: 6 hours (forecasts update 2-4x/day)
- **NOAA precip**: 1 hour (hourly observations)

### Database Growth
- **USGS**: ~11,500 readings/station/120 days = ~92K/station
- **CWMS**: ~11,500 readings/location/120 days = ~80K total
- **Total**: ~500K readings/120 days
- **Retention**: Keep full timeseries, prune after analysis

### Indexing Strategy
```sql
-- Timeseries queries (recent data)
CREATE INDEX ON {schema}.{table} (location_id, timestamp DESC);

-- Staleness checks (latest reading)
CREATE INDEX ON {schema}.{table} (location_id) WHERE timestamp > NOW() - INTERVAL '7 days';

-- Analytics (flood events)
CREATE INDEX ON {schema}.{table} (timestamp) WHERE value > threshold;
```

## Error Handling Patterns

### Network Failures
```rust
match http_client.get(&url).send() {
    Ok(response) => { /* process */ },
    Err(e) => {
        eprintln!("Network error for {}: {}", location, e);
        // Continue to next location
        return Ok(0);  // No new readings
    }
}
```

### API Errors (404, 500, etc.)
```rust
if !response.status().is_success() {
    return Err(format!("API error: {}", response.status()).into());
}
// Caller handles error, continues polling other locations
```

### Database Errors
```rust
client.execute(...).map_err(|e| {
    eprintln!("Database error: {}", e);
    // Don't crash daemon, report 0 insertions
})?;
```

### Invalid Data
```rust
let value_decimal = Decimal::from_f64_retain(reading.value)
    .ok_or_else(|| format!("Invalid value: {}", reading.value))?;
```

## Testing Strategy

### Unit Tests
- API response parsing
- Data model conversions
- URL construction
- Error handling

### Integration Tests
- Database round-trip (insert + query)
- Idempotent warehousing
- Staleness calculation
- Endpoint JSON format

### System Tests
- Full backfill (limited time range)
- Polling loop (1-2 iterations)
- Graceful degradation (mock API failures)
- Multi-source polling

## Monitoring & Observability

### Daemon Logs
```
✓ Poll complete: 45 new readings (8 USGS stations, 7 CWMS locations)
Failed to poll CWMS Mississippi River at Grafton: 404 Not Found
```

### Database Queries
```sql
-- Check data freshness per source
SELECT 
    'USGS' as source,
    site_code as location,
    MAX(reading_time) as latest,
    NOW() - MAX(reading_time) as staleness
FROM usgs_raw.gauge_readings
GROUP BY site_code;

-- Check ingestion rates
SELECT 
    DATE_TRUNC('hour', reading_time) as hour,
    COUNT(*) as readings
FROM usgs_raw.gauge_readings
WHERE reading_time > NOW() - INTERVAL '24 hours'
GROUP BY hour
ORDER BY hour;
```

### Endpoint Metrics
- Response time per site
- Data availability percentage
- Staleness by source
- Error rates per source

## Conclusion

The flood monitoring service now has a proven, extensible architecture for multi-source data ingestion. The USGS and CWMS implementations provide clear patterns for adding NWS, NOAA, and future sources with minimal code changes and maximum reliability.

**Key Success Factors:**
1. Separate schemas per source
2. Idempotent warehousing
3. Graceful error handling
4. Staleness-driven backfill
5. Configuration over code
6. Single responsibility per function

This architecture supports the long-term goal of comprehensive flood prediction by combining real-time observations (USGS/CWMS), forecasts (NWS), precipitation (NOAA), and advanced analytics (Python/R).
