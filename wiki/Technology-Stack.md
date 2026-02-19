# Technology Stack

FloPro's technology choices prioritize **reliability, performance, and operational simplicity** for a mission-critical flood monitoring system.

## Core Technologies

### Rust (Edition 2024)

**Why Rust?**

✅ **Memory Safety** - No segfaults, no buffer overflows, no undefined behavior  
✅ **Performance** - Zero-cost abstractions, compiled to native code  
✅ **Concurrency** - Safe parallelism without data races  
✅ **Type Safety** - Compile-time guarantees prevent entire classes of bugs  
✅ **Error Handling** - `Result<T, E>` forces explicit error handling  
✅ **Modern Tooling** - Cargo, rustfmt, clippy, excellent LSP support  
✅ **Small Binaries** - ~5MB for our service (vs 50MB+ for Go/JVM)  

**Alternatives Considered:**

**Python:**
- ❌ Runtime errors (TypeErrors, AttributeErrors in production)
- ❌ GIL limits concurrency
- ❌ Deployment complexity (virtualenv, dependencies)
- ✅ Faster prototyping (but this is production code)

**Go:**
- ✅ Simple, fast compilation
- ❌ Lacks strong type system (interface{} everywhere)
- ❌ Nil pointer exceptions possible
- ❌ No sum types (our Result<T, E> pattern)

**Node.js:**
- ❌ Callback hell, async complexity
- ❌ Runtime errors from type mismatches
- ❌ High memory usage
- ✅ Large ecosystem (not needed for our use case)

**C/C++:**
- ✅ Maximum performance
- ❌ Memory management burden
- ❌ No modern package manager
- ❌ Undefined behavior landmines

### PostgreSQL 14+

**Why PostgreSQL?**

✅ **ACID Compliance** - Critical for flood alert integrity  
✅ **Time Series Support** - Native TIMESTAMPTZ, efficient range queries  
✅ **Advanced Indexing** - Partial indexes, expression indexes, BRIN  
✅ **Mature & Stable** - 30+ years of development  
✅ **Rich SQL** - Window functions, CTEs, materialized views  
✅ **Open Source** - No vendor lock-in, free forever  
✅ **Excellent Rust Support** - `postgres` crate with chrono integration  

**Feature Requirements:**
- `with-chrono-0_4` - DateTime<Utc> support for timestamps
- UNIQUE constraints - Idempotent ingestion
- Materialized views - Fast dashboard queries
- GiST indexes (future) - Geospatial queries with PostGIS

**Alternatives Considered:**

**TimescaleDB:**
- ✅ Optimized for time series
- ❌ Adds complexity (chunking, compression policies)
- ❌ Overkill for 8 stations
- 💡 Could migrate later if scaling to 1000+ sites

**InfluxDB:**
- ✅ Built for time series
- ❌ Eventual consistency (unacceptable for alerts)
- ❌ Limited SQL support
- ❌ No strong Rust client

**SQLite:**
- ✅ Simple, embedded
- ❌ Single writer limitation
- ❌ No replication
- ❌ Limited concurrent readers
- 💡 Fine for prototype, not production

**MySQL/MariaDB:**
- ✅ Widely used
- ❌ Weaker type system than PostgreSQL
- ❌ Less sophisticated query optimizer
- ❌ No partial indexes

### Rust Crate Ecosystem

#### HTTP Client: reqwest (0.11)

```toml
reqwest = { version = "0.11", features = ["blocking", "rustls-tls"], default-features = false }
```

**Why reqwest?**
- ✅ Industry standard HTTP client
- ✅ Blocking API for simple sequential requests
- ✅ rustls-tls avoids OpenSSL dependency (easier deployment)
- ✅ Excellent error handling

**Configuration:**
- `blocking` - Synchronous API (our polling is sequential)
- `rustls-tls` - Pure Rust TLS (no OpenSSL dependency)
- `default-features = false` - Opt-in features only (smaller binary)

**Alternatives:**
- `ureq` - Lighter but less feature-complete
- `hyper` - Too low-level for our needs
- `curl` bindings - Requires system libcurl

#### Database Client: postgres (0.19)

```toml
postgres = { version = "0.19", features = ["with-chrono-0_4"] }
```

**Why postgres crate?**
- ✅ Mature, well-tested
- ✅ Native chrono support for DateTime
- ✅ Synchronous API (matches our polling loop)
- ✅ Transaction support
- ✅ Prepared statements

