use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    sync::{Arc, Mutex},
};
use tower_http::{cors::CorsLayer, services::ServeDir};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CubeState {
    name: String,
    role: String,
    pos: [f32; 3],
    size: [f32; 3],
    rot: [f32; 3],
    color: String,
    mat: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AIStep {
    op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    face: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dist: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inset: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    factor: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pos: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PromptPayload {
    prompt: String,
    #[serde(default)]
    workspace: Vec<CubeState>,
}

#[derive(Debug, Serialize)]
struct PlanResponse {
    explanation: String,
    steps: Vec<AIStep>,
}

#[derive(Debug, Serialize)]
struct ErrorDiagnostic {
    error: String,
    details: String,
}

type SharedWorkspace = Arc<Mutex<Vec<CubeState>>>;

async fn get_workspace(State(ws): State<SharedWorkspace>) -> Json<Vec<CubeState>> {
    Json(ws.lock().unwrap().clone())
}

async fn plan_from_prompt(
    State(ws): State<SharedWorkspace>,
    Json(payload): Json<PromptPayload>,
) -> impl IntoResponse {
    let prompt_trimmed = payload.prompt.trim();
    if prompt_trimmed.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::to_value(ErrorDiagnostic {
                error: "EmptyPromptError".into(),
                details: "Prompt cannot be empty. Provide a 3D modeling instruction.".into(),
            }).unwrap()),
        );
    }

    if !payload.workspace.is_empty() {
        *ws.lock().unwrap() = payload.workspace.clone();
    }

    let lower = prompt_trimmed.to_lowercase();
    let root_name = payload
        .workspace
        .first()
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "Origin_Cube".to_string());

    let steps = if lower.contains("obelisk") || lower.contains("tower") {
        vec![
            AIStep {
                op: "extrude".into(),
                target: Some(root_name),
                face: Some("top".into()),
                dist: Some(1.2),
                inset: Some(0.85),
                color: Some("#12879B".into()),
                name: None, mode: None, factor: None, pos: None, size: None,
            },
            AIStep {
                op: "extrude".into(),
                target: None,
                face: Some("top".into()),
                dist: Some(1.5),
                inset: Some(0.75),
                color: Some("#5EB58B".into()),
                name: None, mode: None, factor: None, pos: None, size: None,
            },
            AIStep {
                op: "taper".into(),
                target: None,
                mode: Some("top-in".into()),
                factor: Some(0.2),
                name: None, face: None, dist: None, inset: None, pos: None, size: None, color: None,
            },
        ]
    } else {
        vec![
            AIStep {
                op: "extrude".into(),
                target: Some(root_name),
                face: Some("top".into()),
                dist: Some(1.0),
                inset: Some(0.8),
                color: Some("#5EB58B".into()),
                name: None, mode: None, factor: None, pos: None, size: None,
            },
            AIStep {
                op: "taper".into(),
                target: None,
                mode: Some("top-in".into()),
                factor: Some(0.5),
                name: None, face: None, dist: None, inset: None, pos: None, size: None, color: None,
            },
        ]
    };

    let response = PlanResponse {
        explanation: format!("Rust AI Core generated {} sharp-cube steps for '{}':", steps.len(), prompt_trimmed),
        steps,
    };

    (StatusCode::OK, Json(serde_json::to_value(response).unwrap()))
}

#[tokio::main]
async fn main() {
    // Starts with nothing but a single 1x1x1 Origin Cube
    let initial = vec![CubeState {
        name: "Origin_Cube".into(),
        role: "Root 1x1x1 sharp origin cube".into(),
        pos: [0.0, 0.5, 0.0],
        size: [1.0, 1.0, 1.0],
        rot: [0.0, 0.0, 0.0],
        color: "#5EB58B".into(),
        mat: "matte".into(),
    }];

    let state = Arc::new(Mutex::new(initial));

    let app = Router::new()
        .route("/api/brain", get(get_workspace))
        .route("/api/prompt", post(plan_from_prompt))
        .fallback_service(ServeDir::new("."))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("NEUROPIL Sharp-Cube Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
