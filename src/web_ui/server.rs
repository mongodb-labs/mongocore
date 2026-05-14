use std::sync::Arc;

use axum::{
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::Embed;

use super::handlers;
use super::WebUiState;

#[derive(Embed)]
#[folder = "src/web_ui/assets/"]
struct Assets;

pub fn create_router(state: Arc<WebUiState>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/assets/{*path}", get(serve_asset))
        .route("/api/status", get(handlers::status))
        .route("/api/metrics", get(handlers::metrics))
        .route("/api/operations", get(handlers::operations))
        .route("/api/queries", get(handlers::queries))
        .route("/api/pipelines", get(handlers::pipelines))
        .route("/api/errors", get(handlers::errors))
        .route("/api/ingestion", get(handlers::ingestion))
        .route("/api/llm", get(handlers::llm))
        .route("/api/cache", get(handlers::cache))
        .with_state(state)
}

async fn serve_index() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(content) => {
            Html(String::from_utf8_lossy(content.data.as_ref()).to_string()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn serve_asset(axum::extract::Path(path): axum::extract::Path<String>) -> impl IntoResponse {
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_from_path(&path);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                .body(axum::body::Body::from(content.data.to_vec()))
                .unwrap()
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn mime_from_path(path: &str) -> &'static str {
    if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".html") {
        "text/html"
    } else {
        "application/octet-stream"
    }
}
