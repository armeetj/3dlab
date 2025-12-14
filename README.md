# 3DLab

A WebGPU-based 3D MRI volume renderer. Rust server reads HDF5 files, Rust/WASM client renders in browser.

## Run

```bash
# Terminal 1: Server (port 9000)
HDF5_DIR=/opt/homebrew/opt/hdf5 cargo run -p server

# Terminal 2: Client (port 3000)
cd client && trunk serve
```

Open http://localhost:3000
