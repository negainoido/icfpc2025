use axum::{extract::State, http::StatusCode, response::Json};
use sqlx::MySqlPool;
use tracing::error;

use crate::{
    database::{
        abort_session, complete_session, create_session_if_no_active, fail_session, get_active_session,
        get_active_session_by_user, get_all_sessions, get_api_logs_for_session, get_session_by_id,
        log_api_request,
    },
    icfpc_client::IcfpClient,
    models::{
        ApiError, ErrorResponse, ExploreRequest, ExploreResponse, ExploreUpstreamRequest, GuessRequest,
        GuessResponse, GuessUpstreamRequest, SelectRequest, SelectResponse, Session, SessionDetail,
        SessionsListResponse,
    },
};

impl From<ApiError> for (StatusCode, Json<ErrorResponse>) {
    fn from(err: ApiError) -> Self {
        let (status_code, error_type, message) = match err {
            ApiError::Database(ref db_err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DatabaseError",
                format!("Database error: {}", db_err),
            ),
            ApiError::Http(ref http_err) => (
                StatusCode::BAD_GATEWAY,
                "HttpError",
                format!("HTTP request error: {}", http_err),
            ),
            ApiError::SessionAlreadyActive => (
                StatusCode::CONFLICT,
                "SessionAlreadyActive",
                "Session already active".to_string(),
            ),
            ApiError::NoActiveSession => (
                StatusCode::NOT_FOUND,
                "NoActiveSession",
                "No active session".to_string(),
            ),
            ApiError::SessionNotFound => (
                StatusCode::NOT_FOUND,
                "SessionNotFound",
                "Session not found".to_string(),
            ),
            ApiError::InvalidRequest(ref msg) => (
                StatusCode::BAD_REQUEST,
                "InvalidRequest",
                msg.clone(),
            ),
        };

        error!("API Error: {} (Status: {})", err, status_code.as_u16());
        
        let error_response = ErrorResponse {
            error: error_type.to_string(),
            message,
        };

        (status_code, Json(error_response))
    }
}

impl From<ApiError> for StatusCode {
    fn from(err: ApiError) -> Self {
        let (status_code, _) = <ApiError as Into<(StatusCode, Json<ErrorResponse>)>>::into(err);
        status_code
    }
}

pub async fn select(
    State(pool): State<MySqlPool>,
    Json(payload): Json<SelectRequest>,
) -> Result<Json<SelectResponse>, (StatusCode, Json<ErrorResponse>)> {
    // まずトランザクション内でセッションを作成（アクティブセッションがある場合は失敗）
    let session = create_session_if_no_active(&pool, payload.user_name.as_deref())
        .await
        .map_err(ApiError::from)?;

    let icfp_client = IcfpClient::new()?;
    
    // セッション作成成功後にICFPCのAPIを呼び出し
    match icfp_client.select(&payload).await {
        Ok(upstream_response) => {
            // API呼び出し成功時のログを記録
            log_api_request(
                &pool,
                &session.session_id,
                "select",
                Some(&serde_json::to_string(&payload).unwrap_or_default()),
                Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
                Some(200),
            )
            .await
            .map_err(ApiError::from)?;

            let response = SelectResponse {
                session_id: session.session_id,
                problem_name: upstream_response.problem_name,
            };

            Ok(Json(response))
        }
        Err(api_error) => {
            // ICFPC API呼び出し失敗時はセッションをfailedステータスに変更
            let _ = fail_session(&pool, &session.session_id).await;
            
            // エラーログを記録
            let error_msg = format!("{}", api_error);
            let status_code = match api_error {
                ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
                ApiError::Http(_) => StatusCode::BAD_GATEWAY,
                ApiError::SessionAlreadyActive => StatusCode::CONFLICT,
                ApiError::NoActiveSession | ApiError::SessionNotFound => StatusCode::NOT_FOUND,
                ApiError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            };
            
            // エラーログを記録（セッションはfailedになっているので外部キー制約は問題なし）
            let _ = log_api_request(
                &pool,
                &session.session_id,
                "select",
                Some(&serde_json::to_string(&payload).unwrap_or_default()),
                Some(&error_msg),
                Some(status_code.as_u16() as i32),
            )
            .await;
            
            Err(api_error.into())
        }
    }
}

