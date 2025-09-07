use anyhow::Context;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

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

    fn add_auth_headers(
        &self,
        request_builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        if let (Some(client_id), Some(client_secret)) = (&self.client_id, &self.client_secret) {
            request_builder
                .header("CF-Access-Client-Id", client_id)
                .header("CF-Access-Client-Secret", client_secret)
        } else {
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

        let response = self
            .add_auth_headers(self.client.post(&url))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("Failed to send select request to {}", url))?;

        if response.status().is_success() {
            let result: SelectResponse = response
                .json()
                .await
                .context("Failed to parse select response JSON")?;
            Ok(result)
        } else {
            let status = response.status();
            let text = response
                .text()
                .await
                .context("Failed to read error response body")?;
            anyhow::bail!("Select API request failed with status {}: {}", status, text);
        }
    }

    pub async fn abort_session(&self, session_id: &str) -> anyhow::Result<()> {
        let url = format!("{}/sessions/{}/abort", self.base_url, session_id);

        let response = self
            .add_auth_headers(self.client.put(&url))
            .send()
            .await
            .with_context(|| format!("Failed to send abort request to {}", url))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response
                .text()
                .await
                .context("Failed to read abort error response body")?;
            anyhow::bail!("Session abort failed with status {}: {}", status, text);
        }
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

        let response = self
            .add_auth_headers(self.client.post(&url))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("Failed to send explore request to {}", url))?;

        if response.status().is_success() {
            let result: ExploreResponse = response
                .json()
                .await
                .context("Failed to parse explore response JSON")?;
            Ok(result)
        } else {
            let status = response.status();
            let text = response
                .text()
                .await
                .context("Failed to read explore error response body")?;
            anyhow::bail!(
                "Explore API request failed with status {}: {}",
                status,
                text
            );
        }
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

        let response = self
            .add_auth_headers(self.client.post(&url))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("Failed to send guess request to {}", url))?;

        if response.status().is_success() {
            let result: GuessResponse = response
                .json()
                .await
                .context("Failed to parse guess response JSON")?;
            Ok(result)
        } else {
            let status = response.status();
            let text = response
                .text()
                .await
                .context("Failed to read guess error response body")?;
            anyhow::bail!("Guess API request failed with status {}: {}", status, text);
        }
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
