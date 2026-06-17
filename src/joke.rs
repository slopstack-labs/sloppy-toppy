//! The unnecessary part: a background worker that periodically asks an LLM to
//! roast your machine, and ships the punchline back over a channel so the
//! render loop never blocks on inference.
//!
//! Provider is selected via `SLOPPY_PROVIDER` (ollama | anthropic | openai).

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::metrics::{fmt_bytes, Snapshot};

/// Seconds between unsolicited roasts.
const ROAST_INTERVAL: Duration = Duration::from_secs(15);

/// What the worker sends back to the UI.
pub enum JokeMsg {
    /// A fresh roast landed.
    Roast(String),
    /// Something went wrong; carries a (snarky) explanation.
    Error(String),
    /// A request is in flight.
    Thinking,
}

/// Handle the UI holds onto: the latest stats go in, jokes come out.
pub struct JokeChannel {
    pub rx: Receiver<JokeMsg>,
    latest: Arc<Mutex<Snapshot>>,
    nudge: Sender<()>,
}

impl JokeChannel {
    /// Push the most recent snapshot so the next roast has fresh material.
    pub fn update_stats(&self, snap: Snapshot) {
        if let Ok(mut guard) = self.latest.lock() {
            *guard = snap;
        }
    }

    /// Ask for a roast right now (e.g. user pressed `j`), bypassing the timer.
    pub fn roast_now(&self) {
        let _ = self.nudge.send(());
    }
}

// ── Provider selection ────────────────────────────────────────────────────────

enum Provider {
    Ollama,
    Anthropic,
    OpenAi,
}

fn provider() -> Provider {
    match std::env::var("SLOPPY_PROVIDER")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "anthropic" => Provider::Anthropic,
        "openai" => Provider::OpenAi,
        _ => Provider::Ollama,
    }
}

fn model() -> String {
    if let Ok(m) = std::env::var("SLOPPY_MODEL") {
        return m;
    }
    match provider() {
        Provider::Anthropic => "claude-haiku-4-5".to_string(),
        Provider::OpenAi => "gpt-4o-mini".to_string(),
        Provider::Ollama => "llama3.2".to_string(),
    }
}

// ── Prompt ────────────────────────────────────────────────────────────────────

fn build_prompt(snap: &Snapshot) -> String {
    let proc_list: String = snap
        .top_procs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                "  {}. {} — {:.0}% CPU, {}",
                i + 1,
                p.name,
                p.cpu,
                fmt_bytes(p.mem_bytes)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are a crude, mean-spirited AI roasting a computer and its loser user.\n\
OUTPUT RULES — break any of these and you fail:\n\
- Exactly ONE sentence. Hard limit: 20 words.\n\
- Sexually suggestive or outright dirty. Use process names as innuendo.\n\
- Cruel and specific. Name actual processes from the list.\n\
- No preamble. No \"Oh dear\". No metaphors. No ellipsis. Just the burn.\n\
\n\
STYLE EXAMPLES (do not repeat these, match the tone):\n\
- \"firefox is eating 80% of your RAM the same way you eat — desperately and with no results.\"\n\
- \"spotify and discord are both running just to remind you nobody wants to call.\"\n\
- \"your swap is fuller than your social calendar, which is saying something.\"\n\
\n\
MACHINE STATS:\n\
CPU: {cpu:.0}% · {cores} cores · load {load:.2}\n\
RAM: {mem_used} / {mem_total} ({mem_pct:.0}%)\n\
Swap: {swap_used} / {swap_total}\n\
Uptime: {up} min\n\
Processes:\n\
{procs}\n\
\n\
ONE sentence. 20 words max. Go.",
        cpu = snap.cpu_overall,
        cores = snap.per_core.len(),
        load = snap.load_one,
        mem_used = fmt_bytes(snap.mem_used),
        mem_total = fmt_bytes(snap.mem_total),
        mem_pct = snap.mem_frac() * 100.0,
        swap_used = fmt_bytes(snap.swap_used),
        swap_total = fmt_bytes(snap.swap_total),
        up = snap.uptime_secs / 60,
        procs = proc_list,
    )
}

// ── Provider-specific fetch functions ─────────────────────────────────────────

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

fn fetch_roast_ollama(snap: &Snapshot) -> Result<String, String> {
    let host = std::env::var("OLLAMA_HOST")
        .ok()
        .map(|h| {
            if h.starts_with("http") {
                h
            } else {
                format!("http://{h}")
            }
        })
        .unwrap_or_else(|| "http://localhost:11434".to_string());

    let url = format!("{host}/api/generate");
    let m = model();
    let body = serde_json::json!({
        "model": m,
        "prompt": build_prompt(snap),
        "stream": false,
        "options": { "temperature": 1.1 }
    });

    let response = ureq::post(&url)
        .timeout(Duration::from_secs(120))
        .send_json(body)
        .map_err(|err| match err {
            ureq::Error::Status(code, _) => {
                format!("Ollama returned {code} — is model '{m}' pulled?")
            }
            ureq::Error::Transport(_) => {
                "can't reach Ollama. Run `ollama serve` and pull a model first.".to_string()
            }
        })?;

    let parsed: OllamaResponse = response
        .into_json()
        .map_err(|e| format!("couldn't parse Ollama's reply: {e}"))?;

    clean(parsed.response)
}

