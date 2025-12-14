.PHONY: run dev server client build clean

# Set HDF5 path for macOS
export HDF5_DIR=/opt/homebrew/opt/hdf5

# Run both server and client
run:
	@./run.sh

# Development mode (same as run)
dev: run

# Run only server
server:
	cargo run -p server

# Run only client (WASM)
client:
	cd client && trunk serve

# Build everything
build:
	cargo build -p server
	cd client && trunk build --release

# Clean build artifacts
clean:
	cargo clean
	rm -rf client/dist
