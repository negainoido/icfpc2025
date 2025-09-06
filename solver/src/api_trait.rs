use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ApiClientTrait: Send + Sync {
    async fn select_problem(&self, problem_name: &str) -> Result<()>;
    async fn explore(&self, plans: Vec<String>) -> Result<(Vec<Vec<u8>>, u32)>;
}
