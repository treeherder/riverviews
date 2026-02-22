/// USGS Peak Streamflow Database Parser
///
/// Parses annual peak streamflow data from USGS NWIS Peak Streamflow database.
/// Format: Tab-delimited RDB (Research Data BYte-stream)
/// Source: https://nwis.waterdata.usgs.gov/{state}/nwis/peak?site_no={site}&agency_cd=USGS&format=rdb
///
/// This authoritative historical dataset contains complete annual peak stage and discharge
/// records spanning 80-110+ years for most Illinois River gauges. Each row represents
/// the highest water level and flow rate recorded during a water year (Oct 1 - Sep 30).
///
/// Used to populate nws.flood_events table with ground-truth historical flood events
/// for training predictive models and validating alert systems.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::collections::HashMap;

/// Parsed peak flow record from USGS RDB format
#[derive(Debug, Clone)]
pub struct PeakFlowRecord {
    pub site_code: String,
    pub peak_date: NaiveDate,
    pub peak_time: Option<NaiveTime>,
    pub peak_discharge_cfs: Option<f64>,
    pub peak_qualification_codes: Vec<String>,
    pub gage_height_ft: Option<f64>,
    pub gage_height_qualification_codes: Vec<String>,
    pub water_year: Option<u16>,
    pub alternate_gage_height_ft: Option<f64>,
}

/// Flood event derived from peak flow record + threshold comparison
#[derive(Debug, Clone)]
pub struct FloodEvent {
    pub site_code: String,
    pub crest_time: NaiveDateTime,
    pub peak_stage_ft: f64,
    pub severity: FloodSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloodSeverity {
    Flood,      // Minor flooding (stage >= flood_stage_ft)
    Moderate,   // Moderate flooding (stage >= moderate_flood_stage_ft)
    Major,      // Major flooding (stage >= major_flood_stage_ft)
}

impl FloodSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            FloodSeverity::Flood => "flood",
            FloodSeverity::Moderate => "moderate",
            FloodSeverity::Major => "major",
        }
    }

    /// Determine severity based on peak stage and thresholds
    pub fn from_stage(
        peak_stage_ft: f64,
        flood_stage_ft: f64,
        moderate_stage_ft: f64,
        major_stage_ft: f64,
    ) -> Option<Self> {
        if peak_stage_ft >= major_stage_ft {
            Some(FloodSeverity::Major)
        } else if peak_stage_ft >= moderate_stage_ft {
            Some(FloodSeverity::Moderate)
        } else if peak_stage_ft >= flood_stage_ft {
            Some(FloodSeverity::Flood)
        } else {
            None // Below flood stage
        }
    }
}

/// Station flood thresholds for event detection
#[derive(Debug, Clone)]
pub struct FloodThresholds {
    pub flood_stage_ft: f64,
    pub moderate_flood_stage_ft: f64,
    pub major_flood_stage_ft: f64,
}

