use postgres::{Client, NoTls};
use std::env;

fn main() {
    dotenv::dotenv().ok();
    let db_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    println!("Connecting to: {}", db_url);
    
    let mut client = Client::connect(&db_url, NoTls)
        .expect("Failed to connect");
    
    println!("✓ Connected successfully");
    
    // Test 1: Check if nws schema exists
    let result = client.query_one(
        "SELECT EXISTS (SELECT FROM information_schema.schemata WHERE schema_name = 'nws')",
        &[]
    );
    
    match result {
        Ok(row) => {
            let exists: bool = row.get(0);
            println!("✓ nws schema exists: {}", exists);
        }
        Err(e) => println!("✗ Error checking schema: {}", e),
    }
    
    // Test 2: Check if flood_events table exists
    let result = client.query_one(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_schema = 'nws' 
            AND table_name = 'flood_events'
        )",
        &[]
    );
    
    match result {
        Ok(row) => {
            let exists: bool = row.get(0);
            println!("✓ nws.flood_events table exists: {}", exists);
        }
        Err(e) => println!("✗ Error checking table: {}", e),
    }
    
    // Test 3: List nws tables
    let result = client.query(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = 'nws'",
        &[]
    );
    
    match result {
        Ok(rows) => {
            println!("✓ Tables in nws schema:");
            for row in rows {
                let name: String = row.get(0);
                println!("  - {}", name);
            }
        }
        Err(e) => println!("✗ Error listing tables: {}", e),
    }
}
