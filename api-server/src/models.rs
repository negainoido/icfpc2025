use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Session already active")]
    SessionAlreadyActive,
    #[error("No active session")]
    NoActiveSession,
    #[error("Session not found")]
    SessionNotFound,
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}


#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: i32,
    pub session_id: String,
    pub user_name: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct ApiLog {
    pub id: i32,
    pub session_id: String,
    pub endpoint: String,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub response_status: Option<i32>,
    pub created_at: DateTime<Utc>,
}

// Select API types
#[derive(Debug, Serialize, Deserialize)]
pub struct SelectRequest {
    #[serde(rename = "problemName")]
    pub problem_name: String,
    pub user_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelectUpstreamRequest {
    pub id: String,
    #[serde(rename = "problemName")]
    pub problem_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelectUpstreamResponse {
    #[serde(rename = "problemName")]
    pub problem_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelectResponse {
    pub session_id: String,
    #[serde(rename = "problemName")]
    pub problem_name: String,
}

// Explore API types
#[derive(Debug, Serialize, Deserialize)]
pub struct ExploreRequest {
    pub session_id: String,
    pub plans: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExploreUpstreamRequest {
    pub id: String,
    pub plans: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExploreUpstreamResponse {
    pub results: Vec<Vec<i32>>,
    #[serde(rename = "queryCount")]
    pub query_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExploreResponse {
    pub session_id: String,
    pub results: Vec<Vec<i32>>,
    #[serde(rename = "queryCount")]
    pub query_count: i32,
}

// Guess API types
#[derive(Debug, Serialize, Deserialize)]
pub struct Connection {
    pub from: DoorLocation,
    pub to: DoorLocation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DoorLocation {
    pub room: i32,
    pub door: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Map {
    pub rooms: Vec<i32>,
    #[serde(rename = "startingRoom")]
    pub starting_room: i32,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuessRequest {
    pub session_id: String,
    pub map: Map,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuessUpstreamRequest {
    pub id: String,
    pub map: Map,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuessUpstreamResponse {
    pub correct: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuessResponse {
    pub session_id: String,
    pub correct: bool,
}

// Session detail response types
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionDetail {
    pub session: Session,
    pub api_logs: Vec<ApiLog>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionsListResponse {
    pub sessions: Vec<Session>,
}

