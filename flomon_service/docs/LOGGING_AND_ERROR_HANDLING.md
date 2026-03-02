# Logging and Error Handling

## Overview

The Flood Monitoring Service uses structured logging to provide context-rich diagnostic information about data source operations. All log messages include:

- **Timestamp** (in log files)
- **Log Level** (DEBUG, INFO, WARN, ERROR)
- **Data Source** (USGS, CWMS, ASOS, DB, SYS)
- **Site/Location Identifier** (when applicable)
- **Failure Classification** (EXPECTED, UNEXPECTED, UNKNOWN)
- **Detailed Error Message**

## Log Outputs

The daemon writes to two destinations:

1. **Console (stdout/stderr)** - Clean, emoji-enhanced output for interactive monitoring
2. **Log File** (`flomon_service.log`) - Timestamped, machine-parseable records

### Log File Location

By default: `./flomon_service.log` in the working directory

To change the location, modify `main.rs`:
```rust
let log_file = "/var/log/flomon/service.log";  // Custom path
logging::init_logger(log_level, Some(log_file), console_timestamps);
```

### Log Levels

| Level | Purpose | Example |
|-------|---------|---------|
| **DEBUG** | Verbose diagnostic info | Expected failures from known-offline stations |
| **INFO** | Normal operations | Successful data ingestion, backfill completion |
| **WARN** | Degraded but functional | Partial failures, unknown failure types |
| **ERROR** | Service degradation | Unexpected API errors, database failures |

### UNEXPECTED Failures

**Characteristics:**
- HTTP errors (500, 503, timeout)
- Parse errors (API format changed)
- Database connection failures
- Authentication errors

## Log File Analysis

### Viewing Recent Errors

```bash
# Last 50 error/warning messages
grep -E "(ERROR|WARN)" flomon_service.log | tail -50

# Errors for specific site
grep "05568500" flomon_service.log | grep ERROR

# Failures in last hour
grep "$(date -u +%Y-%m-%d\ %H)" flomon_service.log | grep -E "(ERROR|WARN)"
```

### Counting Failure Rates

```bash
# Total USGS failures today
grep "$(date -u +%Y-%m-%d)" flomon_service.log | grep "USGS" | grep -c ERROR

# Breakdown by failure type
grep "$(date -u +%Y-%m-%d)" flomon_service.log | grep "USGS" | grep -o "\[.*\]" | sort | uniq -c

# Success rate for last 24 hours
total=$(grep "Backfilling" flomon_service.log | tail -1 | grep -o "[0-9]* USGS" | awk '{print $1}')
failed=$(grep -c "failed \[" flomon_service.log | tail -1)
echo "Success rate: $(( ($total - $failed) * 100 / $total ))%"
```

### Monitoring Trends

```bash
# Group failures by hour (identify outage windows)
awk '/ERROR|WARN/ {print substr($0, 1, 13)}' flomon_service.log | sort | uniq -c

# Most problematic sites
grep -o "\[0-9]\{8\}\]" flomon_service.log | sort | uniq -c | sort -rn | head -10
```

## Integration with Monitoring Tools

### Log Rotation

Prevent unbounded log growth with logrotate:

```bash
# Create /etc/logrotate.d/flomon_service
/var/log/flomon/service.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 0644 riverviews riverviews
}
```

### Syslog Integration

Forward critical errors to syslog:

Modify `logging.rs` to add:
```rust
use syslog::{Facility, Formatter3164};

// In log() method, add:
if level >= LogLevel::Error {
    let formatter = Formatter3164::init("flomon_service", Facility::LOG_DAEMON);
    syslog::unix(formatter).err(message).ok();
}
```

## Summary

The logging system is designed to:

1. **Identify Problems** - Clear, site-specific error messages
2. **Classify Severity** - Automatic tagging of expected vs. unexpected failures
3. **Facilitate Diagnosis** - Structured logs with timestamps and context
4. **Support Operations** - Machine-parseable format for monitoring tools
5. **Document Behavior** - Audit trail of all data source operations