use reqwest::Client;
use crate::{SelectRequest, SelectResponse};
use anyhow::Context;
use serde_json::json;

pub struct ApiClient {
    client: Client,
    base_url: String,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl ApiClient {
    pub(crate) fn new(base_url: String) -> Self {
        let client_id = std::env::var("CLIENT_ID").ok();
        let client_secret = std::env::var("CLIENT_SECRET").ok();

        Self {
            client: Client::new(),
            base_url,
            client_id,
            client_secret,
        }
    }

    fn add_auth_headers(&self, request_builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let (Some(client_id), Some(client_secret)) = (&self.client_id, &self.client_secret) {
            println!("CLIENT_ID: {:?}", client_id);
            println!("CLIENT_SECRET: {:?}", client_secret);
            request_builder
                .header("CF-Access-Client-Id", client_id)
                .header("CF-Access-Client-Secret", client_secret)
        } else {
            request_builder
        }
    }

    pub(crate) async fn select(&self, problem_name: String, user_name: Option<String>) -> anyhow::Result<SelectResponse> {
        let url = format!("{}/api/select", self.base_url);
        println!("{}", url);
        let request = SelectRequest {
            problem_name,
            user_name,
        };
        println!("{}", json!(request).to_string());

        let response = self.add_auth_headers(self.client.post(&url))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("Failed to send select request to {}", url))?;

        if response.status().is_success() {
            let result: SelectResponse = response.json().await
                .context("Failed to parse select response JSON")?;
            Ok(result)
        } else {
            let status = response.status();
            let text = response.text().await.context("Failed to read error response body")?;
            anyhow::bail!("Select API request failed with status {}: {}", status, text);
        }
    }

    pub(crate) async fn abort_session(&self, session_id: &str) -> anyhow::Result<()> {
        let url = format!("{}/api/sessions/{}/abort", self.base_url, session_id);

        let response = self.add_auth_headers(self.client.put(&url))
            .send()
            .await
            .with_context(|| format!("Failed to send abort request to {}", url))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().await.context("Failed to read abort error response body")?;
            anyhow::bail!("Session abort failed with status {}: {}", status, text);
        }
    }
}