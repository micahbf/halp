use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ANSI escape codes for styling
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

const SPINNER_FRAMES: &[char] = &['⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷'];

pub struct Spinner {
    stop_flag: Arc<AtomicBool>,
}

impl Spinner {
    pub fn start() -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = stop_flag.clone();

        tokio::spawn(async move {
            let mut idx = 0;
            let mut interval = tokio::time::interval(Duration::from_millis(80));

            while !flag_clone.load(Ordering::Relaxed) {
                eprint!("\r{}", SPINNER_FRAMES[idx]);
                let _ = io::stderr().flush();
                idx = (idx + 1) % SPINNER_FRAMES.len();
                interval.tick().await;
            }
        });

        Self { stop_flag }
    }

    pub fn stop(self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

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
    spinner: Option<Spinner>,
}

impl StderrStreamer {
    pub fn new(spinner: Option<Spinner>) -> Self {
        Self {
            started: false,
            spinner,
        }
    }

    pub fn finish(&mut self) {
        // Stop spinner if it's still running (no data was written)
        if let Some(spinner) = self.spinner.take() {
            spinner.stop();
            eprint!("\r \r");
            let _ = io::stderr().flush();
        }
        if self.started {
            eprintln!("{}", RESET);
        }
    }
}

impl Write for StderrStreamer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.started {
            self.started = true;
            if let Some(spinner) = self.spinner.take() {
                spinner.stop();
                eprint!("\r \r");
                let _ = io::stderr().flush();
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_format() {
        let response = "COMMAND: ls -la\nEXPLANATION: Lists all files including hidden ones";
        let parsed = parse_response(response);

        assert_eq!(parsed.command, Some("ls -la".to_string()));
        assert_eq!(
            parsed.explanation,
            Some("Lists all files including hidden ones".to_string())
        );
    }

    #[test]
    fn test_parse_command_only() {
        let response = "COMMAND: pwd";
        let parsed = parse_response(response);

        assert_eq!(parsed.command, Some("pwd".to_string()));
        assert_eq!(parsed.explanation, None);
    }

    #[test]
    fn test_parse_explanation_only() {
        let response = "EXPLANATION: This explains something";
        let parsed = parse_response(response);

        // Falls back to first non-empty line
        assert_eq!(
            parsed.command,
            Some("EXPLANATION: This explains something".to_string())
        );
        assert_eq!(
            parsed.explanation,
            Some("This explains something".to_string())
        );
    }

    #[test]
    fn test_parse_code_block_fallback() {
        let response = "Here's the command:\n```bash\ngrep -r \"pattern\" .\n```";
        let parsed = parse_response(response);

        assert_eq!(parsed.command, Some("grep -r \"pattern\" .".to_string()));
        assert_eq!(parsed.explanation, None);
    }

    #[test]
    fn test_parse_code_block_no_language() {
        let response = "```\necho hello\n```";
        let parsed = parse_response(response);

        assert_eq!(parsed.command, Some("echo hello".to_string()));
    }

    #[test]
    fn test_parse_first_line_fallback() {
        let response = "git status\nThis shows the status";
        let parsed = parse_response(response);

        assert_eq!(parsed.command, Some("git status".to_string()));
    }

    #[test]
    fn test_parse_empty_response() {
        let response = "";
        let parsed = parse_response(response);

        assert_eq!(parsed.command, None);
        assert_eq!(parsed.explanation, None);
    }

    #[test]
    fn test_parse_whitespace_only() {
        let response = "   \n\n   ";
        let parsed = parse_response(response);

        assert_eq!(parsed.command, None);
        assert_eq!(parsed.explanation, None);
    }

    #[test]
    fn test_parse_multiline_command() {
        let response = "COMMAND: docker run -it \\\n  --name test \\\n  ubuntu\nEXPLANATION: Runs ubuntu";
        let parsed = parse_response(response);

        // Command ends at first newline per current implementation
        assert_eq!(parsed.command, Some("docker run -it \\".to_string()));
        assert_eq!(parsed.explanation, Some("Runs ubuntu".to_string()));
    }

    #[test]
    fn test_parse_command_before_explanation() {
        let response = "Some text\nCOMMAND: ls\nMore text\nEXPLANATION: Lists files";
        let parsed = parse_response(response);

        assert_eq!(parsed.command, Some("ls".to_string()));
        assert_eq!(parsed.explanation, Some("Lists files".to_string()));
    }

    #[test]
    fn test_parse_trims_whitespace() {
        let response = "COMMAND:    echo hello   \nEXPLANATION:   Says hello   ";
        let parsed = parse_response(response);

        assert_eq!(parsed.command, Some("echo hello".to_string()));
        assert_eq!(parsed.explanation, Some("Says hello".to_string()));
    }

    #[test]
    fn test_null_writer() {
        let mut writer = NullWriter;
        let result = write!(writer, "test output");
        assert!(result.is_ok());
        assert!(writer.flush().is_ok());
    }
}
