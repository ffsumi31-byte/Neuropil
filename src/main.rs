use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Node {
    id: String,
    group: u8,
    summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Link {
    source: String,
    target: String,
    weight: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BrainGraph {
    nodes: Vec<Node>,
    links: Vec<Link>,
}

type SharedBrain = Arc<Mutex<BrainGraph>>;

async fn get_brain(State(brain): State<SharedBrain>) -> Json<BrainGraph> {
    let data = brain.lock().unwrap().clone();
    Json(data)
}

async fn learn_concept(
    State(brain): State<SharedBrain>,
    Json(payload): Json<BrainGraph>,
) -> &'static str {
    let mut current = brain.lock().unwrap();
    for node in payload.nodes {
        if !current.nodes.iter().any(|n| n.id == node.id) {
            current.nodes.push(node);
        }
    }
    for link in payload.links {
        current.links.push(link);
    }
    "Concept integrated into 3D brain"
}

#[tokio::main]
async fn main() {
    let brain = Arc::new(Mutex::new(BrainGraph::default()));

    let app = Router::new()
        .route("/api/brain", get(get_brain))
        .route("/api/learn", post(learn_concept))
        .layer(CorsLayer::permissive())
        .with_state(brain);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
