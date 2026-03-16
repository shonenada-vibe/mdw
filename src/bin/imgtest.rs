use std::env;

use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    buffer::Buffer,
    crossterm::{
        event::{self, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget},
};
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filename = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: imgtest <path/to/image>");
        std::process::exit(1);
    });

    let picker = Picker::from_query_stdio().unwrap_or_else(|e| {
        eprintln!("Picker::from_query_stdio failed: {e:?}, falling back to halfblocks");
        Picker::halfblocks()
    });

    let proto_type = format!("{:?}", picker.protocol_type());
    let font_size = picker.font_size();

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let image_source = image::open(&filename)?;
    let (iw, ih) = (image_source.width(), image_source.height());
    let mut image_state = picker.new_resize_protocol(image_source);

    // Simulate mdw's display_height computation
    let gutter_width: usize = 2;
    let gutter_total: usize = gutter_width + 3; // "NN │ "

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(f.area());

            let content_area = chunks[0];
            let image_cols = content_area.width.saturating_sub(gutter_total as u16);

            // Compute display_height like mdw does
            let (fw, fh) = (font_size.0.max(1) as f64, font_size.1.max(1) as f64);
            let available_px_w = image_cols as f64 * fw;
            let scale = (available_px_w / iw as f64).min(1.0);
            let display_px_h = ih as f64 * scale;
            let display_height = ((display_px_h / fh).ceil() as u16).clamp(1, 50);

            let visible_rows = display_height.min(content_area.height);

            // Render gutter (like mdw)
            let lineno_style = Style::default().fg(ratatui::style::Color::DarkGray);
            for row in 0..visible_rows {
                let gutter_line = Line::from(vec![
                    Span::styled(format!("{:>width$} ", "~", width = gutter_width), lineno_style),
                    Span::styled("│ ", lineno_style),
                ]);
                let gutter_rect = Rect {
                    x: content_area.x,
                    y: content_area.y + row,
                    width: gutter_total as u16,
                    height: 1,
                };
                f.render_widget(Paragraph::new(gutter_line), gutter_rect);
            }

            // Render image (like mdw)
            let image_rect = Rect {
                x: content_area.x + gutter_total as u16,
                y: content_area.y,
                width: content_area.width.saturating_sub(gutter_total as u16),
                height: visible_rows,
            };

            let image_widget = StatefulImage::default();
            f.render_stateful_widget(image_widget, image_rect, &mut image_state);

            // Render scrollbar over content_area (like mdw)
            let mut scrollbar_state =
                ScrollbarState::new(display_height as usize).position(0);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            f.render_stateful_widget(scrollbar, content_area, &mut scrollbar_state);

            // Status bar
            let status = format!(
                " {} | Proto: {} | Font: {:?} | Img: {}x{} | display_h: {} | visible: {} | rect: {}x{} ",
                filename, proto_type, font_size, iw, ih, display_height, visible_rows,
                image_rect.width, image_rect.height,
            );
            f.render_widget(
                Paragraph::new(status)
                    .style(Style::default().bg(ratatui::style::Color::Blue).fg(ratatui::style::Color::White))
                    .alignment(Alignment::Left),
                chunks[1],
            );
        })?;

        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
