"""Flood precursor detection and pattern analysis.

Identifies early warning signals in gauge data before flood events.
"""

import numpy as np
import pandas as pd
from typing import List, Dict, Optional
from dataclasses import dataclass
from datetime import timedelta
import logging

logger = logging.getLogger(__name__)


@dataclass
class PrecursorEvent:
    """A detected precursor condition."""
    precursor_type: str
    detected_at: pd.Timestamp
    value: float
    severity: str  # 'minor', 'moderate', 'major'
    hours_before_peak: float
    description: str
    
    def __str__(self) -> str:
        return (
            f"{self.precursor_type} ({self.severity}) at {self.detected_at}\n"
            f"  Value: {self.value:.2f}\n"
            f"  {self.hours_before_peak:.1f} hours before peak\n"
            f"  {self.description}"
        )


def calculate_rise_rate(
    stage: pd.Series,
    window_hours: int = 6
) -> pd.Series:
    """Calculate stage rise rate in feet per day.
    
    Args:
        stage: Time series of stage values
        window_hours: Window for rate calculation
        
    Returns:
        Series of rise rates (ft/day)
    """
    # Ensure hourly frequency
    stage_hourly = stage.resample('1H').mean().interpolate()
    
    # Calculate change over window
    rise = stage_hourly.diff(periods=window_hours)
    
    # Convert to ft/day
    rise_rate_per_day = rise * (24.0 / window_hours)
    
    return rise_rate_per_day


def detect_rapid_rise(
    stage: pd.Series,
    threshold_ft_per_day: float = 0.5,
    min_duration_hours: int = 3
) -> List[PrecursorEvent]:
    """Detect rapid river rise events.
    
    Args:
        stage: Time series of stage values
        threshold_ft_per_day: Minimum rise rate to flag
        min_duration_hours: Minimum sustained duration
        
    Returns:
        List of detected rapid rise events
    """
    rise_rate = calculate_rise_rate(stage, window_hours=6)
    
    # Find periods exceeding threshold
    rapid = rise_rate > threshold_ft_per_day
    
    # Group consecutive periods
    events = []
    in_event = False
    event_start = None
    
    for timestamp, is_rapid in rapid.items():
        if is_rapid and not in_event:
            # Event starts
            event_start = timestamp
            in_event = True
        elif not is_rapid and in_event:
            # Event ends
            duration = (timestamp - event_start).total_seconds() / 3600
            if duration >= min_duration_hours:
                max_rate = rise_rate[event_start:timestamp].max()
                
                # Classify severity
                if max_rate > 2.0:
                    severity = 'major'
                elif max_rate > 1.0:
                    severity = 'moderate'
                else:
                    severity = 'minor'
                
                events.append(PrecursorEvent(
                    precursor_type='rapid_rise',
                    detected_at=event_start,
                    value=max_rate,
                    severity=severity,
                    hours_before_peak=0,  # Updated by caller
                    description=f"Rapid rise of {max_rate:.2f} ft/day for {duration:.1f} hours"
                ))
            in_event = False
    
    logger.info(f"Detected {len(events)} rapid rise events")
    return events


def detect_sustained_rise(
    stage: pd.Series,
    threshold_ft: float = 2.0,
    window_days: int = 7
) -> List[PrecursorEvent]:
    """Detect sustained river rise over multiple days.
    
    Args:
        stage: Time series of stage values
        threshold_ft: Minimum total rise
        window_days: Window for measuring rise
        
    Returns:
        List of detected sustained rise events
    """
    events = []
    
    # Daily resampling
    stage_daily = stage.resample('1D').mean()
    
    # Rolling min/max over window
    rolling_min = stage_daily.rolling(window=window_days, min_periods=window_days).min()
    rolling_max = stage_daily.rolling(window=window_days, min_periods=window_days).max()
    
    total_rise = rolling_max - rolling_min
    
    # Detect where rise exceeds threshold
    sustained = total_rise > threshold_ft
    
    # Find start of sustained rise periods
    started = sustained & ~sustained.shift(1, fill_value=False)
    
    for timestamp in started[started].index:
        rise_value = total_rise[timestamp]
        
        # Classify severity
        if rise_value > 6.0:
            severity = 'major'
        elif rise_value > 4.0:
            severity = 'moderate'
        else:
            severity = 'minor'
        
        events.append(PrecursorEvent(
            precursor_type='sustained_rise',
            detected_at=timestamp,
            value=rise_value,
            severity=severity,
            hours_before_peak=0,
            description=f"Sustained rise of {rise_value:.2f} ft over {window_days} days"
        ))
    
    logger.info(f"Detected {len(events)} sustained rise events")
    return events


