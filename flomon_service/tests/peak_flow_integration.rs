/// Integration tests for Peak Flow event ingestion
///
/// These tests verify:
/// 1. Database schema can accept flood event data
/// 2. Peak flow parser correctly identifies floods
/// 3. Full pipeline: RDB → parse → identify → insert → query
/// 4. Data types match PostgreSQL schema expectations
///
/// Prerequisites:
/// - PostgreSQL running with flopro_db database
/// - DATABASE_URL set in .env
/// - sql/003_flood_metadata.sql migration applied
///
/// Run with: cargo test --test peak_flow_integration -- --test-threads=1

use flomon_service::config::load_config;
use flomon_service::ingest::peak_flow::{
    parse_rdb, identify_flood_events, FloodThresholds, FloodSeverity,
};

use chrono::{Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use postgres::{Client, NoTls};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::env;

// Test RDB data (Peoria Pool sample from real USGS data)
const TEST_RDB: &str = r#"#
# U.S. Geological Survey
# National Water Information System
# Retrieved: 2026-02-20 00:56:35 EST
#
# This file contains the annual peak streamflow data.
#
# This information includes the following fields:
#
#  agency_cd     Agency Code
#  site_no       USGS station number
#  peak_dt       Date of peak streamflow (format YYYY-MM-DD)
#  peak_tm       Time of peak streamflow (24 hour format, 00:00 - 23:59)
#  peak_va       Annual peak streamflow value in cfs
#  peak_cd       Peak Discharge-Qualification codes
#  gage_ht       Gage height for the associated peak streamflow in feet
#  gage_ht_cd    Gage height qualification codes
#
# Sites in this file include:
#  USGS 05567500 ILLINOIS RIVER AT PEORIA, IL
#
agency_cd	site_no	peak_dt	peak_tm	peak_va	peak_cd	gage_ht	gage_ht_cd	year_last_pk	ag_dt	ag_tm	ag_gage_ht	ag_gage_ht_cd
5s	15s	10d	6s	8s	33s	8s	27s	4s	10d	6s	8s	27s
USGS	05567500	1982-12-04		44800		20.21						
USGS	05567500	1986-10-04		35300		19.61						
USGS	05567500	2013-04-18	07:45	28700		18.79						
USGS	05567500	2015-12-29	10:30	31400		19.09						
USGS	05567500	2019-05-02		18800		17.65						
"#;

fn get_test_client() -> Client {
    use flomon_service::db::connect_and_verify;
    
    // Use validation helper with clear error messages
    connect_and_verify(&["usgs_raw", "nws", "usace"])
        .unwrap_or_else(|e| {
            eprintln!("\n{}\n", "=".repeat(80));
            eprintln!("INTEGRATION TEST SETUP ERROR");
            eprintln!("{}", "=".repeat(80));
            eprintln!("\n{}\n", e);
            eprintln!("{}", "=".repeat(80));
            eprintln!("\nRun setup validation: ./scripts/validate_db_setup.sh\n");
            panic!("Database setup validation failed");
        })
}

fn clean_test_data(client: &mut Client) {
    // Delete test flood events to ensure clean slate
    client.execute(
        "DELETE FROM nws.flood_events WHERE site_code = '05567500' AND data_source LIKE '%TEST%'",
        &[]
    ).ok();
}

#[test]
fn test_database_schema_exists() {
    let mut client = get_test_client();
    
    // Verify nws.flood_events table exists
    let result = client.query_one(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_schema = 'nws' 
            AND table_name = 'flood_events'
        )",
        &[]
    ).expect("Failed to query schema");
    
    let exists: bool = result.get(0);
    assert!(exists, "nws.flood_events table does not exist - run sql/003_flood_metadata.sql");
}

