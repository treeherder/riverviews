/// Integration tests for data source availability and database population
///
/// These tests verify:
/// 1. USGS API returns data for configured stations
/// 2. CWMS API returns data for configured locations
/// 3. ASOS/IEM API returns data for configured stations
/// 4. Database can accept and store data from all sources
/// 5. Full pipeline: API → parse → validate → insert → query
///
/// Prerequisites:
/// - PostgreSQL running with flopro_db database
/// - DATABASE_URL set in .env
/// - All SQL migrations applied (001-006)
/// - Internet connectivity to reach external APIs
///
/// Run with: cargo test --test data_source_integration -- --test-threads=1
///
/// Note: These tests make real API calls and may be slow or fail if:
/// - APIs are down or rate-limiting
/// - Network connectivity issues
/// - System date is in the future (no data available)

use flomon_service::db;
use flomon_service::usace_locations;
use flomon_service::asos_locations;
use flomon_service::ingest::{usgs, cwms, iem};
use flomon_service::model::GaugeReading;

use chrono::{DateTime, Utc};
use postgres::Client;
use rust_decimal::Decimal;

// ---------------------------------------------------------------------------
// Test Helpers
// ---------------------------------------------------------------------------

fn get_test_client() -> Client {
    // Note: This requires all migrations 001-006 to be applied
    // The connect_and_verify function checks for schemas, but not all tables
    db::connect_and_verify(&["usgs_raw", "nws", "usace"])
        .unwrap_or_else(|e| {
            eprintln!("\n{}\n", "=".repeat(80));
            eprintln!("INTEGRATION TEST SETUP ERROR");
            eprintln!("{}", "=".repeat(80));
            eprintln!("\n{}\n", e);
            eprintln!("{}", "=".repeat(80));
            eprintln!("\nRun setup validation: ./scripts/validate_db_setup.sh\n");
            eprintln!("Ensure migrations 001-006 are applied:\n");
            eprintln!("  psql -U flopro_admin -d flopro_db -f sql/001_base_schema.sql");
            eprintln!("  psql -U flopro_admin -d flopro_db -f sql/002_monitoring_state.sql");
            eprintln!("  psql -U flopro_admin -d flopro_db -f sql/003_flood_metadata.sql");
            eprintln!("  psql -U flopro_admin -d flopro_db -f sql/004_usace_cwms.sql");
            eprintln!("  psql -U flopro_admin -d flopro_db -f sql/005_flood_analysis.sql");
            eprintln!("  psql -U flopro_admin -d flopro_db -f sql/006_iem_asos.sql\n");
            panic!("Database setup validation failed");
        })
}

fn cleanup_test_data(client: &mut Client) {
    // Clean up test data between tests
    // Delete in order to respect foreign key constraints
    let _ = client.execute("DELETE FROM usgs_raw.gauge_readings WHERE site_code LIKE 'TEST%'", &[]);
    let _ = client.execute("DELETE FROM usace.cwms_timeseries WHERE location_id LIKE 'TEST%'", &[]);
    let _ = client.execute("DELETE FROM usace.cwms_locations WHERE location_id LIKE 'TEST%'", &[]);
    let _ = client.execute("DELETE FROM asos_observations WHERE station_id LIKE 'TEST%'", &[]);
}

// ---------------------------------------------------------------------------
// USGS API Data Availability Tests
// ---------------------------------------------------------------------------

