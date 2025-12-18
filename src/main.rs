mod config;
mod output;
mod prompt;
mod providers;

use clap::Parser;
use output::{parse_response, NullWriter, Spinner, StderrStreamer};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "halp")]
#[command(version)]
#[command(about = "Get shell commands from natural language", long_about = None)]
struct Cli {
    /// Natural language description of the command you need
    #[arg(required = true, num_args = 1..)]
    query: Vec<String>,

    /// Suppress explanation (command only)
    #[arg(short, long)]
    quiet: bool,

    /// Show explanation only (no command output)
    #[arg(short, long)]
    explain: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    // Load configuration
    let config = match config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Build the prompt
    let user_query = cli.query.join(" ");
    let system_prompt = prompt::build_system_prompt(config.system_prompt.as_deref());

    // Create the provider
    let provider = providers::create_provider(&config);

    // Stream the response
    let response = if cli.quiet {
        let mut writer = NullWriter;
        provider
            .stream_completion(&user_query, &system_prompt, &mut writer)
            .await
    } else {
        let spinner = Spinner::start();
        let mut writer = StderrStreamer::new(Some(spinner));
        let result = provider
            .stream_completion(&user_query, &system_prompt, &mut writer)
            .await;
        writer.finish();
        result
    };

    let response = match response {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Parse and output the response
    let parsed = parse_response(&response);

    if cli.explain {
        // Explanation-only mode: output explanation to stdout
        if let Some(explanation) = parsed.explanation {
            println!("{}", explanation);
        }
    } else {
        // Normal mode: output command to stdout
        if let Some(command) = parsed.command {
            println!("{}", command);
        } else {
            eprintln!("Could not extract command from response");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}