def analyze_precursors(
    stage: pd.Series,
    peak_time: pd.Timestamp,
    lookback_days: int = 14,
    rapid_rise_threshold: float = 0.5,
    sustained_rise_threshold: float = 2.0
) -> List[PrecursorEvent]:
    """Comprehensive precursor analysis for a flood event.
    
    Args:
        stage: Time series of stage values
        peak_time: Timestamp of flood peak
        lookback_days: Days before peak to analyze
        rapid_rise_threshold: Threshold for rapid rise (ft/day)
        sustained_rise_threshold: Threshold for sustained rise (ft)
        
    Returns:
        List of all detected precursor events with timing relative to peak
    """
    # Extract window before peak
    window_start = peak_time - timedelta(days=lookback_days)
    stage_window = stage[window_start:peak_time]
    
    logger.info(f"Analyzing precursors from {window_start} to {peak_time}")
    
    # Detect different precursor types
    rapid_events = detect_rapid_rise(stage_window, threshold_ft_per_day=rapid_rise_threshold)
    sustained_events = detect_sustained_rise(stage_window, threshold_ft=sustained_rise_threshold)
    
    # Combine and update hours_before_peak
    all_events = rapid_events + sustained_events
    
    for event in all_events:
        event.hours_before_peak = (peak_time - event.detected_at).total_seconds() / 3600
    
    # Sort by detection time
    all_events.sort(key=lambda e: e.detected_at)
    
    logger.info(f"Found {len(all_events)} total precursor events")
    return all_events


def compute_precursor_metrics(events: List[PrecursorEvent]) -> Dict[str, float]:
    """Compute summary metrics from precursor events.
    
    Args:
        events: List of detected precursor events
        
    Returns:
        Dictionary of metrics
    """
    if not events:
        return {
            'total_events': 0,
            'earliest_warning_hours': 0,
            'max_rise_rate': 0,
            'major_events': 0
        }
    
    rapid_events = [e for e in events if e.precursor_type == 'rapid_rise']
    major_events = [e for e in events if e.severity == 'major']
    
    return {
        'total_events': len(events),
        'earliest_warning_hours': max(e.hours_before_peak for e in events),
        'max_rise_rate': max((e.value for e in rapid_events), default=0),
        'rapid_rise_events': len(rapid_events),
        'sustained_rise_events': len([e for e in events if e.precursor_type == 'sustained_rise']),
        'major_events': len(major_events)
    }


if __name__ == "__main__":
    # Example usage
    logging.basicConfig(level=logging.INFO)
    
    # Generate synthetic flood event
    dates = pd.date_range('2024-01-01', periods=500, freq='1H')
    
    # Gradual rise with rapid acceleration
    stage = pd.Series(
        10 + 0.01 * np.arange(500) + 
        np.where(np.arange(500) > 300, (np.arange(500) - 300) / 20, 0) +
        np.random.normal(0, 0.2, 500),
        index=dates
    )
    
    peak_time = dates[450]  # Near end
    
    # Analyze precursors
    events = analyze_precursors(stage, peak_time)
    
    print(f"Found {len(events)} precursor events:\n")
    for event in events:
        print(event)
        print()
    
    # Summary metrics
    metrics = compute_precursor_metrics(events)
    print("Metrics:")
    for key, value in metrics.items():
        print(f"  {key}: {value}")
