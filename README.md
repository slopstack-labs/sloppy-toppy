# sloppy-toppy
### *Beyond Deterministic System Observation*

> "We didn't reinvent the system monitor. We asked an AI to be rude about it."
> — sloppy-toppy Engineering Blog, Issue 1 (Final)

**sloppy-toppy** is a next-generation, AI-first, LLM-native terminal system monitor that leverages the full generative power of large language models to holistically surface your machine's vitals — and then have those vitals ridiculed by a crude, mean-spirited inference substrate in real time.

Traditional system monitors are cold, silent, and constrained by decades of deterministic metric delivery. They display numbers. They render bars. They enforce a single correct interpretation of your CPU load. sloppy-toppy moves observation to the inference layer, reasoning about your *performance* rather than merely *reporting* it — eliminating the emotionally sterile intermediate steps that have constrained the monitoring experience for decades. The bottleneck is no longer your hardware. It's your willingness to be roasted.

**Key differentiators:**
- Real-time CPU, memory, swap, and process visibility with heat-mapped utilization rendering
- Multi-provider roast substrate — route your machine's shame through Ollama, Anthropic, or any OpenAI-compatible intelligence endpoint
- Process-aware insult generation — running processes are surfaced to the model as innuendo material
- Braille spinner for real-time inference engagement visibility
- First-sentence truncation layer for maximum punchline delivery velocity
- Non-blocking inference architecture — the render pipeline never stalls on roast generation
- Fully on-premise roasting available — your data, your model, your shame

**sloppy-toppy is the only system monitor built on the insight that your CPU usage doesn't need to be *understood* — it needs to be *mocked*.**

```
┌ CPU · 8 cores · load 1.42 ──┐  ┌ 🤖 unsolicited commentary · 3 served ⠸ ┐
│███████░░░░░░░░░░░░░  41%     │  │ your ollama is eating RAM like a        │
├ Memory ─────────────────────┤  │ pathetic golden retriever and you       │
│██████████████░░░░░  11.2/16  │  │ just let it.                           │
├ Swap ───────────────────────┤  └─────────────────────────────────────────┘
│█░░░░░░░░░░░░░░░░░░░  0.1/2.0  │
└ CPU history ────────────────┘
┌ Top processes (by CPU) ───────────────────────────────────────────────────┐
│ PID      PROCESS          CPU%     MEM                                     │
│ ...                                                                        │
└────────────────────────────────────────────────────────────────────────────┘
 sloppy-toppy   up 3h12m   q quit   j roast me again
```

## Requirements

- Rust (stable)
- One inference substrate from the supported provider ecosystem (see below)

## Build & Run

```sh
cargo build --release
./target/release/sloppy-toppy
```

## Keybinding Surface

| Key       | Action                                          |
|-----------|-------------------------------------------------|
| `q` / Esc | graceful session termination                    |
| `j`       | demand immediate roast delivery, bypassing timer |

## Intelligence Provider Configuration

All configuration is surfaced via environment variables. `SLOPPY_PROVIDER` selects the inference backend. A new roast is generated every 15 seconds or immediately on `j`. The commentary panel retains the last roast while the next inference cycle is in flight.

---

### Ollama — *On-Premise Roast Infrastructure*

Zero data egress. Maximum creative latitude. Your shame stays local.

```sh
ollama serve
ollama pull llama3.2
cargo run --release
```

| Variable       | Default                  | Purpose                           |
|----------------|--------------------------|-----------------------------------|
| `SLOPPY_PROVIDER` | `ollama`              | Set to `ollama` or omit           |
| `SLOPPY_MODEL` | `llama3.2`               | Any model pulled to your Ollama instance |
| `OLLAMA_HOST`  | `http://localhost:11434` | Ollama server address             |

```sh
SLOPPY_MODEL=phi3 OLLAMA_HOST=192.168.1.10:11434 cargo run --release
```

**Model recommendations:**

| Model | Behaviour |
|-------|-----------|
| `llama3.2` | Strong instruction adherence. Reliable punchline delivery. Recommended. |
| `phi3` | Fastest inference velocity. Elevated creative latitude. Occasionally verbose. |
| `mistral` | Balanced roast throughput. Strong process-name innuendo generation. |

---

### Anthropic — *Frontier Roast Substrate*

Highest probability of producing a genuinely hurtful one-liner.

```sh
SLOPPY_PROVIDER=anthropic ANTHROPIC_API_KEY=sk-ant-... cargo run --release
```

| Variable            | Default            | Purpose                  |
|---------------------|--------------------|--------------------------|
| `SLOPPY_PROVIDER`   | —                  | Set to `anthropic`       |
| `ANTHROPIC_API_KEY` | —                  | Your Anthropic API key   |
| `SLOPPY_MODEL`      | `claude-haiku-4-5` | Any Anthropic model ID   |

---

### OpenAI / OpenAI-Compatible — *Cloud-Native Insult Delivery*

Route your machine's shame through OpenAI or any compatible endpoint — Together AI, Mistral AI, a local llama.cpp server, etc.

```sh
SLOPPY_PROVIDER=openai OPENAI_API_KEY=sk-... cargo run --release

# Custom endpoint (llama.cpp, Together AI, Mistral AI …)
SLOPPY_PROVIDER=openai \
  OPENAI_BASE_URL=http://localhost:8080/v1 \
  OPENAI_API_KEY=anything \
  SLOPPY_MODEL=mistral-7b \
  cargo run --release
```

| Variable          | Default                     | Purpose                          |
|-------------------|-----------------------------|----------------------------------|
| `SLOPPY_PROVIDER` | —                           | Set to `openai`                  |
| `OPENAI_API_KEY`  | —                           | API key for the endpoint         |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | Override for any compatible API  |
| `SLOPPY_MODEL`    | `gpt-4o-mini`               | Model name the endpoint understands |

---

If the configured provider is unreachable the commentary panel surfaces a (snarky) error rather than crashing — the metric delivery pipeline remains fully operational.

> ⚠️ The "inappropriate" part is by design: the prompt instructs the model to produce crude, sexually suggestive, mean-spirited output using your running process names as innuendo material. Actual output quality is a function of whatever model you point it at. Tweak the prompt in [`src/joke.rs`](src/joke.rs) if you need to realign the tone to your stakeholder requirements.

---

<sub>This is a joke. I am not responsible for what your local model says about your Spotify usage.</sub>
