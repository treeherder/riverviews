/// Structured logging for flood monitoring service
///
/// Provides context-rich logging with site/location identifiers,
/// timestamps, and severity levels. Supports both console output
/// and file-based logging for daemon operations.

use chrono::Utc;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Log Levels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warning => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

// ---------------------------------------------------------------------------
// Data Source Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataSource {
    Usgs,
    Cwms,
    Asos,
    Database,
    System,
}

impl fmt::Display for DataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataSource::Usgs => write!(f, "USGS"),
            DataSource::Cwms => write!(f, "CWMS"),
            DataSource::Asos => write!(f, "ASOS"),
            DataSource::Database => write!(f, "DB"),
            DataSource::System => write!(f, "SYS"),
        }
    }
}

// ---------------------------------------------------------------------------
// Failure Classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureType {
    /// Expected failure - station may be offline, decommissioned, or in maintenance
    Expected,
    /// Unexpected failure - indicates service degradation or configuration issue
    Unexpected,
    /// Unknown - cannot determine if this is expected or not
    Unknown,
}

impl fmt::Display for FailureType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FailureType::Expected => write!(f, "EXPECTED"),
            FailureType::Unexpected => write!(f, "UNEXPECTED"),
            FailureType::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

// ---------------------------------------------------------------------------
// Logger Configuration
// ---------------------------------------------------------------------------

/// Global logger instance
static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);

pub struct Logger {
    /// Minimum log level to display
    min_level: LogLevel,
    /// Optional file path for logging
    log_file: Option<String>,
    /// Whether to include timestamps in console output
    console_timestamps: bool,
}

impl Logger {
    /// Initialize the global logger
    pub fn init(min_level: LogLevel, log_file: Option<String>, console_timestamps: bool) {
        let logger = Logger {
            min_level,
            log_file,
            console_timestamps,
        };
        
        *LOGGER.lock().unwrap() = Some(logger);
    }
    
    /// Log a message with the global logger
    fn log(&self, level: LogLevel, source: &DataSource, site_id: Option<&str>, message: &str) {
        if level < self.min_level {
            return;
        }
        
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        
        // Format the log entry
        let site_part = site_id.map(|s| format!(" [{}]", s)).unwrap_or_default();
        let log_entry = format!(
            "{} {} {} {}{}: {}",
            timestamp,
            level,
            source,
            source,
            site_part,
            message
        );
        
        // Console output
        if self.console_timestamps {
            match level {
                LogLevel::Error => eprintln!("{}", log_entry),
                LogLevel::Warning => eprintln!("   {}", log_entry),
                LogLevel::Info => println!("   {}", message),
                LogLevel::Debug => println!("   [DEBUG] {}", message),
            }
        } else {
            match level {
                LogLevel::Error => eprintln!("   ✗ {}{}: {}", source, site_part, message),
                LogLevel::Warning => eprintln!("   ⚠ {}{}: {}", source, site_part, message),
                LogLevel::Info => println!("   {}", message),
                LogLevel::Debug => {}  // Skip debug in non-timestamp mode
            }
        }
        
        // File output
        if let Some(ref path) = self.log_file {
            if let Err(e) = Self::append_to_file(path, &log_entry) {
                eprintln!("Failed to write to log file {}: {}", path, e);
            }
        }
    }
    
