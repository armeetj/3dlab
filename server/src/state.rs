use shared::VolumeInfo;
use std::collections::HashMap;
use std::path::Path;

use crate::hdf5_reader::HDF5Volume;

/// Application state shared across all request handlers
pub struct AppState {
    /// Map of volume ID to volume data
    pub volumes: HashMap<String, HDF5Volume>,
}

impl AppState {
    /// Create new app state by scanning a directory for H5 files
    pub async fn new(samples_dir: &str) -> Self {
        let mut volumes = HashMap::new();

        let path = Path::new(samples_dir);
        if path.exists() && path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if file_path.extension().map_or(false, |e| e == "h5") {
                        match HDF5Volume::open(&file_path).await {
                            Ok(volume) => {
                                log::info!("Loaded volume: {} ({:?})", volume.info.name, volume.info.dimensions);
                                volumes.insert(volume.info.id.clone(), volume);
                            }
                            Err(e) => {
                                log::warn!("Failed to load {:?}: {}", file_path, e);
                            }
                        }
                    }
                }
            }
        } else {
            log::warn!("Samples directory not found: {}", samples_dir);
        }

        Self { volumes }
    }

    /// Get volume info list
    pub fn list_volumes(&self) -> Vec<VolumeInfo> {
        self.volumes.values().map(|v| v.info.clone()).collect()
    }

    /// Get a specific volume
    pub fn get_volume(&self, id: &str) -> Option<&HDF5Volume> {
        self.volumes.get(id)
    }
}
