use serde::{Deserialize, Serialize};

/// Information about a volume available on the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    /// Unique identifier for this volume
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Full resolution dimensions [x, y, z]
    pub dimensions: [u32; 3],
    /// Low-res preview dimensions [x, y, z]
    pub low_res_dimensions: [u32; 3],
    /// Size of low-res preview in bytes
    pub low_res_size: u64,
    /// Size of full resolution in bytes
    pub full_res_size: u64,
    /// Value range [min, max]
    pub value_range: [f32; 2],
}

/// Response for listing available volumes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeListResponse {
    pub volumes: Vec<VolumeInfo>,
}

/// Response for volume metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMetadataResponse {
    pub info: VolumeInfo,
}

/// Response for upload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub id: String,
    pub success: bool,
    pub message: Option<String>,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}
