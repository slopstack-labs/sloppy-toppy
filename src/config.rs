//! Loads ~/.config/sloppy-toppy/config.toml and merges with env vars.
//! Env vars always win.

use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawConfig {
    provider: Option<String>,
    model: Option<String>,
    ollama_host: Option<String>,
    anthropic_api_key: Option<String>,
    openai_api_key: Option<String>,
    openai_base_url: Option<String>,
    roast_interval_secs: Option<u64>,
    alert_cpu: Option<f32>,
    alert_mem: Option<f32>,
}

pub struct Config {
    pub provider: String,
    pub model: String,
    pub ollama_host: String,
    pub anthropic_api_key: String,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub roast_interval: Duration,
    pub alert_cpu: f32,
    pub alert_mem: f32,
}

impl Config {
    pub fn load() -> Self {
        let raw: RawConfig = config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();

        let env = |key: &str| std::env::var(key).ok();

        let provider = env("SLOPPY_PROVIDER")
            .or(raw.provider)
            .unwrap_or_else(|| "ollama".to_string())
            .to_lowercase();

        Config {
            model: env("SLOPPY_MODEL").or(raw.model).unwrap_or_default(),
            ollama_host: env("OLLAMA_HOST")
                .or(raw.ollama_host)
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
            anthropic_api_key: env("ANTHROPIC_API_KEY")
                .or(raw.anthropic_api_key)
                .unwrap_or_default(),
            openai_api_key: env("OPENAI_API_KEY")
                .or(raw.openai_api_key)
                .unwrap_or_default(),
            openai_base_url: env("OPENAI_BASE_URL")
                .or(raw.openai_base_url)
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            roast_interval: Duration::from_secs(raw.roast_interval_secs.unwrap_or(15)),
            alert_cpu: raw.alert_cpu.unwrap_or(90.0),
            alert_mem: raw.alert_mem.unwrap_or(90.0),
            provider,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("sloppy-toppy")
            .join("config.toml"),
    )
}