/// Parse USGS Peak Streamflow RDB format
///
/// RDB format structure:
/// - Lines starting with '#' are comments (metadata header)
/// - First non-comment line: tab-delimited column names
/// - Second non-comment line: tab-delimited format descriptors (e.g., "5s", "10d")
/// - Remaining lines: tab-delimited data rows
///
/// Key fields:
/// - agency_cd: "USGS"
/// - site_no: 8-digit station code
/// - peak_dt: Date (YYYY-MM-DD)
/// - peak_tm: Time (HH:MM 24-hour, often empty for older records)
/// - peak_va: Peak discharge (cfs)
/// - peak_cd: Qualification codes (comma-separated)
/// - gage_ht: Gage height (feet) - THIS IS THE FLOOD STAGE VALUE
/// - gage_ht_cd: Gage height qualification codes
///
/// # Arguments
/// * `rdb_text` - Raw RDB format text from USGS API
///
/// # Returns
/// Vector of parsed peak flow records
pub fn parse_rdb(rdb_text: &str) -> Result<Vec<PeakFlowRecord>, String> {
    let lines: Vec<&str> = rdb_text.lines().collect();
    
    // Skip comment lines (start with #)
    let mut data_lines = lines.iter()
        .filter(|line| !line.trim().starts_with('#') && !line.trim().is_empty());
    
    // First non-comment line: column headers
    let header_line = data_lines.next()
        .ok_or("No header line found in RDB data")?;
    let headers: Vec<&str> = header_line.split('\t').collect();
    
    // Build column index map
    let mut col_map: HashMap<&str, usize> = HashMap::new();
    for (idx, &header) in headers.iter().enumerate() {
        col_map.insert(header, idx);
    }
    
    // Second non-comment line: format descriptors (skip)
    data_lines.next().ok_or("No format line found in RDB data")?;
    
    // Parse data rows
    let mut records = Vec::new();
    for line in data_lines {
        let fields: Vec<&str> = line.split('\t').collect();
        
        // Required fields
        let site_code = fields.get(*col_map.get("site_no").ok_or("Missing site_no column")?)
            .ok_or("Missing site_no value")?
            .to_string();
        
        let peak_dt_str = fields.get(*col_map.get("peak_dt").ok_or("Missing peak_dt column")?)
            .ok_or("Missing peak_dt value")?;
        
        let peak_date = NaiveDate::parse_from_str(peak_dt_str, "%Y-%m-%d")
            .map_err(|e| format!("Invalid peak_dt '{}': {}", peak_dt_str, e))?;
        
        // Optional time field (often empty for historical records)
        let peak_time = col_map.get("peak_tm")
            .and_then(|&idx| fields.get(idx))
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| NaiveTime::parse_from_str(s.trim(), "%H:%M").ok());
        
        // Optional discharge field
        let peak_discharge_cfs = col_map.get("peak_va")
            .and_then(|&idx| fields.get(idx))
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| s.trim().parse::<f64>().ok());
        
        // Peak qualification codes (comma-separated)
        let peak_qualification_codes = col_map.get("peak_cd")
            .and_then(|&idx| fields.get(idx))
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.split(',').map(|c| c.trim().to_string()).collect())
            .unwrap_or_default();
        
        // CRITICAL: Gage height is the flood stage indicator
        let gage_height_ft = col_map.get("gage_ht")
            .and_then(|&idx| fields.get(idx))
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| s.trim().parse::<f64>().ok());
        
        // Gage height qualification codes
        let gage_height_qualification_codes = col_map.get("gage_ht_cd")
            .and_then(|&idx| fields.get(idx))
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.split(',').map(|c| c.trim().to_string()).collect())
            .unwrap_or_default();
        
        // Water year (optional)
        let water_year = col_map.get("year_last_pk")
            .and_then(|&idx| fields.get(idx))
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| s.trim().parse::<u16>().ok());
        
        // Alternate gage height (max for year if different from peak discharge time)
        let alternate_gage_height_ft = col_map.get("ag_gage_ht")
            .and_then(|&idx| fields.get(idx))
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| s.trim().parse::<f64>().ok());
        
        records.push(PeakFlowRecord {
            site_code,
            peak_date,
            peak_time,
            peak_discharge_cfs,
            peak_qualification_codes,
            gage_height_ft,
            gage_height_qualification_codes,
            water_year,
            alternate_gage_height_ft,
        });
    }
    
    Ok(records)
}

