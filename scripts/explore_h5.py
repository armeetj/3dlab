"""Explore the structure and keys of an HDF5 file."""

import sys
from pathlib import Path

import h5py


def explore_h5(filepath: str, indent: int = 0) -> None:
    """Recursively explore and print the structure of an HDF5 file."""

    def print_attrs(obj, prefix: str) -> None:
        """Print attributes of an HDF5 object."""
        if obj.attrs:
            for key, val in obj.attrs.items():
                print(f"{prefix}  @{key}: {val}")

    def visit(name: str, obj) -> None:
        """Visit each item in the HDF5 file."""
        prefix = "  " * (name.count("/") + 1)
        if isinstance(obj, h5py.Dataset):
            print(f"{prefix}{name} [Dataset: shape={obj.shape}, dtype={obj.dtype}]")
            print_attrs(obj, prefix)
        elif isinstance(obj, h5py.Group):
            print(f"{prefix}{name}/ [Group]")
            print_attrs(obj, prefix)

    with h5py.File(filepath, "r") as f:
        print(f"\nExploring: {filepath}")
        print("=" * 60)

        # Print root attributes
        if f.attrs:
            print("\nRoot attributes:")
            for key, val in f.attrs.items():
                print(f"  @{key}: {val}")

        # Print top-level keys
        print(f"\nTop-level keys: {list(f.keys())}")
        print("\nFull structure:")
        print("-" * 60)

        f.visititems(visit)


if __name__ == "__main__":
    default_path = Path(__file__).parent.parent / "samples" / "MTR_087.h5"

    if len(sys.argv) > 1:
        h5_path = sys.argv[1]
    elif default_path.exists():
        h5_path = str(default_path)
    else:
        print("Usage: uv run explore_h5.py <path_to_h5_file>")
        sys.exit(1)

    explore_h5(h5_path)