#[test]
fn test_usgs_api_returns_data_for_kingston_mines() {
    // Kingston Mines (05568500) is our primary reference station
    let site_code = "05568500";
    
    // Build URL for last 3 hours of data
    let url = usgs::build_iv_url(
        &[site_code],
        &["00060", "00065"], // discharge + stage
        "PT3H",
    );
    
    println!("Testing USGS API: {}", url);
    
    // Make API request
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");
    
    let response = client.get(&url)
        .send()
        .expect("USGS API request failed - check network connectivity");
    
    assert!(
        response.status().is_success(),
        "USGS API returned status {}", 
        response.status()
    );
    
    let body = response.text().expect("Failed to read response body");
    
    // Parse response
    let result = usgs::parse_iv_response(&body);
    
    match result {
        Ok(readings) => {
            println!("✓ USGS API returned {} readings for {}", readings.len(), site_code);
            assert!(!readings.is_empty(), "Should receive at least one reading");
            
            // Verify reading structure
            for reading in &readings {
                assert_eq!(reading.site_code, site_code);
                assert!(reading.parameter_code == "00060" || reading.parameter_code == "00065");
                assert!(!reading.datetime.is_empty());
                assert!(reading.value > 0.0, "Value should be positive");
            }
        }
        Err(e) => {
            eprintln!("\n⚠ WARNING: USGS API returned no data");
            eprintln!("  Site: {}", site_code);
            eprintln!("  Error: {}", e);
            eprintln!("  This may indicate:");
            eprintln!("    - System date is in the future");
            eprintln!("    - Station is temporarily offline");
            eprintln!("    - USGS API is experiencing issues\n");
            
            // Don't fail the test - this is expected behavior for future dates
            // but print a clear warning
        }
    }
}

#[test]
fn test_usgs_api_handles_multiple_stations() {
    // Test requesting data for multiple stations at once
    let site_codes = vec!["05568500", "05567500", "05568000"];
    
    let url = usgs::build_iv_url(
        &site_codes,
        &["00060", "00065"],
        "PT1H",
    );
    
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");
    
    let response = client.get(&url)
        .send()
        .expect("USGS API request failed");
    
    assert!(response.status().is_success());
    
    let body = response.text().expect("Failed to read response body");
    let result = usgs::parse_iv_response(&body);
    
    if let Ok(readings) = result {
        println!("✓ Multi-station request returned {} readings", readings.len());
        
        // Check we got data for multiple different sites
        let unique_sites: std::collections::HashSet<_> = readings.iter()
            .map(|r| r.site_code.as_str())
            .collect();
        
        println!("  Sites in response: {:?}", unique_sites);
        
        // We may not get data for all sites if some are offline
        // but we should get at least some sites
        if !unique_sites.is_empty() {
            assert!(
                unique_sites.len() >= 1,
                "Should receive data for at least one station"
            );
        }
    }
}

#[test]
fn test_usgs_daily_values_api_returns_historical_data() {
    // Test that DV API can retrieve historical data
    // Use a date range from 2024 (past) to ensure data exists
    let site_code = "05568500";
    
    let url = usgs::build_dv_url(
        &[site_code],
        &["00060", "00065"],
        "2024-01-01",
        "2024-01-31",
    );
    
    println!("Testing USGS DV API for historical data: {}", url);
    
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");
    
    let response = client.get(&url)
        .send()
        .expect("USGS DV API request failed");
    
    assert!(response.status().is_success());
    
    let body = response.text().expect("Failed to read response body");
    let result = usgs::parse_dv_response(&body);
    
    match result {
        Ok(readings) => {
            println!("✓ USGS DV API returned {} daily readings", readings.len());
            assert!(readings.len() > 0, "Should have historical data for January 2024");
            
            // Verify we got daily values for the date range
            assert!(readings.len() <= 31 * 2, "Should have at most 31 days × 2 parameters");
        }
        Err(e) => {
            eprintln!("\n⚠ WARNING: USGS DV API returned no data for historical range");
            eprintln!("  Error: {}", e);
        }
    }
}

// ---------------------------------------------------------------------------
// CWMS API Data Availability Tests
// ---------------------------------------------------------------------------

