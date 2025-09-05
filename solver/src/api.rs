use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
    team_id: String,
}

#[derive(Debug, Serialize)]
struct ExploreRequest {
    id: String,
    plans: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ExploreResponse {
    results: Vec<Vec<u8>>,
    #[serde(rename = "queryCount")]
    query_count: u32,
}

#[derive(Debug, Serialize)]
struct SelectRequest {
    id: String,
    #[serde(rename = "problemName")]
    problem_name: String,
}

#[derive(Debug, Deserialize)]
struct SelectResponse {
    #[serde(rename = "problemName")]
    problem_name: String,
}

impl ApiClient {
    pub fn new(base_url: String, team_id: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            team_id,
        }
    }

    pub async fn select_problem(&self, problem_name: &str) -> Result<()> {
        let url = format!("{}/select", self.base_url);
        let request = SelectRequest {
            id: self.team_id.clone(),
            problem_name: problem_name.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Select problem failed with status {}: {}",
                status,
                text
            ));
        }

        let select_response: SelectResponse = response.json().await?;
        println!("Selected problem: {}", select_response.problem_name);
        Ok(())
    }

    pub async fn explore(&self, plans: Vec<String>) -> Result<(Vec<Vec<u8>>, u32)> {
        println!("[API] Calling /explore with {} plans:", plans.len());
        for (i, plan) in plans.iter().enumerate() {
            println!("  Plan {}: '{}'", i + 1, plan);
        }
        
        let url = format!("{}/explore", self.base_url);
        let request = ExploreRequest {
            id: self.team_id.clone(),
            plans: plans.clone(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Explore failed with status {}: {}",
                status,
                text
            ));
        }

        let explore_response: ExploreResponse = response.json().await?;
        println!("[API] Response: {} results, total query count: {}", 
                 explore_response.results.len(), explore_response.query_count);
        for (i, result) in explore_response.results.iter().enumerate() {
            println!("  Result {}: {:?}", i + 1, result);
        }
        Ok((explore_response.results, explore_response.query_count))
    }
}