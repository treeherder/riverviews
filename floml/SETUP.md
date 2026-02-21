# FloML Setup Guide

## Prerequisites

You need Python 3.8+ and the venv package installed.

### On Ubuntu/Debian:
```bash
sudo apt install python3.13-venv  # Or python3-venv for your Python version
```

### On other systems:
Python venv is usually included by default.

## Installation

```bash
cd /home/fiver/projects/flopro/floml

# Create virtual environment
python3 -m venv venv

# Activate virtual environment
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install dependencies
pip install --upgrade pip
pip install -r requirements.txt
```

## Quick Test

```bash
# Test database connection
python floml/db.py

# Should output: ✓ Database connected - X USGS sites configured
```

## Run Analysis

```bash
# Activate environment first
source venv/bin/activate

# Analyze all recent flood events
python scripts/analyze_events.py

# Analyze specific site
python scripts/analyze_events.py --site-code 05568500

# Include stage-discharge regression
python scripts/analyze_events.py --site-code 05568500 --regression
```

## Interactive Development

```bash
source venv/bin/activate
jupyter notebook notebooks/
```

## Module Usage

```python
from floml.db import get_engine
from floml.regression import fit_segmented_regression
from floml.correlation import correlate_stations
from floml.precursors import analyze_precursors

# Your analysis code here...
```

## Troubleshooting

### Database connection fails
- Check `.env` file has correct DATABASE_URL
- Verify PostgreSQL is running: `pg_isready -h localhost`
- Test connection: `psql -h localhost -U flopro_admin -d flopro_db`

### Import errors
- Make sure virtual environment is activated: `source venv/bin/activate`
- Reinstall dependencies: `pip install -r requirements.txt`

### No data found
- Run Rust ingestion first: `cd ../flomon_service && cargo run --bin historical_ingest`
- Check data exists: `psql ... -c "SELECT COUNT(*) FROM usgs_raw.gauge_readings"`