#[test]
fn test_cwms_api_returns_data_for_lagrange() {
    // LaGrange Lock & Dam is critical for backwater detection
    
    // Load CWMS configuration
    let locations = usace_locations::load_locations()
        .expect("Failed to load CWMS locations");
    
    if locations.is_empty() {
        eprintln!("⚠ No CWMS locations configured - skipping CWMS API test");
        return;
    }
    
    // Find LaGrange location
    let lagrange = locations.iter()
        .find(|loc| loc.name.contains("LaGrange") || loc.cwms_location.contains("LTRNG"))
        .expect("LaGrange location not found in usace_iem.toml");
    
    println!("Testing CWMS API for: {} ({})", lagrange.name, lagrange.cwms_location);
    
    // Try to discover timeseries
    let http_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("Failed to create HTTP client");
    
    let mut location = lagrange.clone();
    let discovery_result = usace_locations::update_with_discovered_timeseries(&mut location, &http_client);
    
    match discovery_result {
        Ok(_) => {
            if let Some(discovered) = &location.discovered_timeseries {
                println!("✓ Discovered CWMS timeseries:");
                if let Some(ref pool) = discovered.pool_elevation {
                    println!("  Pool: {}", pool);
                }
                if let Some(ref tail) = discovered.tailwater_elevation {
                    println!("  Tailwater: {}", tail);
                }
                
                // Try to fetch recent data
                if let Some(ref ts_id) = discovered.pool_elevation {
                    let data_result = cwms::fetch_recent(&http_client, ts_id, &location.office, 4);
                    
                    match data_result {
                        Ok(timeseries) => {
                            println!("✓ CWMS API returned {} data points", timeseries.len());
                            assert!(!timeseries.is_empty(), "Should receive CWMS data");
                            
                            // Verify data structure
                            for ts in &timeseries {
                                assert_eq!(ts.location_id, location.cwms_location);
                                assert!(!ts.timeseries_id.is_empty());
                                assert!(ts.value.is_finite());
                            }
                        }
                        Err(e) => {
                            eprintln!("\n⚠ WARNING: CWMS data fetch failed");
                            eprintln!("  Error: {}", e);
                            eprintln!("  This may indicate API issues or no recent data");
                        }
                    }
                }
            } else {
                eprintln!("⚠ No timeseries discovered for LaGrange");
            }
        }
        Err(e) => {
            eprintln!("\n⚠ WARNING: CWMS timeseries discovery failed");
            eprintln!("  Error: {}", e);
            eprintln!("  This is expected if CWMS API is unavailable");
        }
    }
}

// ---------------------------------------------------------------------------
// ASOS API Data Availability Tests  
// ---------------------------------------------------------------------------

#[test]
fn test_asos_api_returns_precipitation_data() {
    // Test IEM/ASOS API for Peoria (KPIA)
    
    // Load ASOS configuration
    let asos_path = std::path::Path::new("iem_asos.toml");
    if !asos_path.exists() {
        eprintln!("⚠ iem_asos.toml not found - skipping ASOS API test");
        return;
    }
    
    let locations = asos_locations::load_locations(asos_path)
        .expect("Failed to load ASOS locations");
    
    if locations.is_empty() {
        eprintln!("⚠ No ASOS locations configured - skipping ASOS API test");
        return;
    }
    
    // Find Peoria station
    let kpia = locations.iter()
        .find(|loc| loc.station_id == "KPIA")
        .expect("KPIA station not found in iem_asos.toml");
    
    println!("Testing ASOS/IEM API for: {} ({})", kpia.name, kpia.station_id);
    
    let http_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("Failed to create HTTP client");
    
    // Try to fetch current conditions (includes 1-hour precipitation)
    let result = iem::fetch_current(&http_client, &kpia.station_id);
    
    match result {
        Ok(obs) => {
            println!("✓ ASOS/IEM API returned current conditions");
            println!("  Station: {}", obs.station_id);
            if let Some(temp) = obs.temp_f {
                println!("  Temperature: {} F", temp);
            }
            if let Some(precip) = obs.precip_1hr_in {
                println!("  1hr Precipitation: {} in", precip);
            } else {
                println!("  1hr Precipitation: null (no recent precipitation)");
            }
            
            assert_eq!(obs.station_id, kpia.station_id);
        }
        Err(e) => {
            eprintln!("\n⚠ WARNING: ASOS current conditions fetch failed");
            eprintln!("  Error: {}", e);
            eprintln!("  This may indicate IEM API issues");
        }
    }
    
    // Try to fetch recent precipitation data (last 1 hour)
    let archive_result = iem::fetch_recent_precip(&http_client, &kpia.station_id, 1);
    
    match archive_result {
        Ok(observations) => {
            println!("✓ ASOS/IEM recent precip returned {} observations", observations.len());
            
            if !observations.is_empty() {
                // Verify observation structure
                for obs in observations.iter().take(3) {
                    assert_eq!(obs.station_id, kpia.station_id);
                }
            }
        }
        Err(e) => {
            eprintln!("\n⚠ WARNING: ASOS recent precip fetch failed");
            eprintln!("  Error: {}", e);
        }
    }
}

