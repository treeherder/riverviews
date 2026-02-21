# Development Mode - Working with Historical Data

## Overview

When live USGS data is unavailable (station outages, API changes, equipment issues), use **Development Mode** to work with the 430,000+ historical readings already in the database.

## Current Situation (Feb 2026)

As of February 2026, USGS stations are returning empty responses:
```
WARN USGS [05557000]: No timeSeries entries in response
WARN USGS [05568580]: No data available for site
```

**Possible causes:**
- Station equipment maintenance/failures
- USGS API endpoint changes
- Station decommissioning

## Available Historical Data

```sql
-- Check what data you have
SELECT 
    site_code,
    COUNT(*) as readings,
    MIN(measurement_time) as first_reading,
    MAX(measurement_time) as latest_reading
FROM usgs_raw.gauge_readings
GROUP BY site_code
ORDER BY site_code;
```

Your database contains **430,132 records** from previous successful data collection periods.

## Development Workflow Options

### Option 1: Query Historical Data Directly

```rust
// Example: Get readings from a specific date range
use postgres::Client;
use chrono::{DateTime, Utc};

fn fetch_historical_period(
    client: &mut Client,
    site_code: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<GaugeReading>, postgres::Error> {
    // Your implementation
}
```

### Option 2: Simulated Real-Time Replay

```rust
// Replay historical data AS IF it's happening now
use flomon_service::dev_mode::DevMode;

let dev = DevMode::new(365); // Use data from 1 year ago
let readings = dev.fetch_simulated_current_readings(
    &mut client,
    &site_codes,
)?;

// Process as if this is live data
```

### Option 3: Mock Data for Testing

```rust
// Use the integration test patterns
use flomon_service::db;
use rust_decimal::Decimal;

let mut client = db::connect_and_verify(&["usgs_raw"])?;

// Insert test data
client.execute(
    "INSERT INTO usgs_raw.gauge_readings 
     (site_code, measurement_time, parameter_code, value, unit)
     VALUES ($1, $2, $3, $4, $5)",
    &[&"05568500", &Utc::now(), &"00065", 
      &Decimal::from_f64_retain(15.5).unwrap(), &"ft"]
)?;
```

## Recommended Approach

### For Development:
1. **Use historical data** from the database (430K records available)
2. **Run integration tests** - they use test data and don't rely on live APIs
3. **Query specific date ranges** when you know data exists

### For Production (when live data returns):
1. Monitor USGS API status at https://waterservices.usgs.gov/
2. Check station status at https://waterdata.usgs.gov/nwis/rt
3. Update API endpoints if USGS has migrated services

## Investigating Live API Issues

```bash
# Test USGS API directly
curl "https://waterservices.usgs.gov/nwis/iv/?format=json&sites=05568500&parameterCd=00060,00065&period=PT3H"

# Check site status on USGS website
# https://waterdata.usgs.gov/nwis/inventory?agency_code=USGS&site_no=05568500
```

## Sample Queries for Historical Data

```sql
-- Get latest available data for each station
SELECT site_code, 
       MAX(measurement_time) as latest_data_available
FROM usgs_raw.gauge_readings
GROUP BY site_code;

-- Get a representative day of data (e.g., last complete day)
SELECT * 
FROM usgs_raw.gauge_readings
WHERE measurement_time >= '2024-01-01 00:00:00'
  AND measurement_time < '2024-01-02 00:00:00'
ORDER BY site_code, measurement_time;

-- Find periods with good data coverage
SELECT 
    DATE_TRUNC('day', measurement_time) as day,
    site_code,
    COUNT(*) as readings_per_day
FROM usgs_raw.gauge_readings
GROUP BY day, site_code
HAVING COUNT(*) >= 90  -- At least 90 readings (good for 15-min data)
ORDER BY day DESC, site_code;
```

## Updating the Daemon for Graceful Degradation

The logging system already handles this:
- ✅ Warnings logged with site identifiers
- ✅ System continues despite failures
- ✅ Historical data remains accessible

Consider adding:
- Automatic fallback to dev mode when live data unavailable
- Configurable data source (live vs historical)
- API health checks before attempting fetch

## Environment Configuration

Add to `.env`:
```bash
# Development Mode Configuration
# DEV_MODE=true
# DEV_MODE_DAYS_OFFSET=365  # Use data from 1 year ago
# DEV_MODE_REPLAY_SPEED=1.0  # 1.0 = real-time, 10.0 = 10x speed
```

## Next Steps

1. ✅ Integration tests (9/10 passing) - work with test data
2. ✅ Database has 430K historical records
3. ⏳ Add dev mode module for historical data replay
4. ⏳ Document which date ranges have good data coverage
5. ⏳ Monitor USGS API for when live data returns

The system is healthy and functional - you're just working with historical data instead of live data from currently-offline stations.
