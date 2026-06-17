//! Application state shared between the event loop and the renderer.

use crate::joke::JokeChannel;
use crate::metrics::{Collector, Snapshot};

/// What to show in the commentary panel body.
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
    /// True while a request is in-flight — drives the spinner.
    pub is_thinking: bool,
    /// Increments every tick; used to advance the spinner frame.
    pub tick_count: u64,
    pub roast_count: u64,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let mut collector = Collector::new();
        let snapshot = collector.sample();
        App {
            collector,
            snapshot,
            jokes: crate::joke::spawn(),
            joke_state: JokeState::Booting,
            is_thinking: false,
            tick_count: 0,
            roast_count: 0,
            should_quit: false,
        }
    }

    pub fn tick(&mut self) {
        self.snapshot = self.collector.sample();
        self.jokes.update_stats(self.snapshot.clone());
        self.tick_count = self.tick_count.wrapping_add(1);
    }

    pub fn drain_jokes(&mut self) {
        while let Ok(msg) = self.jokes.rx.try_recv() {
            match msg {
                crate::joke::JokeMsg::Thinking => {
                    self.is_thinking = true;
                    // keep whatever is in joke_state so the last roast stays visible
                }
                crate::joke::JokeMsg::Roast(text) => {
                    self.is_thinking = false;
                    self.roast_count += 1;
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
