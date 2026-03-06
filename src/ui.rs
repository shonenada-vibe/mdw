use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};

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
}
