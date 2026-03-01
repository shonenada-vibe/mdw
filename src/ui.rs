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

    // Content
    let mut content = Paragraph::new(app.rendered_content().clone())
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
