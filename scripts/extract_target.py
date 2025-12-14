"""Extract target from h5 file, downsample, and save absolute values."""

import sys
from pathlib import Path

import h5py
import numpy as np


def extract_target(input_path: str, output_path: str, downsample_factor: int = 2) -> None:
    """Extract target, take first echo, downsample, and save abs values."""
    with h5py.File(input_path, "r") as f:
        # target shape: (512, 512, 160, 2, 1)
        # dim 3 (size 2) is echoes - take first echo (index 0)
        # dim 4 (size 1) - squeeze it out
        target = f["target"][:, :, :, 0, 0]  # shape: (512, 512, 160)
        print(f"Original shape: {f['target'].shape}")
        print(f"After selecting echo 0: {target.shape}")

        # Take absolute value
        target = np.abs(target)
        print(f"After abs: dtype={target.dtype}")

        # Downsample in all 3 dimensions
        ds = downsample_factor
        target = target[::ds, ::ds, ::ds]
        print(f"After {ds}x downsample: {target.shape}")

    # Save to new h5 file
    with h5py.File(output_path, "w") as f:
        f.create_dataset("target", data=target, dtype=np.float32)
        print(f"\nSaved to: {output_path}")
        print(f"Final shape: {target.shape}, dtype: float32")


if __name__ == "__main__":
    default_input = Path(__file__).parent.parent / "samples" / "MTR_087.h5"
    default_output = Path(__file__).parent.parent / "samples" / "target.h5"

    input_path = sys.argv[1] if len(sys.argv) > 1 else str(default_input)
    output_path = sys.argv[2] if len(sys.argv) > 2 else str(default_output)
    downsample = int(sys.argv[3]) if len(sys.argv) > 3 else 2

    extract_target(input_path, output_path, downsample)
