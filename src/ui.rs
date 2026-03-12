use unicode_width::UnicodeWidthStr;

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

    let theme = app.config().theme.clone();

    if app.split_view() {
        render_split_view(frame, app, chunks[0], &theme);
    } else {
        render_single_view(frame, app, chunks[0], &theme);
    }

    // Scrollbar
    let mut scrollbar_state = ScrollbarState::new(app.total_lines())
        .position(app.scroll_offset() as usize);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    frame.render_stateful_widget(scrollbar, chunks[0], &mut scrollbar_state);

    // Status bar
    render_status_bar(frame, app, chunks[1], &theme);

    // Frontmatter popup overlay
    if app.frontmatter_popup_index().is_some() {
        render_frontmatter_popup(frame, app, &theme);
    }

    // Help panel overlay
    if app.show_help() {
        render_help_overlay(frame, app, &theme);
    }
}

fn render_split_view(frame: &mut Frame, app: &mut App, area: Rect, theme: &ThemeConfig) {
    // Split horizontally: source | divider | rendered
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Percentage(50),
        ])
        .split(area);

    let source_area = panes[0];
    let divider_area = panes[1];
    let rendered_area = panes[2];

    // Render vertical divider
    let divider_style = Style::default().fg(theme.line_number.0);
    let divider_lines: Vec<Line<'static>> = (0..divider_area.height)
        .map(|_| Line::from(Span::styled("│", divider_style)))
        .collect();
    frame.render_widget(Paragraph::new(divider_lines), divider_area);

    // Render source pane (raw text with line numbers)
    render_source_pane(frame, app, source_area, theme);

    // Render rendered pane (markdown output)
    render_content_pane(frame, app, rendered_area, theme);
}

fn render_source_pane(frame: &mut Frame, app: &App, area: Rect, theme: &ThemeConfig) {
    let raw = app.raw_content();
    let raw_lines: Vec<&str> = raw.lines().collect();
    let total_raw_lines = raw_lines.len().max(1);
    let gutter_width = if total_raw_lines == 0 { 1 } else { total_raw_lines.ilog10() as usize + 1 };
    let scroll_offset = app.scroll_offset() as usize;
    let viewport_height = area.height as usize;

    let lineno_style = Style::default().fg(theme.line_number.0);
    let sep_style = Style::default().fg(theme.line_number.0);
    let code_style = Style::default()
        .fg(theme.code_block_fg.0);

    let mut display_lines: Vec<Line<'static>> = Vec::with_capacity(viewport_height);

    for i in 0..viewport_height {
        let line_idx = scroll_offset + i;
        if line_idx >= total_raw_lines {
            // Render tilde for lines past end of file
            display_lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>width$} ", "~", width = gutter_width),
                    lineno_style,
                ),
                Span::styled("│ ", sep_style),
            ]));
        } else {
            let line_text = raw_lines[line_idx];
            display_lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>width$} ", line_idx + 1, width = gutter_width),
                    lineno_style,
                ),
                Span::styled("│ ", sep_style),
                Span::styled(line_text.to_string(), code_style),
            ]));
        }
    }

    let paragraph = Paragraph::new(display_lines);
    frame.render_widget(paragraph, area);
}

fn render_content_pane(frame: &mut Frame, app: &mut App, area: Rect, theme: &ThemeConfig) {
    let scroll_offset = app.scroll_offset() as usize;
    let viewport_height = area.height as usize;
    let gutter_width = app.gutter_width();
    let search_query = app.search_query().to_string();
    let has_search = !search_query.is_empty() && !app.search_matches().is_empty();
    let search_matches: Vec<usize> = app.search_matches().to_vec();
    let hover_line = app.hover_line();
    let line_wrap = app.config().behavior.line_wrap;

    let lineno_style = Style::default().fg(theme.line_number.0);
    let sep_style = Style::default().fg(theme.line_number.0);
    let hover_bg = theme.hover_bg.0;

    let visible_blocks = compute_visible_blocks(app.content_blocks(), scroll_offset, viewport_height, area);
    let gutter_total_width = gutter_width + 3;

    render_blocks(
        frame, app, &visible_blocks, area, gutter_width, gutter_total_width,
        &lineno_style, &sep_style, hover_bg, hover_line,
        has_search, &search_matches, &search_query, theme, line_wrap,
    );
}

