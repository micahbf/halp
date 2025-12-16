use crate::config::Config;
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

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn stream_completion(
        &self,
        prompt: &str,
        system: &str,
        output: &mut (dyn Write + Send),
    ) -> Result<String, String> {
        let client = reqwest::Client::new();

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

        let mut full_response = String::new();
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE events
            while let Some(event_end) = buffer.find("\n\n") {
                let event_data = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                for line in event_data.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            continue;
                        }

                        if let Ok(event) = serde_json::from_str::<StreamEvent>(data) {
                            match event {
                                StreamEvent::ContentBlockDelta { delta, .. } => {
                                    if let Delta::TextDelta { text } = delta {
                                        full_response.push_str(&text);
                                        let _ = write!(output, "{}", text);
                                        let _ = output.flush();
                                    }
                                }
                                StreamEvent::Error { error } => {
                                    return Err(format!("API error: {}", error.message));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(full_response)
    }
}
