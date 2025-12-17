use std::io::Write;
use std::time::Duration;

/// Default request timeout in seconds
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum response size (1MB)
pub const MAX_RESPONSE_SIZE: usize = 1_048_576;

/// Handles SSE stream processing with buffer management and size limits
pub struct SseProcessor {
    buffer: String,
    full_response: String,
    max_size: usize,
}

impl SseProcessor {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            full_response: String::new(),
            max_size: MAX_RESPONSE_SIZE,
        }
    }

    /// Returns the accumulated full response
    pub fn into_response(self) -> String {
        self.full_response
    }

    /// Append a chunk to the buffer
    pub fn push_chunk(&mut self, chunk: &[u8]) {
        self.buffer.push_str(&String::from_utf8_lossy(chunk));
    }

    /// Process complete SSE events from the buffer.
    /// Calls the provided closure for each "data: " line (excluding [DONE]).
    /// Returns Err if the closure returns an error or if response size exceeds limit.
    pub fn process_events<F>(&mut self, mut handler: F) -> Result<(), String>
    where
        F: FnMut(&str) -> Result<Option<String>, String>,
    {
        while let Some(event_end) = self.buffer.find("\n\n") {
            let event_data: String = self.buffer.drain(..event_end + 2).collect();

            for line in event_data.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }

                    if let Some(text) = handler(data)? {
                        self.full_response.push_str(&text);

                        // Check size limit
                        if self.full_response.len() > self.max_size {
                            return Err(format!(
                                "Response too large (>{} bytes)",
                                self.max_size
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Convenience method to process events and write text to output
    pub fn process_events_with_output<F>(
        &mut self,
        output: &mut (dyn Write + Send),
        handler: F,
    ) -> Result<(), String>
    where
        F: FnMut(&str) -> Result<Option<String>, String>,
    {
        let response_before = self.full_response.len();
        self.process_events(handler)?;

        // Write any new text to output
        if self.full_response.len() > response_before {
            let new_text = &self.full_response[response_before..];
            let _ = write!(output, "{}", new_text);
            let _ = output.flush();
        }
        Ok(())
    }
}

impl Default for SseProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a reqwest client with default timeout
pub fn create_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_single_event() {
        let mut processor = SseProcessor::new();
        processor.push_chunk(b"data: {\"text\": \"hello\"}\n\n");

        let mut received = Vec::new();
        processor
            .process_events(|data| {
                received.push(data.to_string());
                Ok(Some("hello".to_string()))
            })
            .unwrap();

        assert_eq!(received, vec!["{\"text\": \"hello\"}"]);
        assert_eq!(processor.into_response(), "hello");
    }

    #[test]
    fn test_process_multiple_events() {
        let mut processor = SseProcessor::new();
        processor.push_chunk(b"data: first\n\ndata: second\n\n");

        let mut count = 0;
        processor
            .process_events(|_| {
                count += 1;
                Ok(Some("x".to_string()))
            })
            .unwrap();

        assert_eq!(count, 2);
        assert_eq!(processor.into_response(), "xx");
    }

    #[test]
    fn test_ignores_done() {
        let mut processor = SseProcessor::new();
        processor.push_chunk(b"data: hello\n\ndata: [DONE]\n\n");

        let mut count = 0;
        processor
            .process_events(|_| {
                count += 1;
                Ok(Some("x".to_string()))
            })
            .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn test_incomplete_event_buffered() {
        let mut processor = SseProcessor::new();
        processor.push_chunk(b"data: partial");

        let mut count = 0;
        processor
            .process_events(|_| {
                count += 1;
                Ok(None)
            })
            .unwrap();

        assert_eq!(count, 0); // Not processed yet

        processor.push_chunk(b"\n\n");
        processor
            .process_events(|_| {
                count += 1;
                Ok(None)
            })
            .unwrap();

        assert_eq!(count, 1); // Now processed
    }

    #[test]
    fn test_size_limit_exceeded() {
        let mut processor = SseProcessor::new();
        processor.max_size = 10; // Small limit for testing

        processor.push_chunk(b"data: test\n\n");
        let result = processor.process_events(|_| Ok(Some("this is too long".to_string())));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too large"));
    }

    #[test]
    fn test_handler_error_propagates() {
        let mut processor = SseProcessor::new();
        processor.push_chunk(b"data: test\n\n");

        let result = processor.process_events(|_| Err("parse error".to_string()));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "parse error");
    }
}
