# FloML Analysis Scripts

Terminal-based tools for visualizing and analyzing the Illinois River flood monitoring sensor network.

## 📊 Available Tools

### 1. Live Zone Dashboard (`zone_dashboard.py`) ⭐

Real-time ncurses dashboard showing all 7 zones simultaneously.

**Usage:**
```bash
python3 zone_dashboard.py
```

**Features:**
- 📊 Live grid layout (3x3) with all zones visible at once
- 🎨 Color-coded freshness indicators (green <30m, yellow <120m, red >120m)
- 🔄 Auto-refreshes every 30 seconds (only redraws when data changes)
- 📡 Sensor counts by source (USGS, CWMS, ASOS)
- 🌊 Current stage, discharge, and precipitation readings
- ⌨️ Interactive: 'q' to quit, 'r' to force refresh
- ⚡ Smart redraw: Only updates when sensor values actually change

**Best for:** Live monitoring during flood events

### 2. Zone Detail Viewer (`visualize_zones.py`)

Command-line visualization for detailed analysis.

**Usage:**
```bash
# Show full system overview + geographic map
python3 visualize_zones.py

# Show system overview only
python3 visualize_zones.py overview

# Show geographic sensor map only
python3 visualize_zones.py map

# Show detailed view of a specific zone
python3 visualize_zones.py 2
```

**Features:**
- 🎨 Color-coded output (green=fresh, yellow=stale, red=no data)
- 📍 Sensors grouped by role (direct, boundary, precip, proxy)
- ⏱️ Real-time staleness indicators
- 🌊 Source identification (USGS, USACE/CWMS, ASOS)

**Best for:** Deep dive into specific zones

### 3. Correlation Analysis (`demo_correlation.py`)

Shows real-time sensor correlations and hydrologic insights.

**Usage:**
```bash
python3 demo_correlation.py
```

**Features:**
- 📊 Precipitation vs stage correlation
- ⏱️ Expected lag times and responses
- 💧 Hydrologic insights from sensor differentials
- 🌊 Flow gradient analysis (upstream vs backwater)

**Best for:** Understanding the science behind flood prediction

### 4. Historical Event Analysis (`analyze_events.py`)

Analyzes historical flood events from the database.

**Usage:**
```bash
python3 analyze_events.py
```

## 🚀 Quick Start

```bash
cd /home/fiver/projects/riverviews/floml/scripts

# Live monitoring dashboard (recommended)
python3 zone_dashboard.py

# Detailed zone analysis
python3 visualize_zones.py 2

# Correlation analysis
python3 demo_correlation.py
```

## 📋 Zone Architecture

### Zone Hierarchy (0-6)

- **Zone 0**: Mississippi River — Backwater Source (2-5 day lead time)
- **Zone 1**: Lower Illinois River — Backwater Interface (6-24 hour lead)
- **Zone 2**: Upper Peoria Lake — Property Location (current conditions)
- **Zone 3**: Mackinaw River Tributary Basin (6-12 hour lead)
- **Zone 4**: Middle Illinois River — Upstream Response (12-48 hour lead)
- **Zone 5**: Upper Illinois River — Des Plaines Junction (2-4 day lead)
- **Zone 6**: Chicago Area Waterway System (4-8 day lead)

### Data Sources

- **🌊 USGS**: Stream gauges (stage, discharge)
- **🔒 USACE/CWMS**: Lock & dam measurements (pool/tailwater elevation)
- **☁️ IEM/ASOS**: Weather stations (precipitation)

### Sensor Roles

- **direct**: Measures conditions at the zone
- **boundary**: Inflow/outflow to the zone
- **precip**: Rainfall over catchment area
- **proxy**: Indicators correlated to zone conditions

## 🔧 Technical Requirements

The visualization tools query the monitoring daemon's REST API:
```
http://localhost:8080/zone/{zone_id}
```

Ensure the daemon is running:
```bash
ps aux | grep flomon_service

# Start daemon if needed
cd /home/fiver/projects/riverviews/flomon_service
./target/release/flomon_service --endpoint 8080 &
```
