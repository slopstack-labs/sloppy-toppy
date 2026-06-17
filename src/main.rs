//! sloppy-toppy: a system monitor that refuses to mind its own business.

mod app;
mod config;
mod joke;
mod metrics;
mod ui;

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
};
use ratatui::crossterm::execute;

use app::App;
use config::Config;

const TICK: Duration = Duration::from_millis(1000);

fn main() -> std::io::Result<()> {
    let config = Config::load();
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let mut app = App::new(&config);
    let result = run(&mut terminal, &mut app);
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        app.drain_jokes();
        terminal.draw(|frame| ui::draw(frame, app))?;

        let timeout = TICK.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if app.history_visible {
                            app.history_visible = false;
                        } else {
                            app.should_quit = true;
                        }
                    }
                    KeyCode::Char('j') => {
                        if app.history_visible {
                            let max = app.roast_history.len().saturating_sub(1);
                            app.history_scroll = (app.history_scroll + 1).min(max);
                        } else {
                            app.jokes.roast_now();
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up if app.history_visible => {
                        app.history_scroll = app.history_scroll.saturating_sub(1);
                    }
                    KeyCode::Down if app.history_visible => {
                        let max = app.roast_history.len().saturating_sub(1);
                        app.history_scroll = (app.history_scroll + 1).min(max);
                    }
                    KeyCode::Char('h') => {
                        app.history_visible = !app.history_visible;
                        app.history_scroll = 0;
                    }
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        if app.history_visible {
                            app.history_scroll = app.history_scroll.saturating_sub(1);
                        } else {
                            app.proc_offset = app.proc_offset.saturating_sub(1);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if app.history_visible {
                            let max = app.roast_history.len().saturating_sub(1);
                            app.history_scroll = (app.history_scroll + 1).min(max);
                        } else {
                            let max = app.snapshot.top_procs.len().saturating_sub(1);
                            app.proc_offset = (app.proc_offset + 1).min(max);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        if last_tick.elapsed() >= TICK {
            app.tick();
            last_tick = Instant::now();
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
