use axum::{
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::sync::Arc;

use shared::{ErrorResponse, VolumeListResponse, VolumeMetadataResponse};

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub available_samples: Vec<String>,
}

/// GET /api/health - Health check with available samples
pub async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let samples: Vec<String> = state
        .volumes
        .keys()
        .filter(|id| id.starts_with("target"))
        .cloned()
        .collect();

    Json(HealthResponse {
        status: "ok".to_string(),
        available_samples: samples,
    })
}

/// GET /api/volumes - List all available volumes
pub async fn list_volumes(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let volumes = state.list_volumes();
    Json(VolumeListResponse { volumes })
}

/// GET /api/volumes/:id/info - Get volume metadata
pub async fn get_volume_info(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.get_volume(&id) {
        Some(volume) => Ok(Json(VolumeMetadataResponse {
            info: volume.info.clone(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Volume '{}' not found", id),
            }),
        )),
    }
}

/// GET /api/volumes/:id/low - Get low-res volume data (64^3)
pub async fn get_volume_low(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.get_volume(&id) {
        Some(volume) => match volume.get_low_res_data().await {
            Ok(data) => Ok((
                [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
                data,
            )),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to read volume: {}", e),
                }),
            )),
        },
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Volume '{}' not found", id),
            }),
        )),
    }
}

/// GET /api/volumes/:id/full - Get full-res volume data
pub async fn get_volume_full(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.get_volume(&id) {
        Some(volume) => match volume.get_full_res_data().await {
            Ok(data) => Ok((
                [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
                data,
            )),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to read volume: {}", e),
                }),
            )),
        },
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Volume '{}' not found", id),
            }),
        )),
    }
}

/// GET /api/volumes/:id/at/:resolution - Get volume data at specific resolution
/// Resolution is the target size for the largest dimension (e.g., 64, 128, 256)
pub async fn get_volume_at_resolution(
    State(state): State<Arc<AppState>>,
    Path((id, resolution)): Path<(String, usize)>,
) -> impl IntoResponse {
    // Clamp resolution to reasonable bounds
    let resolution = resolution.clamp(16, 512);

    match state.get_volume(&id) {
        Some(volume) => match volume.get_data_at_resolution(resolution).await {
            Ok((data, dims)) => {
                // Return binary data with dimensions in headers
                let mut headers = HeaderMap::new();
                headers.insert(
                    axum::http::header::CONTENT_TYPE,
                    HeaderValue::from_static("application/octet-stream"),
                );
                headers.insert(
                    "x-volume-dims",
                    HeaderValue::from_str(&format!("{},{},{}", dims[0], dims[1], dims[2])).unwrap(),
                );
                Ok((headers, data))
            },
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to read volume: {}", e),
                }),
            )),
        },
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Volume '{}' not found", id),
            }),
        )),
    }
}