**Feature: with-chrono-0_4**
- Enables `DateTime<Utc>` as query parameter
- Automatic conversion to/from TIMESTAMPTZ
- Type safety for temporal queries

**Alternatives:**
- `tokio-postgres` - Async (unnecessary complexity for our case)
- `diesel` - ORM overhead, compile-time schema (inflexible for migrations)
- `sqlx` - Compile-time SQL checking (nice but slows builds)

#### JSON Parsing: serde + serde_json (1.0)

```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

**Why serde?**
- ✅ Zero-copy deserialization
- ✅ Compile-time type safety
- ✅ Industry standard
- ✅ Excellent error messages

**Pattern:**
```rust
#[derive(Deserialize)]
struct UsgsResponse {
    value: Value,
}
```

**Alternatives:**
- `json` crate - Less type-safe
- Manual parsing - Error-prone

#### Time Handling: chrono (0.4)

```toml
chrono = { version = "0.4", features = ["serde"] }
```

**Why chrono?**
- ✅ Rich datetime API
- ✅ Timezone support (USGS uses Central Time)
- ✅ ISO 8601 parsing
- ✅ Database integration via postgres

**Key Types:**
- `DateTime<Utc>` - Database timestamps
- `DateTime<FixedOffset>` - USGS API responses
- `Duration` - Staleness calculations

**Alternatives:**
- `time` crate - Less mature, different API
- stdlib - Insufficient for our needs

#### Configuration: dotenv (0.15)

```toml
dotenv = "0.15"
```

**Why dotenv?**
- ✅ Standard 12-factor app pattern
- ✅ Keeps secrets out of code
- ✅ Easy local development

**Usage:**
```bash
# .env
DATABASE_URL=postgresql://user:pass@localhost/flomon_db
```

```rust
dotenv::dotenv().ok();
let db_url = env::var("DATABASE_URL")?;
```

## Development Tools

### Cargo (Rust Build System)

**Why Cargo?**
- ✅ Built-in dependency management
- ✅ Reproducible builds (Cargo.lock)
- ✅ Integrated testing (`cargo test`)
- ✅ Benchmarking support
- ✅ Documentation generation (`cargo doc`)

**Key Commands:**
```bash
cargo build --release       # Optimized production build
cargo test                  # Run unit tests
cargo test --ignored        # Run integration tests (require API/DB)
cargo check                 # Fast compile check
cargo clippy               # Linting
cargo fmt                  # Code formatting
```

### Edition 2024

```toml
[package]
edition = "2024"
```

**Why Edition 2024?**
- ✅ Latest language features
- ✅ Improved error messages
- ✅ Better async/await (if we need it later)
- ✅ Forward compatibility

### Git for Version Control

**Branching Strategy:**
- `main` - Production-ready code
- Feature branches - New functionality
- No `develop` branch (small team, continuous deployment)

**Commit Message Convention:**
```
<type>: <brief summary>

<detailed explanation>
<rationale for design decisions>
<what changed and why>
```

**Types:**
- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation only
- `refactor:` - Code restructure without behavior change
- `test:` - Add/modify tests
- `chore:` - Dependencies, tooling

## Deployment Architecture

### Target Platform: Linux VPS

**Recommended Specs:**
- **CPU:** 1-2 cores (polling is I/O-bound)
- **RAM:** 1-2 GB (mostly database)
- **Storage:** 20 GB SSD (database + logs)
- **OS:** Ubuntu 22.04 LTS or Debian 12

**Why Linux?**
- ✅ Native Rust support
- ✅ PostgreSQL performance
- ✅ systemd integration
- ✅ SSH access for maintenance

### Single Binary Deployment

```bash
# Build optimized binary
cargo build --release

# Binary is self-contained
./target/release/historical_ingest
./target/release/flomon_service  # (future)

# No runtime dependencies beyond libc
ldd target/release/historical_ingest
# linux-vdso.so.1
# libc.so.6
# /lib64/ld-linux-x86-64.so.2
```

**Why Single Binary?**
- ✅ No complex deployment
- ✅ No dependency conflicts
- ✅ Easy rollback (just swap binary)
- ✅ Smaller attack surface

### systemd Service

**Future: /etc/systemd/system/flomon.service**
```ini
[Unit]
Description=FloPro Flood Monitoring Service
After=network.target postgresql.service

