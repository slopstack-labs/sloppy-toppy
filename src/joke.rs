//! Background LLM worker — asks the configured provider to roast your machine.
//! Provider/model/keys come from Config, not env vars directly.

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::config::Config;
use crate::metrics::{fmt_bytes, Snapshot};

pub enum JokeMsg {
    Roast(String),
    Error(String),
    Thinking,
}

pub struct JokeChannel {
    pub rx: Receiver<JokeMsg>,
    latest: Arc<Mutex<Snapshot>>,
    nudge: Sender<()>,
}

impl JokeChannel {
    pub fn update_stats(&self, snap: Snapshot) {
        if let Ok(mut guard) = self.latest.lock() {
            *guard = snap;
        }
    }
    pub fn roast_now(&self) {
        let _ = self.nudge.send(());
    }
}

// ── Provider config ───────────────────────────────────────────────────────────

pub struct JokeConfig {
    provider: String,
    model: String,
    ollama_host: String,
    anthropic_api_key: String,
    openai_api_key: String,
    openai_base_url: String,
    pub roast_interval: Duration,
}

impl JokeConfig {
    pub fn from_config(cfg: &Config) -> Self {
        let model = if cfg.model.is_empty() {
            match cfg.provider.as_str() {
                "anthropic" => "claude-haiku-4-5",
                "openai" => "gpt-4o-mini",
                _ => "llama3.2",
            }
            .to_string()
        } else {
            cfg.model.clone()
        };
        JokeConfig {
            provider: cfg.provider.clone(),
            model,
            ollama_host: cfg.ollama_host.clone(),
            anthropic_api_key: cfg.anthropic_api_key.clone(),
            openai_api_key: cfg.openai_api_key.clone(),
            openai_base_url: cfg.openai_base_url.clone(),
            roast_interval: cfg.roast_interval,
        }
    }
}

// ── Prompt ────────────────────────────────────────────────────────────────────

fn build_prompt(snap: &Snapshot) -> String {
    // Keep the process list short so small local models don't choke.
    let proc_list: String = snap
        .top_procs
        .iter()
        .take(8)
        .map(|p| format!("{} {:.0}%", p.name, p.cpu))
        .collect::<Vec<_>>()
        .join(", ");

    let temp_str = snap
        .cpu_temp
        .map(|t| format!(" {t:.0}°C"))
        .unwrap_or_default();

    format!(
        "Roast this machine and its user in ONE filthy sentence (max 15 words). \
Be crude, mean, sexually suggestive. Name a real process. No preamble.\n\
CPU:{cpu:.0}%({cores}c) RAM:{mem_pct:.0}% swap:{swap_pct:.0}% \
net↓{net_rx}/s net↑{net_tx}/s disk:{disk_pct:.0}%{temp}\n\
procs: {procs}\nONE sentence:",
        cpu = snap.cpu_overall,
        cores = snap.per_core.len(),
        mem_pct = snap.mem_frac() * 100.0,
        swap_pct = snap.swap_frac() * 100.0,
        net_rx = fmt_bytes(snap.net_rx_bps),
        net_tx = fmt_bytes(snap.net_tx_bps),
        disk_pct = snap.disk_frac() * 100.0,
        temp = temp_str,
        procs = proc_list,
    )
}

// ── Fetch functions ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

fn fetch_roast_ollama(snap: &Snapshot, cfg: &JokeConfig) -> Result<String, String> {
    let host = if cfg.ollama_host.starts_with("http") {
        cfg.ollama_host.clone()
    } else {
        format!("http://{}", cfg.ollama_host)
    };
    let url = format!("{host}/api/generate");
    let body = serde_json::json!({
        "model": cfg.model,
        "prompt": build_prompt(snap),
        "stream": false,
        "options": { "temperature": 1.1 }
    });
    let response = ureq::post(&url)
        .timeout(Duration::from_secs(120))
        .send_json(body)
        .map_err(|err| match err {
            ureq::Error::Status(code, _) => format!("Ollama returned {code} — is model '{}' pulled?", cfg.model),
            ureq::Error::Transport(_) => "can't reach Ollama. Run `ollama serve` and pull a model first.".to_string(),
        })?;
    let parsed: OllamaResponse = response
        .into_json()
        .map_err(|e| format!("couldn't parse Ollama's reply: {e}"))?;
    clean(parsed.response)
}

#[derive(Deserialize)]
struct AnthropicContent { text: String }
#[derive(Deserialize)]
struct AnthropicResponse { content: Vec<AnthropicContent> }

