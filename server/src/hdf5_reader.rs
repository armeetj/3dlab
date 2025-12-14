use hdf5::File;
use ndarray::Array3;
use shared::VolumeInfo;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HDF5Error {
    #[error("HDF5 error: {0}")]
    Hdf5(#[from] hdf5::Error),
    #[error("Dataset not found: {0}")]
    DatasetNotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Represents a loaded HDF5 volume
pub struct HDF5Volume {
    pub info: VolumeInfo,
    pub path: PathBuf,
    /// Cached low-res data (generated on load)
    low_res_cache: Vec<u8>,
}

impl HDF5Volume {
    /// Open an HDF5 file and extract volume metadata
    pub async fn open(path: &Path) -> Result<Self, HDF5Error> {
        let path_buf = path.to_path_buf();
        let path_clone = path_buf.clone();

        // Read the file in a blocking task
        let (info, low_res_cache) = tokio::task::spawn_blocking(move || {
            Self::read_volume_sync(&path_clone)
        })
        .await
        .unwrap()?;

        Ok(Self {
            info,
            path: path_buf,
            low_res_cache,
        })
    }

    fn read_volume_sync(path: &Path) -> Result<(VolumeInfo, Vec<u8>), HDF5Error> {
        let file = File::open(path)?;

        // Try to find the target dataset
        let dataset = file
            .dataset("target")
            .or_else(|_| file.dataset("volume"))
            .or_else(|_| file.dataset("data"))
            .map_err(|_| HDF5Error::DatasetNotFound("target, volume, or data".to_string()))?;

        let shape = dataset.shape();
        if shape.len() < 3 {
            return Err(HDF5Error::DatasetNotFound("Need at least 3D data".to_string()));
        }

        let dims = [shape[0] as u32, shape[1] as u32, shape[2] as u32];

        // Read the data as f32
        let data: Array3<f32> = dataset.read()?;

        // Calculate value range
        let min_val = data.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_val = data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        // Generate low-res version (downsample to ~64^3)
        let low_res = Self::downsample(&data, 64);
        let low_res_shape = low_res.shape();
        let low_res_dims = [
            low_res_shape[0] as u32,
            low_res_shape[1] as u32,
            low_res_shape[2] as u32,
        ];
        let low_res_bytes = Self::to_bytes(&low_res);

        // Calculate sizes
        let low_res_size = low_res_bytes.len() as u64;
        let full_res_size = (data.len() * std::mem::size_of::<f32>()) as u64;

        // Generate ID from filename
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let name = id.clone();

        let info = VolumeInfo {
            id,
            name,
            dimensions: dims,
            low_res_dimensions: low_res_dims,
            low_res_size,
            full_res_size,
            value_range: [min_val, max_val],
        };

        Ok((info, low_res_bytes))
    }

    /// Downsample volume to approximately target_size in each dimension
    fn downsample(data: &Array3<f32>, target_size: usize) -> Array3<f32> {
        let shape = data.shape();
        let max_dim = shape.iter().max().copied().unwrap_or(1);
        let factor = (max_dim / target_size).max(1);

        if factor == 1 {
            return data.clone();
        }

        let new_shape = [
            shape[0] / factor,
            shape[1] / factor,
            shape[2] / factor,
        ];

        let mut result = Array3::zeros(new_shape);

        for x in 0..new_shape[0] {
            for y in 0..new_shape[1] {
                for z in 0..new_shape[2] {
                    // Simple point sampling (could use averaging for better quality)
                    result[[x, y, z]] = data[[x * factor, y * factor, z * factor]];
                }
            }
        }

        result
    }

    /// Convert ndarray to bytes
    fn to_bytes(data: &Array3<f32>) -> Vec<u8> {
        let slice = data.as_slice().unwrap_or(&[]);
        let bytes: Vec<u8> = slice
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        bytes
    }

    /// Get low-res data (from cache)
    pub async fn get_low_res_data(&self) -> Result<Vec<u8>, HDF5Error> {
        Ok(self.low_res_cache.clone())
    }

    /// Get full-res data (read from file)
    pub async fn get_full_res_data(&self) -> Result<Vec<u8>, HDF5Error> {
        let path = self.path.clone();

        tokio::task::spawn_blocking(move || {
            let file = File::open(&path)?;
            let dataset = file
                .dataset("target")
                .or_else(|_| file.dataset("volume"))
                .or_else(|_| file.dataset("data"))
                .map_err(|_| HDF5Error::DatasetNotFound("target, volume, or data".to_string()))?;

            let data: Array3<f32> = dataset.read()?;
            Ok(Self::to_bytes(&data))
        })
        .await
        .unwrap()
    }

    /// Get volume data resampled to a target resolution
    /// Returns (bytes, [x, y, z] dimensions)
    pub async fn get_data_at_resolution(&self, target_size: usize) -> Result<(Vec<u8>, [u32; 3]), HDF5Error> {
        let path = self.path.clone();

        tokio::task::spawn_blocking(move || {
            let file = File::open(&path)?;
            let dataset = file
                .dataset("target")
                .or_else(|_| file.dataset("volume"))
                .or_else(|_| file.dataset("data"))
                .map_err(|_| HDF5Error::DatasetNotFound("target, volume, or data".to_string()))?;

            let data: Array3<f32> = dataset.read()?;
            let resampled = Self::downsample(&data, target_size);
            let shape = resampled.shape();
            let dims = [shape[0] as u32, shape[1] as u32, shape[2] as u32];
            Ok((Self::to_bytes(&resampled), dims))
        })
        .await
        .unwrap()
    }
}
