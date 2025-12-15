# Build stage for WASM client
FROM rust:latest AS client-builder
RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk wasm-bindgen-cli

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY shared ./shared
COPY client ./client
COPY server/Cargo.toml ./server/Cargo.toml
RUN mkdir -p server/src && echo "fn main() {}" > server/src/main.rs

WORKDIR /app/client
RUN trunk build --release

# Build stage for server
FROM rust:latest AS server-builder
RUN apt-get update && apt-get install -y libhdf5-dev pkg-config && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY shared ./shared
COPY server ./server
COPY client/Cargo.toml ./client/Cargo.toml
RUN mkdir -p client/src && echo "fn main() {}" > client/src/main.rs && touch client/src/lib.rs

WORKDIR /app/server
RUN cargo build --release

# Runtime stage - use same base as build to ensure HDF5 library compatibility
FROM rust:latest AS runtime
RUN apt-get update && apt-get install -y libhdf5-dev ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy server binary
COPY --from=server-builder /app/target/release/server ./server

# Copy client dist
COPY --from=client-builder /app/client/dist ./client/dist

# Copy sample data (target_ prefixed files only)
COPY samples/target_* ./samples/

EXPOSE 9000
CMD ["./server"]
