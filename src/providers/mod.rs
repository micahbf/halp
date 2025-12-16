pub mod anthropic;
pub mod openai;

use crate::config::{Config, Provider as ProviderType};
use async_trait::async_trait;
use std::io::Write;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn stream_completion(
        &self,
        prompt: &str,
        system: &str,
        output: &mut (dyn Write + Send),
    ) -> Result<String, String>;
}

pub fn create_provider(config: &Config) -> Box<dyn LlmProvider> {
    match config.provider {
        ProviderType::Anthropic => Box::new(anthropic::AnthropicProvider::new(config)),
        ProviderType::OpenAI => Box::new(openai::OpenAIProvider::new(config)),
    }
}
