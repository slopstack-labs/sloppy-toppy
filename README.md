# sloppy-toppy

A terminal system monitor in the spirit of `htop`/`btop` — CPU, memory, swap,
and top processes — that **unnecessarily** pipes your machine's vitals through an
LLM so it can periodically roast you.

Built with [ratatui](https://ratatui.rs), [sysinfo](https://docs.rs/sysinfo),
and your choice of Ollama, Anthropic, or any OpenAI-compatible API for the bad attitude.

```
┌ CPU · 8 cores · load 1.42 ──┐  ┌ 🤖 unsolicited commentary · 3 served ┐
│███████░░░░░░░░░░░░░  41%     │  │ 41% CPU and you still can't close    │
├ Memory ─────────────────────┤  │ your 90 browser tabs — impressive    │
│██████████████░░░░░  11.2/16  │  │ commitment to mediocrity.            │
├ Swap ───────────────────────┤  │                                      │
│█░░░░░░░░░░░░░░░░░░░  0.1/2.0  │  └──────────────────────────────────────┘
└ CPU history ────────────────┘
┌ Top processes (by CPU) ──────────────────────────────────────────────┐
│ PID      PROCESS          CPU%     MEM                                 │
│ ...                                                                    │
└───────────────────────────────────────────────────────────────────────┘
 sloppy-toppy   up 3h12m   q quit   j roast me again
```

## Requirements

- Rust (stable)
- One of:
  - [Ollama](https://ollama.com) running locally (default)
  - An Anthropic API key
  - An OpenAI API key (or any OpenAI-compatible endpoint)

## Setup

```sh
cargo build --release

# then run with your preferred provider (see Configuration below)
./target/release/sloppy-toppy
```

## Keys

| Key       | Action                   |
|-----------|--------------------------|
| `q` / Esc | quit                     |
| `j`       | demand a fresh roast now |

## Configuration

All configuration is through environment variables. `SLOPPY_PROVIDER` picks the
backend; everything else is optional.

### Ollama (default)

```sh
# start Ollama, pull a model, then run
ollama serve
ollama pull llama3.2
cargo run --release
```

| Variable        | Default                  | Purpose                               |
|-----------------|--------------------------|---------------------------------------|
| `SLOPPY_PROVIDER` | `ollama`               | Set to `ollama` (or omit)             |
| `SLOPPY_MODEL`  | `llama3.2`               | Any model you've pulled               |
| `OLLAMA_HOST`   | `http://localhost:11434` | Ollama server address                 |

```sh
SLOPPY_MODEL=mistral OLLAMA_HOST=192.168.1.10:11434 cargo run --release
```

### Anthropic

```sh
SLOPPY_PROVIDER=anthropic ANTHROPIC_API_KEY=sk-ant-... cargo run --release
```

| Variable           | Default           | Purpose                        |
|--------------------|-------------------|--------------------------------|
| `SLOPPY_PROVIDER`  | —                 | Set to `anthropic`             |
| `ANTHROPIC_API_KEY`| —                 | Your Anthropic API key         |
| `SLOPPY_MODEL`     | `claude-haiku-4-5`| Any Anthropic model ID         |

### OpenAI / OpenAI-compatible

```sh
SLOPPY_PROVIDER=openai OPENAI_API_KEY=sk-... cargo run --release

# custom endpoint (e.g. local llama.cpp server, Together AI, Mistral AI …)
SLOPPY_PROVIDER=openai \
  OPENAI_BASE_URL=http://localhost:8080/v1 \
  OPENAI_API_KEY=anything \
  SLOPPY_MODEL=mistral-7b \
  cargo run --release
```

| Variable          | Default                      | Purpose                             |
|-------------------|------------------------------|-------------------------------------|
| `SLOPPY_PROVIDER` | —                            | Set to `openai`                     |
| `OPENAI_API_KEY`  | —                            | API key for the endpoint            |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1`  | Override for any compatible API     |
| `SLOPPY_MODEL`    | `gpt-4o-mini`                | Model name the endpoint understands |

---

A new roast is generated every 15 seconds (or instantly when you press `j`),
built from your live CPU/RAM/swap/process numbers. The fetch runs on a
background thread, so inference never stalls the UI.

If the provider can't be reached the commentary panel shows a (snarky) error
instead of crashing — the rest of the monitor keeps working fine.

> ⚠️ The "inappropriate" part is by design: the prompt asks the model for
> crude, mean-spirited humor. Output is entirely up to whatever model you point
> it at. Tweak the prompt in [`src/joke.rs`](src/joke.rs) if you want it tamer.
