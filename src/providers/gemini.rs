use crate::config::Config;
use crate::providers::streaming::{create_client, SseProcessor};
use crate::providers::LlmProvider;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::io::Write;

const DEFAULT_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

pub struct GeminiProvider {
    api_key: String,
    model: String,
    api_url: Option<String>,
}

#[derive(Serialize)]
struct GeminiRequest {
    system_instruction: SystemInstruction,
    contents: Vec<Content>,
}

#[derive(Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Deserialize, Debug)]
struct StreamChunk {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Deserialize, Debug)]
struct CandidateContent {
    parts: Option<Vec<ResponsePart>>,
}

#[derive(Deserialize, Debug)]
struct ResponsePart {
    text: Option<String>,
}

impl GeminiProvider {
    pub fn new(config: &Config) -> Self {
        Self {
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            api_url: config.api_base_url.clone(),
        }
    }

    fn build_url(&self) -> String {
        self.api_url.clone().unwrap_or_else(|| {
            format!(
                "{}/{}:streamGenerateContent?alt=sse",
                DEFAULT_API_BASE, self.model
            )
        })
    }
}

fn extract_text(data: &str) -> Result<Option<String>, String> {
    match serde_json::from_str::<StreamChunk>(data) {
        Ok(chunk) => {
            let text = chunk
                .candidates
                .and_then(|c| c.into_iter().next())
                .and_then(|c| c.content)
                .and_then(|c| c.parts)
                .and_then(|p| p.into_iter().next())
                .and_then(|p| p.text);
            Ok(text)
        }
        Err(e) => Err(format!("Failed to parse API response: {}", e)),
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn stream_completion(
        &self,
        prompt: &str,
        system: &str,
        output: &mut (dyn Write + Send),
    ) -> Result<String, String> {
        let client = create_client();

        let request = GeminiRequest {
            system_instruction: SystemInstruction {
                parts: vec![Part {
                    text: system.to_string(),
                }],
            },
            contents: vec![Content {
                parts: vec![Part {
                    text: prompt.to_string(),
                }],
            }],
        };

        let url = self.build_url();

        let response = client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
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
