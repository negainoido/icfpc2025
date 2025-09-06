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
    models::{ApiError, ApiResponse, SelectRequest, SelectResponse, ExploreRequest, ExploreResponse, ExploreUpstreamRequest, GuessRequest, GuessResponse, GuessUpstreamRequest},
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
    Json(payload): Json<SelectRequest>,
) -> Result<Json<ApiResponse<SelectResponse>>, StatusCode> {
    if has_active_session(&pool).await.map_err(StatusCode::from)? {
        return Err(StatusCode::from(ApiError::SessionAlreadyActive));
    }

    let icfp_client = IcfpClient::new().map_err(StatusCode::from)?;
    let upstream_response = icfp_client.select(&payload).await.map_err(StatusCode::from)?;

    let session = create_session(&pool).await.map_err(StatusCode::from)?;

    log_api_request(
        &pool,
        &session.session_id,
        "select",
        Some(&serde_json::to_string(&payload).unwrap_or_default()),
        Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
        Some(200),
    )
    .await
    .map_err(StatusCode::from)?;

    let response = SelectResponse {
        session_id: session.session_id,
        problem_name: upstream_response.problem_name,
    };

    Ok(Json(ApiResponse::success(
        response,
        Some("Session created and select request completed".to_string()),
    )))
}

pub async fn explore(
    State(pool): State<MySqlPool>,
    Json(payload): Json<ExploreRequest>,
) -> Result<Json<ApiResponse<ExploreResponse>>, StatusCode> {
    let session = get_active_session(&pool)
        .await
        .map_err(StatusCode::from)?
        .ok_or_else(|| StatusCode::from(ApiError::NoActiveSession))?;

    if session.session_id != payload.session_id {
        return Err(StatusCode::from(ApiError::InvalidRequest("Session ID mismatch".to_string())));
    }

    let upstream_request = ExploreUpstreamRequest {
        id: payload.id,
        plans: payload.plans,
    };

    let icfp_client = IcfpClient::new().map_err(StatusCode::from)?;
    let request_body = serde_json::to_string(&upstream_request).unwrap_or_default();
    
    let upstream_response = icfp_client
        .explore(&upstream_request)
        .await
        .map_err(StatusCode::from)?;

    log_api_request(
        &pool,
        &session.session_id,
        "explore",
        Some(&request_body),
        Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
        Some(200),
    )
    .await
    .map_err(StatusCode::from)?;

    let response = ExploreResponse {
        session_id: payload.session_id,
        results: upstream_response.results,
        query_count: upstream_response.query_count,
    };

    Ok(Json(ApiResponse::success(
        response,
        Some("Explore request completed".to_string()),
    )))
}

pub async fn guess(
    State(pool): State<MySqlPool>,
    Json(payload): Json<GuessRequest>,
) -> Result<Json<ApiResponse<GuessResponse>>, StatusCode> {
    let session = get_active_session(&pool)
        .await
        .map_err(StatusCode::from)?
        .ok_or_else(|| StatusCode::from(ApiError::NoActiveSession))?;

    if session.session_id != payload.session_id {
        return Err(StatusCode::from(ApiError::InvalidRequest("Session ID mismatch".to_string())));
    }

    let upstream_request = GuessUpstreamRequest {
        id: payload.id,
        map: payload.map,
    };

    let icfp_client = IcfpClient::new().map_err(StatusCode::from)?;
    let request_body = serde_json::to_string(&upstream_request).unwrap_or_default();
    
    let upstream_response = icfp_client
        .guess(&upstream_request)
        .await
        .map_err(StatusCode::from)?;

    log_api_request(
        &pool,
        &session.session_id,
        "guess",
        Some(&request_body),
        Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
        Some(200),
    )
    .await
    .map_err(StatusCode::from)?;

    complete_session(&pool, &session.session_id)
        .await
        .map_err(StatusCode::from)?;

    let response = GuessResponse {
        session_id: payload.session_id,
        correct: upstream_response.correct,
    };

    Ok(Json(ApiResponse::success(
        response,
        Some("Guess request completed and session terminated".to_string()),
    )))
}