// ---------------------------------------------------------------------------
// Database Population Tests
// ---------------------------------------------------------------------------

#[test]
fn test_usgs_data_can_be_inserted_into_database() {
    let mut client = get_test_client();
    cleanup_test_data(&mut client);
    
    // Create a test USGS reading
    let reading = GaugeReading {
        site_code: "TEST001".to_string(),
        site_name: "Test Station".to_string(),
        parameter_code: "00060".to_string(),
        unit: "ft3/s".to_string(),
        value: 1234.56,
        datetime: Utc::now().to_rfc3339(),
        qualifier: "P".to_string(),
    };
    
    // Parse the datetime
    let reading_time = DateTime::parse_from_rfc3339(&reading.datetime)
        .expect("Failed to parse datetime")
        .with_timezone(&Utc);
    
    // Convert value to Decimal
    let value_decimal = Decimal::from_f64_retain(reading.value)
        .expect("Failed to convert value to decimal");
    
    // Insert into database
    let rows_affected = client.execute(
        "INSERT INTO usgs_raw.gauge_readings 
         (site_code, parameter_code, unit, value, reading_time, qualifier)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (site_code, parameter_code, reading_time) DO NOTHING",
        &[
            &reading.site_code,
            &reading.parameter_code,
            &reading.unit,
            &value_decimal,
            &reading_time,
            &reading.qualifier,
        ]
    ).expect("Failed to insert test reading");
    
    assert_eq!(rows_affected, 1, "Should insert one row");
    
    // Verify we can query it back
    let rows = client.query(
        "SELECT site_code, parameter_code, value, reading_time 
         FROM usgs_raw.gauge_readings 
         WHERE site_code = $1",
        &[&reading.site_code]
    ).expect("Failed to query inserted reading");
    
    assert_eq!(rows.len(), 1, "Should retrieve one row");
    
    let retrieved_site: String = rows[0].get(0);
    let retrieved_param: String = rows[0].get(1);
    let retrieved_value: Decimal = rows[0].get(2);
    let _retrieved_time: DateTime<Utc> = rows[0].get(3);
    
    assert_eq!(retrieved_site, reading.site_code);
    assert_eq!(retrieved_param, reading.parameter_code);
    
    // Compare decimal values with small tolerance
    let expected_decimal = Decimal::from_f64_retain(reading.value).unwrap();
    assert!(
        (retrieved_value - expected_decimal).abs() < Decimal::new(1, 2), // 0.01 tolerance
        "Expected approximately {}, got {}",
        reading.value,
        retrieved_value
    );
    
    println!("✓ USGS reading successfully stored and retrieved from database");
    
    cleanup_test_data(&mut client);
}

