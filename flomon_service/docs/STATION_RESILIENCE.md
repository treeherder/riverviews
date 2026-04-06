# Station Resilience Strategy

## Problem Statement

USGS gauge stations can go offline temporarily or permanently due to:
- Equipment maintenance
- Communication failures  
- Extreme flood events (gauge overflow/damage)
- Ice conditions
- Decommissioning

Our system must remain operational when individual stations fail without requiring manual intervention.


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
2. **Automatic Backfill**: When station recovers, fetch missed data ✅ *Implemented in 007_backfill_tracking.sql*
3. **Redundant Stations**: Define backup stations for critical locations
4. **Parameter Fallbacks**: Estimate missing values from nearby stations
5. **USGS Status API**: Query site status instead of waiting for failures

## Network Resilience

### Timeout and Retry Strategy

**Implemented Mitigations:**
- **Increased Timeout**: 45 seconds (up from 15s) to handle slow network paths
- **Exponential Backoff Retry**: 3 attempts with 1s, 2s, 4s delays between retries
- **Daily Values Fallback**: If instantaneous values endpoint times out, fall back to daily values endpoint (different server infrastructure)
- **Thread Pool Parallelization**: 8 worker threads for parallel polling (prevents one timeout from blocking others)

**Configuration:**
```bash
# Set worker count (default: 8)
export POLL_WORKERS=12
```

### Cloud Deployment Network Issues

**Symptom**: When deployed on GCE (Google Cloud Engine), USGS NWIS API requests frequently timeout even though the same requests work from local development machines.

**Root Causes:**
1. **Route Inefficiency**: Cloud provider networks may take suboptimal routes to USGS servers
2. **Rate Limiting**: USGS may rate-limit requests from cloud IP ranges more aggressively
3. **Firewall/NAT Overhead**: Cloud egress through NAT gateways adds latency
4. **Geographic Routing**: USGS servers may prioritize requests from certain geographic regions

**Diagnostic Commands:**
```bash
# Test from GCE VM
time curl -w "\nTotal: %{time_total}s\n" \
  "https://waterservices.usgs.gov/nwis/iv/?sites=05568500&period=PT1H&format=json"

# Compare with local machine
# If GCE times are >10s and local <2s, network path is the issue

# Traceroute to identify bottleneck
traceroute waterservices.usgs.gov
```

### VPN/Proxy Solution (Future Enhancement)

If network timeouts persist after implementing retry logic and fallbacks, consider:

**Option 1: Cloud VPN to Residential/University Network**
- Deploy lightweight VPN client on GCE VM
- Route USGS requests through a residential ISP connection
- Bypasses cloud-specific routing/rate-limiting issues
- **Trade-off**: Adds VPN maintenance complexity

**Option 2: HTTP Proxy Service**
- Use a proxy service (e.g., ScraperAPI, Bright Data) with residential IPs
- Only for USGS requests (CWMS and ASOS stay direct)
- **Trade-off**: Monthly cost (~$30-100), additional latency

**Option 3: Multi-Region Fallback**
- Deploy daemon in multiple cloud regions (us-central, us-east, us-west)
- Primary daemon in us-central1 (closest to Illinois)
- On timeout, failover to alternate region
- **Trade-off**: Higher infrastructure cost

**Recommendation**: 
1. Monitor station_health table for persistent timeout patterns
2. If >50% of polls timeout for >7 days, implement Option 1 (VPN)
3. VPN setup: WireGuard on home Raspberry Pi, GCE client connects, route only USGS traffic

**Implementation Sketch:**
```bash
# On home network (Raspberry Pi / spare machine)
sudo apt install wireguard
wg genkey | tee privatekey | wg pubkey > publickey
# Configure WireGuard server (10.0.0.1/24)

# On GCE VM
sudo apt install wireguard
# Add WireGuard client config
# Route 52.35.0.0/16 (USGS IP range) through VPN tunnel
sudo ip route add 52.35.0.0/16 via 10.0.0.1 dev wg0
```

**Query station_health for timeout analysis:**
```sql
-- Check which stations have persistent timeouts
SELECT 
    station_id,
    consecutive_failures,
    last_error,
    last_successful_poll,
    NOW() - last_successful_poll AS time_since_success
FROM public.station_health
WHERE source_type = 'USGS'
  AND consecutive_failures > 5
ORDER BY consecutive_failures DESC;
```

## References

- USGS Site Status: https://waterdata.usgs.gov/nwis/inventory
- NWS Site Maintenance: https://water.noaa.gov/about/
- Station Registry: `src/stations.rs`
- Integration Tests: `stations.rs::integration_tests`
- Backfill Tracking: `sql/007_backfill_tracking.sql`
- WireGuard VPN: https://www.wireguard.com/quickstart/
