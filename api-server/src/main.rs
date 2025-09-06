use axum::{
    http::{HeaderValue, Method},
    routing::post,
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod database;
mod handlers;
mod icfpc_client;
mod models;

use database::{create_pool, init_database};
use handlers::{explore, guess, select};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let pool = create_pool().await.expect("Failed to create database pool");
    
    // Initialize the database schema
    init_database(&pool)
        .await
        .expect("Failed to initialize database");

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    let app = Router::new()
        .route("/select", post(select))
        .route("/explore", post(explore))
        .route("/guess", post(guess))
        .with_state(pool)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("🚀 ICFPC 2025 Proxy API Server running on http://localhost:8080");
    println!("Available endpoints:");
    println!("  POST /select  - Create new session and call ICFP select API");
    println!("  POST /explore - Call ICFP explore API with session");
    println!("  POST /guess   - Call ICFP guess API and terminate session");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}