#[test]
fn test_database_schema_has_required_columns() {
    let mut client = get_test_client();
    
    // Check all required columns exist with correct types
    let columns = client.query(
        "SELECT column_name, data_type 
         FROM information_schema.columns 
         WHERE table_schema = 'nws' 
         AND table_name = 'flood_events'
         ORDER BY ordinal_position",
        &[]
    ).expect("Failed to query columns");
    
    let column_names: Vec<String> = columns.iter()
        .map(|row| row.get::<_, String>(0))
        .collect();
    
    // Verify essential columns
    assert!(column_names.contains(&"id".to_string()));
    assert!(column_names.contains(&"site_code".to_string()));
    assert!(column_names.contains(&"event_start".to_string()));
    assert!(column_names.contains(&"crest_time".to_string()));
    assert!(column_names.contains(&"peak_stage_ft".to_string()));
    assert!(column_names.contains(&"severity".to_string()));
    assert!(column_names.contains(&"data_source".to_string()));
}

#[test]
fn test_parse_rdb_produces_valid_records() {
    let records = parse_rdb(TEST_RDB)
        .expect("Failed to parse test RDB data");
    
    assert_eq!(records.len(), 5, "Should parse 5 peak flow records");
    
    // Verify first record
    assert_eq!(records[0].site_code, "05567500");
    assert_eq!(records[0].peak_date, NaiveDate::from_ymd_opt(1982, 12, 4).unwrap());
    assert_eq!(records[0].gage_height_ft, Some(20.21));
    assert_eq!(records[0].peak_discharge_cfs, Some(44800.0));
    
    // Verify record with timestamp
    assert_eq!(records[2].peak_date, NaiveDate::from_ymd_opt(2013, 4, 18).unwrap());
    assert_eq!(records[2].peak_time, Some(NaiveTime::from_hms_opt(7, 45, 0).unwrap()));
}

#[test]
fn test_identify_flood_events_with_peoria_thresholds() {
    let records = parse_rdb(TEST_RDB).unwrap();
    
    // Peoria Pool thresholds from usgs_stations.toml
    let thresholds = FloodThresholds {
        flood_stage_ft: 18.0,
        moderate_flood_stage_ft: 20.0,
        major_flood_stage_ft: 22.0,
    };
    
    let events = identify_flood_events(&records, &thresholds);
    
    // 4 out of 5 records should be floods (17.65 ft is below 18.0 ft threshold)
    assert_eq!(events.len(), 4, "Should identify 4 flood events");
    
    // Check severity classification
    let major_floods: Vec<_> = events.iter()
        .filter(|e| e.severity == FloodSeverity::Major)
        .collect();
    assert_eq!(major_floods.len(), 0, "No major floods (all below 22.0 ft)");
    
    let moderate_floods: Vec<_> = events.iter()
        .filter(|e| e.severity == FloodSeverity::Moderate)
        .collect();
    assert_eq!(moderate_floods.len(), 1, "1982-12-04 at 20.21 ft is moderate");
    
    let minor_floods: Vec<_> = events.iter()
        .filter(|e| e.severity == FloodSeverity::Flood)
        .collect();
    assert_eq!(minor_floods.len(), 3, "3 minor floods");
}

#[test]
fn test_insert_flood_event_into_database() {
    let mut client = get_test_client();
    clean_test_data(&mut client);
    
    // Create a test flood event (UTC timezone-aware)
    let crest_time = Utc.with_ymd_and_hms(2013, 4, 18, 7, 45, 0).unwrap();
    let event_start = crest_time - Duration::hours(24);
    
    // Insert test event
    let inserted = client.execute(
        "INSERT INTO nws.flood_events 
         (site_code, event_start, event_end, crest_time, peak_stage_ft, severity, data_source)
         VALUES ($1, $2, NULL, $3, $4, $5, $6)",
        &[
            &"05567500",
            &event_start,
            &crest_time,
            &Decimal::from_f64_retain(18.79).unwrap(),
            &"flood",
            &"TEST DATA",
        ]
    ).expect("Failed to insert test flood event");
    
    assert_eq!(inserted, 1, "Should insert exactly one row");
    
    // Verify we can query it back
    let result = client.query_one(
        "SELECT site_code, peak_stage_ft, severity, crest_time 
         FROM nws.flood_events 
         WHERE site_code = '05567500' AND data_source = 'TEST DATA'
         ORDER BY crest_time DESC
         LIMIT 1",
        &[]
    ).expect("Failed to query inserted event");
    
    let site_code: String = result.get(0);
    let peak_stage_ft: Decimal = result.get(1);
    let severity: String = result.get(2);
    let queried_crest: chrono::DateTime<Utc> = result.get(3);
    
    assert_eq!(site_code, "05567500");
    assert!((peak_stage_ft.to_f64().unwrap() - 18.79).abs() < 0.01, "Expected ~18.79, got {}", peak_stage_ft);
    assert_eq!(severity, "flood");
    assert_eq!(queried_crest, crest_time);
    
    // Clean up
    clean_test_data(&mut client);
}

