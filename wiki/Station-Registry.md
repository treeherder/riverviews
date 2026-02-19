# Station Registry

FloPro monitors **8 USGS gauge stations** across the Illinois River system, providing real-time flood monitoring with upstream early warning capability.

## Monitored Stations

### Overview Table

| Site Code | Station Name | Status | Parameters | Lead Time to Peoria |
|-----------|--------------|--------|------------|---------------------|
| 05568500 | Illinois River at Kingston Mines, IL | ✅ Active | Discharge + Stage | Downstream (0 hrs) |
| 05567500 | Illinois River at Peoria, IL | ✅ Active | Discharge + Stage | At Peoria (0 hrs) |
| 05568000 | Illinois River at Chillicothe, IL | ✅ Active | Discharge + Stage | 6-12 hours |
| 05570000 | Spoon River at Seville, IL | ✅ Active | Discharge + Stage | Tributary |
| 05552500 | Illinois River at Marseilles, IL | ✅ Active | Discharge + Stage | 24-36 hours |
| 05536890 | Chicago Sanitary & Ship Canal at Romeoville, IL | ✅ Active | Discharge + Stage | Chicago inflow |
| 05557000 | Illinois River at Henry, IL | ❌ Offline | — | — |
| 05568580 | Mackinaw River near Green Valley, IL | ❌ Offline | — | — |

**Status as of:** February 19, 2026  
**Verification:** Integration test `station_api_verify_all_registry_stations`

---

## Critical Stations (20-min staleness threshold)

### 05568500 - Illinois River at Kingston Mines, IL

**Classification:** Primary Downstream Gauge

**Location:**
- Latitude: 40.556139° N
- Longitude: -89.778722° W
- River Mile: ~157 (above Mississippi confluence)

**Significance:**
- **Primary reference gauge** for Peoria flood impacts
- Located just downstream of Peoria Lock & Dam
- Direct measurement of flood levels affecting Kingston Mines and South Peoria
- Historical data from **October 1939** (87 years)

**Expected Parameters:**
- `00060` - Discharge (ft³/s)
- `00065` - Gage height (ft)

**Staleness Threshold:** 20 minutes

**Why Critical:**
- Closest official gauge to downtown Peoria
- Used for NWS flood forecasts
- Determines flood stage exceedance
- Property damage correlation

**USGS Station Page:** https://waterdata.usgs.gov/nwis/inventory?site_no=05568500

---

### 05567500 - Illinois River at Peoria, IL

**Classification:** Peoria Pool Gauge

**Location:**
- Latitude: 40.691944° N
- Longitude: -89.588889° W
- Just above Peoria Lock & Dam

**Significance:**
- Measures pool level in Peoria navigation pool
- Upstream of Kingston Mines
- Complements Kingston Mines readings
- Can detect flood stage earlier than Kingston Mines in some events

**Expected Parameters:**
- `00060` - Discharge (ft³/s)
- `00065` - Gage height (ft)

**Staleness Threshold:** 20 minutes

**Why Critical:**
- Located within Peoria city limits
- Pool level indicator
- Validates Kingston Mines readings
- Early indication of local runoff

---

### 05568000 - Illinois River at Chillicothe, IL

**Classification:** Upstream Early Warning

**Location:**
- Latitude: 40.921389° N
- Longitude: -89.476111° W
- ~20 river miles upstream of Peoria

**Significance:**
- **6-12 hour lead time** to Peoria
- Upstream of Peoria pool
- Monitors flow entering Peoria reach
- Historical comparison for flood frequency analysis

**Expected Parameters:**
- `00060` - Discharge (ft³/s)
- `00065` - Gage height (ft)

**Staleness Threshold:** 20 minutes

**Why Critical:**
- First warning point for upstream floods
- Allows time to prepare flood mitigation
- Validates flow predictions
- Critical for Corps of Engineers dam operations

---

## Normal Monitoring Stations (60-min staleness threshold)

### 05570000 - Spoon River at Seville, IL

**Classification:** Tributary Monitoring

**Location:**
- Latitude: 40.481667° N
- Longitude: -90.344167° W
- Western tributary to Illinois River

**Significance:**
- Monitors major western tributary
- Detects local flash flooding
- Independent rainfall runoff indicator
- Can contribute 10-20% of Peoria discharge during heavy rain

**Expected Parameters:**
- `00060` - Discharge (ft³/s)
- `00065` - Gage height (ft)

