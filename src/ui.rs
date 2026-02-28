use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
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

    // Content
    let content = Paragraph::new(app.rendered_content().clone())
        .scroll((app.scroll_offset(), 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(content, chunks[0]);

    // Scrollbar
    let mut scrollbar_state = ScrollbarState::new(app.total_lines())
        .position(app.scroll_offset() as usize);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    frame.render_stateful_widget(scrollbar, chunks[0], &mut scrollbar_state);

    // Status bar
    let scroll_info = format!(
        " {} | {}/{} ",
        app.file_path_display(),
        app.scroll_offset() + 1,
        app.total_lines(),
    );
    let status_bar = Paragraph::new(Line::from(vec![
        Span::styled(
            scroll_info,
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(
            format!(" {} ", app.status_message()),
            Style::default().bg(Color::DarkGray).fg(Color::Yellow),
        ),
    ]))
    .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(status_bar, chunks[1]);
}