// Anthropic response shapes ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

fn fetch_roast_anthropic(snap: &Snapshot) -> Result<String, String> {
    let key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY is not set.".to_string())?;

    let m = model();
    let body = serde_json::json!({
        "model": m,
        "max_tokens": 256,
        "messages": [{ "role": "user", "content": build_prompt(snap) }]
    });

    let response = ureq::post("https://api.anthropic.com/v1/messages")
        .timeout(Duration::from_secs(120))
        .set("x-api-key", &key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .send_json(body)
        .map_err(|err| match err {
            ureq::Error::Status(401, _) => {
                "Anthropic rejected the API key — check ANTHROPIC_API_KEY.".to_string()
            }
            ureq::Error::Status(code, _) => format!("Anthropic returned {code}."),
            ureq::Error::Transport(_) => "can't reach Anthropic's API.".to_string(),
        })?;

    let parsed: AnthropicResponse = response
        .into_json()
        .map_err(|e| format!("couldn't parse Anthropic's reply: {e}"))?;

    let text = parsed
        .content
        .into_iter()
        .next()
        .map(|c| c.text)
        .unwrap_or_default();

    clean(text)
}

// OpenAI-compatible response shapes ───────────────────────────────────────────

#[derive(Deserialize)]
struct OaiMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OaiChoice {
    message: OaiMessage,
}

#[derive(Deserialize)]
struct OaiResponse {
    choices: Vec<OaiChoice>,
}

fn fetch_roast_openai(snap: &Snapshot) -> Result<String, String> {
    let key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY is not set.".to_string())?;

    let base = std::env::var("OPENAI_BASE_URL")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let base = base.trim_end_matches('/').to_string();
    let url = format!("{base}/chat/completions");

    let m = model();
    let body = serde_json::json!({
        "model": m,
        "max_tokens": 256,
        "messages": [{ "role": "user", "content": build_prompt(snap) }]
    });

    let response = ureq::post(&url)
        .timeout(Duration::from_secs(120))
        .set("Authorization", &format!("Bearer {key}"))
        .set("content-type", "application/json")
        .send_json(body)
        .map_err(|err| match err {
            ureq::Error::Status(401, _) => {
                "OpenAI rejected the API key — check OPENAI_API_KEY.".to_string()
            }
            ureq::Error::Status(code, _) => format!("OpenAI-compatible API returned {code}."),
            ureq::Error::Transport(_) => format!("can't reach {base}."),
        })?;

    let parsed: OaiResponse = response
        .into_json()
        .map_err(|e| format!("couldn't parse OpenAI response: {e}"))?;

    let text = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .unwrap_or_default();

    clean(text)
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

fn fetch_roast(snap: &Snapshot) -> Result<String, String> {
    match provider() {
        Provider::Ollama => fetch_roast_ollama(snap),
        Provider::Anthropic => fetch_roast_anthropic(snap),
        Provider::OpenAi => fetch_roast_openai(snap),
    }
}

fn clean(text: String) -> Result<String, String> {
    let t = text.trim().trim_matches('"').to_string();
    if t.is_empty() {
        return Err("the model returned nothing. Even it gave up on you.".to_string());
    }
    // Cut at the first sentence boundary so rambling models stay punchy.
    let end = t
        .char_indices()
        .find(|&(_, c)| matches!(c, '.' | '!' | '?'))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(t.len());
    Ok(t[..end].to_string())
}

// ── Worker thread ─────────────────────────────────────────────────────────────

/// Spawn the worker thread and return the channel handle for the UI.
pub fn spawn() -> JokeChannel {
    let (tx, rx) = mpsc::channel::<JokeMsg>();
    let (nudge_tx, nudge_rx) = mpsc::channel::<()>();
    let latest = Arc::new(Mutex::new(Snapshot::default()));
    let worker_latest = Arc::clone(&latest);

    thread::spawn(move || {
        // A short warm-up so the first snapshot has real data before roast #1.
        thread::sleep(Duration::from_secs(2));
        let mut last_run = Instant::now() - ROAST_INTERVAL;

        loop {
            let waited = nudge_rx.recv_timeout(time_until_next(last_run));
            // `Disconnected` means the UI is gone; shut the thread down.
            if let Err(mpsc::RecvTimeoutError::Disconnected) = waited {
                break;
            }
            last_run = Instant::now();

            let snap = worker_latest
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default();

            if tx.send(JokeMsg::Thinking).is_err() {
                break;
            }

            let msg = match fetch_roast(&snap) {
                Ok(roast) => JokeMsg::Roast(roast),
                Err(why) => JokeMsg::Error(why),
            };
            if tx.send(msg).is_err() {
                break;
            }
        }
    });

    JokeChannel {
        rx,
        latest,
        nudge: nudge_tx,
    }
}

fn time_until_next(last_run: Instant) -> Duration {
    ROAST_INTERVAL.saturating_sub(last_run.elapsed())
}