**Staleness Threshold:** 60 minutes

**Flood Contribution:**
- Normal flow: 500-2,000 cfs
- Flood flow: 10,000-20,000 cfs
- Peak contribution (1993 flood): 32,000 cfs

---

### 05552500 - Illinois River at Marseilles, IL

**Classification:** Major Upstream Reference

**Location:**
- Latitude: 41.332222° N
- Longitude: -88.706944° W
- ~100 river miles upstream of Peoria
- Just below confluence of Kankakee and Des Plaines rivers

**Significance:**
- **24-36 hour lead time** to Peoria
- Monitors combined flow from Chicago metro area and Kankakee River
- Long-term historical record
- Critical for multi-day flood forecasting

**Expected Parameters:**
- `00060` - Discharge (ft³/s)
- `00065` - Gage height (ft)

**Staleness Threshold:** 60 minutes

**Why Important:**
- Earliest upstream warning point in our network
- Captures Chicago-area runoff
- Validates NWS river forecasts
- Supports return interval analysis

---

### 05536890 - Chicago Sanitary & Ship Canal at Romeoville, IL

**Classification:** Chicago Metro Inflow Monitoring

**Location:**
- Latitude: 41.6367° N
- Longitude: -88.0920° W
- On Chicago Sanitary & Ship Canal
- Downstream of MWRD Lockport facility

**Significance:**
- Monitors flow from Chicago metropolitan area
- Tracks MWRD (Metropolitan Water Reclamation District) releases
- Can spike flows significantly during heavy rain events
- Independent of natural river flow

**Expected Parameters:**
- `00060` - Discharge (ft³/s)
- `00065` - Gage height (ft)

**Staleness Threshold:** 60 minutes

**Special Characteristics:**
- **Engineered canal** (not natural river)
- Flow controlled by MWRD operations
- Can reverse flow under certain conditions
- Major contributor during intense Chicago rainfall

**Typical Flow:**
- Normal: 2,000-4,000 cfs
- Heavy rain release: 10,000-15,000 cfs
- Maximum capacity: ~20,000 cfs

---

## Offline/Decommissioned Stations

### 05557000 - Illinois River at Henry, IL

**Status:** ❌ Offline (no data from API as of Feb 2026)

**Location:**
- Latitude: 41.111667° N
- Longitude: -89.356111° W
- ~60 river miles upstream of Peoria

**Last Known Significance:**
- Mid-river monitoring point
- 12-18 hour lead time to Peoria
- Gap filler between Marseilles and Chillicothe

**Decommissioned:** Unknown (recent)  
**Reason:** Likely equipment failure or budget cuts  
**Impact:** Reduced mid-river visibility, but Marseilles + Chillicothe provide adequate coverage

**Registry Status:** Kept in registry for historical reference

---

### 05568580 - Mackinaw River near Green Valley, IL

**Status:** ❌ Offline (no data from API as of Feb 2026)

**Location:**
- Latitude: 40.405972° N
- Longitude: -89.648333° W
- Eastern tributary near Peoria

**Last Known Significance:**
- Eastern tributary monitoring
- Local flash flood detection
- Complemented Spoon River (western tributary) data

**Decommissioned:** Unknown (recent)  
**Reason:** Likely equipment failure or budget cuts  
**Impact:** Reduced tributary visibility on east side

**Registry Status:** Kept in registry for historical reference

---

## Station Selection Criteria

### Why These 8 Stations?

**Geographic Coverage:**
- ✅ Upstream early warning (Marseilles, Chillicothe)
- ✅ At-site monitoring (Peoria, Kingston Mines)
- ✅ Tributary detection (Spoon River, Mackinaw)
- ✅ Chicago-area inflow (Romeoville Canal)

**Data Quality:**
- ✅ Long historical records (1939+)
- ✅ 15-minute measurement frequency
- ✅ USGS maintained and quality-controlled
- ✅ Free public data via API

**Operational Criteria:**
- ✅ Both discharge and stage available
- ✅ Real-time telemetry (when operational)
- ✅ Stable datum (no significant station moves)
- ✅ Relevant to Peoria flooding

### Stations NOT Included (and Why)

**05558300 - Illinois River at La Salle, IL:**
- Too far upstream (150+ river miles)
- Redundant with Marseilles (10 miles apart)

