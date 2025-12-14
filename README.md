# 3DLab

A WebGL-based 3D MRI volume renderer. Rust server reads HDF5 files, Rust/WASM client renders in browser. Originally made to address a problem I had when working on AI models for MRI recon at Caltech. The 3D volumes I was working with were too large to visualize with standard python plotting tools. At best, I would generate 2D cross-section visualizations with matplotlib or plotly. I wrote this simple renderer to quickly visualize 3D MRI volumes in the browser, but it should work for any 3D volume stored in HDF5 format.

**Important:** This is a hobby project and not production-ready software. I'm no graphics/shader specialist and much of the project was written with heavy assistance from my good friend Opus at Claude Code :)

For now, the project is a bit rough around the edges, but it works well enough for my needs. Feel free to fork and improve! I've chosen a client-server architecture to allow for potential future expansion (e.g., loading volumes from a database, more advanced chunking and rendering of massive volumes, etc).

The client is written in `egui`/`eframe` and compiled to WebAssembly with `trunk`. The core volume renderer uses `glow` and targets WebGL in the browser. The server serves volumes from HDF5 files stored in the `samples/` folder and is written in `axum`.

## Features

This is a hobby project I've setup for my own use, so features are limited. If you want to help bring any new features to life, check out the "Contributing" section below!

- [x] volume rendering in browser (WebGL ray marching)
- [x] trackball rotation (quaternion-based, no gimbal lock)
- [x] quality slider (adjust step size for performance/quality tradeoff)
- [x] opacity control
- [x] hover info card (voxel coordinates, value, intensity)
- [x] occupancy grid optimization (skip empty regions)
- [x] XYZ axes visualization
- [x] naive server serves entire HDF5 volumes
- [ ] async volume loading (non-blocking UI)
- [ ] server-side chunking for massive volumes
- [ ] cross-section viewer
- [ ] download current view / export view as PNG
- [ ] serialize current view state in shareable URL

## Run

```bash
# Run Server (port 9000)
HDF5_DIR=/opt/homebrew/opt/hdf5 cargo run -p server

# Run Client (port 3000)
cd client && trunk serve
```

Open http://localhost:3000

## HDF5 File Format

- Place HDF5 files in the `samples/` folder with the prefix `target_` (e.g., `target_087.h5`).
- You can download a sample volume sample here: [target_087.h5 (21MB)](https://gofile.io/d/7ik1om) or [target_087_full.h5 (170MB)](https://gofile.io/d/55oxow)
- This sample is taken from Stanford's SKM-TEA dataset [here](https://aimi.stanford.edu/datasets/skm-tea-knee-mri).

### Expected Structure

The HDF5 file must contain a 3D dataset named one of:
- `target` (preferred)
- `volume`
- `data`

### Dataset Requirements

- **Shape**: 3D array `[X, Y, Z]`
- **Dtype**: `float32` (will be read as f32)
- **Values**: Any range (automatically normalized for display)

### Example Structure

```
target_087.h5
├── target [Dataset: shape=(256, 256, 256), dtype=float32]
```

### Explore Your HDF5 Files

Use the explore script to inspect the structure of any HDF5 file:

```bash
uv run scripts/explore_h5.py path/to/your/file.h5
```

Example output:
```
Exploring: samples/target_087.h5
============================================================

Top-level keys: ['target']

Full structure:
------------------------------------------------------------
  target [Dataset: shape=(256, 256, 256), dtype=float32]
```

### Converting Your Data

If your data is in a different format (NumPy, NIfTI, DICOM, etc.), convert it to HDF5:

```python
import h5py
import numpy as np

# Load your 3D volume (shape should be [X, Y, Z])
volume = np.load("your_volume.npy").astype(np.float32)

# Save as HDF5
with h5py.File("samples/target_myvolume.h5", "w") as f:
    f.create_dataset("target", data=volume)
```

## Controls

- **Drag**: Rotate volume
- **Scroll**: Zoom in/out
- **Sidebar**: Adjust rotation, quality, opacity
- **Hover**: View voxel info


## Contributing
- First, create an issue describing what you want to change. Let's discuss it before you start coding. This helps ensure we're on the same page and saves you time.
- Fork the repository and create a new branch for your feature or bug fix.
- Make a PR and we'll review it together. Then we can merge it in!
