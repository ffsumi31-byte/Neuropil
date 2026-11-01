//! Neuropil — sharp-cube 3D studio. The server *is* the AI: all planning, geometry and
//! export happen in Rust (see brain.rs, builders.rs, geom.rs, nlu.rs). The browser only draws.

mod brain;
mod builders;
mod geom;
mod nlu;

use axum::{
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use brain::{AiRequest, AiResponse, OpRequest};
use geom::Cube;
use serde::Deserialize;
use std::env;
use tower_http::cors::CorsLayer;

// The frontend is embedded in the binary, so it works no matter where Render starts the process.
async fn index() -> Html<&'static str> {
    Html(include_str!("../index.html"))
}

async fn styles() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], include_str!("../styles.css"))
}

async fn script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript; charset=utf-8")], include_str!("../studio.js"))
}

async fn favicon() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "engine": "neuropil-rust",
        "version": env!("CARGO_PKG_VERSION"),
        "objects": builders::KINDS.len(),
        "objectKinds": builders::KINDS,
    }))
}

/// Prompt in, new workspace + plan log + reply out. No external AI is called.
async fn ai(Json(req): Json<AiRequest>) -> Json<AiResponse> {
    Json(brain::think(req))
}

/// One structured edit from the inspector (push / extrude / taper / mirror).
async fn op(Json(req): Json<OpRequest>) -> impl IntoResponse {
    match brain::apply_op(req) {
        Ok(r) => (
            StatusCode::OK,
            Json(serde_json::to_value(&r).unwrap_or(serde_json::Value::Null)),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "OpError", "details": e })),
        ),
    }
}

#[derive(Deserialize)]
struct ExportReq {
    workspace: Vec<Cube>,
}

/// OBJ is generated here, in Rust, with every transform baked into world space.
async fn export_obj(Json(req): Json<ExportReq>) -> impl IntoResponse {
    if req.workspace.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Workspace is empty - nothing to export.".to_string(),
        );
    }
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        geom::to_obj(&req.workspace),
    )
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index))
        .route("/index.html", get(index))
        .route("/styles.css", get(styles))
        .route("/studio.js", get(script))
        .route("/favicon.ico", get(favicon))
        .route("/api/health", get(health))
        .route("/api/ai", post(ai))
        .route("/api/prompt", post(ai)) // legacy path used by brain_ingest.py
        .route("/api/op", post(op))
        .route("/api/export/obj", post(export_obj))
        .layer(CorsLayer::permissive());

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("NEUROPIL Rust engine listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
