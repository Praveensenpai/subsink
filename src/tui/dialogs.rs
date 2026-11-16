use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use super::app::App;
use super::theme::*;

pub fn render_syncing(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner = spinner_chars[app.spinner_tick % spinner_chars.len()];

    let popup_area = centered_rect(60, 30, area);
    frame.render_widget(Clear, popup_area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!("  {} ", spinner),
                Style::default()
                    .fg(COLOR_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &app.status_message,
                Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Analyzing audio stream & voice activity timing...",
            Style::default().fg(COLOR_MUTED),
        )),
    ];

    let block = active_block("Processing Subtitle Sync");
    frame.render_widget(Paragraph::new(text).block(block), popup_area);
}

pub fn render_completed(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup_area = centered_rect(70, 50, area);
    frame.render_widget(Clear, popup_area);

    let text = vec![
        Line::from(Span::styled(
            "  ✓ Subtitle Successfully Synced!",
            Style::default()
                .fg(COLOR_SUCCESS)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Timing Shift: ",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&app.sync_result_text),
        ]),
        Line::from(vec![
            Span::styled(
                "  Saved File:   ",
                Style::default()
                    .fg(COLOR_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                app.output_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Actions: [p] Play in mpv  •  [q / Esc] Exit",
            Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
        )),
    ];

    let block = active_block("Synchronization Complete");
    frame.render_widget(Paragraph::new(text).block(block), popup_area);
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
