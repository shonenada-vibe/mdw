use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};

use crate::app::App;

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    app.set_viewport_height(chunks[0].height);

    let theme = &app.config().theme;

    // Content with line numbers
    let total = app.total_lines();
    let gutter_width = if total == 0 { 1 } else { total.ilog10() as usize + 1 };
    let lineno_style = Style::default().fg(theme.line_number.0);
    let sep_style = Style::default().fg(theme.line_number.0);

    let numbered_lines: Vec<Line<'static>> = app
        .rendered_content()
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let mut spans = vec![
                Span::styled(format!("{:>width$} ", i + 1, width = gutter_width), lineno_style),
                Span::styled("│ ", sep_style),
            ];
            spans.extend(line.spans.iter().cloned());
            Line::from(spans)
        })
        .collect();

    let mut content = Paragraph::new(numbered_lines)
        .scroll((app.scroll_offset(), 0));
    if app.config().behavior.line_wrap {
        content = content.wrap(Wrap { trim: false });
    }
    frame.render_widget(content, chunks[0]);

    // Scrollbar
    let mut scrollbar_state = ScrollbarState::new(app.total_lines())
        .position(app.scroll_offset() as usize);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    frame.render_stateful_widget(scrollbar, chunks[0], &mut scrollbar_state);

    // Status bar
    let status_bar_bg = theme.status_bar_bg.0;
    let status_bar_fg = theme.status_bar_fg.0;
    let status_bar_message_fg = theme.status_bar_message_fg.0;

    let scroll_info = format!(
        " {} | {}/{} ",
        app.file_path_display(),
        app.scroll_offset() + 1,
        app.total_lines(),
    );
    let status_bar = Paragraph::new(Line::from(vec![
        Span::styled(
            scroll_info,
            Style::default().bg(status_bar_bg).fg(status_bar_fg),
        ),
        Span::styled(
            format!(" {} ", app.status_message()),
            Style::default().bg(status_bar_bg).fg(status_bar_message_fg),
        ),
    ]))
    .style(Style::default().bg(status_bar_bg));

    frame.render_widget(status_bar, chunks[1]);

    // Help panel overlay
    if app.show_help() {
        let area = frame.area();
        let help_w = 50u16.min(area.width.saturating_sub(4));
        let help_h = 18u16.min(area.height.saturating_sub(4));
        let help_area = Rect {
            x: (area.width.saturating_sub(help_w)) / 2,
            y: (area.height.saturating_sub(help_h)) / 2,
            width: help_w,
            height: help_h,
        };

        let kb = &app.config().keybindings;
        let help_bg = theme.status_bar_bg.0;
        let help_fg = theme.status_bar_fg.0;
        let key_fg = theme.mermaid_edge_label.0;

        let key_style = Style::default().fg(key_fg).add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(help_fg);

        let fmt_keys = |combos: &[crate::config::KeyCombo]| -> String {
            combos
                .iter()
                .map(|c| {
                    let mut s = String::new();
                    if c.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                        s.push_str("Ctrl+");
                    }
                    if c.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
                        s.push_str("Alt+");
                    }
                    if c.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                        s.push_str("Shift+");
                    }
                    match c.code {
                        crossterm::event::KeyCode::Char(ch) => s.push(ch),
                        crossterm::event::KeyCode::Up => s.push_str("Up"),
                        crossterm::event::KeyCode::Down => s.push_str("Down"),
                        crossterm::event::KeyCode::PageUp => s.push_str("PgUp"),
                        crossterm::event::KeyCode::PageDown => s.push_str("PgDn"),
                        crossterm::event::KeyCode::Home => s.push_str("Home"),
                        crossterm::event::KeyCode::End => s.push_str("End"),
                        crossterm::event::KeyCode::Esc => s.push_str("Esc"),
                        _ => s.push_str("?"),
                    }
                    s
                })
                .collect::<Vec<_>>()
                .join(", ")
        };

        let entries: Vec<(&str, String)> = vec![
            ("Quit", fmt_keys(&kb.quit)),
            ("Scroll Down", fmt_keys(&kb.scroll_down)),
            ("Scroll Up", fmt_keys(&kb.scroll_up)),
            ("Half Page Down", fmt_keys(&kb.half_page_down)),
            ("Half Page Up", fmt_keys(&kb.half_page_up)),
            ("Page Down", fmt_keys(&kb.page_down)),
            ("Page Up", fmt_keys(&kb.page_up)),
            ("Go to Top", fmt_keys(&kb.top)),
            ("Go to Bottom", fmt_keys(&kb.bottom)),
            ("Toggle Help", fmt_keys(&kb.toggle_help)),
        ];

        let max_desc = entries.iter().map(|(d, _)| d.len()).max().unwrap_or(0);

        let mut help_lines: Vec<Line<'static>> = Vec::new();
        help_lines.push(Line::from(""));
        for (desc, keys) in &entries {
            let pad = max_desc.saturating_sub(desc.len());
            help_lines.push(Line::from(vec![
                Span::styled(format!("  {desc}{} ", " ".repeat(pad)), desc_style),
                Span::styled(keys.clone(), key_style),
            ]));
        }
        help_lines.push(Line::from(""));
        help_lines.push(Line::from(Span::styled(
            "  Press any key to close".to_string(),
            Style::default().fg(theme.line_number.0),
        )));

        let help_block = Block::default()
            .title(" Keyboard Shortcuts ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .style(Style::default().bg(help_bg).fg(help_fg));

        let help_paragraph = Paragraph::new(help_lines).block(help_block);

        frame.render_widget(Clear, help_area);
        frame.render_widget(help_paragraph, help_area);
    }
}
