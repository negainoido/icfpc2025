use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    Extension,
};
use serde_json::json;
use sqlx::MySqlPool;
use std::fs;

use crate::models::{ApiResponse, CreateUserRequest, UpdateUserRequest, User, SpaceshipFileResponse, Solution, CreateSolutionRequest};

pub async fn get_spaceship_file(
    Path(filename): Path<String>,
) -> Result<Json<ApiResponse<SpaceshipFileResponse>>, StatusCode> {
    // Security check: only allow alphanumeric characters and hyphens to prevent directory traversal
    if !filename.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(StatusCode::BAD_REQUEST);
    }

    let file_path = format!("resources/spaceship/{}.txt", filename);
    
    match fs::read_to_string(&file_path) {
        Ok(content) => Ok(Json(ApiResponse {
            success: true,
            data: Some(SpaceshipFileResponse {
                filename: filename.clone(),
                content,
            }),
            message: Some("File retrieved successfully".to_string()),
        })),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(StatusCode::NOT_FOUND)
        },
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_solutions(State(pool): State<MySqlPool>) -> Result<Json<ApiResponse<Vec<Solution>>>, StatusCode> {
    match sqlx::query_as::<_, Solution>("SELECT * FROM solutions ORDER BY ts DESC")
        .fetch_all(&pool)
        .await
    {
        Ok(solutions) => Ok(Json(ApiResponse {
            success: true,
            data: Some(solutions),
            message: Some("Solutions retrieved successfully".to_string()),
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_solution(
    Path(id): Path<i32>,
    State(pool): State<MySqlPool>,
) -> Result<Json<ApiResponse<Solution>>, StatusCode> {
    match sqlx::query_as::<_, Solution>("SELECT * FROM solutions WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
    {
        Ok(solution) => Ok(Json(ApiResponse {
            success: true,
            data: Some(solution),
            message: Some("Solution retrieved successfully".to_string()),
        })),
        Err(sqlx::Error::RowNotFound) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create_solution(
    State(pool): State<MySqlPool>,
    Json(payload): Json<CreateSolutionRequest>,
) -> Result<Json<ApiResponse<Solution>>, StatusCode> {
    match sqlx::query(
        "INSERT INTO solutions (problem_id, problem_type, status, solver, score) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&payload.problem_id)
    .bind(&payload.problem_type)
    .bind(&payload.status)
    .bind(&payload.solver)
    .bind(&payload.score)
    .execute(&pool)
    .await
    {
        Ok(result) => {
            let id = result.last_insert_id() as i32;
            match sqlx::query_as::<_, Solution>("SELECT * FROM solutions WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
            {
                Ok(solution) => Ok(Json(ApiResponse {
                    success: true,
                    data: Some(solution),
                    message: Some("Solution created successfully".to_string()),
                })),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}