fn render_single_view(frame: &mut Frame, app: &mut App, area: Rect, theme: &ThemeConfig) {
    let scroll_offset = app.scroll_offset() as usize;
    let viewport_height = area.height as usize;
    let gutter_width = app.gutter_width();
    let search_query = app.search_query().to_string();
    let has_search = !search_query.is_empty() && !app.search_matches().is_empty();
    let search_matches: Vec<usize> = app.search_matches().to_vec();
    let hover_line = app.hover_line();
    let line_wrap = app.config().behavior.line_wrap;

    let lineno_style = Style::default().fg(theme.line_number.0);
    let sep_style = Style::default().fg(theme.line_number.0);
    let hover_bg = theme.hover_bg.0;

    let visible_blocks = compute_visible_blocks(app.content_blocks(), scroll_offset, viewport_height, area);
    let gutter_total_width = gutter_width + 3;

    render_blocks(
        frame, app, &visible_blocks, area, gutter_width, gutter_total_width,
        &lineno_style, &sep_style, hover_bg, hover_line,
        has_search, &search_matches, &search_query, theme, line_wrap,
    );
}

fn compute_visible_blocks(blocks: &[ContentBlock], scroll_offset: usize, viewport_height: usize, area: Rect) -> Vec<VisibleBlock> {
    let mut visible_blocks: Vec<VisibleBlock> = Vec::new();
    let mut cumulative_row: usize = 0;

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
            area.y + (block_start - scroll_offset) as u16
        } else {
            area.y
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

    visible_blocks
}

