use std::io::{self, Write};

// ANSI escape codes for styling
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub struct ParsedResponse {
    pub command: Option<String>,
    pub explanation: Option<String>,
}

pub fn parse_response(response: &str) -> ParsedResponse {
    let mut command = None;
    let mut explanation = None;

    // Look for COMMAND: prefix
    if let Some(cmd_start) = response.find("COMMAND:") {
        let after_prefix = &response[cmd_start + 8..];
        // Command ends at newline or EXPLANATION:
        let cmd_end = after_prefix
            .find('\n')
            .or_else(|| after_prefix.find("EXPLANATION:"))
            .unwrap_or(after_prefix.len());
        let cmd = after_prefix[..cmd_end].trim();
        if !cmd.is_empty() {
            command = Some(cmd.to_string());
        }
    }

    // Look for EXPLANATION: prefix
    if let Some(exp_start) = response.find("EXPLANATION:") {
        let after_prefix = &response[exp_start + 12..];
        let exp = after_prefix.trim();
        if !exp.is_empty() {
            explanation = Some(exp.to_string());
        }
    }

    // Fallback: if no COMMAND: found, try to extract a code block or the first line
    if command.is_none() {
        // Try to find a code block
        if let Some(code_start) = response.find("```") {
            let after_backticks = &response[code_start + 3..];
            // Skip language identifier if present
            let content_start = after_backticks.find('\n').map(|i| i + 1).unwrap_or(0);
            let content = &after_backticks[content_start..];
            if let Some(code_end) = content.find("```") {
                let cmd = content[..code_end].trim();
                if !cmd.is_empty() {
                    command = Some(cmd.to_string());
                }
            }
        }
    }

    // Last resort: use the first non-empty line as command
    if command.is_none() {
        if let Some(first_line) = response.lines().find(|l| !l.trim().is_empty()) {
            command = Some(first_line.trim().to_string());
        }
    }

    ParsedResponse {
        command,
        explanation,
    }
}

/// A writer that streams to stderr with dim styling
pub struct StderrStreamer {
    started: bool,
}

impl StderrStreamer {
    pub fn new() -> Self {
        Self { started: false }
    }

    pub fn finish(&mut self) {
        if self.started {
            eprint!("{}\n", RESET);
        }
    }
}

impl Write for StderrStreamer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.started {
            self.started = true;
            eprint!("{}", DIM);
        }
        io::stderr().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stderr().flush()
    }
}

/// A writer that discards all output (for quiet mode)
pub struct NullWriter;

impl Write for NullWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
