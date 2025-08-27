use axum::{
    http::{HeaderValue, Method},
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod database;
mod handlers;
mod models;

use handlers::{get_spaceship_file, get_solutions, create_solution};
use database::create_pool;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let pool = create_pool().await.expect("Failed to create database pool");

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    let app = Router::new()
        .route("/", get(health_check))
        .route("/api/health", get(health_check))
        .route("/api/spaceship/:filename", get(get_spaceship_file))
        .route("/api/solutions", get(get_solutions).post(create_solution))
        .with_state(pool)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 Server running on http://localhost:8080");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "message": "ICFPC 2025 API Server is running",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}