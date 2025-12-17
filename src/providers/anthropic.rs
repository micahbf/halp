use crate::config::Config;
use crate::providers::streaming::{create_client, SseProcessor};
use crate::providers::LlmProvider;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::Write;

const DEFAULT_API_URL: &str = "https://api.anthropic.com/v1/messages";

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    api_url: String,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: serde_json::Value },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: Delta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: serde_json::Value },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: ErrorInfo },
}

#[derive(Deserialize, Debug)]
struct ContentBlock {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    block_type: String,
    #[allow(dead_code)]
    text: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum Delta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    #[allow(dead_code)]
    InputJsonDelta { partial_json: String },
}

#[derive(Deserialize, Debug)]
struct ErrorInfo {
    message: String,
}

impl AnthropicProvider {
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
    match serde_json::from_str::<StreamEvent>(data) {
        Ok(event) => match event {
            StreamEvent::ContentBlockDelta { delta, .. } => {
                if let Delta::TextDelta { text } = delta {
                    Ok(Some(text))
                } else {
                    Ok(None)
                }
            }
            StreamEvent::Error { error } => Err(format!("API error: {}", error.message)),
            _ => Ok(None),
        },
        Err(e) => Err(format!("Failed to parse API response: {}", e)),
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn stream_completion(
        &self,
        prompt: &str,
        system: &str,
        output: &mut (dyn Write + Send),
    ) -> Result<String, String> {
        let client = create_client();

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 1024,
            system: system.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            stream: true,
        };

        let response = client
            .post(&self.api_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
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
