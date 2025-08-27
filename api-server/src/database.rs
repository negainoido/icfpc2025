use sqlx::{mysql::MySqlPoolOptions, MySqlPool, Row};
use std::env;

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
        CREATE TABLE IF NOT EXISTS users (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255) UNIQUE NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
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