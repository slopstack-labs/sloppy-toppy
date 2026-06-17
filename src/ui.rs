//! All ratatui rendering lives here.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table, Wrap,
};
use ratatui::Frame;

use crate::app::{App, JokeState};
use crate::metrics::{fmt_bytes, Snapshot};

/// Pick a color for a 0..=100 utilization value: green -> yellow -> red.
fn heat(pct: f64) -> Color {
    if pct >= 85.0 {
        Color::Red
    } else if pct >= 60.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12), // meters + joke
            Constraint::Min(6),     // process table
            Constraint::Length(1),  // footer
        ])
        .split(frame.area());

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(root[0]);

    draw_meters(frame, top[0], app);
    draw_joke(frame, top[1], app);
    draw_processes(frame, root[1], &app.snapshot);
    draw_footer(frame, root[2], app);
}

fn draw_meters(frame: &mut Frame, area: Rect, app: &App) {
    let snap = &app.snapshot;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // cpu gauge
            Constraint::Length(3), // mem gauge
            Constraint::Length(3), // swap gauge
            Constraint::Min(3),    // cpu history sparkline
        ])
        .split(area);

    // CPU
    let cpu = snap.cpu_overall as f64;
    frame.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" CPU · {} cores · load {:.2} ", snap.per_core.len(), snap.load_one)),
            )
            .gauge_style(Style::default().fg(heat(cpu)))
            .ratio((cpu / 100.0).clamp(0.0, 1.0))
            .label(format!("{cpu:.0}%")),
        rows[0],
    );

    // Memory
    let mem_pct = snap.mem_frac() * 100.0;
    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" Memory "))
            .gauge_style(Style::default().fg(heat(mem_pct)))
            .ratio(snap.mem_frac().clamp(0.0, 1.0))
            .label(format!(
                "{} / {}",
                fmt_bytes(snap.mem_used),
                fmt_bytes(snap.mem_total)
            )),
        rows[1],
    );

    // Swap
    let swap_pct = snap.swap_frac() * 100.0;
    let swap_label = if snap.swap_total == 0 {
        "no swap (living dangerously)".to_string()
    } else {
        format!("{} / {}", fmt_bytes(snap.swap_used), fmt_bytes(snap.swap_total))
    };
    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" Swap "))
            .gauge_style(Style::default().fg(heat(swap_pct)))
            .ratio(snap.swap_frac().clamp(0.0, 1.0))
            .label(swap_label),
        rows[2],
    );

    // CPU history sparkline
    frame.render_widget(
        Sparkline::default()
            .block(Block::default().borders(Borders::ALL).title(" CPU history "))
            .data(&app.collector.cpu_history)
            .max(100)
            .style(Style::default().fg(Color::Cyan)),
        rows[3],
    );
}

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn draw_joke(frame: &mut Frame, area: Rect, app: &App) {
    let (body, color): (Vec<Line>, Color) = match &app.joke_state {
        JokeState::Booting => (
            vec![Line::from("warming up…").italic()],
            Color::DarkGray,
        ),
        JokeState::Roast(text) => (
            text.lines().map(|line| Line::from(line.to_string())).collect(),
            Color::White,
        ),
        JokeState::Error(why) => (
            vec![
                Line::from(Span::styled(
                    "the LLM is sulking:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(why.clone()),
            ],
            Color::Red,
        ),
    };

    let spinner_str = if app.is_thinking {
        let frame_idx = (app.tick_count as usize) % SPINNER.len();
        format!(" {} ", SPINNER[frame_idx])
    } else {
        String::new()
    };
    let spinner_color = if app.is_thinking { Color::Magenta } else { color };

    let title = Line::from(vec![
        Span::raw(format!(" 🤖 unsolicited commentary · {} served", app.roast_count)),
        Span::styled(spinner_str, Style::default().fg(Color::Magenta)),
        Span::raw(" "),
    ]);

    frame.render_widget(
        Paragraph::new(body)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(spinner_color))
                    .title(title),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_processes(frame: &mut Frame, area: Rect, snap: &Snapshot) {
    let header = Row::new(vec![
        Cell::from("PID"),
        Cell::from("PROCESS"),
        Cell::from("CPU%"),
        Cell::from("MEM"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED));

    let rows = snap.top_procs.iter().map(|proc_| {
        Row::new(vec![
            Cell::from(proc_.pid.to_string()),
            Cell::from(proc_.name.clone()),
            Cell::from(format!("{:.1}", proc_.cpu))
                .style(Style::default().fg(heat(proc_.cpu as f64))),
            Cell::from(fmt_bytes(proc_.mem_bytes)),
        ])
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(16),
            Constraint::Length(8),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Top processes (by CPU) "),
    )
    .column_spacing(1)
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_widget(table, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let up = app.snapshot.uptime_secs;
    let footer = Line::from(vec![
        Span::styled(" sloppy-toppy ", Style::default().bg(Color::Magenta).fg(Color::Black)),
        Span::raw(format!(
            "  up {}h{:02}m  ",
            up / 3600,
            (up % 3600) / 60
        )),
        Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" quit  "),
        Span::styled("j", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" roast me again "),
    ]);
    frame.render_widget(
        Paragraph::new(footer).block(Block::default().borders(Borders::NONE)),
        area,
    );
}
