use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use sqlx::MySqlPool;

use crate::{
    database::{
        complete_session, create_session, get_active_session, has_active_session, log_api_request,
    },
    icfpc_client::IcfpClient,
    models::{ApiError, ApiResponse, ExploreRequest, GuessRequest, SelectResponse},
};

impl From<ApiError> for StatusCode {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Http(_) => StatusCode::BAD_GATEWAY,
            ApiError::SessionAlreadyActive => StatusCode::CONFLICT,
            ApiError::NoActiveSession | ApiError::SessionNotFound => StatusCode::NOT_FOUND,
            ApiError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
        }
    }
}

pub async fn select(
    State(pool): State<MySqlPool>,
) -> Result<Json<ApiResponse<SelectResponse>>, StatusCode> {
    if has_active_session(&pool).await.map_err(|e| e.into())? {
        return Err(ApiError::SessionAlreadyActive.into());
    }

    let icfp_client = IcfpClient::new().map_err(|e| e.into())?;
    let upstream_response = icfp_client.select().await.map_err(|e| e.into())?;

    let session = create_session(&pool).await.map_err(|e| e.into())?;

    log_api_request(
        &pool,
        &session.session_id,
        "select",
        None,
        Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
        Some(200),
    )
    .await
    .map_err(|e| e.into())?;

    let response = SelectResponse {
        session_id: session.session_id,
        upstream_response,
    };

    Ok(Json(ApiResponse::success(
        response,
        Some("Session created and select request completed".to_string()),
    )))
}

pub async fn explore(
    State(pool): State<MySqlPool>,
    Json(payload): Json<ExploreRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let session = get_active_session(&pool)
        .await
        .map_err(|e| e.into())?
        .ok_or(ApiError::NoActiveSession)?;

    if session.session_id != payload.session_id {
        return Err(ApiError::InvalidRequest("Session ID mismatch".to_string()).into());
    }

    let icfp_client = IcfpClient::new().map_err(|e| e.into())?;
    let request_body = serde_json::to_string(&payload.explore_data).unwrap_or_default();
    
    let upstream_response = icfp_client
        .explore(payload.explore_data)
        .await
        .map_err(|e| e.into())?;

    log_api_request(
        &pool,
        &session.session_id,
        "explore",
        Some(&request_body),
        Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
        Some(200),
    )
    .await
    .map_err(|e| e.into())?;

    Ok(Json(ApiResponse::success(
        upstream_response,
        Some("Explore request completed".to_string()),
    )))
}

pub async fn guess(
    State(pool): State<MySqlPool>,
    Json(payload): Json<GuessRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let session = get_active_session(&pool)
        .await
        .map_err(|e| e.into())?
        .ok_or(ApiError::NoActiveSession)?;

    if session.session_id != payload.session_id {
        return Err(ApiError::InvalidRequest("Session ID mismatch".to_string()).into());
    }

    let icfp_client = IcfpClient::new().map_err(|e| e.into())?;
    let request_body = serde_json::to_string(&payload.guess_data).unwrap_or_default();
    
    let upstream_response = icfp_client
        .guess(payload.guess_data)
        .await
        .map_err(|e| e.into())?;

    log_api_request(
        &pool,
        &session.session_id,
        "guess",
        Some(&request_body),
        Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
        Some(200),
    )
    .await
    .map_err(|e| e.into())?;

    complete_session(&pool, &session.session_id)
        .await
        .map_err(|e| e.into())?;

    Ok(Json(ApiResponse::success(
        upstream_response,
        Some("Guess request completed and session terminated".to_string()),
    )))
}