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

pub async fn get_users(State(pool): State<MySqlPool>) -> Result<Json<ApiResponse<Vec<User>>>, StatusCode> {
    match sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC")
        .fetch_all(&pool)
        .await
    {
        Ok(users) => Ok(Json(ApiResponse {
            success: true,
            data: Some(users),
            message: Some("Users retrieved successfully".to_string()),
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_user(
    State(pool): State<MySqlPool>,
    Path(id): Path<i32>,
) -> Result<Json<ApiResponse<User>>, StatusCode> {
    match sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
    {
        Ok(user) => Ok(Json(ApiResponse {
            success: true,
            data: Some(user),
            message: Some("User retrieved successfully".to_string()),
        })),
        Err(sqlx::Error::RowNotFound) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create_user(
    State(pool): State<MySqlPool>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<ApiResponse<User>>, StatusCode> {
    match sqlx::query_as::<_, User>(
        "INSERT INTO users (name, email) VALUES (?, ?) RETURNING id, name, email, created_at, updated_at"
    )
    .bind(&payload.name)
    .bind(&payload.email)
    .fetch_one(&pool)
    .await
    {
        Ok(user) => Ok(Json(ApiResponse {
            success: true,
            data: Some(user),
            message: Some("User created successfully".to_string()),
        })),
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            Err(StatusCode::CONFLICT)
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn update_user(
    State(pool): State<MySqlPool>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<User>>, StatusCode> {
    let mut query = "UPDATE users SET ".to_string();
    let mut updates = Vec::new();
    let mut params = Vec::new();

    if let Some(name) = &payload.name {
        updates.push("name = ?");
        params.push(name.as_str());
    }

    if let Some(email) = &payload.email {
        updates.push("email = ?");
        params.push(email.as_str());
    }

    if updates.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    query.push_str(&updates.join(", "));
    query.push_str(" WHERE id = ?");

    let mut sql_query = sqlx::query(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(id);

    match sql_query.execute(&pool).await {
        Ok(result) if result.rows_affected() == 0 => Err(StatusCode::NOT_FOUND),
        Ok(_) => {
            match sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
            {
                Ok(user) => Ok(Json(ApiResponse {
                    success: true,
                    data: Some(user),
                    message: Some("User updated successfully".to_string()),
                })),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn delete_user(
    State(pool): State<MySqlPool>,
    Path(id): Path<i32>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
    {
        Ok(result) if result.rows_affected() == 0 => Err(StatusCode::NOT_FOUND),
        Ok(_) => Ok(Json(ApiResponse {
            success: true,
            data: None,
            message: Some("User deleted successfully".to_string()),
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

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