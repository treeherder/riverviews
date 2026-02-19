# Data Sources

FloPro monitors water levels and discharge rates from the USGS National Water Information System (NWIS).

## Primary Data Source: USGS NWIS

### What is NWIS?

The [USGS National Water Information System](https://waterservices.usgs.gov/) is the United States' authoritative source for real-time stream flow and water level data. USGS operates over 10,000 automated stream gauges nationwide, providing continuous measurements for flood forecasting, water resource management, and scientific research.

### Why USGS Data?

✅ **Authoritative** - Federal agency with 140+ years of expertise  
✅ **Reliable** - >95% gauge uptime, automated quality control  
✅ **Free & Public** - Open data via REST APIs  
✅ **Long History** - Some stations date to 1880s  
✅ **Real-time** - 15-minute measurement intervals  
✅ **Well Documented** - Comprehensive API documentation and metadata  

## Two NWIS APIs: IV and DV

FloPro uses **two complementary USGS endpoints** to provide both historical context and operational detail:

### Instantaneous Values (IV) API

**Endpoint:** `https://waterservices.usgs.gov/nwis/iv/`

**Purpose:** Real-time operational monitoring

| Attribute | Value |
|-----------|-------|
| **Update Frequency** | Every 15 minutes |
| **Time Resolution** | 15-minute intervals |
| **Data Retention** | Last 120 days only |
| **Typical API Lag** | 15-30 minutes |
| **Records per Day** | 96 (4 per hour) |
| **Use Case** | Flood alerting, real-time monitoring |

**Coverage Limitation:** ⚠️ The IV API maintains only a **120-day rolling window**. Historical data older than ~4 months is archived and unavailable via this endpoint.

**Example Request:**
```
GET /nwis/iv/?sites=05568500&parameterCd=00060,00065&period=P1D&format=json
```

### Daily Values (DV) API

**Endpoint:** `https://waterservices.usgs.gov/nwis/dv/`

**Purpose:** Historical flood analysis

| Attribute | Value |
|-----------|-------|
| **Update Frequency** | Daily |
| **Time Resolution** | Daily mean values |
| **Data Retention** | Full station history (1939+ for our sites) |
| **Typical API Lag** | 1-2 days |
| **Records per Day** | 1 (daily mean) |
| **Use Case** | Trend analysis, return intervals, climate patterns |

**Historical Coverage:** ✅ The DV API provides **87 years of historical data** for our primary stations (October 1939 to present).

**Example Request:**
```
GET /nwis/dv/?sites=05568500&parameterCd=00060,00065&startDT=2023-01-01&endDT=2023-12-31&format=json
```

## Dual-Source Strategy

FloPro uses **both APIs** to create a comprehensive dataset:

```
Timeline:
├─────────────────────────────────────────────┬──────────────────┐
│     Historical Context (DV)                 │  Recent (IV)     │
│     Daily means: 1939 - 125 days ago        │  15-min: Last    │
│     ~31,755 days × 8 sites                  │  120 days        │
│     = 508,080 daily records                 │  = 184,320       │
│                                             │    readings      │
└─────────────────────────────────────────────┴──────────────────┘
                                              ↑
                                         Overlap zone
                                    (both sources available)
```

**Why Both?**
- **Long-term patterns**: DV provides 87 years for flood frequency analysis
- **Real-time detail**: IV provides sub-hourly resolution for active events
- **No gaps**: Overlap ensures continuous coverage at transition point
- **Optimal storage**: Daily means for history, high-res for recent events

## Monitored Parameters

FloPro tracks two primary hydrological parameters:

### Parameter 00060: Discharge (Streamflow)

**Definition:** Volume of water flowing past the gauge per unit time  
**Unit:** Cubic feet per second (cfs or ft³/s)  
**Significance:** Direct measure of flood magnitude  
**Critical Threshold:** >50,000 cfs at Kingston Mines indicates flood conditions  

**Why it matters:**
- Primary flood indicator
- Correlates directly with inundation levels
- Used for comparison across different river segments

### Parameter 00065: Gage Height (Stage)

**Definition:** Water surface elevation relative to gauge datum  
**Unit:** Feet (ft)  
**Significance:** Determines overland flooding extent  
**NWS Thresholds:** Action (17 ft), Flood (18 ft), Major (23 ft) at Peoria  

**Why it matters:**
- Direct correlation to local flooding impacts
- NWS flood categories based on stage, not discharge
- Property owners understand stage more intuitively than discharge

## Data Quality Characteristics

### Measurement Reliability

**Temporal Resolution:**
- New IV readings every **15 minutes** (00, 15, 30, 45 of each hour)
- Recommended polling interval: **15-20 minutes**
- Staleness threshold: Data older than **45-60 minutes** indicates potential outage

**Data Qualifiers:**
- **P (Provisional)**: Subject to revision, ±5-10% accuracy
- **A (Approved)**: Final QA/QC complete, ±2-5% accuracy
- Most real-time data is provisional; approval can take months

**Coordinate System:**
- **Datum**: WGS84 (EPSG:4326)
- **Format**: Decimal degrees latitude/longitude

### Missing Data Handling

**Sentinel Values:**
- USGS uses **-999999** to indicate no data available
- FloPro parser automatically filters sentinel values
- No sentinel values stored in database

**Common Outage Causes:**
- Equipment maintenance (scheduled)
- Extreme flood events (gauge overflow)
- Ice conditions (winter)
- Communication failures (cellular/satellite)
- Power outages

**FloPro Response:**
- Continues operating with available stations
- Tracks staleness in `monitoring_state` table
- Sends alerts when critical stations go offline
- See [[Station Resilience]] for handling strategy

## API Response Format

### WaterML (JSON)

Responses use WaterML format rendered as JSON:

```json
{
  "value": {
    "timeSeries": [
      {
        "sourceInfo": {
          "siteName": "ILLINOIS RIVER AT KINGSTON MINES, IL",
          "siteCode": [{ "value": "05568500" }],
          "geoLocation": {
            "geogLocation": {
              "latitude": 40.556139,
              "longitude": -89.778722
            }
          }
        },
        "variable": {
          "variableCode": [{ "value": "00060" }],
          "unit": { "unitCode": "ft3/s" },
          "noDataValue": -999999
        },
        "values": [{
          "value": [
            {
              "value": "42300",
              "dateTime": "2026-02-19T14:45:00.000-06:00",
              "qualifiers": ["P"]
            }
          ]
        }]
      }
    ]
  }
}
```

**Key Parsing Notes:**
- Values are **strings** (`"42300"`) not numbers
- Timestamps in ISO 8601 with timezone offset
- One `timeSeries` per site × parameter combination
- Empty value arrays possible (station offline)

See `src/ingest/usgs.rs` for complete parsing implementation.

## Monitoring Network

FloPro monitors **8 USGS gauge stations** across the Illinois River system. See [[Station Registry]] for complete list.

**Geographic Coverage:**
- **Upstream**: Chicago metro area (Romeoville)
- **Mid-River**: Marseilles, Henry, Mackinaw River
- **Peoria Area**: Chillicothe, Peoria, Kingston Mines
- **Tributary**: Spoon River

**Lead Time Strategy:**
- Upstream gauges provide **24-48 hour warning**
- Multiple sites enable cross-validation
- Tributary monitoring detects local flooding

## Future Data Sources (Planned)

### NWS Advanced Hydrologic Prediction Service (AHPS)
- Flood forecasts and predicted crests
- Official flood stage thresholds
- Historical flood event narratives

### NOAA Weather Data
- Precipitation forecasts
- Radar-based rainfall estimates
- Soil moisture conditions

### USACE Lock Operations
- Dam release schedules
- Pool level targets
- Navigation notices

---

**Related Pages:**
- [[Database Architecture]] - How we store this data
- [[Technology Stack]] - Why we chose USGS over alternatives
- [[Historical Data Ingestion]] - 87-year backfill process
