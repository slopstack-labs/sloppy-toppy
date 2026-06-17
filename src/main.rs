//! sloppy-toppy: a system monitor that refuses to mind its own business.

mod app;
mod joke;
mod metrics;
mod ui;

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

use app::App;

/// How often we re-sample metrics and repaint.
const TICK: Duration = Duration::from_millis(1000);

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
) -> std::io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        app.drain_jokes();
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Block for input, but never longer than the time left in this tick,
        // so metrics keep refreshing even when nobody's touching the keyboard.
        let timeout = TICK.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                        KeyCode::Char('j') => app.jokes.roast_now(),
                        _ => {}
                    }
                }
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
