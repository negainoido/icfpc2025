use crate::models::{ApiError, SelectRequest, SelectUpstreamResponse, ExploreUpstreamRequest, ExploreUpstreamResponse, GuessUpstreamRequest, GuessUpstreamResponse};
use reqwest::Client;
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

    pub async fn select(&self, request: &SelectRequest) -> Result<SelectUpstreamResponse, ApiError> {
        let url = format!("{}/select", self.base_url);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status().as_u16();
        
        if status != 200 {
            let error_text = response.text().await?;
            return Err(ApiError::InvalidRequest(format!("ICFP API returned status {}: {}", status, error_text)));
        }

        let body = response.json::<SelectUpstreamResponse>().await?;
        Ok(body)
    }

    pub async fn explore(&self, request: &ExploreUpstreamRequest) -> Result<ExploreUpstreamResponse, ApiError> {
        let url = format!("{}/explore", self.base_url);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status().as_u16();
        
        if status != 200 {
            let error_text = response.text().await?;
            return Err(ApiError::InvalidRequest(format!("ICFP API returned status {}: {}", status, error_text)));
        }

        let body = response.json::<ExploreUpstreamResponse>().await?;
        Ok(body)
    }

    pub async fn guess(&self, request: &GuessUpstreamRequest) -> Result<GuessUpstreamResponse, ApiError> {
        let url = format!("{}/guess", self.base_url);
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status().as_u16();
        
        if status != 200 {
            let error_text = response.text().await?;
            return Err(ApiError::InvalidRequest(format!("ICFP API returned status {}: {}", status, error_text)));
        }

        let body = response.json::<GuessUpstreamResponse>().await?;
        Ok(body)
    }
}