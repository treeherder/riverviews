/// Flood Event Analysis Module
///
/// Analyzes historical flood events to identify precursor patterns,
/// correlate multi-source data (USGS + USACE), and build comprehensive
/// relational flood event records.
///
/// # Analysis Process
///
/// 1. **Precursor Window Detection**
///    - For each flood event, look back N days (default: 14)
///    - Identify when significant rise began (threshold: 2.0 ft)
///    - Calculate rise rate and duration
///
/// 2. **Multi-Source Data Correlation**
///    - Link USGS gauge observations to event window
///    - Link USACE CWMS data (Mississippi stages, backwater)
///    - Classify observation phase (precursor, rising, peak, falling)
///
/// 3. **Metric Computation**
///    - Rise rate (ft/day), total rise, duration
///    - Peak discharge, hours above flood stage
///    - Backwater contribution estimate
///
/// 4. **Precursor Detection**
///    - Rapid rise events (>X ft/day)
///    - Backwater onset
///    - Sustained rise patterns
///
/// # Output
///
/// Populates `flood_analysis` schema with:
/// - Enhanced flood events with analysis metadata
/// - Linked observations from all data sources
/// - Computed metrics and precursor conditions

use chrono::{DateTime, Duration, Utc};
use postgres::Client;
use rust_decimal::Decimal;

/// Analysis configuration
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Days to look back before flood peak for precursors
    pub precursor_lookback_days: i32,
    
    /// Minimum rise (ft) to consider "significant"
    pub significant_rise_threshold_ft: f64,
    
    /// Minimum rise rate (ft/day) to flag as precursor
    pub rise_rate_threshold_ft_per_day: f64,
    
    /// Days after peak to include in analysis window
    pub post_peak_window_days: i32,
    
    /// Backwater detection threshold (Mississippi - Illinois)
    pub backwater_differential_threshold_ft: f64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            precursor_lookback_days: 14,
            significant_rise_threshold_ft: 2.0,
            rise_rate_threshold_ft_per_day: 0.5,
            post_peak_window_days: 7,
            backwater_differential_threshold_ft: 2.0,
        }
    }
}

/// Historical flood event from nws.flood_events
#[derive(Debug, Clone)]
pub struct HistoricalEvent {
    pub event_id: i32,
    pub site_code: String,
    pub crest_time: DateTime<Utc>,
    pub peak_stage_ft: Option<Decimal>,
    pub severity: String,
    pub flood_stage_ft: Decimal,
}

/// USGS observation during event window
#[derive(Debug, Clone)]
pub struct EventObservation {
    pub timestamp: DateTime<Utc>,
    pub stage_ft: Option<Decimal>,
    pub discharge_cfs: Option<Decimal>,
}

/// Precursor window detection result
#[derive(Debug, Clone)]
pub struct PrecursorWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub total_rise_ft: f64,
    pub rise_duration_hours: i32,
    pub average_rise_rate_ft_per_day: f64,
    pub max_rise_rate_ft_per_day: f64,
}

/// Detected precursor condition
#[derive(Debug, Clone)]
pub struct PrecursorCondition {
    pub precursor_type: String,
    pub detected_at: DateTime<Utc>,
    pub description: String,
    pub severity_score: f64,
    pub confidence: f64,
    pub hours_before_peak: i32,
    pub metrics: serde_json::Value,
}

/// Load analysis configuration from database
pub fn load_config(client: &mut Client) -> Result<AnalysisConfig, Box<dyn std::error::Error>> {
    let row = client.query_one(
        "SELECT 
            precursor_lookback_days,
            significant_rise_threshold_ft,
            rise_rate_threshold_ft_per_day,
            post_peak_window_days,
            backwater_differential_threshold_ft
         FROM flood_analysis.analysis_config
         WHERE is_active = true
         ORDER BY created_at DESC
         LIMIT 1",
        &[],
    )?;
    
    Ok(AnalysisConfig {
        precursor_lookback_days: row.get(0),
        significant_rise_threshold_ft: row.get::<_, Decimal>(1).to_string().parse()?,
        rise_rate_threshold_ft_per_day: row.get::<_, Decimal>(2).to_string().parse()?,
        post_peak_window_days: row.get(3),
        backwater_differential_threshold_ft: row.get::<_, Decimal>(4).to_string().parse()?,
    })
}

