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

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: i32,
    pub session_id: String,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SelectResponse {
    pub session_id: String,
    #[serde(flatten)]
    pub upstream_response: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExploreRequest {
    pub session_id: String,
    #[serde(flatten)]
    pub explore_data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuessRequest {
    pub session_id: String,
    #[serde(flatten)]
    pub guess_data: serde_json::Value,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T, message: Option<String>) -> Self {
        Self {
            success: true,
            data: Some(data),
            message,
        }
    }

    pub fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            message: Some(message),
        }
    }
}