#[allow(clippy::too_many_arguments)]
fn render_blocks(
    frame: &mut Frame,
    app: &mut App,
    visible_blocks: &[VisibleBlock],
    content_area: Rect,
    gutter_width: usize,
    gutter_total_width: usize,
    lineno_style: &Style,
    sep_style: &Style,
    hover_bg: ratatui::style::Color,
    hover_line: Option<usize>,
    has_search: bool,
    search_matches: &[usize],
    search_query: &str,
    theme: &ThemeConfig,
    line_wrap: bool,
) {
    // Build a visual-row-to-logical-line map for mouse handling.
    // Indexed by (screen_y - content_area.y). Value is the logical line index.
    let viewport_h = content_area.height as usize;
    let mut visual_line_map: Vec<Option<usize>> = vec![None; viewport_h];

    for vb in visible_blocks {
        let block_start = vb.block_start;
        let visible_start = vb.visible_start;
        let visible_rows = vb.visible_rows;
        let sy = vb.screen_y;
        let block_idx = vb.block_idx;

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
                    let gtw = gutter_total_width as u16;
                    let content_w = content_area.width.saturating_sub(gtw) as usize;
                    let viewport_bottom = content_area.y + content_area.height;
                    let mut current_y = sy;

                    for (i, line) in visible_lines.iter().enumerate() {
                        if current_y >= viewport_bottom {
                            break;
                        }

                        let abs_row = block_start + visible_start + i;
                        let is_hovered = hover_line == Some(abs_row);
                        let ln_style = if is_hovered {
                            lineno_style.bg(hover_bg)
                        } else {
                            *lineno_style
                        };
                        let sp_style = if is_hovered {
                            sep_style.bg(hover_bg)
                        } else {
                            *sep_style
                        };

                        let content_spans = if has_search && search_matches.contains(&abs_row) {
                            let mut search_spans = highlight_search_spans(&line.spans, search_query, theme);
                            if is_hovered {
                                for s in &mut search_spans {
                                    s.style = s.style.bg(hover_bg);
                                }
                            }
                            search_spans
                        } else if is_hovered {
                            line.spans.iter().map(|s| {
                                Span::styled(s.content.clone(), s.style.bg(hover_bg))
                            }).collect()
                        } else {
                            line.spans.iter().cloned().collect()
                        };

                        // Estimate visual rows this line will take when wrapped
                        let visual_rows = if line_wrap && content_w > 0 {
                            let display_w: usize = line.spans.iter()
                                .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                                .sum();
                            ((display_w + content_w - 1) / content_w).max(1) as u16
                        } else {
                            1
                        };
                        let available = viewport_bottom - current_y;
                        let row_h = visual_rows.min(available);

                        // Render gutter (line number on first row, rest blank)
                        let gutter_rect = Rect {
                            x: content_area.x,
                            y: current_y,
                            width: gtw.min(content_area.width),
                            height: row_h,
                        };
                        let gutter_line = Line::from(vec![
                            Span::styled(
                                format!("{:>width$} ", abs_row + 1, width = gutter_width),
                                ln_style,
                            ),
                            Span::styled("│ ", sp_style),
                        ]);
                        frame.render_widget(Paragraph::new(vec![gutter_line]), gutter_rect);

                        // Render content
                        let content_rect = Rect {
                            x: content_area.x + gtw,
                            y: current_y,
                            width: content_area.width.saturating_sub(gtw),
                            height: row_h,
                        };
                        let content_line = Line::from(content_spans);
                        let mut paragraph = Paragraph::new(vec![content_line]);
                        if line_wrap {
                            paragraph = paragraph.wrap(Wrap { trim: false });
                        }
                        frame.render_widget(paragraph, content_rect);

                        // Record visual-row-to-logical-line mapping
                        for vy in current_y..current_y + row_h {
                            let map_idx = (vy - content_area.y) as usize;
                            if map_idx < viewport_h {
                                visual_line_map[map_idx] = Some(abs_row);
                            }
                        }

                        current_y += row_h;
                    }
                }
            }
            BlockKind::ImageWithProtocol | BlockKind::ImageWithError(_) | BlockKind::ImageFallback(_) => {
                for row in 0..visible_rows {
                    let abs_row = block_start + visible_start + row;
                    let is_hovered = hover_line == Some(abs_row);
                    let ln_style = if is_hovered {
                        lineno_style.bg(hover_bg)
                    } else {
                        *lineno_style
                    };
                    let sp_style = if is_hovered {
                        sep_style.bg(hover_bg)
                    } else {
                        *sep_style
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

                    // Record visual-row-to-logical-line mapping for image rows
                    let map_idx = (sy + row as u16 - content_area.y) as usize;
                    if map_idx < viewport_h {
                        visual_line_map[map_idx] = Some(abs_row);
                    }
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

    // Store the visual line map for mouse handling.
    // Convert Option<usize> to usize, using usize::MAX as sentinel for unmapped rows.
    let map: Vec<usize> = visual_line_map.into_iter()
        .map(|v| v.unwrap_or(usize::MAX))
        .collect();
    app.set_visual_line_map(map);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect, theme: &ThemeConfig) {
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
        let split_indicator = if app.split_view() { " [Split]" } else { "" };
        let scroll_info = format!(
            " {} | {}/{}{} ",
            app.file_path_display(),
            app.scroll_offset() + 1,
            app.total_lines(),
            split_indicator,
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

    frame.render_widget(status_bar, area);
}

fn render_help_overlay(frame: &mut Frame, app: &App, theme: &ThemeConfig) {
    let area = frame.area();
    let help_w = 50u16.min(area.width.saturating_sub(4));
    let help_h = 20u16.min(area.height.saturating_sub(4));
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
        ("Split View", fmt_keys(&kb.toggle_split_view)),
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

fn render_frontmatter_popup(frame: &mut Frame, app: &App, theme: &ThemeConfig) {
    let idx = match app.frontmatter_popup_index() {
        Some(i) => i,
        None => return,
    };
    let entries = app.frontmatter_entries();
    let (key, value) = match entries.get(idx) {
        Some(entry) => entry,
        None => return,
    };

    let area = frame.area();
    let popup_w = (area.width * 80 / 100).max(30).min(area.width.saturating_sub(4));

    // Split value into lines for display
    let value_lines: Vec<&str> = value.lines().collect();
    let content_height = value_lines.len().max(1) as u16 + 4; // +4 for border, title padding, footer
    let popup_h = content_height.min(area.height.saturating_sub(4));

    let popup_area = Rect {
        x: (area.width.saturating_sub(popup_w)) / 2,
        y: (area.height.saturating_sub(popup_h)) / 2,
        width: popup_w,
        height: popup_h,
    };

    let bg = theme.status_bar_bg.0;
    let fg = theme.status_bar_fg.0;

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(""));
    for vl in &value_lines {
        lines.push(Line::from(Span::styled(
            format!("  {vl}"),
            Style::default().fg(fg),
        )));
    }
    if value_lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (empty)".to_string(),
            Style::default().fg(theme.line_number.0),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press any key to close".to_string(),
        Style::default().fg(theme.line_number.0),
    )));

    let title = format!(" {} ", key);
    let popup_block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .style(Style::default().bg(bg).fg(fg));

    let scroll = app.frontmatter_popup_scroll();
    let paragraph = Paragraph::new(lines)
        .block(popup_block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    frame.render_widget(Clear, popup_area);
    frame.render_widget(paragraph, popup_area);
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