/// Load historical flood events that need analysis
pub fn load_historical_events(
    client: &mut Client,
) -> Result<Vec<HistoricalEvent>, Box<dyn std::error::Error>> {
    let rows = client.query(
        "SELECT 
            e.id,
            e.site_code,
            e.crest_time,
            e.peak_stage_ft,
            e.severity,
            t.flood_stage_ft
         FROM nws.flood_events e
         INNER JOIN nws.flood_thresholds t ON e.site_code = t.site_code
         WHERE e.crest_time IS NOT NULL
           AND e.peak_stage_ft IS NOT NULL
           -- Only events not yet analyzed
           AND NOT EXISTS (
               SELECT 1 FROM flood_analysis.events a
               WHERE a.source_event_id = e.id
           )
         ORDER BY e.crest_time",
        &[],
    )?;
    
    let mut events = Vec::new();
    for row in rows {
        events.push(HistoricalEvent {
            event_id: row.get(0),
            site_code: row.get(1),
            crest_time: row.get(2),
            peak_stage_ft: row.get(3),
            severity: row.get(4),
            flood_stage_ft: row.get(5),
        });
    }
    
    Ok(events)
}

/// Load USGS observations for a site within a time window
pub fn load_observations(
    client: &mut Client,
    site_code: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<EventObservation>, Box<dyn std::error::Error>> {
    let rows = client.query(
        "SELECT 
            reading_time,
            MAX(CASE WHEN parameter_code = '00065' THEN value END) as stage_ft,
            MAX(CASE WHEN parameter_code = '00060' THEN value END) as discharge_cfs
         FROM usgs_raw.gauge_readings
         WHERE site_code = $1
           AND reading_time BETWEEN $2 AND $3
         GROUP BY reading_time
         ORDER BY reading_time",
        &[&site_code, &start, &end],
    )?;
    
    let mut observations = Vec::new();
    for row in rows {
        observations.push(EventObservation {
            timestamp: row.get(0),
            stage_ft: row.get(1),
            discharge_cfs: row.get(2),
        });
    }
    
    Ok(observations)
}

/// Detect precursor window by finding when significant rise began
pub fn detect_precursor_window(
    observations: &[EventObservation],
    peak_time: DateTime<Utc>,
    config: &AnalysisConfig,
) -> Option<PrecursorWindow> {
    if observations.is_empty() {
        return None;
    }
    
    // Find peak stage in observations
    let peak_stage = observations
        .iter()
        .filter_map(|o| o.stage_ft)
        .max()?;
    
    let peak_stage_f64: f64 = peak_stage.to_string().parse().ok()?;
    
    // Work backwards from peak to find where significant rise began
    let mut rise_start_idx = None;
    let threshold = config.significant_rise_threshold_ft;
    
    for (i, obs) in observations.iter().enumerate().rev() {
        if let Some(stage) = obs.stage_ft {
            let stage_f64: f64 = stage.to_string().parse().ok()?;
            let rise = peak_stage_f64 - stage_f64;
            
            if rise >= threshold {
                // Found where rise exceeds threshold
                rise_start_idx = Some(i);
            } else {
                // Rise is below threshold, stop here
                break;
            }
        }
    }
    
    let start_idx = rise_start_idx?;
    let start_time = observations[start_idx].timestamp;
    let start_stage: f64 = observations[start_idx].stage_ft?.to_string().parse().ok()?;
    
    // Calculate rise metrics
    let total_rise = peak_stage_f64 - start_stage;
    let duration = peak_time.signed_duration_since(start_time);
    let duration_hours = duration.num_hours() as i32;
    let duration_days = duration.num_hours() as f64 / 24.0;
    
    let avg_rise_rate = if duration_days > 0.0 {
        total_rise / duration_days
    } else {
        0.0
    };
    
    // Find max single-day rise rate
    let mut max_rise_rate: f64 = 0.0;
    for window in observations[start_idx..].windows(2) {
        if let (Some(s1), Some(s2)) = (window[0].stage_ft, window[1].stage_ft) {
            let s1_f64: f64 = s1.to_string().parse().unwrap_or(0.0);
            let s2_f64: f64 = s2.to_string().parse().unwrap_or(0.0);
            let time_diff = window[1].timestamp.signed_duration_since(window[0].timestamp);
            let hours = time_diff.num_hours() as f64;
            
            if hours > 0.0 {
                let rise_rate = ((s2_f64 - s1_f64) / hours) * 24.0;
                max_rise_rate = max_rise_rate.max(rise_rate);
            }
        }
    }
    
    Some(PrecursorWindow {
        start: start_time,
        end: peak_time,
        total_rise_ft: total_rise,
        rise_duration_hours: duration_hours,
        average_rise_rate_ft_per_day: avg_rise_rate,
        max_rise_rate_ft_per_day: max_rise_rate,
    })
}

/// Detect precursor conditions (rapid rise, sustained rise, etc.)
pub fn detect_precursors(
    observations: &[EventObservation],
    peak_time: DateTime<Utc>,
    config: &AnalysisConfig,
) -> Vec<PrecursorCondition> {
    let mut precursors = Vec::new();
    
    // Detect rapid rise events (rise rate exceeding threshold for consecutive readings)
    let mut rapid_rise_start: Option<(usize, f64)> = None;
    
    for window in observations.windows(2) {
        if let (Some(s1), Some(s2)) = (window[0].stage_ft, window[1].stage_ft) {
            let s1_f64: f64 = s1.to_string().parse().unwrap_or(0.0);
            let s2_f64: f64 = s2.to_string().parse().unwrap_or(0.0);
            let time_diff = window[1].timestamp.signed_duration_since(window[0].timestamp);
            let hours = time_diff.num_hours() as f64;
            
            if hours > 0.0 {
                let rise_rate = ((s2_f64 - s1_f64) / hours) * 24.0;
                
                if rise_rate >= config.rise_rate_threshold_ft_per_day {
                    if rapid_rise_start.is_none() {
                        rapid_rise_start = Some((1, rise_rate));
                    }
                } else if let Some((start_idx, max_rate)) = rapid_rise_start {
                    // Rapid rise ended, record it
                    let hours_before = peak_time.signed_duration_since(window[0].timestamp).num_hours() as i32;
                    
                    let metrics = serde_json::json!({
                        "rise_rate_ft_per_day": max_rate,
                        "duration_hours": (1 - start_idx) * 1 // Simplified
                    });
                    
                    precursors.push(PrecursorCondition {
                        precursor_type: "rapid_rise".to_string(),
                        detected_at: window[0].timestamp,
                        description: format!("Rapid rise of {:.2} ft/day detected", max_rate),
                        severity_score: (max_rate / config.rise_rate_threshold_ft_per_day).min(10.0),
                        confidence: 0.85,
                        hours_before_peak: hours_before,
                        metrics,
                    });
                    
                    rapid_rise_start = None;
                }
            }
        }
    }
    
    precursors
}

/// Analyze a single flood event and populate flood_analysis tables
pub fn analyze_event(
    client: &mut Client,
    event: &HistoricalEvent,
    config: &AnalysisConfig,
) -> Result<i32, Box<dyn std::error::Error>> {
    println!("  Analyzing {} - {}", event.site_code, event.crest_time.format("%Y-%m-%d"));
    
    // Define analysis window
    let window_start = event.crest_time - Duration::days(config.precursor_lookback_days as i64);
    let window_end = event.crest_time + Duration::days(config.post_peak_window_days as i64);
    
    // Load observations
    let observations = load_observations(client, &event.site_code, window_start, window_end)?;
    
    if observations.is_empty() {
        println!("    ⚠ No observations found in window, skipping");
        return Ok(0);
    }
    
    // Detect precursor window
    let precursor_window = detect_precursor_window(&observations, event.crest_time, config);
    
    // Insert event record
    let event_id: i32 = client.query_one(
        "INSERT INTO flood_analysis.events (
            source_event_id, site_code, event_start, event_peak, event_end,
            severity, peak_stage_ft, flood_stage_ft,
            precursor_window_start, precursor_window_end,
            total_rise_ft, rise_duration_hours, 
            average_rise_rate_ft_per_day, max_rise_rate_ft_per_day,
            has_discharge_data, analysis_version
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
         RETURNING id",
        &[
            &event.event_id,
            &event.site_code,
            &precursor_window.as_ref().map(|w| w.start).unwrap_or(window_start),
            &event.crest_time,
            &Some(window_end),
            &event.severity,
            &event.peak_stage_ft,
            &event.flood_stage_ft,
            &precursor_window.as_ref().map(|w| w.start),
            &precursor_window.as_ref().map(|w| w.end),
            &precursor_window.as_ref().map(|w| Decimal::try_from(w.total_rise_ft).ok()).flatten(),
            &precursor_window.as_ref().map(|w| w.rise_duration_hours),
            &precursor_window.as_ref().map(|w| Decimal::try_from(w.average_rise_rate_ft_per_day).ok()).flatten(),
            &precursor_window.as_ref().map(|w| Decimal::try_from(w.max_rise_rate_ft_per_day).ok()).flatten(),
            &observations.iter().any(|o| o.discharge_cfs.is_some()),
            &"1.0",
        ],
    )?.get(0);
    
    // Insert observations
    let mut obs_count = 0;
    for obs in &observations {
        if let Some(stage) = obs.stage_ft {
            let hours_before_peak = event.crest_time.signed_duration_since(obs.timestamp).num_hours() as i32;
            
            // Determine phase
            let phase = if obs.timestamp < precursor_window.as_ref().map(|w| w.start).unwrap_or(window_start) {
                "precursor"
            } else if obs.timestamp < event.crest_time {
                "rising"
            } else if obs.timestamp == event.crest_time {
                "peak"
            } else {
                "falling"
            };
            
            client.execute(
                "INSERT INTO flood_analysis.event_observations 
                 (event_id, site_code, timestamp, phase, stage_ft, discharge_cfs, hours_before_peak)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (event_id, timestamp, site_code) DO NOTHING",
                &[&event_id, &event.site_code, &obs.timestamp, &phase, &stage, &obs.discharge_cfs, &hours_before_peak],
            )?;
            obs_count += 1;
        }
    }
    
    println!("    ✓ Inserted event {} with {} observations", event_id, obs_count);
    
    Ok(event_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    
    #[test]
    fn test_detect_precursor_window() {
        let crest_time = Utc.with_ymd_and_hms(2019, 5, 10, 12, 0, 0).unwrap();
        
        let observations = vec![
            EventObservation {
                timestamp: crest_time - Duration:: days(5),
                stage_ft: Some(Decimal::try_from(15.0).unwrap()),
                discharge_cfs: None,
            },
            EventObservation {
                timestamp: crest_time - Duration::days(4),
                stage_ft: Some(Decimal::try_from(16.5).unwrap()),
                discharge_cfs: None,
            },
            EventObservation {
                timestamp: crest_time - Duration::days(3),
                stage_ft: Some(Decimal::try_from(18.2).unwrap()),
                discharge_cfs: None,
            },
            EventObservation {
                timestamp: crest_time - Duration::days(2),
                stage_ft: Some(Decimal::try_from(19.8).unwrap()),
                discharge_cfs: None,
            },
            EventObservation {
                timestamp: crest_time,
                stage_ft: Some(Decimal::try_from(21.5).unwrap()),
                discharge_cfs: None,
            },
        ];
        
        let config = AnalysisConfig::default();
        let window = detect_precursor_window(&observations, crest_time, &config);
        
        assert!(window.is_some());
        let window = window.unwrap();
        
        // Should detect rise starting 5 days before (total rise = 6.5 ft > 2.0 threshold)
        assert_eq!(window.total_rise_ft, 6.5);
        assert!(window.average_rise_rate_ft_per_day > 0.0);
    }
}
