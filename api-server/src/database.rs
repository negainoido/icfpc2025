use crate::models::{ApiError, ApiLog, Session};
use sqlx::{mysql::MySqlPoolOptions, MySqlPool, Row};
use std::env;
use uuid::Uuid;

pub async fn create_pool() -> Result<MySqlPool, sqlx::Error> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:password@localhost:3306/icfpc2025".to_string());

    MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
}

pub async fn init_database(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id INT AUTO_INCREMENT PRIMARY KEY,
            session_id VARCHAR(255) UNIQUE NOT NULL,
            user_name VARCHAR(255) NULL,
            status ENUM('active', 'completed', 'failed') NOT NULL DEFAULT 'active',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            completed_at TIMESTAMP NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS api_logs (
            id INT AUTO_INCREMENT PRIMARY KEY,
            session_id VARCHAR(255) NOT NULL,
            endpoint VARCHAR(50) NOT NULL,
            request_body TEXT,
            response_body TEXT,
            response_status INT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            INDEX idx_session_id (session_id),
            FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn health_check_db(pool: &MySqlPool) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("SELECT 1 as health").fetch_one(pool).await?;
    let health: i32 = row.try_get("health")?;
    Ok(health == 1)
}

pub async fn has_active_session(pool: &MySqlPool) -> Result<bool, ApiError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE status = 'active'")
        .fetch_one(pool)
        .await?;

    Ok(count > 0)
}

pub async fn create_session(
    pool: &MySqlPool,
    user_name: Option<&str>,
) -> Result<Session, ApiError> {
    let session_id = Uuid::new_v4().to_string();

    let result =
        sqlx::query("INSERT INTO sessions (session_id, user_name, status) VALUES (?, ?, 'active')")
            .bind(&session_id)
            .bind(user_name)
            .execute(pool)
            .await?;

    let id = result.last_insert_id() as i32;

    let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;

    Ok(session)
}

pub async fn get_active_session(pool: &MySqlPool) -> Result<Option<Session>, ApiError> {
    let session =
        sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE status = 'active' LIMIT 1")
            .fetch_optional(pool)
            .await?;

    Ok(session)
}

pub async fn get_active_session_by_user(
    pool: &MySqlPool,
    user_name: &str,
) -> Result<Option<Session>, ApiError> {
    let session = sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE status = 'active' AND user_name = ? LIMIT 1",
    )
    .bind(user_name)
    .fetch_optional(pool)
    .await?;

    Ok(session)
}

pub async fn complete_session(pool: &MySqlPool, session_id: &str) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE sessions SET status = 'completed', completed_at = NOW() WHERE session_id = ?",
    )
    .bind(session_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn abort_session(pool: &MySqlPool, session_id: &str) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE sessions SET status = 'failed', completed_at = NOW() WHERE session_id = ? AND status = 'active'"
    )
    .bind(session_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn log_api_request(
    pool: &MySqlPool,
    session_id: &str,
    endpoint: &str,
    request_body: Option<&str>,
    response_body: Option<&str>,
    response_status: Option<i32>,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO api_logs (session_id, endpoint, request_body, response_body, response_status) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(session_id)
    .bind(endpoint)
    .bind(request_body)
    .bind(response_body)
    .bind(response_status)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_all_sessions(pool: &MySqlPool) -> Result<Vec<Session>, ApiError> {
    let sessions = sqlx::query_as::<_, Session>("SELECT * FROM sessions ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;

    Ok(sessions)
}

pub async fn get_session_by_id(
    pool: &MySqlPool,
    session_id: &str,
) -> Result<Option<Session>, ApiError> {
    let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE session_id = ?")
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

    Ok(session)
}

pub async fn get_api_logs_for_session(
    pool: &MySqlPool,
    session_id: &str,
) -> Result<Vec<ApiLog>, ApiError> {
    let logs = sqlx::query_as::<_, ApiLog>(
        "SELECT * FROM api_logs WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    Ok(logs)
}

// Acquire a named MySQL lock to serialize select() requests
pub async fn acquire_select_lock(pool: &MySqlPool) -> Result<bool, ApiError> {
    // timeout 5 seconds; returns 1 if success, 0 if timeout, NULL on error
    let got: Option<i64> = sqlx::query_scalar("SELECT GET_LOCK('icfpc_select_lock', 5)")
        .fetch_one(pool)
        .await?;
    Ok(got.unwrap_or(0) == 1)
}

pub async fn release_select_lock(pool: &MySqlPool) -> Result<(), ApiError> {
    let _released: Option<i64> = sqlx::query_scalar("SELECT RELEASE_LOCK('icfpc_select_lock')")
        .fetch_one(pool)
        .await?;
    Ok(())
}
