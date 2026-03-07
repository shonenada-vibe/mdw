use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};
use ratatui_image::StatefulImage;

use crate::config::ThemeConfig;
use crate::content::ContentBlock;

use crate::app::App;

struct VisibleBlock {
    block_idx: usize,
    block_start: usize,
    visible_start: usize,
    visible_rows: usize,
    screen_y: u16,
}

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    app.set_viewport_height(chunks[0].height);

    let content_area = chunks[0];
    let scroll_offset = app.scroll_offset() as usize;
    let viewport_height = content_area.height as usize;
    let gutter_width = app.gutter_width();
    let search_query = app.search_query().to_string();
    let has_search = !search_query.is_empty() && !app.search_matches().is_empty();
    let search_matches: Vec<usize> = app.search_matches().to_vec();
    let hover_line = app.hover_line();
    let theme = app.config().theme.clone();
    let line_wrap = app.config().behavior.line_wrap;

    let lineno_style = Style::default().fg(theme.line_number.0);
    let sep_style = Style::default().fg(theme.line_number.0);
    let hover_bg = theme.hover_bg.0;

    // First pass: compute which blocks are visible
    let mut visible_blocks: Vec<VisibleBlock> = Vec::new();
    {
        let mut cumulative_row: usize = 0;
        let blocks = app.content_blocks();
        for (block_idx, block) in blocks.iter().enumerate() {
            let block_height = match block {
                ContentBlock::Text { lines } => lines.len(),
                ContentBlock::Image { display_height, .. } => *display_height as usize,
            };

            let block_start = cumulative_row;
            let block_end = cumulative_row + block_height;

            if block_end <= scroll_offset {
                cumulative_row = block_end;
                continue;
            }
            if block_start >= scroll_offset + viewport_height {
                break;
            }

            let visible_start = scroll_offset.saturating_sub(block_start);
            let visible_end = block_height.min(scroll_offset + viewport_height - block_start);
            let visible_rows = visible_end - visible_start;

            if visible_rows == 0 {
                cumulative_row = block_end;
                continue;
            }

            let sy = if block_start >= scroll_offset {
                content_area.y + (block_start - scroll_offset) as u16
            } else {
                content_area.y
            };

            visible_blocks.push(VisibleBlock {
                block_idx,
                block_start,
                visible_start,
                visible_rows,
                screen_y: sy,
            });

            cumulative_row = block_end;
        }
    }

    // Second pass: render each visible block
    // We need to handle text blocks and image blocks differently
    // because image blocks may need mutable access for StatefulImage
    let gutter_total_width = gutter_width + 3;

    for vb in &visible_blocks {
        let block_start = vb.block_start;
        let visible_start = vb.visible_start;
        let visible_rows = vb.visible_rows;
        let sy = vb.screen_y;
        let block_idx = vb.block_idx;

        // Check block type without holding a long-lived borrow
        enum BlockKind {
            Text,
            ImageWithProtocol,
            ImageWithError(String),
            ImageFallback(String),
        }

        let kind = {
            let blocks = app.content_blocks();
            match &blocks[block_idx] {
                ContentBlock::Text { .. } => BlockKind::Text,
                ContentBlock::Image { protocol: Some(_), .. } => BlockKind::ImageWithProtocol,
                ContentBlock::Image { error: Some(err), .. } => BlockKind::ImageWithError(err.clone()),
                ContentBlock::Image { alt_text, .. } => {
                    let alt = if alt_text.is_empty() {
                        "[Image]".to_string()
                    } else {
                        format!("[Image: {alt_text}]")
                    };
                    BlockKind::ImageFallback(alt)
                }
            }
        };

        match kind {
            BlockKind::Text => {
                let blocks = app.content_blocks();
                if let ContentBlock::Text { lines } = &blocks[block_idx] {
                    let visible_lines = &lines[visible_start..visible_start + visible_rows];
                    let mut numbered_lines: Vec<Line<'static>> = Vec::with_capacity(visible_rows);

                    for (i, line) in visible_lines.iter().enumerate() {
                        let abs_row = block_start + visible_start + i;
                        let is_hovered = hover_line == Some(abs_row);
                        let ln_style = if is_hovered {
                            lineno_style.bg(hover_bg)
                        } else {
                            lineno_style
                        };
                        let sp_style = if is_hovered {
                            sep_style.bg(hover_bg)
                        } else {
                            sep_style
                        };

                        let mut spans = vec![
                            Span::styled(
                                format!("{:>width$} ", abs_row + 1, width = gutter_width),
                                ln_style,
                            ),
                            Span::styled("│ ", sp_style),
                        ];

                        if has_search && search_matches.contains(&abs_row) {
                            let mut search_spans = highlight_search_spans(&line.spans, &search_query, &theme);
                            if is_hovered {
                                for s in &mut search_spans {
                                    s.style = s.style.bg(hover_bg);
                                }
                            }
                            spans.extend(search_spans);
                        } else if is_hovered {
                            spans.extend(line.spans.iter().map(|s| {
                                Span::styled(s.content.clone(), s.style.bg(hover_bg))
                            }));
                        } else {
                            spans.extend(line.spans.iter().cloned());
                        }

                        numbered_lines.push(Line::from(spans));
                    }

                    let rect = Rect {
                        x: content_area.x,
                        y: sy,
                        width: content_area.width,
                        height: visible_rows as u16,
                    };

                    let mut paragraph = Paragraph::new(numbered_lines);
                    if line_wrap {
                        paragraph = paragraph.wrap(Wrap { trim: false });
                    }
                    frame.render_widget(paragraph, rect);
                }
            }
            BlockKind::ImageWithProtocol | BlockKind::ImageWithError(_) | BlockKind::ImageFallback(_) => {
                // Render gutter with "~" for image rows
                for row in 0..visible_rows {
                    let abs_row = block_start + visible_start + row;
                    let is_hovered = hover_line == Some(abs_row);
                    let ln_style = if is_hovered {
                        lineno_style.bg(hover_bg)
                    } else {
                        lineno_style
                    };
                    let sp_style = if is_hovered {
                        sep_style.bg(hover_bg)
                    } else {
                        sep_style
                    };

                    let gutter_line = Line::from(vec![
                        Span::styled(
                            format!("{:>width$} ", "~", width = gutter_width),
                            ln_style,
                        ),
                        Span::styled("│ ", sp_style),
                    ]);

                    let gutter_rect = Rect {
                        x: content_area.x,
                        y: sy + row as u16,
                        width: gutter_total_width as u16,
                        height: 1,
                    };
                    frame.render_widget(Paragraph::new(gutter_line), gutter_rect);
                }

                let image_rect = Rect {
                    x: content_area.x + gutter_total_width as u16,
                    y: sy,
                    width: content_area.width.saturating_sub(gutter_total_width as u16),
                    height: visible_rows as u16,
                };

                match kind {
                    BlockKind::ImageWithProtocol => {
                        let blocks_mut = app.content_blocks_mut();
                        if let ContentBlock::Image { protocol: Some(proto), .. } = &mut blocks_mut[block_idx] {
                            let image_widget = StatefulImage::default();
                            frame.render_stateful_widget(image_widget, image_rect, proto);
                        }
                    }
                    BlockKind::ImageWithError(ref err) => {
                        let err_text = format!("[Image Error: {err}]");
                        let err_style = Style::default().fg(theme.blockquote.0);
                        let err_line = Line::from(Span::styled(err_text, err_style));
                        frame.render_widget(Paragraph::new(err_line), image_rect);
                    }
                    BlockKind::ImageFallback(alt) => {
                        let alt_style = Style::default().fg(theme.blockquote.0);
                        let alt_line = Line::from(Span::styled(alt, alt_style));
                        frame.render_widget(Paragraph::new(alt_line), image_rect);
                    }
                    _ => {}
                }
            }
        }
    }

    // Scrollbar
    let mut scrollbar_state = ScrollbarState::new(app.total_lines())
        .position(app.scroll_offset() as usize);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    frame.render_stateful_widget(scrollbar, chunks[0], &mut scrollbar_state);

    // Status bar
    let status_bar_bg = theme.status_bar_bg.0;
    let status_bar_fg = theme.status_bar_fg.0;
    let status_bar_message_fg = theme.status_bar_message_fg.0;

    let status_bar = if app.search_mode() {
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("/{}", app.search_query()),
                Style::default().bg(status_bar_bg).fg(status_bar_fg),
            ),
            Span::styled(
                "\u{2588}",
                Style::default().bg(status_bar_bg).fg(status_bar_fg),
            ),
        ]))
        .style(Style::default().bg(status_bar_bg))
    } else {
        let scroll_info = format!(
            " {} | {}/{} ",
            app.file_path_display(),
            app.scroll_offset() + 1,
            app.total_lines(),
        );
        Paragraph::new(Line::from(vec![
            Span::styled(
                scroll_info,
                Style::default().bg(status_bar_bg).fg(status_bar_fg),
            ),
            Span::styled(
                format!(" {} ", app.status_message()),
                Style::default().bg(status_bar_bg).fg(status_bar_message_fg),
            ),
        ]))
        .style(Style::default().bg(status_bar_bg))
    };

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
                        _ => s.push('?'),
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
            ("Search", fmt_keys(&kb.search_forward)),
            ("Next Match", fmt_keys(&kb.search_next)),
            ("Prev Match", fmt_keys(&kb.search_prev)),
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

fn highlight_search_spans<'a>(
    spans: &[Span<'a>],
    query: &str,
    theme: &ThemeConfig,
) -> Vec<Span<'a>> {
    let highlight_style = Style::default()
        .fg(theme.search_match_fg.0)
        .bg(theme.search_match_bg.0)
        .add_modifier(Modifier::BOLD);
    let query_lower = query.to_lowercase();
    let mut result = Vec::new();

    for span in spans {
        let text = span.content.as_ref();
        let text_lower = text.to_lowercase();
        let mut last = 0;

        for (start, _) in text_lower.match_indices(&query_lower) {
            let end = start + query.len();
            if start > last {
                result.push(Span::styled(
                    text[last..start].to_string(),
                    span.style,
                ));
            }
            result.push(Span::styled(
                text[start..end].to_string(),
                highlight_style,
            ));
            last = end;
        }

        if last < text.len() {
            result.push(Span::styled(
                text[last..].to_string(),
                span.style,
            ));
        } else if last == 0 {
            result.push(span.clone());
        }
    }

    result
}