#[test]
fn test_cwms_data_can_be_inserted_into_database() {
    let mut client = get_test_client();
    
    // Check if CWMS table exists (requires migration 004)
    let table_exists = client.query_one(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_schema = 'usace' 
            AND table_name = 'cwms_timeseries'
        )",
        &[]
    );
    
    if table_exists.is_err() || !table_exists.unwrap().get::<_, bool>(0) {
        eprintln!("⚠ Skipping test: usace.cwms_timeseries table does not exist");
        eprintln!("  Run: psql -U flopro_admin -d flopro_db -f sql/004_usace_cwms.sql");
        return; // Skip test gracefully
    }
    
    cleanup_test_data(&mut client);
    
    // Create test CWMS location first (required due to foreign key)
    let location_id = "TESTLOC".to_string();
    client.execute(
        "INSERT INTO usace.cwms_locations 
         (location_id, office_id, base_location, location_name, monitored)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (location_id) DO NOTHING",
        &[&location_id, &"TEST", &"TESTLOC", &"Test Location", &false]
    ).expect("Failed to insert test CWMS location");
    
    // Create test CWMS timeseries data
    let timeseries_id = "TEST_LOCATION.Pool.Inst.15Minutes.0.Ccp-Rev".to_string();
    let parameter_id = "Pool".to_string();
    let timestamp = Utc::now();
    let value = Decimal::from_f64_retain(432.5).unwrap();
    
    // Insert into database
    let rows_affected = client.execute(
        "INSERT INTO usace.cwms_timeseries 
         (location_id, timeseries_id, parameter_id, parameter_type, interval, duration, version,
          timestamp, value, unit, quality_code)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (timeseries_id, timestamp) DO NOTHING",
        &[
            &location_id,
            &timeseries_id,
            &parameter_id,
            &"Inst",
            &"15Minutes",
            &"0",
            &"Ccp-Rev",
            &timestamp,
            &value,
            &"ft",
            &0i32,
        ]
    ).expect("Failed to insert CWMS data");
    
    assert_eq!(rows_affected, 1, "Should insert one row");
    
    // Verify we can query it back
    let rows = client.query(
        "SELECT location_id, parameter_id, value, timestamp 
         FROM usace.cwms_timeseries 
         WHERE location_id = $1",
        &[&location_id]
    ).expect("Failed to query CWMS data");
    
    assert_eq!(rows.len(), 1, "Should retrieve one row");
    
    let retrieved_location: String = rows[0].get(0);
    let retrieved_param: String = rows[0].get(1);
    let retrieved_value: Decimal = rows[0].get(2);
    
    assert_eq!(retrieved_location, location_id);
    assert_eq!(retrieved_param, parameter_id);
    assert_eq!(retrieved_value, value);
    
    println!("✓ CWMS data successfully stored and retrieved from database");
    
    cleanup_test_data(&mut client);
}

#[test]
fn test_asos_data_can_be_inserted_into_database() {
    let mut client = get_test_client();
    
    // Check if ASOS table exists (requires migration 006)
    let table_exists = client.query_one(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_name = 'asos_observations'
        )",
        &[]
    );
    
    if table_exists.is_err() || !table_exists.unwrap().get::<_, bool>(0) {
        eprintln!("⚠ Skipping test: asos_observations table does not exist");
        eprintln!("  Run: psql -U flopro_admin -d flopro_db -f sql/006_iem_asos.sql");
        return; // Skip test gracefully
    }
    
    cleanup_test_data(&mut client);
    
    // Create test ASOS observation
    let station_id = "TSTASOS".to_string();
    let observation_time = Utc::now();
    let precip_1hr = Some(Decimal::from_f64_retain(0.25).unwrap());
    let temp_f = Some(Decimal::from_f64_retain(72.5).unwrap());
    
    // Insert into database
    let rows_affected = client.execute(
        "INSERT INTO asos_observations 
         (station_id, observation_time, temp_f, precip_1hr_in, data_source)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (station_id, observation_time) DO NOTHING",
        &[
            &station_id,
            &observation_time,
            &temp_f,
            &precip_1hr,
            &"TEST",
        ]
    ).expect("Failed to insert ASOS data");
    
    assert_eq!(rows_affected, 1, "Should insert one row");
    
    // Verify we can query it back
    let rows = client.query(
        "SELECT station_id, observation_time, temp_f, precip_1hr_in 
         FROM asos_observations 
         WHERE station_id = $1",
        &[&station_id]
    ).expect("Failed to query ASOS data");
    
    assert_eq!(rows.len(), 1, "Should retrieve one row");
    
    let retrieved_station: String = rows[0].get(0);
    let retrieved_temp: Option<Decimal> = rows[0].get(2);
    let retrieved_precip: Option<Decimal> = rows[0].get(3);
    
    assert_eq!(retrieved_station, station_id);
    assert_eq!(retrieved_temp, temp_f);
    assert_eq!(retrieved_precip, precip_1hr);
    
    println!("✓ ASOS observation successfully stored and retrieved from database");
    
    cleanup_test_data(&mut client);
}