#[test]
fn test_full_pipeline_rdb_to_database() {
    let mut client = get_test_client();
    clean_test_data(&mut client);
    
    // Step 1: Parse RDB
    let records = parse_rdb(TEST_RDB)
        .expect("Failed to parse RDB");
    assert_eq!(records.len(), 5);
    
    // Step 2: Identify floods
    let thresholds = FloodThresholds {
        flood_stage_ft: 18.0,
        moderate_flood_stage_ft: 20.0,
        major_flood_stage_ft: 22.0,
    };
    let events = identify_flood_events(&records, &thresholds);
    assert_eq!(events.len(), 4);
    
    // Step 3: Insert into database
    let mut tx = client.transaction()
        .expect("Failed to start transaction");
    
    for event in &events {
        // Convert NaiveDateTime to DateTime<Utc>
        let crest_utc = Utc.from_utc_datetime(&event.crest_time);
        let event_start = crest_utc - Duration::hours(24);
        
        tx.execute(
            "INSERT INTO nws.flood_events 
             (site_code, event_start, event_end, crest_time, peak_stage_ft, severity, data_source)
             VALUES ($1, $2, NULL, $3, $4, $5, $6)",
            &[
                &event.site_code,
                &event_start,
                &crest_utc,
                &Decimal::from_f64_retain(event.peak_stage_ft).unwrap(),
                &event.severity.as_str(),
                &"TEST - Full Pipeline",
            ]
        ).expect("Failed to insert event");
    }
    
    tx.commit().expect("Failed to commit transaction");
    
    // Step 4: Query and verify
    let count: i64 = client.query_one(
        "SELECT COUNT(*) FROM nws.flood_events 
         WHERE site_code = '05567500' AND data_source = 'TEST - Full Pipeline'",
        &[]
    ).expect("Failed to count events").get(0);
    
    assert_eq!(count, 4, "Should have inserted 4 flood events");
    
    // Verify the worst flood (1982-12-04, 20.21 ft, moderate)
    let worst = client.query_one(
        "SELECT peak_stage_ft, severity, crest_time::date 
         FROM nws.flood_events 
         WHERE site_code = '05567500' AND data_source = 'TEST - Full Pipeline'
         ORDER BY peak_stage_ft DESC
         LIMIT 1",
        &[]
    ).expect("Failed to query worst flood");
    
    let peak: Decimal = worst.get(0);
    let severity: String = worst.get(1);
    let date: NaiveDate = worst.get(2);
    
    assert!((peak.to_f64().unwrap() - 20.21).abs() < 0.01, "Expected ~20.21, got {}", peak);
    assert_eq!(severity, "moderate");
    assert_eq!(date, NaiveDate::from_ymd_opt(1982, 12, 4).unwrap());
    
    // Clean up
    clean_test_data(&mut client);
}

