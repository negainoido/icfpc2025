use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SpaceshipFileResponse {
    pub filename: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Solution {
    pub id: i32,
    pub problem_id: i32,
    pub problem_type: Option<String>,
    pub status: Option<String>,
    pub solver: String,
    pub score: Option<i32>,
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSolutionRequest {
    pub problem_id: i32,
    pub problem_type: Option<String>,
    pub status: Option<String>,
    pub solver: String,
    pub score: Option<i32>,
}