    fn append_to_file(path: &str, entry: &str) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{}", entry)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Public Logging Functions
// ---------------------------------------------------------------------------

/// Initialize the global logger
pub fn init_logger(min_level: LogLevel, log_file: Option<&str>, console_timestamps: bool) {
    Logger::init(min_level, log_file.map(String::from), console_timestamps);
}

/// Log a general informational message
pub fn info(source: DataSource, site_id: Option<&str>, message: &str) {
    if let Some(logger) = LOGGER.lock().unwrap().as_ref() {
        logger.log(LogLevel::Info, &source, site_id, message);
    }
}

/// Log a warning message
pub fn warn(source: DataSource, site_id: Option<&str>, message: &str) {
    if let Some(logger) = LOGGER.lock().unwrap().as_ref() {
        logger.log(LogLevel::Warning, &source, site_id, message);
    }
}

/// Log an error message
pub fn error(source: DataSource, site_id: Option<&str>, message: &str) {
    if let Some(logger) = LOGGER.lock().unwrap().as_ref() {
        logger.log(LogLevel::Error, &source, site_id, message);
    }
}

/// Log a debug message
pub fn debug(source: DataSource, site_id: Option<&str>, message: &str) {
    if let Some(logger) = LOGGER.lock().unwrap().as_ref() {
        logger.log(LogLevel::Debug, &source, site_id, message);
    }
}

// ---------------------------------------------------------------------------
// Failure Classification Helpers
// ---------------------------------------------------------------------------

/// Classify a USGS station failure based on the error type and context
pub fn classify_usgs_failure(_site_code: &str, error_message: &str) -> FailureType {
    // Check for known patterns that indicate expected failures
    
    // Empty timeSeries or sentinel values often means station is offline
    if error_message.contains("empty or contained sentinel values")
        || error_message.contains("No timeSeries entries") {
        // Some stations are known to be offline or seasonal
        // This could be expanded with a database of known-offline stations
        FailureType::Unknown
    }
    // HTTP errors might indicate service issues
    else if error_message.contains("HTTP error") {
        FailureType::Unexpected
    }
    // Parse errors suggest API changes or bugs
    else if error_message.contains("Parse error") {
        FailureType::Unexpected
    }
    else {
        FailureType::Unknown
    }
}

/// Classify a CWMS location failure
pub fn classify_cwms_failure(_location_id: &str, error_message: &str) -> FailureType {
    if error_message.contains("HTTP") || error_message.contains("timeout") {
        FailureType::Unexpected
    } else if error_message.contains("No data") {
        FailureType::Unknown
    } else {
        FailureType::Unknown
    }
}

/// Classify an ASOS station failure
pub fn classify_asos_failure(_station_id: &str, error_message: &str) -> FailureType {
    if error_message.contains("HTTP") || error_message.contains("timeout") {
        FailureType::Unexpected
    } else if error_message.contains("No data") {
        FailureType::Unknown
    } else {
        FailureType::Unknown
    }
}

// ---------------------------------------------------------------------------
// Structured Failure Logging
// ---------------------------------------------------------------------------

/// Log a data source failure with automatic classification
pub fn log_usgs_failure(site_code: &str, operation: &str, err: &dyn std::error::Error) {
    let error_msg = err.to_string();
    let failure_type = classify_usgs_failure(site_code, &error_msg);
    
    let message = format!(
        "{} failed [{}]: {}",
        operation,
        failure_type,
        error_msg
    );
    
    match failure_type {
        FailureType::Expected => debug(DataSource::Usgs, Some(site_code), &message),
        FailureType::Unexpected => error(DataSource::Usgs, Some(site_code), &message),
        FailureType::Unknown => warn(DataSource::Usgs, Some(site_code), &message),
    }
}

/// Log a CWMS failure with classification
pub fn log_cwms_failure(location_id: &str, operation: &str, err: &dyn std::error::Error) {
    let error_msg = err.to_string();
    let failure_type = classify_cwms_failure(location_id, &error_msg);
    
    let message = format!(
        "{} failed [{}]: {}",
        operation,
        failure_type,
        error_msg
    );
    
    match failure_type {
        FailureType::Expected => debug(DataSource::Cwms, Some(location_id), &message),
        FailureType::Unexpected => error(DataSource::Cwms, Some(location_id), &message),
        FailureType::Unknown => warn(DataSource::Cwms, Some(location_id), &message),
    }
}

/// Log an ASOS failure with classification
pub fn log_asos_failure(station_id: &str, operation: &str, err: &dyn std::error::Error) {
    let error_msg = err.to_string();
    let failure_type = classify_asos_failure(station_id, &error_msg);
    
    let message = format!(
        "{} failed [{}]: {}",
        operation,
        failure_type,
        error_msg
    );
    
    match failure_type {
        FailureType::Expected => debug(DataSource::Asos, Some(station_id), &message),
        FailureType::Unexpected => error(DataSource::Asos, Some(station_id), &message),
        FailureType::Unknown => warn(DataSource::Asos, Some(station_id), &message),
    }
}

// ---------------------------------------------------------------------------
// Backfill Summary Logging
// ---------------------------------------------------------------------------

/// Log a summary of backfill operations
pub fn log_backfill_summary(source: DataSource, total: usize, successful: usize, failed: usize) {
    let message = format!(
        "Backfill complete: {}/{} successful, {} failed",
        successful,
        total,
        failed
    );
    
    if failed == 0 {
        info(source, None, &message);
    } else if successful == 0 {
        error(source, None, &message);
    } else {
        warn(source, None, &message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warning);
        assert!(LogLevel::Warning < LogLevel::Error);
    }
    
    #[test]
    fn test_failure_classification() {
        let empty_series_error = "No data available for site: No timeSeries entries in response";
        let result = classify_usgs_failure("05568500", empty_series_error);
        assert_eq!(result, FailureType::Unknown);
        
        let http_error = "HTTP error: 500";
        let result = classify_usgs_failure("05568500", http_error);
        assert_eq!(result, FailureType::Unexpected);
    }
}
