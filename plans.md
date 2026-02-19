# Peoria Flood Monitoring Service - Project Plans

## Monitoring Network Overview

### Locks & Dams Upstream of Peoria on the Illinois Waterway

The Illinois Waterway has 8 locks and dams total. We will prioritize tributary water systems and locks/dams upstream of Peoria while we build the minimum viable product for this experiment. As our research progresses we may add downstream sites before expanding into other risk categories, as downstream blockage can change the flood model depending on how much water the upstream flood plains are already holding—a data set we are still determining how to utilize.

| # | Lock & Dam | Location | River Mile (above Mississippi) | Notes |
|---|------------|----------|-------------------------------|-------|
| 1 | Starved Rock L&D | Near Utica/Ottawa, IL | ~231 | Closest major upstream structure; very relevant to Peoria flood timing |
| 2 | Marseilles L&D | Marseilles, IL | ~244 | Marseilles Canal bypasses the dam; large Tainter gate structure |
| 3 | Dresden Island L&D | Morris, IL | ~271 | Where the Kankakee and Des Plaines rivers join to form the Illinois |
| 4 | Brandon Road L&D | Joliet, IL | ~286 | Controls flow from the Chicago metro area |
| 5 | Lockport L&D | Lockport, IL | ~291 | On the Chicago Sanitary & Ship Canal; critical chokepoint for Chicago-area releases |
| 6 | T.J. O'Brien L&D | Chicago (Calumet River) | ~326 | Controls Lake Michigan inflow to the system |

These six locks and dams are located between Peoria and Lake Michigan.

### Key Monitoring Points

#### Dresden Island Lock & Dam
Particularly important because it sits at the confluence of the Kankakee and Des Plaines rivers—this is where the Illinois River is effectively born. High flow readings here may indicate relatively early warning of future downstream conditions.

#### Lockport & Brandon Road Locks
These gauges are most closely affected by the Chicago metro area. MWRD (Metropolitan Water Reclamation District) releases during heavy rain events can meaningfully spike flows downstream, and these two locks will show evidence of this signature first.

#### Starved Rock Lock & Dam
The last early-warning point before water reaches Peoria. At typical flow velocities, a flood pulse from Starved Rock can reach Peoria in roughly **24–48 hours** depending on conditions, giving us meaningful lead time to enact actionable steps for flood damage mitigation and evacuation.

### Dam Types & Operational Considerations

**Wicket Dams** (Peoria and LaGrange): Use the older wicket dam design, which gets laid flat during high flows—meaning the dam essentially disappears during flood events and the river runs as open water.

**Tainter Gate Structures** (Upstream locks): Use fixed concrete gated structures with Tainter gates.

This matters for the monitoring service because the Corps of Engineers' operational decisions at each structure affect pool levels.

### USGS Gauge Locations

The USGS typically places gauges just above or below each lock. The USGS NWIS site search at `waterservices.usgs.gov` filtered to Illinois and the Illinois River will show all active stations that can be mapped to these structures.

---

## Project To-Do List

### Infrastructure tasks for USGS gauges service
- [X] Determine polling frequency and schedule
- [X] Prepare database (PostgreSQL?)    
    - [X] Create schema for historical data with instantaneous value integration
    - [X] Monitor and update staleness to know availability/frequency of IV data updates

- [ ] Cross reference actual recorded flood events with collected data
    - [ ] find frame of reference for historical flood events within the data
        - [ ] potentially use regression analysis to identify thresholds of change in this data set (and others) relative to the impact of recorded and potentially unrecorded near-threshold flood events
        - [ ] design a model for alerting on the severity of near-threshold events
        - [ ] prioritize alerting on upstream threshold events that trend towards being impactful in upper peoria lake area, specifically the eastern bank when possible.
- [ ] Set up VPS for hosting
- [ ] Determine alert infrastructure
- [ ] Design dashboard interface
---

## Technical Reference

## USGS NWIS Instantaneous Values (IV) API

### API Endpoint

```
https://waterservices.usgs.gov/nwis/iv/
```

### Example Request

```
GET /nwis/iv/?sites=05568500&parameterCd=00060,00065&period=PT3H&format=json&siteStatus=active
```

**Parameters:**
- `sites` — Comma-separated list of 8-digit USGS site codes
- `parameterCd` — Comma-separated parameter codes (00060=discharge, 00065=stage)
- `period` — ISO 8601 duration (PT1H=1 hour, PT3H=3 hours, P7D=7 days)
- `format` — Response format (json)
- `siteStatus` — Filter to active sites only