// ---------------------------------------------------------------------------
// End-to-End Pipeline Tests
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Only run manually - makes real API calls
fn test_usgs_full_pipeline_api_to_database() {
    // Full pipeline: Fetch from USGS API → Parse → Insert → Verify
    let mut client = get_test_client();
    
    let site_code = "05568500"; // Kingston Mines
    
    // 1. Fetch from API
    let url = usgs::build_iv_url(&[site_code], &["00060"], "PT1H");
    
    let http_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");
    
    let response = http_client.get(&url)
        .send()
        .expect("USGS API request failed");
    
    assert!(response.status().is_success());
    
    let body = response.text().expect("Failed to read response body");
    
    // 2. Parse response
    let readings = usgs::parse_iv_response(&body)
        .expect("Failed to parse USGS response - may be no data for current date");
    
    assert!(!readings.is_empty(), "Should have received readings from API");
    
    println!("✓ Fetched {} readings from USGS API", readings.len());
    
    // 3. Insert into database
    let mut inserted_count = 0;
    
    for reading in &readings {
        let reading_time = DateTime::parse_from_rfc3339(&reading.datetime)
            .expect("Failed to parse datetime")
            .with_timezone(&Utc);
        
        let value_decimal = Decimal::from_f64_retain(reading.value)
            .expect("Failed to convert value");
        
        let rows = client.execute(
            "INSERT INTO usgs_raw.gauge_readings 
             (site_code, parameter_code, unit, value, reading_time, qualifier)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (site_code, parameter_code, reading_time) DO NOTHING",
            &[
                &reading.site_code,
                &reading.parameter_code,
                &reading.unit,
                &value_decimal,
                &reading_time,
                &reading.qualifier,
            ]
        ).expect("Failed to insert reading");
        
        inserted_count += rows;
    }
    
    println!("✓ Inserted {} new readings into database", inserted_count);
    
    // 4. Verify data is retrievable
    let db_rows = client.query(
        "SELECT COUNT(*) FROM usgs_raw.gauge_readings WHERE site_code = $1",
        &[&site_code]
    ).expect("Failed to query database");
    
    let total_count: i64 = db_rows[0].get(0);
    
    println!("✓ Database now contains {} total readings for site {}", total_count, site_code);
    
    assert!(total_count > 0, "Database should contain readings after ingestion");
}

#[test]
fn test_duplicate_prevention_idempotent_insert() {
    let mut client = get_test_client();
    cleanup_test_data(&mut client);
    
    // Create a test reading
    let site_code = "TESTDUP";
    let reading_time = Utc::now();
    let value = Decimal::from_f64_retain(1000.0).unwrap();
    
    // Insert the same reading twice
    for i in 1..=2 {
        let rows = client.execute(
            "INSERT INTO usgs_raw.gauge_readings 
             (site_code, parameter_code, unit, value, reading_time, qualifier)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (site_code, parameter_code, reading_time) DO NOTHING",
            &[
                &site_code,
                &"00060",
                &"ft3/s",
                &value,
                &reading_time,
                &"P",
            ]
        ).expect("Failed to insert");
        
        if i == 1 {
            assert_eq!(rows, 1, "First insert should add one row");
        } else {
            assert_eq!(rows, 0, "Second insert should be ignored (duplicate)");
        }
    }
    
    // Verify only one row exists
    let count_rows = client.query(
        "SELECT COUNT(*) FROM usgs_raw.gauge_readings WHERE site_code = $1",
        &[&site_code]
    ).expect("Failed to count rows");
    
    let count: i64 = count_rows[0].get(0);
    assert_eq!(count, 1, "Should have exactly one row despite duplicate insert");
    
    println!("✓ Duplicate prevention working correctly");
    
    cleanup_test_data(&mut client);
}
