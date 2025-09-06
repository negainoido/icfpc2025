use axum::{extract::State, http::StatusCode, response::Json};
use sqlx::MySqlPool;
use tracing::error;

use crate::{
    database::{
        abort_session, complete_session, create_session_if_no_active, create_session_or_enqueue,
        delete_pending_request, fail_session, get_active_session, get_active_session_by_user,
        get_all_sessions, get_api_logs_for_session, get_session_by_id, log_api_request,
        save_pending_request,
    },
    icfpc_client::IcfpClient,
    models::{
        ApiError, ErrorResponse, ExploreRequest, ExploreResponse, ExploreUpstreamRequest,
        GuessRequest, GuessResponse, GuessUpstreamRequest, SelectRequest, SelectResponse, Session,
        SessionDetail, SessionsListResponse, SessionExport, SessionInfo, ApiHistoryEntry,
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
            ApiError::InvalidRequest(ref msg) => {
                (StatusCode::BAD_REQUEST, "InvalidRequest", msg.clone())
            }
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
    // キューオプションに応じてセッションを作成
    let session = create_session_or_enqueue(&pool, payload.user_name.as_deref(), payload.enqueue)
        .await
        .map_err(ApiError::from)?;

    // pendingセッションの場合はキューに入れただけなので、ICFPC APIは呼ばない
    if session.status == "pending" {
        // pending requestを保存
        save_pending_request(&pool, &session.session_id, &payload.problem_name)
            .await
            .map_err(ApiError::from)?;

        let response = SelectResponse {
            session_id: session.session_id,
            problem_name: None,
            status: "pending".to_string(),
        };
        return Ok(Json(response));
    }

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
                problem_name: Some(upstream_response.problem_name),
                status: "active".to_string(),
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
                return Err(ApiError::InvalidRequest("Session ID mismatch".to_string()).into());
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
            )
            .into());
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
                return Err(ApiError::InvalidRequest("Session ID mismatch".to_string()).into());
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
            )
            .into());
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

            let next_session = complete_session(&pool, &session.session_id)
                .await
                .map_err(ApiError::from)?;

            // 次のセッションがアクティベートされた場合は自動実行
            if let Some((next_session_id, user_name)) = next_session {
                let _ = execute_pending_select(&pool, &next_session_id, Some(&user_name)).await;
            }

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
        return Err(ApiError::InvalidRequest("Session is not active".to_string()).into());
    }

    let next_session = abort_session(&pool, &session_id)
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

    // 次のセッションがアクティベートされた場合は自動実行
    if let Some((next_session_id, user_name)) = next_session {
        let _ = execute_pending_select(&pool, &next_session_id, Some(&user_name)).await;
    }

    Ok(StatusCode::OK)
}

pub async fn export_session(
    State(pool): State<MySqlPool>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<SessionExport>, (StatusCode, Json<ErrorResponse>)> {
    let session = get_session_by_id(&pool, &session_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::SessionNotFound)?;

    let api_logs = get_api_logs_for_session(&pool, &session_id)
        .await
        .map_err(ApiError::from)?;

    let session_info = SessionInfo {
        session_id: session.session_id,
        user_name: session.user_name,
        status: session.status,
        created_at: session.created_at,
        completed_at: session.completed_at,
    };

    let api_history: Vec<ApiHistoryEntry> = api_logs
        .into_iter()
        .map(|log| {
            let request = log.request_body.as_ref().and_then(|body| {
                serde_json::from_str(body).ok()
            });
            
            let response = log.response_body.as_ref().and_then(|body| {
                serde_json::from_str(body).ok()
            });

            ApiHistoryEntry {
                endpoint: log.endpoint,
                timestamp: log.created_at,
                request,
                response,
                status: log.response_status,
            }
        })
        .collect();

    let export = SessionExport {
        session_info,
        api_history,
    };

    Ok(Json(export))
}

async fn execute_pending_select(
    pool: &MySqlPool,
    session_id: &str,
    user_name: Option<&str>,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await?;

    // トランザクション内でセッションがactiveであることを確認
    let session_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM sessions WHERE session_id = ? FOR UPDATE")
            .bind(session_id)
            .fetch_optional(&mut *tx)
            .await?;

    match session_status {
        Some(status) if status == "active" => {
            // セッションがactiveの場合のみ処理を続行
        }
        _ => {
            // セッションがactive以外、または存在しない場合は処理をスキップ
            tx.rollback().await?;
            return Ok(());
        }
    }

    // pending requestを取得
    let problem_name = match sqlx::query_scalar::<_, String>(
        "SELECT problem_name FROM pending_requests WHERE session_id = ?",
    )
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(problem_name) => problem_name,
        None => {
            tx.rollback().await?;
            return Ok(()); // 保存されたリクエストがない場合はスキップ
        }
    };

    // コミットしてトランザクション終了
    tx.commit().await?;

    let icfp_client = IcfpClient::new()?;

    let payload = SelectRequest {
        problem_name: problem_name.clone(),
        user_name: user_name.map(String::from),
        enqueue: false,
    };

    // ICFPC APIを呼び出し
    match icfp_client.select(&payload).await {
        Ok(upstream_response) => {
            // API呼び出し成功時のログを記録
            let _ = log_api_request(
                pool,
                session_id,
                "select",
                Some(&serde_json::to_string(&payload).unwrap_or_default()),
                Some(&serde_json::to_string(&upstream_response).unwrap_or_default()),
                Some(200),
            )
            .await;

            // pending requestを削除
            let _ = delete_pending_request(pool, session_id).await;
        }
        Err(_api_error) => {
            // ICFPC API呼び出し失敗時はセッションをfailedステータスに変更
            let _ = fail_session(pool, session_id).await;

            // pending requestを削除
            let _ = delete_pending_request(pool, session_id).await;
        }
    }

    Ok(())
}
