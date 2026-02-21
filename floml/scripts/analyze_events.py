#!/usr/bin/env python3
"""Example flood event analysis workflow.

Demonstrates using FloML to analyze historical flood events:
1. Load event data from database
2. Detect precursor patterns
3. Fit stage-discharge relationships
4. Compute correlations between stations
5. Store results back to database
"""

import sys
import logging
import argparse
from datetime import timedelta
import pandas as pd
import numpy as np

from floml.db import get_engine, verify_schemas
from floml.precursors import analyze_precursors, compute_precursor_metrics
from floml.regression import fit_stage_discharge
from floml.correlation import correlate_stations

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


def load_flood_events(engine, site_code=None):
    """Load historical flood events from database."""
    query = """
        SELECT e.id, e.site_code, e.crest_time, e.peak_stage_ft, 
               e.severity, t.flood_stage_ft
        FROM nws.flood_events e
        JOIN nws.flood_thresholds t ON e.site_code = t.site_code
        WHERE e.crest_time IS NOT NULL
    """
    
    if site_code:
        query += f" AND e.site_code = '{site_code}'"
    
    query += " ORDER BY e.crest_time DESC LIMIT 10"
    
    events = pd.read_sql(query, engine)
    logger.info(f"Loaded {len(events)} flood events")
    return events


def load_stage_data(engine, site_code, start_time, end_time):
    """Load stage data for a site and time window."""
    query = """
        SELECT reading_time as timestamp, value as stage_ft
        FROM usgs_raw.gauge_readings
        WHERE site_code = %(site_code)s
          AND parameter_code = '00065'
          AND reading_time BETWEEN %(start)s AND %(end)s
        ORDER BY reading_time
    """
    
    data = pd.read_sql(
        query, 
        engine,
        params={'site_code': site_code, 'start': start_time, 'end': end_time}
    )
    
    if len(data) > 0:
        data.set_index('timestamp', inplace=True)
        logger.info(f"Loaded {len(data)} stage readings for {site_code}")
    else:
        logger.warning(f"No stage data found for {site_code}")
    
    return data


def analyze_event(engine, event_row):
    """Analyze a single flood event."""
    site_code = event_row['site_code']
    crest_time = pd.Timestamp(event_row['crest_time'])
    event_id = event_row['id']
    
    logger.info(f"\n{'='*60}")
    logger.info(f"Analyzing Event {event_id}: {site_code} at {crest_time}")
    logger.info(f"Peak: {event_row['peak_stage_ft']:.2f} ft ({event_row['severity']})")
    logger.info(f"{'='*60}\n")
    
    # Load stage data for precursor window (14 days before peak)
    lookback_days = 14
    window_start = crest_time - timedelta(days=lookback_days)
    window_end = crest_time + timedelta(days=1)
    
    stage_data = load_stage_data(engine, site_code, window_start, window_end)
    
    if stage_data.empty:
        logger.warning("No data available for analysis")
        return None
    
    # Analyze precursors
    logger.info("🔍 Detecting precursor patterns...")
    precursors = analyze_precursors(
        stage_data['stage_ft'],
        peak_time=crest_time,
        lookback_days=lookback_days
    )
    
    if precursors:
        print(f"\n📋 Found {len(precursors)} precursor events:")
        for p in precursors:
            print(f"  • {p.precursor_type:15s} {p.hours_before_peak:6.1f}h before peak - {p.description}")
        
        metrics = compute_precursor_metrics(precursors)
        print(f"\n📊 Precursor Metrics:")
        print(f"  Earliest warning: {metrics['earliest_warning_hours']:.1f} hours")
        print(f"  Max rise rate: {metrics['max_rise_rate']:.2f} ft/day")
        print(f"  Major events: {metrics['major_events']}")
    else:
        print("  No significant precursors detected")
    
    return {
        'event_id': event_id,
        'site_code': site_code,
        'precursor_count': len(precursors),
        'metrics': compute_precursor_metrics(precursors) if precursors else {}
    }


def analyze_stage_discharge(engine, site_code):
    """Analyze stage-discharge relationship for a station."""
    logger.info(f"\n{'='*60}")
    logger.info(f"Stage-Discharge Analysis: {site_code}")
    logger.info(f"{'='*60}\n")
    
    # Load paired stage and discharge data
    query = """
        SELECT reading_time,
               MAX(CASE WHEN parameter_code = '00065' THEN value END) as stage_ft,
               MAX(CASE WHEN parameter_code = '00060' THEN value END) as discharge_cfs
        FROM usgs_raw.gauge_readings
        WHERE site_code = %(site_code)s
          AND reading_time > NOW() - INTERVAL '1 year'
        GROUP BY reading_time
        HAVING MAX(CASE WHEN parameter_code = '00065' THEN value END) IS NOT NULL
           AND MAX(CASE WHEN parameter_code = '00060' THEN value END) IS NOT NULL
        ORDER BY reading_time
    """
    
    data = pd.read_sql(query, engine, params={'site_code': site_code})
    
    if len(data) < 50:
        logger.warning(f"Insufficient data for regression (only {len(data)} points)")
        return None
    
    logger.info(f"Loaded {len(data)} paired observations")
    
    # Fit segmented regression
    try:
        result = fit_stage_discharge(
            data['discharge_cfs'],
            data['stage_ft'],
            n_segments=3
        )
        
        print(f"\n📈 Stage-Discharge Regression:")
        print(f"  R² = {result.r_squared:.4f}")
        print(f"  RMSE = {result.rmse:.2f} ft")
        print(f"  Breakpoints: {result.breakpoints}")
        print(f"  Slopes: {result.slopes}")
        
        return result
    except Exception as e:
        logger.error(f"Regression failed: {e}")
        return None


def main():
    parser = argparse.ArgumentParser(description='Analyze flood events')
    parser.add_argument('--site-code', help='Analyze specific site only')
    parser.add_argument('--regression', action='store_true', help='Include stage-discharge regression')
    args = parser.parse_args()
    
    try:
        # Connect to database
        logger.info("Connecting to database...")
        engine = get_engine()
        verify_schemas(engine)
        print("✓ Database connected\n")
        
        # Load flood events
        events = load_flood_events(engine, args.site_code)
        
        if events.empty:
            print("No flood events found in database")
            return
        
        print(f"Found {len(events)} flood events to analyze\n")
        
        # Analyze each event
        results = []
        for idx, event in events.iterrows():
            result = analyze_event(engine, event)
            if result:
                results.append(result)
        
        # Optional: stage-discharge regression
        if args.regression and args.site_code:
            analyze_stage_discharge(engine, args.site_code)
        
        # Summary
        print(f"\n{'='*60}")
        print(f"Analysis Complete")
        print(f"{'='*60}")
        print(f"Events analyzed: {len(results)}")
        
        if results:
            avg_precursors = np.mean([r['precursor_count'] for r in results])
            print(f"Average precursors per event: {avg_precursors:.1f}")
        
    except Exception as e:
        logger.error(f"Analysis failed: {e}", exc_info=True)
        return 1
    
    return 0


if __name__ == "__main__":
    sys.exit(main())