#[test]
fn test_severity_enum_values_accepted_by_database() {
    let mut client = get_test_client();
    clean_test_data(&mut client);
    
    let crest_time = Utc.with_ymd_and_hms(2020, 1, 1, 12, 0, 0).unwrap();
    let event_start = crest_time - Duration::hours(24);
    
    // Test all three severity levels
    for (severity, stage) in [("flood", 18.5), ("moderate", 20.5), ("major", 24.0)] {
        let result = client.execute(
            "INSERT INTO nws.flood_events 
             (site_code, event_start, crest_time, peak_stage_ft, severity, data_source)
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[
                &"05567500",
                &event_start,
                &crest_time,
                &Decimal::from_f64_retain(stage).unwrap(),
                &severity,
                &"TEST - Severity Enum",
            ]
        );
        
        assert!(result.is_ok(), "Failed to insert severity '{}'", severity);
    }
    
    // Verify all three inserted
    let count: i64 = client.query_one(
        "SELECT COUNT(*) FROM nws.flood_events 
         WHERE data_source = 'TEST - Severity Enum'",
        &[]
    ).expect("Failed to count severity test events").get(0);
    
    assert_eq!(count, 3);
    
    // Clean up
    clean_test_data(&mut client);
}

#[test]
fn test_stations_toml_thresholds_match_database_expectations() {
    // Load thresholds from usgs_stations.toml
    let stations = load_config();
    
    // Find stations with thresholds
    let stations_with_thresholds: Vec<_> = stations.iter()
        .filter(|s| s.thresholds.is_some())
        .collect();
    
    assert!(
        stations_with_thresholds.len() >= 5,
        "Should have at least 5 stations with thresholds"
    );
    
    // Verify each threshold set is valid (ascending order)
    for station in stations_with_thresholds {
        let t = station.thresholds.as_ref().unwrap();
        
        assert!(
            t.flood_stage_ft < t.moderate_flood_stage_ft,
            "{}: flood_stage must be < moderate_flood_stage",
            station.site_code
        );
        assert!(
            t.moderate_flood_stage_ft < t.major_flood_stage_ft,
            "{}: moderate_flood_stage must be < major_flood_stage",
            station.site_code
        );
        
        // Verify can create FloodThresholds
        let flood_thresholds = FloodThresholds {
            flood_stage_ft: t.flood_stage_ft,
            moderate_flood_stage_ft: t.moderate_flood_stage_ft,
            major_flood_stage_ft: t.major_flood_stage_ft,
        };
        
        // Test severity classification works
        assert_eq!(
            FloodSeverity::from_stage(
                t.flood_stage_ft + 0.1,
                flood_thresholds.flood_stage_ft,
                flood_thresholds.moderate_flood_stage_ft,
                flood_thresholds.major_flood_stage_ft,
            ),
            Some(FloodSeverity::Flood)
        );
    }
}

#[test]
fn test_duplicate_prevention() {
    let mut client = get_test_client();
    clean_test_data(&mut client);
    
    let crest_time = Utc.with_ymd_and_hms(2013, 4, 18, 7, 45, 0).unwrap();
    let event_start = crest_time - Duration::hours(24);
    
    // Insert first time
    client.execute(
        "INSERT INTO nws.flood_events 
         (site_code, event_start, crest_time, peak_stage_ft, severity, data_source)
         VALUES ($1, $2, $3, $4, $5, $6)",
        &[
            &"05567500",
            &event_start,
            &crest_time,
            &Decimal::from_f64_retain(18.79).unwrap(),
            &"flood",
            &"TEST - Duplicate Check",
        ]
    ).expect("Failed to insert first event");
    
    // Check for duplicate (simulating what ingest_peak_flows.rs does)
    let exists: i64 = client.query_one(
        "SELECT COUNT(*) FROM nws.flood_events 
         WHERE site_code = $1 AND crest_time = $2",
        &[&"05567500", &crest_time],
    ).expect("Failed to check duplicate").get(0);
    
    assert_eq!(exists, 1, "Should find the existing event");
    
    // Should skip insert if exists > 0 (this is what the binary does)
    // Let's verify we CAN query by both site_code and crest_time
    let result = client.query_one(
        "SELECT peak_stage_ft FROM nws.flood_events 
         WHERE site_code = $1 AND crest_time = $2",
        &[&"05567500", &crest_time],
    ).expect("Failed to query by site_code and crest_time");
    
    let stage: Decimal = result.get(0);
    assert!((stage.to_f64().unwrap() - 18.79).abs() < 0.01, "Expected ~18.79, got {}", stage);
    
    // Clean up
    clean_test_data(&mut client);
}