fn fetch_roast_anthropic(snap: &Snapshot, cfg: &JokeConfig) -> Result<String, String> {
    if cfg.anthropic_api_key.is_empty() {
        return Err("ANTHROPIC_API_KEY is not set.".to_string());
    }
    let body = serde_json::json!({
        "model": cfg.model,
        "max_tokens": 256,
        "messages": [{ "role": "user", "content": build_prompt(snap) }]
    });
    let response = ureq::post("https://api.anthropic.com/v1/messages")
        .timeout(Duration::from_secs(120))
        .set("x-api-key", &cfg.anthropic_api_key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .send_json(body)
        .map_err(|err| match err {
            ureq::Error::Status(401, _) => "Anthropic rejected the API key.".to_string(),
            ureq::Error::Status(code, _) => format!("Anthropic returned {code}."),
            ureq::Error::Transport(_) => "can't reach Anthropic's API.".to_string(),
        })?;
    let parsed: AnthropicResponse = response
        .into_json()
        .map_err(|e| format!("couldn't parse Anthropic's reply: {e}"))?;
    clean(parsed.content.into_iter().next().map(|c| c.text).unwrap_or_default())
}

#[derive(Deserialize)]
struct OaiMessage { content: Option<String> }
#[derive(Deserialize)]
struct OaiChoice { message: OaiMessage }
#[derive(Deserialize)]
struct OaiResponse { choices: Vec<OaiChoice> }

fn fetch_roast_openai(snap: &Snapshot, cfg: &JokeConfig) -> Result<String, String> {
    if cfg.openai_api_key.is_empty() {
        return Err("OPENAI_API_KEY is not set.".to_string());
    }
    let base = cfg.openai_base_url.trim_end_matches('/');
    let url = format!("{base}/chat/completions");
    let body = serde_json::json!({
        "model": cfg.model,
        "max_tokens": 256,
        "messages": [{ "role": "user", "content": build_prompt(snap) }]
    });
    let response = ureq::post(&url)
        .timeout(Duration::from_secs(120))
        .set("Authorization", &format!("Bearer {}", cfg.openai_api_key))
        .set("content-type", "application/json")
        .send_json(body)
        .map_err(|err| match err {
            ureq::Error::Status(401, _) => "OpenAI rejected the API key.".to_string(),
            ureq::Error::Status(code, _) => format!("OpenAI-compatible API returned {code}."),
            ureq::Error::Transport(_) => format!("can't reach {base}."),
        })?;
    let parsed: OaiResponse = response
        .into_json()
        .map_err(|e| format!("couldn't parse OpenAI response: {e}"))?;
    clean(parsed.choices.into_iter().next().and_then(|c| c.message.content).unwrap_or_default())
}

fn fetch_roast(snap: &Snapshot, cfg: &JokeConfig) -> Result<String, String> {
    match cfg.provider.as_str() {
        "anthropic" => fetch_roast_anthropic(snap, cfg),
        "openai" => fetch_roast_openai(snap, cfg),
        _ => fetch_roast_ollama(snap, cfg),
    }
}

fn clean(text: String) -> Result<String, String> {
    let t = text.trim().trim_matches('"').to_string();
    if t.is_empty() {
        return Err("the model returned nothing. Even it gave up on you.".to_string());
    }
    let end = t
        .char_indices()
        .find(|&(_, c)| matches!(c, '.' | '!' | '?'))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(t.len());
    Ok(t[..end].to_string())
}

// ── Worker thread ─────────────────────────────────────────────────────────────

pub fn spawn(cfg: JokeConfig) -> JokeChannel {
    let (tx, rx) = mpsc::channel::<JokeMsg>();
    let (nudge_tx, nudge_rx) = mpsc::channel::<()>();
    let latest = Arc::new(Mutex::new(Snapshot::default()));
    let worker_latest = Arc::clone(&latest);
    let roast_interval = cfg.roast_interval;

    thread::spawn(move || {
        thread::sleep(Duration::from_secs(2));
        let mut last_run = Instant::now() - roast_interval;

        loop {
            let waited = nudge_rx.recv_timeout(time_until_next(last_run, roast_interval));
            if let Err(mpsc::RecvTimeoutError::Disconnected) = waited {
                break;
            }
            last_run = Instant::now();

            let snap = worker_latest
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default();

            if tx.send(JokeMsg::Thinking).is_err() { break; }

            let msg = match fetch_roast(&snap, &cfg) {
                Ok(roast) => JokeMsg::Roast(roast),
                Err(why) => JokeMsg::Error(why),
            };
            if tx.send(msg).is_err() { break; }
        }
    });

    JokeChannel { rx, latest, nudge: nudge_tx }
}

fn time_until_next(last_run: Instant, interval: Duration) -> Duration {
    interval.saturating_sub(last_run.elapsed())
}
