# Flood (Protection) Monitoring Service - Project Plans
FloPro is a goofy working name that I don't really plan to stick with but it works well for now.
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
- [ ] Create reliable, reproducible steps for running devops support for the project


---
  
### Infrastructure tasks for CWMS Data API (Corps Water Management System)

USACE data is apparently only available after 2015 without a foia request.

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

