use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

pub const COLOR_ACCENT: Color = Color::Cyan;
pub const COLOR_HIGHLIGHT: Color = Color::Yellow;
pub const COLOR_JIMAKU: Color = Color::LightMagenta;
pub const COLOR_KITSU: Color = Color::LightYellow;
pub const COLOR_SUCCESS: Color = Color::LightGreen;
pub const COLOR_MUTED: Color = Color::DarkGray;
pub const COLOR_TEXT: Color = Color::White;
pub const COLOR_BG_HIGHLIGHT: Color = Color::Rgb(35, 45, 60);

pub fn default_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_MUTED))
        .title(format!(" {} ", title))
        .title_style(
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        )
}

pub fn active_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_ACCENT))
        .title(format!(" {} ", title))
        .title_style(
            Style::default()
                .fg(COLOR_HIGHLIGHT)
                .add_modifier(Modifier::BOLD),
        )
}

pub fn style_selected() -> Style {
    Style::default()
        .fg(COLOR_HIGHLIGHT)
        .bg(COLOR_BG_HIGHLIGHT)
        .add_modifier(Modifier::BOLD)
}

pub fn style_normal() -> Style {
    Style::default().fg(COLOR_TEXT)
}
