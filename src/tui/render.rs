use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::app::{App, AppStep};
use super::dialogs::{render_completed, render_syncing};
use super::theme::*;
use super::views::{render_search_anime, render_select_subtitle, render_select_video};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_header(frame, main_layout[0], app);

    match app.step {
        AppStep::SelectVideo => render_select_video(frame, main_layout[1], app),
        AppStep::SearchAnime => render_search_anime(frame, main_layout[1], app),
        AppStep::SelectSubtitle => render_select_subtitle(frame, main_layout[1], app),
        AppStep::Syncing => render_syncing(frame, main_layout[1], app),
        AppStep::Completed => render_completed(frame, main_layout[1], app),
    }

    render_footer(frame, main_layout[2], app);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let steps = [
        ("1. Video", AppStep::SelectVideo),
        ("2. Database", AppStep::SearchAnime),
        ("3. Subtitle", AppStep::SelectSubtitle),
        ("4. Sync", AppStep::Syncing),
        ("5. Done", AppStep::Completed),
    ];

    let mut spans = vec![Span::styled(
        " ⛩️  S U B S I N K   ",
        Style::default()
            .fg(COLOR_ACCENT)
            .add_modifier(Modifier::BOLD),
    )];

    for (i, (name, step)) in steps.iter().enumerate() {
        let style = if app.step == *step {
            Style::default()
                .fg(COLOR_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        } else if step_order(app.step) > step_order(*step) {
            Style::default().fg(COLOR_SUCCESS)
        } else {
            Style::default().fg(COLOR_MUTED)
        };

        spans.push(Span::styled(format!("[{}]", name), style));
        if i + 1 < steps.len() {
            spans.push(Span::styled(" ──► ", Style::default().fg(COLOR_MUTED)));
        }
    }

    let header_line = Line::from(spans);
    let block = default_block("SubSyncer");
    frame.render_widget(Paragraph::new(header_line).block(block), area);
}

fn step_order(step: AppStep) -> usize {
    match step {
        AppStep::SelectVideo => 1,
        AppStep::SearchAnime => 2,
        AppStep::SelectSubtitle => 3,
        AppStep::Syncing => 4,
        AppStep::Completed => 5,
    }
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let help_text = match app.step {
        AppStep::SelectVideo => {
            "[↑/↓] Navigate  •  [Type] Filter  •  [Enter] Select Video  •  [Esc] Quit"
        }
        AppStep::SearchAnime => {
            "[↑/↓] Navigate  •  [Type] Search  •  [Enter] Choose Anime  •  [Esc] Back"
        }
        AppStep::SelectSubtitle => {
            "[↑/↓/j/k] Navigate  •  [Enter] Download & Sync  •  [Esc/q] Back"
        }
        AppStep::Syncing => "Processing audio voice alignment with ALASS...",
        AppStep::Completed => "[p] Play Video in mpv  •  [q / Esc] Exit",
    };

    let line = Line::from(vec![
        Span::styled(
            " Hotkeys: ",
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(help_text, Style::default().fg(COLOR_TEXT)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_MUTED));
    frame.render_widget(Paragraph::new(line).block(block), area);
}