### Response Structure

Based on live API call to USGS (Feb 18, 2026):

```json
{
  "value": {
    "queryInfo": { ... },  // Request metadata and disclaimer
    "timeSeries": [        // Array - one entry per site+parameter combination
      {
        "sourceInfo": {
          "siteName": "ILLINOIS RIVER AT KINGSTON MINES, IL",
          "siteCode": [
            {
              "value": "05568500",      // 8-digit USGS site number
              "network": "NWIS",
              "agencyCode": "USGS"
            }
          ],
          "timeZoneInfo": {
            "defaultTimeZone": {
              "zoneOffset": "-06:00",         // CST offset
              "zoneAbbreviation": "CST"
            },
            "daylightSavingsTimeZone": {
              "zoneOffset": "-05:00",         // CDT offset
              "zoneAbbreviation": "CDT"
            },
            "siteUsesDaylightSavingsTime": false
          },
          "geoLocation": {
            "geogLocation": {
              "srs": "EPSG:4326",
              "latitude": 40.55343889,
              "longitude": -89.7772722
            }
          }
        },
        "variable": {
          "variableCode": [
            {
              "value": "00060",               // Parameter code (00060=discharge, 00065=stage)
              "network": "NWIS"
            }
          ],
          "variableName": "Streamflow, ft³/s",
          "variableDescription": "Discharge, cubic feet per second",
          "unit": {
            "unitCode": "ft3/s"               // Unit for this parameter
          },
          "noDataValue": -999999.0            // Sentinel value for missing data
        },
        "values": [                           // Usually single element array
          {
            "value": [                        // Array of measurements
              {
                "value": "4820",              // ⚠️ STRING not number!
                "qualifiers": ["P"],          // P=Provisional, A=Approved
                "dateTime": "2026-02-18T21:00:00.000-06:00"  // ISO 8601 with offset
              },
              {
                "value": "5500",
                "qualifiers": ["P"],
                "dateTime": "2026-02-18T22:45:00.000-06:00"
              }
              // ... more values
            ]
          }
        ]
      },
      {
        // Second timeSeries entry for parameter 00065 (stage)
        // Same site, different parameter - same structure
      }
    ]
  }
}
```

### Parsing Requirements

#### 1. Multi-Site Batching
Response contains one `timeSeries` object per site × parameter combination:
- 1 site + 2 parameters → 2 timeSeries entries
- 3 sites + 2 parameters → 6 timeSeries entries

#### 2. String Values ⚠️
Measurement values arrive as JSON strings (`"4820"`) not numbers:
- Must parse with `str::parse::<f64>()`
- Check against `noDataValue` (-999999.0) after parsing

#### 3. Data Qualifiers
- `"P"` = Provisional data (subject to revision)
- `"A"` = Approved data (finalized)
- Array can be empty `[]` — default to `"P"` if absent

#### 4. Timezone Handling
All timestamps include UTC offset (e.g., `-06:00` for CST):
- Peoria basin stations use Central Time
- No DST for gauge measurements (`siteUsesDaylightSavingsTime: false`)

#### 5. Most Recent Value
Values array is sorted chronologically:
- Last element `values[0].value[n-1]` is the most recent reading
- For real-time monitoring, only the latest value is needed

#### 6. Missing Data Detection
If no data available in time period:
- `values[0].value` is empty array `[]`
- OR values contain sentinel `-999999`
- Should return `NwisError::NoDataAvailable`

### Live Data Sample

**Kingston Mines Station (05568500)** - Feb 18, 2026:

| Parameter | Code  | Value     | Timestamp                           | Qualifier |
|-----------|-------|-----------|-------------------------------------|-----------|
| Discharge | 00060 | 5,500 cfs | 2026-02-18T22:45:00.000-06:00      | P         |
| Stage     | 00065 | 2.97 ft   | 2026-02-18T22:45:00.000-06:00      | P         |

### Error Handling

| Condition | Error Type |
|-----------|-----------|
| Empty `timeSeries` array | `NoDataAvailable` |
| Empty `value` array | `NoDataAvailable` |
| Sentinel value `-999999` | `NoDataAvailable` |
| Malformed JSON | `ParseError` |
| Missing required fields | `ParseError` |

