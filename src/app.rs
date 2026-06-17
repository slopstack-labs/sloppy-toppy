//! Application state shared between the event loop and the renderer.

use std::collections::VecDeque;
use std::time::Instant;

use crate::config::Config;
use crate::joke::{JokeChannel, JokeConfig};
use crate::metrics::{Collector, Snapshot};

pub enum JokeState {
    Booting,
    Roast(String),
    Error(String),
}

pub struct App {
    pub collector: Collector,
    pub snapshot: Snapshot,
    pub jokes: JokeChannel,
    pub joke_state: JokeState,
    pub is_thinking: bool,
    pub tick_count: u64,
    pub roast_count: u64,
    pub roast_history: VecDeque<String>,
    pub history_visible: bool,
    pub history_scroll: usize,
    pub proc_offset: usize,
    pub alert_cpu: f32,
    pub alert_mem: f32,
    pub last_alert: Option<Instant>,
    pub alert_flash_ticks: u8,
    pub should_quit: bool,
}

impl App {
    pub fn new(config: &Config) -> Self {
        let mut collector = Collector::new();
        let snapshot = collector.sample();
        App {
            collector,
            snapshot,
            jokes: crate::joke::spawn(JokeConfig::from_config(config)),
            joke_state: JokeState::Booting,
            is_thinking: false,
            tick_count: 0,
            roast_count: 0,
            roast_history: VecDeque::new(),
            history_visible: false,
            history_scroll: 0,
            proc_offset: 0,
            alert_cpu: config.alert_cpu,
            alert_mem: config.alert_mem,
            last_alert: None,
            alert_flash_ticks: 0,
            should_quit: false,
        }
    }

    pub fn tick(&mut self) {
        self.snapshot = self.collector.sample();
        self.jokes.update_stats(self.snapshot.clone());
        self.tick_count = self.tick_count.wrapping_add(1);

        // Clamp scroll offset to valid range as proc list may shrink
        let max_offset = self.snapshot.top_procs.len().saturating_sub(1);
        self.proc_offset = self.proc_offset.min(max_offset);

        // Alert: fire an immediate roast when CPU or RAM crosses the threshold.
        // 60-second cooldown to avoid spamming.
        let cpu_pct = self.snapshot.cpu_overall;
        let mem_pct = (self.snapshot.mem_frac() * 100.0) as f32;
        let now = Instant::now();
        let cooldown_ok = self
            .last_alert
            .map_or(true, |t| now.duration_since(t).as_secs() >= 60);

        if (cpu_pct > self.alert_cpu || mem_pct > self.alert_mem) && cooldown_ok {
            self.jokes.roast_now();
            self.last_alert = Some(now);
            self.alert_flash_ticks = 10;
        }

        if self.alert_flash_ticks > 0 {
            self.alert_flash_ticks -= 1;
        }
    }

    pub fn drain_jokes(&mut self) {
        while let Ok(msg) = self.jokes.rx.try_recv() {
            match msg {
                crate::joke::JokeMsg::Thinking => {
                    self.is_thinking = true;
                }
                crate::joke::JokeMsg::Roast(text) => {
                    self.is_thinking = false;
                    self.roast_count += 1;
                    self.roast_history.push_front(text.clone());
                    self.roast_history.truncate(100);
                    self.joke_state = JokeState::Roast(text);
                }
                crate::joke::JokeMsg::Error(why) => {
                    self.is_thinking = false;
                    self.joke_state = JokeState::Error(why);
                }
            }
        }
    }
}
