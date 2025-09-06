use crate::models::ApiError;
use reqwest::Client;
use serde_json::Value;
use std::env;

pub struct IcfpClient {
    client: Client,
    base_url: String,
    auth_token: String,
}

impl IcfpClient {
    pub fn new() -> Result<Self, ApiError> {
        let base_url = env::var("ICFPC_API_BASE_URL")
            .unwrap_or_else(|_| "https://icfpcontest2025.github.io/api".to_string());
        
        let auth_token = env::var("ICFPC_AUTH_TOKEN")
            .map_err(|_| ApiError::InvalidRequest("ICFPC_AUTH_TOKEN environment variable is required".to_string()))?;

        Ok(Self {
            client: Client::new(),
            base_url,
            auth_token,
        })
    }

    pub async fn select(&self) -> Result<Value, ApiError> {
        let url = format!("{}/select", self.base_url);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .send()
            .await?;

        let status = response.status().as_u16();
        let body = response.json::<Value>().await?;

        if status != 200 {
            return Err(ApiError::InvalidRequest(format!("ICFP API returned status {}: {}", status, body)));
        }

        Ok(body)
    }

    pub async fn explore(&self, explore_data: Value) -> Result<Value, ApiError> {
        let url = format!("{}/explore", self.base_url);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(&explore_data)
            .send()
            .await?;

        let status = response.status().as_u16();
        let body = response.json::<Value>().await?;

        if status != 200 {
            return Err(ApiError::InvalidRequest(format!("ICFP API returned status {}: {}", status, body)));
        }

        Ok(body)
    }

    pub async fn guess(&self, guess_data: Value) -> Result<Value, ApiError> {
        let url = format!("{}/guess", self.base_url);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(&guess_data)
            .send()
            .await?;

        let status = response.status().as_u16();
        let body = response.json::<Value>().await?;

        if status != 200 {
            return Err(ApiError::InvalidRequest(format!("ICFP API returned status {}: {}", status, body)));
        }

        Ok(body)
    }
}