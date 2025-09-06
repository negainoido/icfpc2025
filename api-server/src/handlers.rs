use axum::{extract::State, http::StatusCode, response::Json};
use sqlx::MySqlPool;
use tracing::error;

use crate::{
    database::{
        abort_session, complete_session, create_session, get_active_session, get_all_sessions,
        get_api_logs_for_session, get_session_by_id, has_active_session, log_api_request,
    },
    icfpc_client::IcfpClient,
    models::{
        ApiError, ExploreRequest, ExploreResponse, ExploreUpstreamRequest,
        GuessRequest, GuessResponse, GuessUpstreamRequest, SelectRequest, SelectResponse, Session,
        SessionDetail, SessionsListResponse,
    },
};

impl From<ApiError> for StatusCode {
    fn from(err: ApiError) -> Self {
        let status_code = match err {
            ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Http(_) => StatusCode::BAD_GATEWAY,
            ApiError::SessionAlreadyActive => StatusCode::CONFLICT,
            ApiError::NoActiveSession | ApiError::SessionNotFound => StatusCode::NOT_FOUND,
            ApiError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
        };

        error!("API Error: {} (Status: {})", err, status_code.as_u16());
        status_code
    }
}

pub async fn select(
    State(pool): State<MySqlPool>,
    Json(payload): Json<SelectRequest>,
) -> Result<Json<SelectResponse>, StatusCode> {
    if has_active_session(&pool).await.map_err(StatusCode::from)? {
        return Err(StatusCode::from(ApiError::SessionAlreadyActive));
    }

    let icfp_client = IcfpClient::new().map_err(StatusCode::from)?;
    let upstream_response = icfp_client
        .select(&payload)
        .await
        .map_err(StatusCode::from)?;

    let session = create_session(&pool, payload.user_name.as_deref()).await.map_err(StatusCode::from)?;

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

    Ok(Json(response))
}

pub async fn explore(
    State(pool): State<MySqlPool>,
    Json(payload): Json<ExploreRequest>,
) -> Result<Json<ExploreResponse>, StatusCode> {
    let session = get_active_session(&pool)
        .await
        .map_err(StatusCode::from)?
        .ok_or_else(|| StatusCode::from(ApiError::NoActiveSession))?;

    if session.session_id != payload.session_id {
        return Err(StatusCode::from(ApiError::InvalidRequest(
            "Session ID mismatch".to_string(),
        )));
    }

    let icfp_client = IcfpClient::new().map_err(StatusCode::from)?;

    let upstream_request = ExploreUpstreamRequest {
        id: icfp_client.get_team_id(),
        plans: payload.plans,
    };
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

    Ok(Json(response))
}

pub async fn guess(
    State(pool): State<MySqlPool>,
    Json(payload): Json<GuessRequest>,
) -> Result<Json<GuessResponse>, StatusCode> {
    let session = get_active_session(&pool)
        .await
        .map_err(StatusCode::from)?
        .ok_or_else(|| StatusCode::from(ApiError::NoActiveSession))?;

    if session.session_id != payload.session_id {
        return Err(StatusCode::from(ApiError::InvalidRequest(
            "Session ID mismatch".to_string(),
        )));
    }

    let icfp_client = IcfpClient::new().map_err(StatusCode::from)?;

    let upstream_request = GuessUpstreamRequest {
        id: icfp_client.get_team_id(),
        map: payload.map,
    };
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

    Ok(Json(response))
}

pub async fn get_sessions(
    State(pool): State<MySqlPool>,
) -> Result<Json<SessionsListResponse>, StatusCode> {
    let sessions = get_all_sessions(&pool).await.map_err(StatusCode::from)?;

    let response = SessionsListResponse { sessions };

    Ok(Json(response))
}

pub async fn get_current_session(
    State(pool): State<MySqlPool>,
) -> Result<Json<Option<Session>>, StatusCode> {
    let session = get_active_session(&pool).await.map_err(StatusCode::from)?;

    Ok(Json(session))
}

pub async fn get_session_detail(
    State(pool): State<MySqlPool>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<SessionDetail>, StatusCode> {
    let session = get_session_by_id(&pool, &session_id)
        .await
        .map_err(StatusCode::from)?
        .ok_or_else(|| StatusCode::from(ApiError::SessionNotFound))?;

    let api_logs = get_api_logs_for_session(&pool, &session_id)
        .await
        .map_err(StatusCode::from)?;

    let response = SessionDetail { session, api_logs };

    Ok(Json(response))
}

pub async fn abort_session_handler(
    State(pool): State<MySqlPool>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<StatusCode, StatusCode> {
    let session = get_session_by_id(&pool, &session_id)
        .await
        .map_err(StatusCode::from)?
        .ok_or_else(|| StatusCode::from(ApiError::SessionNotFound))?;

    if session.status != "active" {
        return Err(StatusCode::from(ApiError::InvalidRequest(
            "Session is not active".to_string(),
        )));
    }

    abort_session(&pool, &session_id)
        .await
        .map_err(StatusCode::from)?;

    log_api_request(
        &pool,
        &session_id,
        "abort",
        None,
        Some(&serde_json::json!({"aborted": true}).to_string()),
        Some(200),
    )
    .await
    .map_err(StatusCode::from)?;

    Ok(StatusCode::OK)
}
