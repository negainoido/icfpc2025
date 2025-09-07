use crate::api::{
    ApiClient, ExploreResponse, GuessMap as ApiGuessMap, GuessResponse, SelectResponse,
};
use anyhow::Context;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SessionGuard {
    api_client: ApiClient,
    session_id: String,
    should_abort: Arc<AtomicBool>,
}

impl SessionGuard {
    fn new(api_client: ApiClient, session_id: String) -> Self {
        Self {
            api_client,
            session_id,
            should_abort: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn mark_success(&self) {
        self.should_abort.store(false, Ordering::Relaxed);
    }

    pub async fn explore(&self, plans: &[String]) -> anyhow::Result<ExploreResponse> {
        self.api_client.explore(&self.session_id, plans).await
    }

    pub async fn guess(&self, guess_map: ApiGuessMap) -> anyhow::Result<GuessResponse> {
        self.api_client.guess(&self.session_id, guess_map).await
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if self.should_abort.load(Ordering::Relaxed) {
            let session_id = self.session_id.clone();
            let api_client = self.api_client.clone();

            let thread = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = api_client.abort_session(&session_id).await {
                        eprintln!(
                            "Warning: Failed to abort session {} during drop: {:#}",
                            session_id, e
                        );
                    } else {
                        println!("Session {} aborted successfully during drop", session_id);
                    }
                });
            });
            thread.join().unwrap();
        }
    }
}

pub struct SessionManager {
    api_client: ApiClient,
    pub current_session: Arc<Mutex<Option<String>>>,
}

impl SessionManager {
    pub fn new(api_client: ApiClient) -> Self {
        Self {
            api_client,
            current_session: Arc::new(Mutex::new(None)),
        }
    }

    #[allow(dead_code)]
    async fn start_session(
        &self,
        problem_name: String,
        user_name: Option<String>,
    ) -> anyhow::Result<SelectResponse> {
        let response = self
            .api_client
            .select(problem_name.clone(), user_name.clone())
            .await
            .with_context(|| format!("Failed to start session for problem '{}'", problem_name))?;

        let mut session = self.current_session.lock().await;
        *session = Some(response.session_id.clone().unwrap_or("dummy".to_string()).clone());

        Ok(response)
    }

    pub async fn start_session_with_guard(
        &self,
        problem_name: String,
        user_name: Option<String>,
    ) -> anyhow::Result<SessionGuard> {
        let response = self
            .api_client
            .select(problem_name.clone(), user_name.clone())
            .await
            .with_context(|| format!("Failed to start session for problem '{}'", problem_name))?;

        let mut session = self.current_session.lock().await;
        *session = Some(response.session_id.clone().unwrap_or("dummy".to_string()).clone());

        Ok(SessionGuard::new(
            self.api_client.clone(),
            response.session_id.clone().unwrap_or("dummy".to_string()),
        ))
    }

    #[allow(dead_code)]
    async fn abort_current_session(&self) -> anyhow::Result<()> {
        let session = self.current_session.lock().await;
        if let Some(ref session_id) = *session {
            if let Err(e) = self.api_client.abort_session(session_id).await {
                eprintln!("Warning: Failed to abort session {}: {:#}", session_id, e);
            } else {
                println!("Session {} aborted successfully", session_id);
            }
        }
        Ok(())
    }
}
