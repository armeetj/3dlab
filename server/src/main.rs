mod hdf5_reader;
mod routes;
mod state;

use axum::{
    extract::ConnectInfo,
    middleware::{self, Next},
    response::Response,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use state::AppState;

// ANSI colors
const GREEN: &str = "\x1b[32m";
const BLUE: &str = "\x1b[34m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const MAGENTA: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";

fn method_color(method: &str) -> &'static str {
    match method {
        "GET" => GREEN,
        "POST" => BLUE,
        "PUT" => YELLOW,
        "DELETE" => RED,
        "PATCH" => MAGENTA,
        _ => CYAN,
    }
}

async fn log_request(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let method = request.method().to_string();
    let uri = request.uri().to_string();
    let start = Instant::now();

    let response = next.run(request).await;

    let status = response.status().as_u16();
    let duration = start.elapsed();
    let duration_ms = duration.as_secs_f64() * 1000.0;

    let status_color = if status < 400 { GREEN } else { RED };

    println!(
        "{DIM}{:<15}{RESET} {}{:>7}{} {}{:<50}{} {}{:>3}{} {DIM}{:.1}ms{RESET}",
        addr.ip(),
        method_color(&method),
        method,
        RESET,
        RESET,
        uri,
        DIM,
        status_color,
        status,
        RESET,
        duration_ms,
    );

    response
}

#[tokio::main]
async fn main() {
    println!("{}Starting 3DLab server...{}", CYAN, RESET);

    // Initialize app state (scans samples/ for H5 files)
    let state = Arc::new(AppState::new("samples").await);
    println!("{}Found {} volumes{}", GREEN, state.volumes.len(), RESET);

    // CORS for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers(Any);

    // API routes
    let api_routes = Router::new()
        .route("/health", get(routes::health))
        .route("/volumes", get(routes::list_volumes))
        .route("/volumes/{id}/info", get(routes::get_volume_info))
        .route("/volumes/{id}/low", get(routes::get_volume_low))
        .route("/volumes/{id}/full", get(routes::get_volume_full))
        .route("/volumes/{id}/at/{resolution}", get(routes::get_volume_at_resolution))
        .with_state(state.clone());

    // Main router
    let app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new("client/dist").append_index_html_on_directories(true))
        .layer(cors)
        .layer(middleware::from_fn(log_request));

    let addr = SocketAddr::from(([0, 0, 0, 0], 9000));
    println!("{}Server listening on http://localhost:9000{}", GREEN, RESET);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
