#!/usr/bin/env python3
"""Plot cross-sections of HDF5 volume data."""

import h5py
import matplotlib.pyplot as plt
import numpy as np
from pathlib import Path

# Find target file
samples_dir = Path(__file__).parent.parent / "samples"
target_files = list(samples_dir.glob("target_*.h5"))

if not target_files:
    print("No target_*.h5 files found in samples/")
    exit(1)

h5_path = target_files[0]
print(f"Loading: {h5_path}")

with h5py.File(h5_path, "r") as f:
    data = f["target"][:]

print(f"Shape: {data.shape}, dtype: {data.dtype}")
print(f"Value range: {data.min():.2f} - {data.max():.2f}")

# Plot 3 cross-sections (middle slices along each axis)
fig, axes = plt.subplots(1, 3, figsize=(15, 5))

# XY slice (middle Z)
z_mid = data.shape[2] // 2
axes[0].imshow(data[:, :, z_mid].T, cmap="gray", origin="lower")
axes[0].set_title(f"XY plane (Z={z_mid})")
axes[0].set_xlabel("X")
axes[0].set_ylabel("Y")

# XZ slice (middle Y)
y_mid = data.shape[1] // 2
axes[1].imshow(data[:, y_mid, :].T, cmap="gray", origin="lower", aspect="auto")
axes[1].set_title(f"XZ plane (Y={y_mid})")
axes[1].set_xlabel("X")
axes[1].set_ylabel("Z")

# YZ slice (middle X)
x_mid = data.shape[0] // 2
axes[2].imshow(data[x_mid, :, :].T, cmap="gray", origin="lower", aspect="auto")
axes[2].set_title(f"YZ plane (X={x_mid})")
axes[2].set_xlabel("Y")
axes[2].set_ylabel("Z")

plt.suptitle(f"{h5_path.name} - {data.shape}")
plt.tight_layout()

output_path = Path(__file__).parent.parent / "cross_sections.png"
plt.savefig(output_path, dpi=150)
print(f"Saved to: {output_path}")