/// Convert peak flow records to flood events based on thresholds
///
/// Only peaks where gage_height_ft >= flood_stage_ft are considered flood events.
/// Severity is determined by which threshold was exceeded.
///
/// # Arguments
/// * `records` - Parsed peak flow records
/// * `thresholds` - Station flood thresholds (from usgs_stations.toml)
///
/// # Returns
/// Vector of flood events for database insertion
pub fn identify_flood_events(
    records: &[PeakFlowRecord],
    thresholds: &FloodThresholds,
) -> Vec<FloodEvent> {
    let mut events = Vec::new();
    
    for record in records {
        // Use gage_height_ft as primary flood indicator
        // If missing, fall back to alternate gage height (ag_gage_ht)
        let peak_stage_ft = match record.gage_height_ft {
            Some(ht) => ht,
            None => match record.alternate_gage_height_ft {
                Some(ht) => ht,
                None => continue, // Skip records with no stage data
            },
        };
        
        // Determine severity
        let severity = match FloodSeverity::from_stage(
            peak_stage_ft,
            thresholds.flood_stage_ft,
            thresholds.moderate_flood_stage_ft,
            thresholds.major_flood_stage_ft,
        ) {
            Some(sev) => sev,
            None => continue, // Below flood stage - not a flood event
        };
        
        // Construct crest timestamp
        // Default to noon if time not specified (common for older records)
        let peak_time = record.peak_time.unwrap_or(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        let crest_time = NaiveDateTime::new(record.peak_date, peak_time);
        
        events.push(FloodEvent {
            site_code: record.site_code.clone(),
            crest_time,
            peak_stage_ft,
            severity,
        });
    }
    
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rdb_basic() {
        let rdb_data = r#"# Comment line
# Another comment
agency_cd	site_no	peak_dt	peak_tm	peak_va	peak_cd	gage_ht	gage_ht_cd	year_last_pk
5s	15s	10d	6s	8s	33s	8s	27s	4s
USGS	05567500	2013-04-18		28700		18.79			
USGS	05567500	2015-12-29		31400		19.09			
"#;
        
        let records = parse_rdb(rdb_data).unwrap();
        assert_eq!(records.len(), 2);
        
        assert_eq!(records[0].site_code, "05567500");
        assert_eq!(records[0].peak_date, NaiveDate::from_ymd_opt(2013, 4, 18).unwrap());
        assert_eq!(records[0].gage_height_ft, Some(18.79));
        
        assert_eq!(records[1].gage_height_ft, Some(19.09));
    }

    #[test]
    fn test_identify_flood_events() {
        let records = vec![
            PeakFlowRecord {
                site_code: "05567500".to_string(),
                peak_date: NaiveDate::from_ymd_opt(2013, 4, 18).unwrap(),
                peak_time: None,
                peak_discharge_cfs: Some(28700.0),
                peak_qualification_codes: vec![],
                gage_height_ft: Some(18.79),
                gage_height_qualification_codes: vec![],
                water_year: None,
                alternate_gage_height_ft: None,
            },
            PeakFlowRecord {
                site_code: "05567500".to_string(),
                peak_date: NaiveDate::from_ymd_opt(2019, 5, 2).unwrap(),
                peak_time: None,
                peak_discharge_cfs: Some(18800.0),
                peak_qualification_codes: vec![],
                gage_height_ft: Some(17.65),
                gage_height_qualification_codes: vec![],
                water_year: None,
                alternate_gage_height_ft: None,
            },
        ];
        
        let thresholds = FloodThresholds {
            flood_stage_ft: 18.0,
            moderate_flood_stage_ft: 20.0,
            major_flood_stage_ft: 22.0,
        };
        
        let events = identify_flood_events(&records, &thresholds);
        
        // First record: 18.79 ft >= 18.0 ft (flood stage)
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].peak_stage_ft, 18.79);
        assert_eq!(events[0].severity, FloodSeverity::Flood);
        
        // Second record: 17.65 ft < 18.0 ft (below flood stage - not included)
    }

    #[test]
    fn test_flood_severity_classification() {
        let thresholds = FloodThresholds {
            flood_stage_ft: 18.0,
            moderate_flood_stage_ft: 20.0,
            major_flood_stage_ft: 22.0,
        };
        
        // Below flood stage
        assert!(FloodSeverity::from_stage(17.5, 18.0, 20.0, 22.0).is_none());
        
        // Flood (minor)
        assert_eq!(
            FloodSeverity::from_stage(18.5, 18.0, 20.0, 22.0),
            Some(FloodSeverity::Flood)
        );
        
        // Moderate
        assert_eq!(
            FloodSeverity::from_stage(20.5, 18.0, 20.0, 22.0),
            Some(FloodSeverity::Moderate)
        );
        
        // Major
        assert_eq!(
            FloodSeverity::from_stage(24.0, 18.0, 20.0, 22.0),
            Some(FloodSeverity::Major)
        );
    }

    #[test]
    fn test_parse_with_qualification_codes() {
        let rdb_data = r#"# Comment
agency_cd	site_no	peak_dt	peak_tm	peak_va	peak_cd	gage_ht	gage_ht_cd
5s	15s	10d	6s	8s	33s	8s	27s
USGS	05568500	2008-09-20		101000	5	24.68	
"#;
        
        let records = parse_rdb(rdb_data).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].peak_qualification_codes, vec!["5"]);
        assert_eq!(records[0].peak_discharge_cfs, Some(101000.0));
    }
}
