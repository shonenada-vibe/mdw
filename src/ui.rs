use unicode_width::UnicodeWidthStr;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui_image::StatefulImage;

use crate::config::ThemeConfig;
use crate::content::ContentBlock;

use crate::app::{App, Selection};

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
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    app.set_viewport_height(chunks[0].height);

    let theme = app.config().theme.clone();
    let body_area = chunks[0];
    let content_area = if app.file_tree_view() {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Min(20)])
            .split(body_area);
        render_file_tree_panel(frame, app, panes[0], &theme);
        panes[1]
    } else {
        body_area
    };
    if app.split_view() {
        render_split_view(frame, app, content_area, &theme);
    } else {
        render_single_view(frame, app, content_area, &theme);
    }

    // Scrollbar
    let mut scrollbar_state =
        ScrollbarState::new(app.total_lines()).position(app.scroll_offset() as usize);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    frame.render_stateful_widget(scrollbar, content_area, &mut scrollbar_state);

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

    // Toast notification overlay (top center)
    if let Some(msg) = app.toast_message() {
        render_toast(frame, msg, &theme);
    }
}

fn render_file_tree_panel(frame: &mut Frame, app: &App, area: Rect, theme: &ThemeConfig) {
    let block = Block::default()
        .title(format!(" Files: {} ", app.file_tree().root().display()))
        .borders(Borders::ALL)
        .style(Style::default().fg(theme.status_bar_fg.0));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let selected = app.file_tree_selected();
    let scroll = app.file_tree_scroll();
    let visible = inner.height as usize;
    let cursor_style = Style::default()
        .bg(theme.selection_bg.0)
        .fg(theme.selection_fg.0)
        .add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(theme.status_bar_fg.0);
    let dir_style = Style::default().fg(theme.heading2.0);

    let lines: Vec<Line<'static>> = app
        .file_tree()
        .entries()
        .iter()
        .skip(scroll)
        .take(visible)
        .enumerate()
        .map(|(offset, entry)| {
            let index = scroll + offset;
            let marker = if index == selected { ">" } else { " " };
            let indent = "  ".repeat(entry.depth);
            let label = if entry.is_dir {
                format!("{}/", entry.name)
            } else {
                entry.name.clone()
            };
            let style = if index == selected {
                cursor_style
            } else if entry.is_dir {
                dir_style
            } else {
                normal_style
            };

            Line::from(Span::styled(format!("{marker}{indent}{label}"), style))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
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
    app.set_content_area(rendered_area.x, rendered_area.width);

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
    let gutter_width = if total_raw_lines == 0 {
        1
    } else {
        total_raw_lines.ilog10() as usize + 1
    };
    let scroll_offset = app.scroll_offset() as usize;
    let viewport_height = area.height as usize;

    let lineno_style = Style::default().fg(theme.line_number.0);
    let sep_style = Style::default().fg(theme.line_number.0);
    let code_style = Style::default().fg(theme.code_block_fg.0);

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
    let cursor_line = app.cursor_line();
    let cursor_col = app.cursor_col();
    let line_wrap = app.config().behavior.line_wrap;
    let selection = app.selection().cloned();

    let lineno_style = Style::default().fg(theme.line_number.0);
    let sep_style = Style::default().fg(theme.line_number.0);
    let hover_bg = theme.hover_bg.0;

    let visible_blocks =
        compute_visible_blocks(app.content_blocks(), scroll_offset, viewport_height, area);
    let gutter_total_width = gutter_width + 3;

    render_blocks(
        frame,
        app,
        &visible_blocks,
        area,
        gutter_width,
        gutter_total_width,
        &lineno_style,
        &sep_style,
        hover_bg,
        hover_line,
        cursor_line,
        cursor_col,
        has_search,
        &search_matches,
        &search_query,
        theme,
        line_wrap,
        &selection,
    );
}

fn render_single_view(frame: &mut Frame, app: &mut App, area: Rect, theme: &ThemeConfig) {
    app.set_content_area(area.x, area.width);
    let scroll_offset = app.scroll_offset() as usize;
    let viewport_height = area.height as usize;
    let gutter_width = app.gutter_width();
    let search_query = app.search_query().to_string();
    let has_search = !search_query.is_empty() && !app.search_matches().is_empty();
    let search_matches: Vec<usize> = app.search_matches().to_vec();
    let hover_line = app.hover_line();
    let cursor_line = app.cursor_line();
    let cursor_col = app.cursor_col();
    let line_wrap = app.config().behavior.line_wrap;
    let selection = app.selection().cloned();

    let lineno_style = Style::default().fg(theme.line_number.0);
    let sep_style = Style::default().fg(theme.line_number.0);
    let hover_bg = theme.hover_bg.0;

    let visible_blocks =
        compute_visible_blocks(app.content_blocks(), scroll_offset, viewport_height, area);
    let gutter_total_width = gutter_width + 3;

    render_blocks(
        frame,
        app,
        &visible_blocks,
        area,
        gutter_width,
        gutter_total_width,
        &lineno_style,
        &sep_style,
        hover_bg,
        hover_line,
        cursor_line,
        cursor_col,
        has_search,
        &search_matches,
        &search_query,
        theme,
        line_wrap,
        &selection,
    );
}

fn compute_visible_blocks(
    blocks: &[ContentBlock],
    scroll_offset: usize,
    viewport_height: usize,
    area: Rect,
) -> Vec<VisibleBlock> {
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
    cursor_line: Option<usize>,
    cursor_col: usize,
    has_search: bool,
    search_matches: &[usize],
    search_query: &str,
    theme: &ThemeConfig,
    line_wrap: bool,
    selection: &Option<Selection>,
) {
    // Build a visual-row-to-logical-line map for mouse handling.
    // Indexed by (screen_y - content_area.y). Value is the logical line index.
    let viewport_h = content_area.height as usize;
    let mut visual_line_map: Vec<Option<usize>> = vec![None; viewport_h];
    let cursor_line_bg = theme.code_block_bg.0;

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
                ContentBlock::Image {
                    protocol: Some(_), ..
                } => BlockKind::ImageWithProtocol,
                ContentBlock::Image {
                    error: Some(err), ..
                } => BlockKind::ImageWithError(err.clone()),
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
                        let is_cursor_line = cursor_line == Some(abs_row);
                        let row_bg = if is_cursor_line {
                            Some(cursor_line_bg)
                        } else if is_hovered {
                            Some(hover_bg)
                        } else {
                            None
                        };
                        let ln_style = row_bg.map_or(*lineno_style, |bg| lineno_style.bg(bg));
                        let sp_style = row_bg.map_or(*sep_style, |bg| sep_style.bg(bg));

                        let mut content_spans: Vec<Span<'_>> =
                            if has_search && search_matches.contains(&abs_row) {
                                highlight_search_spans(&line.spans, search_query, theme)
                            } else {
                                line.spans.iter().cloned().collect()
                            };

                        if let Some(bg) = row_bg {
                            content_spans = apply_line_background(&content_spans, bg);
                        }

                        // Apply selection highlighting
                        if let Some(sel) = selection {
                            let (sel_start, sel_end) = if sel.start.0 < sel.end.0
                                || (sel.start.0 == sel.end.0 && sel.start.1 <= sel.end.1)
                            {
                                (sel.start, sel.end)
                            } else {
                                (sel.end, sel.start)
                            };

                            if abs_row >= sel_start.0 && abs_row <= sel_end.0 {
                                let col_start = if abs_row == sel_start.0 {
                                    sel_start.1
                                } else {
                                    0
                                };
                                let col_end = if abs_row == sel_end.0 {
                                    sel_end.1
                                } else {
                                    usize::MAX
                                };
                                if col_start < col_end {
                                    content_spans = highlight_selection_spans(
                                        &content_spans,
                                        col_start,
                                        col_end,
                                        theme,
                                    );
                                }
                            }
                        }

                        if is_cursor_line {
                            content_spans =
                                apply_cursor_to_spans(&content_spans, cursor_col, theme);
                        }

                        // Estimate visual rows this line will take when wrapped
                        let visual_rows = if line_wrap && content_w > 0 {
                            let display_w: usize = line
                                .spans
                                .iter()
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
            BlockKind::ImageWithProtocol
            | BlockKind::ImageWithError(_)
            | BlockKind::ImageFallback(_) => {
                for row in 0..visible_rows {
                    let abs_row = block_start + visible_start + row;
                    let is_hovered = hover_line == Some(abs_row);
                    let is_cursor_line = cursor_line == Some(abs_row);
                    let row_bg = if is_cursor_line {
                        Some(cursor_line_bg)
                    } else if is_hovered {
                        Some(hover_bg)
                    } else {
                        None
                    };
                    let ln_style = row_bg.map_or(*lineno_style, |bg| lineno_style.bg(bg));
                    let sp_style = row_bg.map_or(*sep_style, |bg| sep_style.bg(bg));

                    let gutter_line = Line::from(vec![
                        Span::styled(format!("{:>width$} ", "~", width = gutter_width), ln_style),
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
                        if let ContentBlock::Image {
                            protocol: Some(proto),
                            ..
                        } = &mut blocks_mut[block_idx]
                        {
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

                if cursor_line.is_some_and(|line| {
                    line >= block_start + visible_start
                        && line < block_start + visible_start + visible_rows
                }) {
                    let cursor_rect = Rect {
                        x: image_rect.x,
                        y: sy + (cursor_line.unwrap() - (block_start + visible_start)) as u16,
                        width: 1.min(image_rect.width),
                        height: 1,
                    };
                    frame.render_widget(
                        Paragraph::new(" ").style(
                            Style::default()
                                .fg(theme.selection_fg.0)
                                .bg(theme.selection_bg.0),
                        ),
                        cursor_rect,
                    );
                }
            }
        }
    }

    // Store the visual line map for mouse handling.
    // Convert Option<usize> to usize, using usize::MAX as sentinel for unmapped rows.
    let map: Vec<usize> = visual_line_map
        .into_iter()
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
        let tree_indicator = if app.file_tree_view() { " [Tree]" } else { "" };
        let visual_indicator = if app.visual_mode() { " [VISUAL]" } else { "" };
        let cursor_indicator = app
            .cursor_line()
            .map(|line| format!(" | {}:{}", line + 1, app.cursor_col() + 1))
            .unwrap_or_default();
        let scroll_info = format!(
            " {} | {}/{}{}{}{}{} ",
            app.file_path_display(),
            app.scroll_offset() + 1,
            app.total_lines(),
            split_indicator,
            tree_indicator,
            visual_indicator,
            cursor_indicator,
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
    let help_h = 24u16.min(area.height.saturating_sub(4));
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
                if c.modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                {
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
        ("Cursor Down", fmt_keys(&kb.scroll_down)),
        ("Cursor Up", fmt_keys(&kb.scroll_up)),
        ("Cursor Left", fmt_keys(&kb.cursor_left)),
        ("Cursor Right", fmt_keys(&kb.cursor_right)),
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
        ("Markmap", fmt_keys(&kb.toggle_markmap)),
        ("File Tree", fmt_keys(&kb.toggle_file_tree)),
        ("Tree Parent", fmt_keys(&kb.file_tree_parent)),
        ("Visual Mode", fmt_keys(&kb.toggle_visual_mode)),
        ("Activate", "Enter".to_string()),
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
    let popup_w = (area.width * 80 / 100)
        .max(30)
        .min(area.width.saturating_sub(4));

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

fn apply_line_background<'a>(spans: &[Span<'a>], bg: ratatui::style::Color) -> Vec<Span<'a>> {
    spans
        .iter()
        .map(|span| Span::styled(span.content.clone(), span.style.bg(bg)))
        .collect()
}

fn apply_cursor_to_spans<'a>(
    spans: &[Span<'a>],
    cursor_col: usize,
    theme: &ThemeConfig,
) -> Vec<Span<'a>> {
    let cursor_style = Style::default()
        .fg(theme.selection_fg.0)
        .bg(theme.selection_bg.0)
        .add_modifier(Modifier::BOLD);

    let mut result = Vec::new();
    let mut current_col = 0usize;
    let mut applied = false;

    for span in spans {
        let text = span.content.as_ref();
        if text.is_empty() {
            result.push(span.clone());
            continue;
        }

        let mut before = String::new();
        let mut cursor = String::new();
        let mut after = String::new();

        for ch in text.chars() {
            let width = UnicodeWidthStr::width(ch.to_string().as_str()).max(1);
            if !applied && cursor_col >= current_col && cursor_col < current_col + width {
                cursor.push(ch);
                applied = true;
            } else if !applied && current_col + width <= cursor_col {
                before.push(ch);
            } else {
                after.push(ch);
            }
            current_col += width;
        }

        let was_empty = before.is_empty() && cursor.is_empty() && after.is_empty();
        if !before.is_empty() {
            result.push(Span::styled(before, span.style));
        }
        if !cursor.is_empty() {
            result.push(Span::styled(cursor, cursor_style));
        }
        if !after.is_empty() {
            result.push(Span::styled(after, span.style));
        }
        if was_empty {
            result.push(span.clone());
        }
    }

    if !applied {
        result.push(Span::styled(" ".to_string(), cursor_style));
    }

    result
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
                result.push(Span::styled(text[last..start].to_string(), span.style));
            }
            result.push(Span::styled(text[start..end].to_string(), highlight_style));
            last = end;
        }

        if last < text.len() {
            result.push(Span::styled(text[last..].to_string(), span.style));
        } else if last == 0 {
            result.push(span.clone());
        }
    }

    result
}

fn render_toast(frame: &mut Frame, msg: &str, theme: &ThemeConfig) {
    let area = frame.area();
    let msg_width = UnicodeWidthStr::width(msg);
    let toast_w = (msg_width as u16 + 4).min(area.width);
    let toast_area = Rect {
        x: (area.width.saturating_sub(toast_w)) / 2,
        y: 0,
        width: toast_w,
        height: 1,
    };

    let style = Style::default()
        .fg(theme.selection_fg.0)
        .bg(theme.selection_bg.0)
        .add_modifier(Modifier::BOLD);

    let toast = Paragraph::new(Line::from(Span::styled(format!(" {msg} "), style)))
        .alignment(Alignment::Center);

    frame.render_widget(Clear, toast_area);
    frame.render_widget(toast, toast_area);
}

fn highlight_selection_spans<'a>(
    spans: &[Span<'a>],
    sel_start_col: usize,
    sel_end_col: usize,
    theme: &ThemeConfig,
) -> Vec<Span<'a>> {
    let selection_style = Style::default()
        .fg(theme.selection_fg.0)
        .bg(theme.selection_bg.0);

    let mut result = Vec::new();
    let mut current_col: usize = 0;

    for span in spans {
        let text = span.content.as_ref();
        let span_width = UnicodeWidthStr::width(text);
        let span_start = current_col;
        let span_end = current_col + span_width;

        if span_end <= sel_start_col || span_start >= sel_end_col {
            // Entirely outside selection
            result.push(span.clone());
        } else if span_start >= sel_start_col && span_end <= sel_end_col {
            // Entirely inside selection
            result.push(Span::styled(text.to_string(), selection_style));
        } else {
            // Partial overlap — split by character
            let mut chars_before = String::new();
            let mut chars_selected = String::new();
            let mut chars_after = String::new();
            let mut col = span_start;

            for ch in text.chars() {
                let w = UnicodeWidthStr::width(ch.to_string().as_str());
                if col + w <= sel_start_col {
                    chars_before.push(ch);
                } else if col >= sel_end_col {
                    chars_after.push(ch);
                } else {
                    chars_selected.push(ch);
                }
                col += w;
            }

            if !chars_before.is_empty() {
                result.push(Span::styled(chars_before, span.style));
            }
            if !chars_selected.is_empty() {
                result.push(Span::styled(chars_selected, selection_style));
            }
            if !chars_after.is_empty() {
                result.push(Span::styled(chars_after, span.style));
            }
        }

        current_col = span_end;
    }

    result
}
