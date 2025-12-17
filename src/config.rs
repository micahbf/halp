use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Provider {
    Anthropic,
    OpenAI,
    Gemini,
}

impl Default for Provider {
    fn default() -> Self {
        Provider::Anthropic
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub provider: Provider,
    pub model: String,
    pub api_key: String,
    pub api_base_url: Option<String>,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    api_base_url: Option<String>,
    system_prompt: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let file_config = Self::load_file_config();

        let provider = Self::resolve_provider(&file_config)?;
        let model = Self::resolve_model(&provider, &file_config);
        let api_key = Self::resolve_api_key(&provider, &file_config)?;
        let api_base_url = Self::resolve_api_base_url(&file_config);

        Ok(Config {
            provider,
            model,
            api_key,
            api_base_url,
            system_prompt: file_config.system_prompt,
        })
    }

    fn config_path() -> Option<PathBuf> {
        // Check XDG_CONFIG_HOME first, then fall back to ~/.config
        let config_dir = env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .ok()
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))?;

        Some(config_dir.join("halp").join("config.toml"))
    }

    fn load_file_config() -> FileConfig {
        Self::config_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or_default()
    }

    fn resolve_provider(file_config: &FileConfig) -> Result<Provider, String> {
        let provider_str = env::var("HALP_PROVIDER")
            .ok()
            .or_else(|| file_config.provider.clone())
            .unwrap_or_else(|| "anthropic".to_string());

        match provider_str.to_lowercase().as_str() {
            "anthropic" | "claude" => Ok(Provider::Anthropic),
            "openai" | "gpt" => Ok(Provider::OpenAI),
            "gemini" | "google" => Ok(Provider::Gemini),
            other => Err(format!(
                "Unknown provider '{}'. Use 'anthropic', 'openai', or 'gemini'.",
                other
            )),
        }
    }

    fn resolve_model(provider: &Provider, file_config: &FileConfig) -> String {
        env::var("HALP_MODEL")
            .ok()
            .or_else(|| file_config.model.clone())
            .unwrap_or_else(|| match provider {
                Provider::Anthropic => "claude-haiku-4-5".to_string(),
                Provider::OpenAI => "gpt-5-nano".to_string(),
                Provider::Gemini => "gemini-2.5-flash".to_string(),
            })
    }

    fn resolve_api_key(provider: &Provider, file_config: &FileConfig) -> Result<String, String> {
        // Priority: HALP_API_KEY > config file > provider-specific env var
        if let Ok(key) = env::var("HALP_API_KEY") {
            return Ok(key);
        }

        if let Some(key) = &file_config.api_key {
            return Ok(key.clone());
        }

        let provider_env = match provider {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::OpenAI => "OPENAI_API_KEY",
            Provider::Gemini => "GEMINI_API_KEY",
        };

        if let Ok(key) = env::var(provider_env) {
            return Ok(key);
        }

        Err(format!(
            "No API key found. Set HALP_API_KEY, add api_key to ~/.config/halp/config.toml, or set {}",
            provider_env
        ))
    }

    fn resolve_api_base_url(file_config: &FileConfig) -> Option<String> {
        env::var("HALP_API_BASE_URL")
            .ok()
            .or_else(|| file_config.api_base_url.clone())
    }
}
