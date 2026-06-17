//! All ratatui rendering lives here.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Sparkline, Table, Wrap,
};
use ratatui::Frame;

use crate::app::{App, JokeState};
use crate::metrics::fmt_bytes;

fn heat(pct: f64) -> Color {
    if pct >= 85.0 { Color::Red } else if pct >= 60.0 { Color::Yellow } else { Color::Green }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(22), // meters + joke
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
    draw_processes(frame, root[1], app);
    draw_footer(frame, root[2], app);

    if app.history_visible {
        draw_history(frame, app);
    }
}

fn draw_meters(frame: &mut Frame, area: Rect, app: &App) {
    let snap = &app.snapshot;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // cpu
            Constraint::Length(3), // mem
            Constraint::Length(3), // swap
            Constraint::Length(3), // temp
            Constraint::Length(3), // network
            Constraint::Length(3), // disk
            Constraint::Min(4),    // cpu history
        ])
        .split(area);

    // CPU
    let cpu = snap.cpu_overall as f64;
    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(format!(
                " CPU · {} cores · load {:.2} ",
                snap.per_core.len(),
                snap.load_one
            )))
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
            .label(format!("{} / {}", fmt_bytes(snap.mem_used), fmt_bytes(snap.mem_total))),
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

    // Temperature
    let (temp_ratio, temp_label, temp_color) = match snap.cpu_temp {
        Some(t) => {
            let v = t.min(100.0) as f64;
            (v / 100.0, format!("{t:.0}°C"), heat(v))
        }
        None => (0.0, "no sensor".to_string(), Color::DarkGray),
    };
    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" CPU Temp "))
            .gauge_style(Style::default().fg(temp_color))
            .ratio(temp_ratio.clamp(0.0, 1.0))
            .label(temp_label),
        rows[3],
    );

    // Network I/O
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↓ ", Style::default().fg(Color::Green)),
            Span::raw(format!("{}/s", fmt_bytes(snap.net_rx_bps))),
            Span::raw("   "),
            Span::styled("↑ ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{}/s", fmt_bytes(snap.net_tx_bps))),
        ]))
        .block(Block::default().borders(Borders::ALL).title(" Network I/O "))
        .alignment(Alignment::Left),
        rows[4],
    );

    // Disk
    let disk_pct = (snap.disk_frac() * 100.0) as u8;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(format!(
                "{} / {} ({disk_pct}%)",
                fmt_bytes(snap.disk_used),
                fmt_bytes(snap.disk_total)
            )),
            Span::raw("   "),
            Span::styled("r:", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}/s  ", fmt_bytes(snap.disk_read_bps))),
            Span::styled("w:", Style::default().fg(Color::Magenta)),
            Span::raw(format!("{}/s", fmt_bytes(snap.disk_write_bps))),
        ]))
        .block(Block::default().borders(Borders::ALL).title(" Disk (/) "))
        .alignment(Alignment::Left),
        rows[5],
    );

    // CPU history sparkline
    frame.render_widget(
        Sparkline::default()
            .block(Block::default().borders(Borders::ALL).title(" CPU history "))
            .data(&app.collector.cpu_history)
            .max(100)
            .style(Style::default().fg(Color::Cyan)),
        rows[6],
    );
}

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn draw_joke(frame: &mut Frame, area: Rect, app: &App) {
    let (body, content_color): (Vec<Line>, Color) = match &app.joke_state {
        JokeState::Booting => (vec![Line::from("warming up…").italic()], Color::DarkGray),
        JokeState::Roast(text) => (
            text.lines().map(|l| Line::from(l.to_string())).collect(),
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
        let idx = (app.tick_count as usize) % SPINNER.len();
        format!(" {} ", SPINNER[idx])
    } else {
        String::new()
    };

    // Alert blink: flash red every other tick for alert_flash_ticks ticks.
    let border_color = if app.alert_flash_ticks > 0 && app.tick_count.is_multiple_of(2) {
        Color::Red
    } else if app.is_thinking {
        Color::Magenta
    } else {
        content_color
    };

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
                    .border_style(Style::default().fg(border_color))
                    .title(title),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_processes(frame: &mut Frame, area: Rect, app: &App) {
    let snap = &app.snapshot;
    let total = snap.top_procs.len();
    let procs = snap.top_procs.get(app.proc_offset..).unwrap_or(&[]);

    let header = Row::new(vec![
        Cell::from("PID"),
        Cell::from("PROCESS"),
        Cell::from("CPU%"),
        Cell::from("MEM"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED));

    let rows = procs.iter().map(|p| {
        Row::new(vec![
            Cell::from(p.pid.to_string()),
            Cell::from(p.name.clone()),
            Cell::from(format!("{:.1}", p.cpu)).style(Style::default().fg(heat(p.cpu as f64))),
            Cell::from(fmt_bytes(p.mem_bytes)),
        ])
    });

    let title = if total > 12 {
        format!(
            " Top processes (by CPU) [{}/{total}] scroll ↕ ",
            app.proc_offset + 1
        )
    } else {
        " Top processes (by CPU) ".to_string()
    };

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(8),
                Constraint::Min(16),
                Constraint::Length(8),
                Constraint::Length(12),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(Color::DarkGray)),
        area,
    );
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let up = app.snapshot.uptime_secs;
    let footer = Line::from(vec![
        Span::styled(" sloppy-toppy ", Style::default().bg(Color::Magenta).fg(Color::Black)),
        Span::raw(format!("  up {}h{:02}m  ", up / 3600, (up % 3600) / 60)),
        Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" quit  "),
        Span::styled("j", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" roast  "),
        Span::styled("h", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" history  scroll ↕ processes "),
    ]);
    frame.render_widget(
        Paragraph::new(footer).block(Block::default().borders(Borders::NONE)),
        area,
    );
}

fn draw_history(frame: &mut Frame, app: &App) {
    let area = centered_rect(85, 85, frame.area());
    frame.render_widget(Clear, area);

    let items: Vec<Line> = if app.roast_history.is_empty() {
        vec![Line::from("no roasts yet — press j to demand one").italic()]
    } else {
        app.roast_history
            .iter()
            .skip(app.history_scroll)
            .enumerate()
            .map(|(i, roast)| Line::from(format!("{:>3}. {roast}", app.history_scroll + i + 1)))
            .collect()
    };

    let title = format!(
        " 📜 roast history · {} total · j/k scroll · h or q close ",
        app.roast_history.len()
    );

    frame.render_widget(
        Paragraph::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let width = r.width * percent_x / 100;
    let height = r.height * percent_y / 100;
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
