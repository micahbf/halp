use crate::config::Config;
use crate::providers::streaming::{create_client, SseProcessor};
use crate::providers::LlmProvider;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::Write;

const DEFAULT_API_URL: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAIProvider {
    api_key: String,
    model: String,
    api_url: String,
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct StreamChunk {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    delta: DeltaContent,
}

#[derive(Deserialize, Debug, Default)]
struct DeltaContent {
    content: Option<String>,
}

impl OpenAIProvider {
    pub fn new(config: &Config) -> Self {
        Self {
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            api_url: config
                .api_base_url
                .clone()
                .unwrap_or_else(|| DEFAULT_API_URL.to_string()),
        }
    }
}

fn extract_text(data: &str) -> Result<Option<String>, String> {
    match serde_json::from_str::<StreamChunk>(data) {
        Ok(chunk) => Ok(chunk
            .choices
            .first()
            .and_then(|c| c.delta.content.clone())),
        Err(e) => Err(format!("Failed to parse API response: {}", e)),
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    async fn stream_completion(
        &self,
        prompt: &str,
        system: &str,
        output: &mut (dyn Write + Send),
    ) -> Result<String, String> {
        let client = create_client();

        let request = OpenAIRequest {
            model: self.model.clone(),
            max_tokens: 1024,
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            stream: true,
        };

        let response = client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("API error ({}): {}", status, body));
        }

        let mut processor = SseProcessor::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
            processor.push_chunk(&chunk);
            processor.process_events_with_output(output, extract_text)?;
        }

        Ok(processor.into_response())
    }
}
