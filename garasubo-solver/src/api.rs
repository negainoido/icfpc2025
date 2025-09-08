use anyhow::Context;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Serialize)]
pub struct ExploreRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    pub plans: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct ExploreResponse {
    pub session_id: Option<String>,
    pub results: Vec<Vec<u8>>,
    #[serde(rename = "queryCount")]
    pub query_count: i32,
}

#[derive(Serialize)]
pub struct GuessRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    pub map: GuessMap,
}

#[derive(Serialize)]
pub struct GuessMap {
    pub rooms: Vec<i32>,
    #[serde(rename = "startingRoom")]
    pub starting_room: usize,
    pub connections: Vec<Connection>,
}

#[derive(Serialize)]
pub struct Connection {
    pub from: RoomDoor,
    pub to: RoomDoor,
}

#[derive(Serialize)]
pub struct RoomDoor {
    pub room: usize,
    pub door: usize,
}

#[derive(Deserialize, Debug)]
pub struct GuessResponse {
    pub session_id: Option<String>,
    pub correct: bool,
}

#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: &String) -> Self {
        let client_id = std::env::var("CLIENT_ID").ok();
        let client_secret = std::env::var("CLIENT_SECRET").ok();

        Self {
            client: Client::new(),
            base_url: base_url.clone(),
            client_id,
            client_secret,
        }
    }

    async fn send_request_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        request_builder: reqwest::RequestBuilder,
    ) -> anyhow::Result<T> {
        let mut retry_count = 0;
        const MAX_RETRIES: usize = 5;
        const BASE_DELAY: Duration = Duration::from_millis(400);

        loop {
            let response_result = request_builder
                .try_clone()
                .context("Failed to clone request builder")?
                .send()
                .await;

            match response_result {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        let text = response
                            .text()
                            .await
                            .context("Failed to read response text")?;
                        let result = serde_json::from_str(&text).with_context(|| {
                            format!("Failed to parse response JSON. Original text: {}", text)
                        })?;
                        return Ok(result);
                    } else if status.is_server_error() && retry_count < MAX_RETRIES {
                        retry_count += 1;
                        let delay = BASE_DELAY * (2_u32.pow(retry_count as u32 - 1));
                        eprintln!(
                            "Server error {}, retrying in {:?} (attempt {}/{})",
                            status,
                            delay,
                            retry_count,
                            MAX_RETRIES + 1
                        );
                        sleep(delay).await;
                    } else {
                        let text = response
                            .text()
                            .await
                            .context("Failed to read error response body")?;
                        anyhow::bail!("API request failed with status {}: {}", status, text);
                    }
                }
                Err(err) => {
                    let should_retry = err.is_connect() || err.is_timeout() || err.is_request();

                    if should_retry && retry_count < MAX_RETRIES {
                        retry_count += 1;
                        let delay = BASE_DELAY * (2_u32.pow(retry_count as u32 - 1));
                        eprintln!(
                            "Connection error: {}, retrying in {:?} (attempt {}/{})",
                            err,
                            delay,
                            retry_count,
                            MAX_RETRIES + 1
                        );
                        sleep(delay).await;
                    } else {
                        anyhow::bail!("Request failed after {} retries: {}", MAX_RETRIES, err);
                    }
                }
            }
        }
    }

    async fn send_request_with_retry_no_response(
        &self,
        request_builder: reqwest::RequestBuilder,
    ) -> anyhow::Result<()> {
        let mut retry_count = 0;
        const MAX_RETRIES: usize = 3;
        const BASE_DELAY: Duration = Duration::from_millis(100);

        loop {
            let response_result = request_builder
                .try_clone()
                .context("Failed to clone request builder")?
                .send()
                .await;

            match response_result {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        return Ok(());
                    } else if status.is_server_error() && retry_count < MAX_RETRIES {
                        retry_count += 1;
                        let delay = BASE_DELAY * (2_u32.pow(retry_count as u32 - 1));
                        eprintln!(
                            "Server error {}, retrying in {:?} (attempt {}/{})",
                            status,
                            delay,
                            retry_count,
                            MAX_RETRIES + 1
                        );
                        sleep(delay).await;
                    } else {
                        let text = response
                            .text()
                            .await
                            .context("Failed to read error response body")?;
                        anyhow::bail!("API request failed with status {}: {}", status, text);
                    }
                }
                Err(err) => {
                    let should_retry = err.is_connect() || err.is_timeout() || err.is_request();

                    if should_retry && retry_count < MAX_RETRIES {
                        retry_count += 1;
                        let delay = BASE_DELAY * (2_u32.pow(retry_count as u32 - 1));
                        eprintln!(
                            "Connection error: {}, retrying in {:?} (attempt {}/{})",
                            err,
                            delay,
                            retry_count,
                            MAX_RETRIES + 1
                        );
                        sleep(delay).await;
                    } else {
                        anyhow::bail!("Request failed after {} retries: {}", MAX_RETRIES, err);
                    }
                }
            }
        }
    }

    fn add_auth_headers(
        &self,
        request_builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        if let (Some(client_id), Some(client_secret)) = (&self.client_id, &self.client_secret) {
            request_builder
                .header("CF-Access-Client-Id", client_id)
                .header("CF-Access-Client-Secret", client_secret)
        } else {
            println!("no client_id or client_secret");
            request_builder
        }
    }

    pub async fn select(
        &self,
        problem_name: String,
        user_name: Option<String>,
    ) -> anyhow::Result<SelectResponse> {
        let url = format!("{}/select", self.base_url);
        println!("{}", url);
        let request = SelectRequest {
            problem_name,
            user_name,
        };
        println!("{}", json!(request).to_string());

        let request_builder = self.add_auth_headers(self.client.post(&url)).json(&request);

        self.send_request_with_retry(request_builder).await
    }

    pub async fn abort_session(&self, session_id: &str) -> anyhow::Result<()> {
        let url = format!("{}/sessions/{}/abort", self.base_url, session_id);

        let request_builder = self.add_auth_headers(self.client.put(&url));

        self.send_request_with_retry_no_response(request_builder)
            .await
    }

    pub async fn explore(
        &self,
        session_id: &str,
        plans: &[String],
    ) -> anyhow::Result<ExploreResponse> {
        let url = format!("{}/explore", self.base_url);
        let request = ExploreRequest {
            session_id: Some(session_id.to_string()),
            user_name: None,
            plans: Vec::from(plans),
        };

        let request_builder = self.add_auth_headers(self.client.post(&url)).json(&request);

        let res = self.send_request_with_retry(request_builder).await;
        res.map(|r: ExploreResponse| {
            println!("Query Count: {}", r.query_count);
            r
        })
    }

    pub async fn guess(
        &self,
        session_id: &str,
        guess_map: GuessMap,
    ) -> anyhow::Result<GuessResponse> {
        let url = format!("{}/guess", self.base_url);
        let request = GuessRequest {
            session_id: Some(session_id.to_string()),
            user_name: None,
            map: guess_map,
        };

        let request_builder = self.add_auth_headers(self.client.post(&url)).json(&request);

        self.send_request_with_retry(request_builder).await
    }
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct SelectResponse {
    #[serde(default)]
    pub id: String,
    pub session_id: Option<String>,
    #[serde(rename = "problemName")]
    problem_name: String,
}

#[derive(Serialize)]
pub struct SelectRequest {
    #[serde(rename = "problemName")]
    problem_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_name: Option<String>,
}
