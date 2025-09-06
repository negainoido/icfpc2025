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
            status ENUM('active', 'completed', 'failed', 'pending') NOT NULL DEFAULT 'active',
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

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pending_requests (
            id INT AUTO_INCREMENT PRIMARY KEY,
            session_id VARCHAR(255) UNIQUE NOT NULL,
            problem_name VARCHAR(255) NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
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

pub async fn create_session_if_no_active(
    pool: &MySqlPool,
    user_name: Option<&str>,
) -> Result<Session, ApiError> {
    let mut tx = pool.begin().await?;
    
    // トランザクション内でアクティブセッションの存在をチェック
    let active_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE status = 'active'")
        .fetch_one(&mut *tx)
        .await?;
    
    if active_count > 0 {
        tx.rollback().await?;
        return Err(ApiError::SessionAlreadyActive);
    }
    
    // アクティブセッションが存在しない場合のみセッション作成
    let session_id = Uuid::new_v4().to_string();
    let result = sqlx::query("INSERT INTO sessions (session_id, user_name, status) VALUES (?, ?, 'active')")
        .bind(&session_id)
        .bind(user_name)
        .execute(&mut *tx)
        .await?;
    
    let id = result.last_insert_id() as i32;
    let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    
    tx.commit().await?;
    Ok(session)
}

pub async fn create_session_or_enqueue(
    pool: &MySqlPool,
    user_name: Option<&str>,
    enqueue: bool,
) -> Result<Session, ApiError> {
    let mut tx = pool.begin().await?;
    
    // トランザクション内でアクティブセッションの存在をチェック（行ロック付き）
    let active_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE status = 'active' FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    
    let session_id = Uuid::new_v4().to_string();
    let status = if active_count > 0 && enqueue {
        "pending"
    } else if active_count > 0 {
        tx.rollback().await?;
        return Err(ApiError::SessionAlreadyActive);
    } else {
        "active"
    };
    
    let result = sqlx::query("INSERT INTO sessions (session_id, user_name, status) VALUES (?, ?, ?)")
        .bind(&session_id)
        .bind(user_name)
        .bind(status)
        .execute(&mut *tx)
        .await?;
    
    let id = result.last_insert_id() as i32;
    let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    
    tx.commit().await?;
    Ok(session)
}

pub async fn get_pending_sessions(pool: &MySqlPool) -> Result<Vec<Session>, ApiError> {
    let sessions = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE status = 'pending' ORDER BY created_at ASC")
        .fetch_all(pool)
        .await?;
    Ok(sessions)
}

pub async fn activate_pending_session(pool: &MySqlPool, session_id: &str) -> Result<(), ApiError> {
    sqlx::query("UPDATE sessions SET status = 'active' WHERE session_id = ? AND status = 'pending'")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn fail_session(pool: &MySqlPool, session_id: &str) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE sessions SET status = 'failed', completed_at = NOW() WHERE session_id = ?"
    )
    .bind(session_id)
    .execute(pool)
    .await?;
    
    Ok(())
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

pub async fn complete_session(pool: &MySqlPool, session_id: &str) -> Result<Option<(String, String)>, ApiError> {
    let mut tx = pool.begin().await?;
    
    // セッション終了
    sqlx::query(
        "UPDATE sessions SET status = 'completed', completed_at = NOW() WHERE session_id = ?",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await?;

    // 現在アクティブなセッションがないことを確認
    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE status = 'active' FOR UPDATE"
    )
    .fetch_one(&mut *tx)
    .await?;

    let next_session = if active_count == 0 {
        // 次のpendingセッションを取得（行ロック付き）
        let next_session: Option<(String, String)> = sqlx::query_as(
            "SELECT session_id, user_name FROM sessions WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1 FOR UPDATE"
        )
        .fetch_optional(&mut *tx)
        .await?;

        // 次のpendingセッションがある場合はアクティベート
        if let Some((next_session_id, _)) = &next_session {
            let updated_rows = sqlx::query("UPDATE sessions SET status = 'active' WHERE session_id = ? AND status = 'pending'")
                .bind(next_session_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
            
            // 更新に成功した場合のみnext_sessionを返す
            if updated_rows > 0 {
                next_session
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    tx.commit().await?;
    Ok(next_session)
}

pub async fn abort_session(pool: &MySqlPool, session_id: &str) -> Result<Option<(String, String)>, ApiError> {
    let mut tx = pool.begin().await?;
    
    // セッション中止
    sqlx::query(
        "UPDATE sessions SET status = 'failed', completed_at = NOW() WHERE session_id = ? AND status = 'active'"
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await?;

    // 現在アクティブなセッションがないことを確認
    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE status = 'active' FOR UPDATE"
    )
    .fetch_one(&mut *tx)
    .await?;

    let next_session = if active_count == 0 {
        // 次のpendingセッションを取得（行ロック付き）
        let next_session: Option<(String, String)> = sqlx::query_as(
            "SELECT session_id, user_name FROM sessions WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1 FOR UPDATE"
        )
        .fetch_optional(&mut *tx)
        .await?;

        // 次のpendingセッションがある場合はアクティベート
        if let Some((next_session_id, _)) = &next_session {
            let updated_rows = sqlx::query("UPDATE sessions SET status = 'active' WHERE session_id = ? AND status = 'pending'")
                .bind(next_session_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
            
            // 更新に成功した場合のみnext_sessionを返す
            if updated_rows > 0 {
                next_session
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    tx.commit().await?;
    Ok(next_session)
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

pub async fn save_pending_request(
    pool: &MySqlPool,
    session_id: &str,
    problem_name: &str,
) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO pending_requests (session_id, problem_name) VALUES (?, ?)")
        .bind(session_id)
        .bind(problem_name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_pending_request(
    pool: &MySqlPool,
    session_id: &str,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM pending_requests WHERE session_id = ?")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}
