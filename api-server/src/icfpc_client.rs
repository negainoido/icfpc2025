use crate::models::{ApiError, SelectRequest, SelectUpstreamRequest, SelectUpstreamResponse, ExploreUpstreamRequest, ExploreUpstreamResponse, GuessUpstreamRequest, GuessUpstreamResponse};
use reqwest::Client;
use std::env;
use tracing::info;

pub struct IcfpClient {
    client: Client,
    base_url: String,
    auth_token: String,
}

impl IcfpClient {
    pub fn new() -> Result<Self, ApiError> {
        let base_url = env::var("ICFPC_API_BASE_URL")
            .unwrap_or_else(|_| "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/".to_string());
        
        let auth_token = env::var("ICFPC_AUTH_TOKEN")
            .map_err(|_| ApiError::InvalidRequest("ICFPC_AUTH_TOKEN environment variable is required".to_string()))?;

        Ok(Self {
            client: Client::new(),
            base_url,
            auth_token,
        })
    }

    pub fn get_team_id(&self) -> String {
        self.auth_token.clone()
    }

    pub async fn select(&self, request: &SelectRequest) -> Result<SelectUpstreamResponse, ApiError> {
        let url = format!("{}/select", self.base_url);
        
        // Create upstream request with team ID from environment variable
        let upstream_request = crate::models::SelectUpstreamRequest {
            id: self.auth_token.clone(),
            problem_name: request.problem_name.clone(),
        };
        
        info!("Sending request to ICFPC select API: {}", 
              serde_json::to_string(&upstream_request).unwrap_or_default());
        
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&upstream_request)
            .send()
            .await?;

        let status = response.status().as_u16();
        
        if status != 200 {
            let error_text = response.text().await?;
            return Err(ApiError::InvalidRequest(format!("ICFPC API returned status {}: {}", status, error_text)));
        }

        let body = response.json::<SelectUpstreamResponse>().await?;
        info!("Received response from ICFPC select API: {}", 
              serde_json::to_string(&body).unwrap_or_default());
        Ok(body)
    }

    pub async fn explore(&self, request: &ExploreUpstreamRequest) -> Result<ExploreUpstreamResponse, ApiError> {
        let url = format!("{}/explore", self.base_url);
        
        info!("Sending request to ICFPC explore API: {}", 
              serde_json::to_string(request).unwrap_or_default());
        
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status().as_u16();
        
        if status != 200 {
            let error_text = response.text().await?;
            return Err(ApiError::InvalidRequest(format!("ICFPC API returned status {}: {}", status, error_text)));
        }

        let body = response.json::<ExploreUpstreamResponse>().await?;
        info!("Received response from ICFPC explore API: {}", 
              serde_json::to_string(&body).unwrap_or_default());
        Ok(body)
    }

    pub async fn guess(&self, request: &GuessUpstreamRequest) -> Result<GuessUpstreamResponse, ApiError> {
        let url = format!("{}/guess", self.base_url);
        
        info!("Sending request to ICFPC guess API: {}", 
              serde_json::to_string(request).unwrap_or_default());
        
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status().as_u16();
        
        if status != 200 {
            let error_text = response.text().await?;
            return Err(ApiError::InvalidRequest(format!("ICFPC API returned status {}: {}", status, error_text)));
        }

        let body = response.json::<GuessUpstreamResponse>().await?;
        info!("Received response from ICFPC guess API: {}", 
              serde_json::to_string(&body).unwrap_or_default());
        Ok(body)
    }
}