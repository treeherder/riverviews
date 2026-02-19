# Station Resilience Strategy

## Problem Statement

USGS gauge stations can go offline temporarily or permanently due to:
- Equipment maintenance
- Communication failures  
- Extreme flood events (gauge overflow/damage)
- Ice conditions
- Decommissioning

Our system must remain operational when individual stations fail without requiring manual intervention.

## Design Principles

1. **Continue Operating**: One offline station should not crash the entire service
2. **Graceful Degradation**: Reduce functionality rather than fail completely
3. **Visibility**: Log warnings but don't spam errors for known-offline stations
4. **Recovery**: Automatically resume when stations come back online
5. **Validation**: Know which stations are expected to work before deployment

## Implementation Strategies

### 1. Station Metadata - Expected Parameters

Each station in `STATION_REGISTRY` declares which parameters it's expected to provide:

```rust
Station {
    site_code: "05568500",
    name: "Illinois River at Kingston Mines, IL",
    expected_parameters: &[PARAM_DISCHARGE, PARAM_STAGE],
    // ...
}
```

This allows the system to:
- Request only available parameters per station
- Detect when expected parameters go missing
- Distinguish between "station offline" vs "doesn't have this parameter"

### 2. API Request Handling - Partial Success

When fetching data from multiple stations:

```rust
// Instead of:
let readings = fetch_all_stations()?;  // Fails if ANY station fails

// Use:
let readings = fetch_all_stations_partial();  // Returns available data, logs failures
```

The `parse_iv_response()` function already handles this:
- USGS API returns empty arrays for offline stations
- Parser skips empty entries and continues
- Returns `NoDataAvailable` only if ALL stations fail

### 3. Database Inserts - Skip Missing Data

Historical ingestion uses `ON CONFLICT DO NOTHING`:

```sql
INSERT INTO gauge_readings (...) VALUES (...)
ON CONFLICT (site_code, parameter_code, reading_time) DO NOTHING
```

This means:
- Missing stations simply don't insert rows
- No error thrown for gaps
- Database remains consistent

### 4. Integration Tests - Manual Verification

Tests marked `#[ignore]` verify stations before deployment:

```bash
# Check all stations return expected data
cargo test --ignored station_api_verify_all_registry_stations

# Check specific station
cargo test --ignored station_api_kingston_mines
```

Output shows:
- ✓ Which stations are online
- ⚠️ Which parameters are missing
- ❌ Which stations are completely offline

**Run these tests:**
- Before deploying new station additions
- Monthly to detect decommissioned stations
- After USGS announces maintenance

### 5. Monitoring & Alerting (Future)

```rust
// Track station availability over time
struct StationHealth {
    site_code: String,
    last_successful_reading: DateTime<Utc>,
    consecutive_failures: u32,
    expected_parameters: Vec<String>,
    missing_parameters: Vec<String>,
}
```

Alert when:
- Station offline > 4 hours (likely maintenance)
- Station offline > 24 hours (investigate)
- Expected parameter missing (configuration drift)

### 6. Graceful Service Degradation

For real-time monitoring:

```rust
// Instead of requiring ALL stations:
let critical_sites = ["05568500", "05568000"];  // Kingston Mines, Chillicothe

if available_readings.contains_critical_sites(&critical_sites) {
    // Can issue flood warnings
} else {
    // Log degraded mode, skip alerting
}
```

## Operational Procedures

### When a Station Goes Offline

1. **Automatic Response**: System continues with remaining stations
2. **Detection**: Monitor logs for repeated failures
3. **Investigation**: Check USGS site status page
4. **Communication**: If permanent, update stakeholders

### When Adding New Stations

1. **Test First**: Run `cargo test --ignored station_api_verify`
2. **Verify Parameters**: Confirm expected_parameters match reality
3. **Document**: Add to STATION_REGISTRY with description
4. **Deploy**: Changes take effect on next ingestion run

### When Removing Stations

1. **Mark Deprecated**: Comment out in STATION_REGISTRY
2. **Database**: Historical data remains (don't delete)
3. **Monitor**: Verify no code depends on removed station
4. **Document**: Note decommission date and reason

## Testing Checklist

Before production deployment:

- [ ] `cargo test` - All unit tests pass
- [ ] `cargo test --ignored station_api_verify_all` - All stations online
- [ ] Check no warnings about missing parameters
- [ ] Verify critical stations (Kingston Mines, Chillicothe) operational
- [ ] Test with one station manually removed (graceful degradation)

## Example Failure Scenarios

### Scenario 1: Temporary Communication Failure

**Symptom**: API returns empty array for one station  
**Effect**: Parser skips that station, continues with others  
**Recovery**: Automatic on next fetch attempt  

### Scenario 2: Equipment Maintenance (24-48h)

**Symptom**: Station offline for extended period  
**Effect**: Database has gap for that station, others unaffected  
**Recovery**: Automatic when maintenance complete  

### Scenario 3: Permanent Decommissioning

**Symptom**: Station never returns data  
**Effect**: Integration test fails, logs show persistent errors  
**Action**: Remove from STATION_REGISTRY, document in changelog  

### Scenario 4: Parameter Removed by USGS

**Symptom**: Station returns discharge but not stage  
**Effect**: Only one parameter stored, integration test warns  
**Action**: Update `expected_parameters` in registry  

## Future Enhancements

1. **Station Health Dashboard**: Real-time availability display
2. **Automatic Backfill**: When station recovers, fetch missed data
3. **Redundant Stations**: Define backup stations for critical locations
4. **Parameter Fallbacks**: Estimate missing values from nearby stations
5. **USGS Status API**: Query site status instead of waiting for failures

## References

- USGS Site Status: https://waterdata.usgs.gov/nwis/inventory
- NWS Site Maintenance: https://water.noaa.gov/about/
- Station Registry: `src/stations.rs`
- Integration Tests: `stations.rs::integration_tests`