[Service]
Type=simple
User=flomon
WorkingDirectory=/opt/flomon
EnvironmentFile=/opt/flomon/.env
ExecStart=/opt/flomon/flomon_service
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

**Why systemd?**
- ✅ Automatic restart on crash
- ✅ Logging via journald
- ✅ Resource limits (cgroups)
- ✅ Standard on all modern Linux

## Testing Strategy

### Test Pyramid

```
        ┌─────────────┐
        │ Integration │  ← Manual (--ignored)
        │   Tests     │    Live API, real DB
        ├─────────────┤
        │             │
        │ Unit Tests  │  ← Automated (CI)
        │             │    Fast, isolated
        └─────────────┘
```

### Unit Tests (Automated)

**Location:** Inline with code
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_valid_response() { ... }
}
```

**Run:** `cargo test`  
**Coverage:** Parser logic, staleness calculations, data model validation

### Integration Tests (Manual)

**Location:** Marked with `#[ignore]`
```rust
#[test]
#[ignore]  // Don't run in CI - depends on external API
fn station_api_verify_all_registry_stations() { ... }
```

**Run:** `cargo test --ignored station_api_verify_all`  
**Coverage:** Live USGS API, database operations, station availability

**Why Manual?**
- External API dependency (USGS might be down)
- Requires database setup
- Rate limiting concerns
- Quick verification before deployment

## Security Considerations

### No External Dependencies at Runtime

**Rust Advantage:**
- Static linking (no shared library attacks)
- No runtime (unlike Python/Node)
- Memory safe (no buffer overflow exploits)

### Secrets Management

**Current: .env file**
```bash
# .env (not in git)
DATABASE_URL=postgresql://user:pass@localhost/flomon_db
ALERT_WEBHOOK_URL=https://...
```

**Future: Environment Variables**
```bash
# systemd service
Environment="DATABASE_URL=postgresql://..."
```

### Database Security

**Connection:**
- TLS/SSL for remote connections
- Password authentication (no trust)
- Dedicated user with limited permissions

**SQL Injection Prevention:**
- Parameterized queries (postgres crate enforces this)
- No string concatenation for SQL
- Type-safe query building

### API Security

**USGS API:**
- Public data (no authentication required)
- HTTPS only (encrypted in transit)
- Rate limiting (2-second delays between requests)

## Performance Characteristics

### Binary Size

```bash
$ ls -lh target/release/historical_ingest
-rwxr-xr-x  1 user  staff   4.8M Feb 19 10:00 historical_ingest
```

**Why Small?**
- Rust compiles to native code
- No VM or runtime included
- rustls instead of OpenSSL

### Memory Usage

**Steady State:**
- Service: ~10-20 MB
- Database connections: ~5 MB each
- Monitoring cache: <1 MB (16 stations)

**Why Low?**
- No garbage collector overhead
- Stack allocation where possible
- Explicit memory management

### Startup Time

```bash
$ time ./target/release/historical_ingest --help
real    0m0.003s
user    0m0.001s
sys     0m0.002s
```

**Why Fast?**
- Native binary (no JVM warmup)
- No dynamic linking
- Minimal initialization

### API Response Parsing

**Typical USGS IV Response:**
- Size: ~50 KB JSON
- Parse time: <1 ms
- Serde zero-copy deserialization

## Future Technology Additions

### Considered for Future Phases

**Async Runtime (tokio):**
- When: If implementing concurrent API polling
- Why: Parallel station requests
- Trade-off: Complexity vs performance gain

**Web Framework (axum/actix-web):**
- When: Building dashboard API
- Why: Real-time SSE updates, REST endpoints
- Choice: axum (tokio-based, type-safe)

**Message Queue (PostgreSQL LISTEN/NOTIFY):**
- When: Real-time alert dispatch
- Why: Already have PostgreSQL, no extra dependency
- Alternative: Redis (if scaling)

**Metrics (prometheus client):**
- When: Production monitoring
- Why: Track poll success rate, API latency, staleness events
- Export: /metrics endpoint

---

**Related Pages:**
- [[Database Architecture]] - PostgreSQL schema design
- [[Staleness Tracking]] - Hybrid cache implementation
- [[Data Sources]] - Why USGS NWIS