pub async fn explore(
    State(pool): State<MySqlPool>,
    Json(payload): Json<ExploreRequest>,
) -> Result<Json<ExploreResponse>, (StatusCode, Json<ErrorResponse>)> {
    // セッションを特定する: session_idまたはuser_nameのいずれかを使用
    let session = match (&payload.session_id, &payload.user_name) {
        (Some(session_id), _) => {
            // session_idが指定された場合は従来通りの処理
            let session = get_active_session(&pool)
                .await
                .map_err(ApiError::from)?
                .ok_or_else(|| ApiError::NoActiveSession)?;

            if session.session_id != *session_id {
                return Err(ApiError::InvalidRequest(
                    "Session ID mismatch".to_string(),
                ).into());
            }
            session
        }
        (None, Some(user_name)) => {
            // user_nameが指定された場合はそのユーザーのアクティブセッションを取得
            get_active_session_by_user(&pool, user_name)
                .await
                .map_err(ApiError::from)?
                .ok_or_else(|| ApiError::NoActiveSession)?
        }
        (None, None) => {
            return Err(ApiError::InvalidRequest(
                "Either session_id or user_name must be specified".to_string(),
            ).into());
        }
    };

    let icfp_client = IcfpClient::new()?;

    let upstream_request = ExploreUpstreamRequest {
        id: icfp_client.get_team_id(),
        plans: payload.plans,
    };
    let request_body = serde_json::to_string(&upstream_request).unwrap_or_default();

    match icfp_client.explore(&upstream_request).await {
        Ok(upstream_response) => {
            log_api_request(
                &pool,
                &session.session_id,
                "explore",
                Some(&request_body),
                Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
                Some(200),
            )
            .await
            .map_err(ApiError::from)?;

            let response = ExploreResponse {
                session_id: session.session_id,
                results: upstream_response.results,
                query_count: upstream_response.query_count,
            };

            Ok(Json(response))
        }
        Err(api_error) => {
            let error_msg = format!("{}", api_error);
            let status_code = match api_error {
                ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
                ApiError::Http(_) => StatusCode::BAD_GATEWAY,
                ApiError::SessionAlreadyActive => StatusCode::CONFLICT,
                ApiError::NoActiveSession | ApiError::SessionNotFound => StatusCode::NOT_FOUND,
                ApiError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            };
            let _ = log_api_request(
                &pool,
                &session.session_id,
                "explore",
                Some(&request_body),
                Some(&error_msg),
                Some(status_code.as_u16() as i32),
            )
            .await;
            Err(api_error.into())
        }
    }
}

pub async fn guess(
    State(pool): State<MySqlPool>,
    Json(payload): Json<GuessRequest>,
) -> Result<Json<GuessResponse>, (StatusCode, Json<ErrorResponse>)> {
    // セッションを特定する: session_idまたはuser_nameのいずれかを使用
    let session = match (&payload.session_id, &payload.user_name) {
        (Some(session_id), _) => {
            // session_idが指定された場合は従来通りの処理
            let session = get_active_session(&pool)
                .await
                .map_err(ApiError::from)?
                .ok_or_else(|| ApiError::NoActiveSession)?;

            if session.session_id != *session_id {
                return Err(ApiError::InvalidRequest(
                    "Session ID mismatch".to_string(),
                ).into());
            }
            session
        }
        (None, Some(user_name)) => {
            // user_nameが指定された場合はそのユーザーのアクティブセッションを取得
            get_active_session_by_user(&pool, user_name)
                .await
                .map_err(ApiError::from)?
                .ok_or_else(|| ApiError::NoActiveSession)?
        }
        (None, None) => {
            return Err(ApiError::InvalidRequest(
                "Either session_id or user_name must be specified".to_string(),
            ).into());
        }
    };

    let icfp_client = IcfpClient::new()?;

    let upstream_request = GuessUpstreamRequest {
        id: icfp_client.get_team_id(),
        map: payload.map,
    };
    let request_body = serde_json::to_string(&upstream_request).unwrap_or_default();

    match icfp_client.guess(&upstream_request).await {
        Ok(upstream_response) => {
            log_api_request(
                &pool,
                &session.session_id,
                "guess",
                Some(&request_body),
                Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
                Some(200),
            )
            .await
            .map_err(ApiError::from)?;

            complete_session(&pool, &session.session_id)
                .await
                .map_err(ApiError::from)?;

            let response = GuessResponse {
                session_id: session.session_id,
                correct: upstream_response.correct,
            };

            Ok(Json(response))
        }
        Err(api_error) => {
            let error_msg = format!("{}", api_error);
            let status_code = match api_error {
                ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
                ApiError::Http(_) => StatusCode::BAD_GATEWAY,
                ApiError::SessionAlreadyActive => StatusCode::CONFLICT,
                ApiError::NoActiveSession | ApiError::SessionNotFound => StatusCode::NOT_FOUND,
                ApiError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            };
            let _ = log_api_request(
                &pool,
                &session.session_id,
                "guess",
                Some(&request_body),
                Some(&error_msg),
                Some(status_code.as_u16() as i32),
            )
            .await;
            Err(api_error.into())
        }
    }
}

pub async fn get_sessions(
    State(pool): State<MySqlPool>,
) -> Result<Json<SessionsListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let sessions = get_all_sessions(&pool).await.map_err(ApiError::from)?;

    let response = SessionsListResponse { sessions };

    Ok(Json(response))
}

pub async fn get_current_session(
    State(pool): State<MySqlPool>,
) -> Result<Json<Option<Session>>, (StatusCode, Json<ErrorResponse>)> {
    let session = get_active_session(&pool).await.map_err(ApiError::from)?;

    Ok(Json(session))
}

pub async fn get_session_detail(
    State(pool): State<MySqlPool>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<SessionDetail>, (StatusCode, Json<ErrorResponse>)> {
    let session = get_session_by_id(&pool, &session_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::SessionNotFound)?;

    let api_logs = get_api_logs_for_session(&pool, &session_id)
        .await
        .map_err(ApiError::from)?;

    let response = SessionDetail { session, api_logs };

    Ok(Json(response))
}

pub async fn abort_session_handler(
    State(pool): State<MySqlPool>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let session = get_session_by_id(&pool, &session_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::SessionNotFound)?;

    if session.status != "active" {
        return Err(ApiError::InvalidRequest(
            "Session is not active".to_string(),
        ).into());
    }

    abort_session(&pool, &session_id)
        .await
        .map_err(ApiError::from)?;

    log_api_request(
        &pool,
        &session_id,
        "abort",
        None,
        Some(&serde_json::json!({"aborted": true}).to_string()),
        Some(200),
    )
    .await
    .map_err(ApiError::from)?;

    Ok(StatusCode::OK)
}
