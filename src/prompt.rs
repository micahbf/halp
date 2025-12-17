use std::env;

pub fn build_system_prompt(custom_template: Option<&str>) -> String {
    let os = get_os();
    let shell = get_shell();
    let cwd = get_cwd();

    match custom_template {
        Some(template) => template
            .replace("{{os}}", &os)
            .replace("{{shell}}", &shell)
            .replace("{{cwd}}", &cwd),
        None => format!(
            r#"You are a command-line assistant. Generate a shell command for the user's request.

Format your response EXACTLY as:
COMMAND: <the exact command to run>
EXPLANATION: <brief one-line explanation>

Context:
- OS: {}
- Shell: {}
- Working directory: {}

Rules:
- Output exactly one command (use && or ; for multi-step operations)
- The command must be valid for the specified OS and shell
- Prefer common, portable commands when possible
- Keep explanation to one concise line
- Never include dangerous commands (rm -rf /, etc) without explicit confirmation flags
- If the request is ambiguous, make a reasonable assumption and note it in the explanation"#,
            os, shell, cwd
        ),
    }
}

fn get_os() -> String {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    format!("{} ({})", os, arch)
}

fn get_shell() -> String {
    env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(String::from))
        .unwrap_or_else(|| "unknown".to_string())
}

fn get_cwd() -> String {
    env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