**05543500 - Illinois River at Seneca, IL:**
- Upstream of our focus area
- Minimal incremental warning value

**05570910 - Spoon River at London Mills, IL:**
- Too far upstream on tributary
- Green Valley gauge (when operational) closer to confluence

**05583000 - La Moine River at Ripley, IL:**
- South of Peoria (downstream tributary)
- Not useful for Peoria flood warning

## Station Health Monitoring

### Verification Process

**Integration Tests:**
```bash
cargo test --ignored station_api_verify_all_registry_stations -- --nocapture
```

**Test Output (Feb 19, 2026):**
```
✓ Illinois River at Kingston Mines, IL (05568500)
✓ Illinois River at Peoria, IL (05567500)
✓ Illinois River at Chillicothe, IL (05568000)
❌ Illinois River at Henry, IL (05557000) - NoDataAvailable
❌ Mackinaw River near Green Valley, IL (05568580) - NoDataAvailable
✓ Spoon River at Seville, IL (05570000)
✓ Illinois River at Marseilles, IL (05552500)
✓ Chicago Sanitary & Ship Canal at Romeoville, IL (05536890)

⚠️ 6 of 8 stations operational (2 offline)
System designed to handle partial station failures gracefully.
```

### Resilience Strategy

**System Response to Offline Stations:**
1. Parser skips empty timeSeries entries
2. No INSERT to gauge_readings table (keep data clean)
3. monitoring_state tracks consecutive failures
4. Status set to 'offline'
5. Alerts sent for critical stations
6. Service continues with available stations

**See:** [[Station Resilience|docs/STATION_RESILIENCE.md]] for complete operational procedures

---

## Registry Implementation

### Code Location

**File:** `src/stations.rs`

### Data Structure

```rust
pub struct Station {
    pub site_code: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub latitude: f64,
    pub longitude: f64,
    pub thresholds: Option<FloodThresholds>,
    pub expected_parameters: &'static [&'static str],
}

pub const STATION_REGISTRY: &[Station] = &[
    Station {
        site_code: "05568500",
        name: "Illinois River at Kingston Mines, IL",
        description: "Primary downstream reference gauge...",
        latitude: 40.556139,
        longitude: -89.778722,
        thresholds: Some(FloodThresholds { ... }),
        expected_parameters: &[PARAM_DISCHARGE, PARAM_STAGE],
    },
    // ... 7 more stations
];
```

### Helper Functions

```rust
// Get all site codes for API requests
pub fn all_site_codes() -> Vec<&'static str>;

// Filter stations by parameter availability
pub fn sites_with_parameter(param_code: &str) -> Vec<&Station>;

// Check if specific station provides parameter
pub fn station_has_parameter(site_code: &str, param_code: &str) -> bool;
```

### Usage Examples

```rust
// Build API URL for all stations
let sites = all_site_codes();
let url = build_iv_url(&sites, &[PARAM_DISCHARGE, PARAM_STAGE], "PT1H");

// Find stations with discharge data
let discharge_sites = sites_with_parameter(PARAM_DISCHARGE);

// Check if Kingston Mines provides stage
if station_has_parameter("05568500", PARAM_STAGE) {
    // ...
}
```

---

## Future Expansion

### Potential Additional Stations

**Upstream:**
- **05552500** - Dresden Island L&D (if new gauge added)
- **05536995** - Des Plaines River near Joliet (separate Chicago tributary)

**Downstream:**
- **05570500** - Spunky Bottoms near Browning, IL (downstream validation)
- **05583000** - La Moine River (south tributary)

**Criteria for Addition:**
- Must provide incremental warning value
- Data must be available via USGS NWIS API
- Historical record preferred (for trend analysis)
- Operational cost justified by flood risk reduction

### Scaling Considerations

**Current Design Supports:**
- ✅ Up to ~50 stations without code changes
- ✅ HashMap-based cache scalable to 1000s of stations
- ✅ Database indexes efficient to millions of readings

**If Scaling to 100+ Stations:**
- Consider async polling (tokio)
- Implement station groups for parallel API requests
- Add database partitioning by site_code
- Implement tiered staleness monitoring (critical vs normal)

---

**Related Pages:**
- [[Data Sources]] - USGS NWIS API details
- [[Station Resilience]] - Handling offline gauges
- [[Database Architecture]] - Sites table schema
- [[Staleness Tracking]] - Per-station thresholds
