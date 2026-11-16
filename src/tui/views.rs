use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};

use super::app::App;
use super::theme::*;

pub fn render_select_video(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_video_picker(frame, chunks[0], app);
    let right_info = build_video_info(app);
    let details = Paragraph::new(right_info).block(default_block("Video Details"));
    frame.render_widget(details, chunks[1]);
}

fn render_video_picker(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    let search_bar = Paragraph::new(format!("❯ {}", app.video_filter))
        .block(active_block("Filter Video Files (Type to search)"))
        .style(Style::default().fg(COLOR_HIGHLIGHT));
    frame.render_widget(search_bar, chunks[0]);

    let items: Vec<ListItem> = app
        .filtered_video_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &real_idx)| {
            let path = &app.videos[real_idx];
            let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("video");
            let is_selected = display_idx == app.selected_video_idx;
            let style = if is_selected {
                style_selected()
            } else {
                style_normal()
            };
            let prefix = if is_selected { " ❯ " } else { "   " };
            ListItem::new(format!("{}{}", prefix, name)).style(style)
        })
        .collect();

    let title = format!("Discovered Videos ({})", app.filtered_video_indices.len());
    let list = List::new(items).block(default_block(&title));
    frame.render_widget(list, chunks[1]);
}

fn build_video_info(app: &App) -> Vec<Line<'static>> {
    let Some(&real_idx) = app.filtered_video_indices.get(app.selected_video_idx) else {
        return vec![Line::from("No video file selected.")];
    };

    let path = &app.videos[real_idx];
    let meta = crate::parser::parse_anime_filename(path);
    let size_str = std::fs::metadata(path)
        .map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0))
        .unwrap_or_else(|_| "Unknown".into());
    let file_name = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_string();
    let path_str = path.display().to_string();

    vec![
        Line::from(vec![
            Span::styled(
                "File: ",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(file_name),
        ]),
        Line::from(vec![
            Span::styled(
                "Size: ",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(size_str),
        ]),
        Line::from(vec![
            Span::styled(
                "Detected Title: ",
                Style::default()
                    .fg(COLOR_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(meta.title),
        ]),
        Line::from(vec![
            Span::styled(
                "Detected Episode: ",
                Style::default()
                    .fg(COLOR_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(meta.episode.unwrap_or_else(|| "None".into())),
        ]),
        Line::from(vec![
            Span::styled("Full Path: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(path_str, Style::default().fg(COLOR_MUTED)),
        ]),
    ]
}

pub fn render_search_anime(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_anime_picker(frame, chunks[0], app);
    let right_info = build_anime_info(app);
    let details = Paragraph::new(right_info).block(default_block("Anime Metadata"));
    frame.render_widget(details, chunks[1]);
}

fn render_anime_picker(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    let search_bar = Paragraph::new(format!("❯ {}", app.anime_query))
        .block(active_block("Search Anime Title (Type to search live)"))
        .style(Style::default().fg(COLOR_HIGHLIGHT));
    frame.render_widget(search_bar, chunks[0]);

    let items: Vec<ListItem> = app
        .filtered_anime_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &real_idx)| {
            let entry = &app.anime_entries[real_idx];
            let is_selected = display_idx == app.selected_anime_idx;
            let style = if is_selected {
                style_selected()
            } else {
                style_normal()
            };
            let prefix = if is_selected { " ❯ " } else { "   " };
            ListItem::new(format!("{}{}", prefix, entry)).style(style)
        })
        .collect();

    let title = format!("Database Matches ({})", app.filtered_anime_indices.len());
    let list = List::new(items).block(default_block(&title));
    frame.render_widget(list, chunks[1]);
}

fn build_anime_info(app: &App) -> Vec<Line<'static>> {
    let Some(&real_idx) = app.filtered_anime_indices.get(app.selected_anime_idx) else {
        return vec![Line::from("No anime entry selected.")];
    };

    let entry = &app.anime_entries[real_idx];
    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Romaji: ",
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(entry.name.clone()),
    ])];
    if let Some(ref en) = entry.english_name {
        lines.push(Line::from(vec![
            Span::styled(
                "English: ",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(en.clone()),
        ]));
    }
    if let Some(ref jp) = entry.japanese_name {
        lines.push(Line::from(vec![
            Span::styled(
                "Japanese: ",
                Style::default()
                    .fg(COLOR_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(jp.clone()),
        ]));
    }

    lines.push(Line::from(build_provider_badges(entry)));
    lines
}

fn build_provider_badges(entry: &crate::provider::UnifiedAnimeEntry) -> Vec<Span<'static>> {
    let mut badges = vec![Span::styled(
        "Providers: ",
        Style::default().fg(COLOR_MUTED),
    )];
    if entry.jimaku_id.is_some() {
        badges.push(Span::styled(
            "[Jimaku.cc] ",
            Style::default()
                .fg(COLOR_JIMAKU)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if entry.kitsunekko_slug.is_some() {
        badges.push(Span::styled(
            "[Kitsunekko] ",
            Style::default()
                .fg(COLOR_KITSU)
                .add_modifier(Modifier::BOLD),
        ));
    }
    badges
}

pub fn render_select_subtitle(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_subtitle_picker(frame, chunks[0], app);
    let right_info = build_subtitle_info(app);
    let details = Paragraph::new(right_info).block(default_block("Release Details"));
    frame.render_widget(details, chunks[1]);
}

fn render_subtitle_picker(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .subtitles
        .iter()
        .enumerate()
        .map(|(i, sub)| {
            let is_selected = i == app.selected_sub_idx;
            let style = if is_selected {
                style_selected()
            } else {
                style_normal()
            };
            let prefix = if is_selected { " ❯ " } else { "   " };
            let prov_color = if sub.provider == "Jimaku.cc" {
                COLOR_JIMAKU
            } else {
                COLOR_KITSU
            };
            Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    format!("[{}] ", sub.language),
                    Style::default().fg(COLOR_SUCCESS),
                ),
                Span::styled(
                    format!("[{}] ", sub.provider),
                    Style::default().fg(prov_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(&sub.title, style),
            ])
            .into()
        })
        .collect();

    let title = format!("Available Subtitle Releases ({})", app.subtitles.len());
    let list = List::new(items).block(active_block(&title));
    frame.render_widget(list, area);
}

fn build_subtitle_info(app: &App) -> Vec<Line<'static>> {
    let Some(sub) = app.subtitles.get(app.selected_sub_idx) else {
        return vec![Line::from("No subtitle selected.")];
    };

    vec![
        Line::from(vec![
            Span::styled(
                "Title: ",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(sub.title.clone()),
        ]),
        Line::from(vec![
            Span::styled(
                "Provider: ",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(sub.provider.clone()),
        ]),
        Line::from(vec![
            Span::styled(
                "Format: ",
                Style::default()
                    .fg(COLOR_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(sub.file_format.clone()),
        ]),
        Line::from(vec![
            Span::styled("Download URL: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(sub.download_url.clone(), Style::default().fg(COLOR_MUTED)),
        ]),
    ]
}
