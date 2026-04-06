# Multi-stage build for flomon_service
# Stage 1: build the Rust binary
FROM rust:1.87-slim AS builder

WORKDIR /build

# Cache dependency compilation separately from source builds
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Build the real source
COPY src ./src
# Touch main.rs so Cargo re-links (avoids cache poisoning from dummy above)
RUN touch src/main.rs && cargo build --release

# -----------------------------------------------------------------------
# Stage 2: minimal runtime image
FROM debian:bookworm-slim

# Runtime libs: libssl for reqwest (rustls means no openssl needed),
# ca-certificates for HTTPS to USGS/CWMS/IEM, libpq for postgres crate.
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libpq5 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/flomon_service .

# TOML configuration files are mounted at runtime via docker-compose volume.
# They must be present in /app at startup — the binary panics if they are missing.
# See docker-compose.yml: volumes section.

EXPOSE 8080

# DATABASE_URL is injected by docker-compose from the .env file.
CMD ["./flomon_service", "--endpoint", "8080"]
