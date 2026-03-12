use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Context;
use ratatui::DefaultTerminal;
use ratatui_image::picker::Picker;

use unicode_width::UnicodeWidthStr;

use crate::config::{Action, Config};
use crate::content::ContentBlock;
use crate::d2;
use crate::event::{AppEvent, EventHandler};
use crate::image_loader;
use crate::markdown;
use crate::markdown::LinkInfo;
use crate::mermaid;
use crate::ui;
use crate::watcher;

#[derive(Clone, Debug)]
pub struct Selection {
    pub start: (usize, usize), // (logical_line, content_col)
    pub end: (usize, usize),
}

pub struct App {
    file_path: PathBuf,
    is_stdin: bool,
    is_markdown: bool,
    is_json: bool,
    is_mermaid: bool,
    is_d2: bool,
    raw_content: String,
    content_blocks: Vec<ContentBlock>,
    total_lines: usize,
    scroll_offset: u16,
    viewport_height: u16,
    should_quit: bool,
    show_help: bool,
    status_message: String,
    config: Config,
    search_mode: bool,
    search_query: String,
    search_matches: Vec<usize>,
    current_match: Option<usize>,
    link_infos: Vec<LinkInfo>,
    footnote_def_lines: HashMap<String, usize>,
    gutter_width: usize,
    hover_line: Option<usize>,
    picker: Option<Picker>,
    base_dir: PathBuf,
    split_view: bool,
    /// Maps visual row (0-based from viewport top) to logical line index.
    /// Built during rendering when line_wrap is enabled.
    visual_line_map: Vec<usize>,
    frontmatter_entries: Vec<(String, String)>,
    frontmatter_popup_index: Option<usize>,
    frontmatter_popup_scroll: u16,
    selection: Option<Selection>,
    toast_message: Option<String>,
    toast_start: Option<Instant>,
}

impl App {
    pub fn new(file_path: PathBuf, config: Config, picker: Option<Picker>) -> anyhow::Result<Self> {
        let ext = file_path.extension().and_then(|e| e.to_str());
        let is_markdown = ext.is_some_and(|e| matches!(e, "md" | "markdown" | "mdx"));
        let is_json = ext.is_some_and(|e| e == "json");
        let is_mermaid = ext.is_some_and(|e| e == "mermaid");
        let is_d2 = ext.is_some_and(|e| e == "d2");

        let base_dir = file_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut app = App {
            file_path,
            is_stdin: false,
            is_markdown,
            is_json,
            is_mermaid,
            is_d2,
            raw_content: String::new(),
            content_blocks: Vec::new(),
            total_lines: 0,
            scroll_offset: 0,
            viewport_height: 0,
            should_quit: false,
            show_help: false,
            status_message: String::new(),
            config,
            search_mode: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match: None,
            link_infos: Vec::new(),
            footnote_def_lines: HashMap::new(),
            gutter_width: 0,
            hover_line: None,
            picker,
            base_dir,
            split_view: false,
            visual_line_map: Vec::new(),
            frontmatter_entries: Vec::new(),
            frontmatter_popup_index: None,
            frontmatter_popup_scroll: 0,
            selection: None,
            toast_message: None,
            toast_start: None,
        };

        app.reload_file()?;
        Ok(app)
    }

    pub fn from_stdin(content: String, config: Config, picker: Option<Picker>) -> anyhow::Result<Self> {
        let mut app = App {
            file_path: PathBuf::from("stdin"),
            is_stdin: true,
            is_markdown: true,
            is_json: false,
            is_mermaid: false,
            is_d2: false,
            raw_content: String::new(),
            content_blocks: Vec::new(),
            total_lines: 0,
            scroll_offset: 0,
            viewport_height: 0,
            should_quit: false,
            show_help: false,
            status_message: String::new(),
            config,
            search_mode: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match: None,
            link_infos: Vec::new(),
            footnote_def_lines: HashMap::new(),
            gutter_width: 0,
            hover_line: None,
            picker,
            base_dir: PathBuf::from("."),
            split_view: false,
            visual_line_map: Vec::new(),
            frontmatter_entries: Vec::new(),
            frontmatter_popup_index: None,
            frontmatter_popup_scroll: 0,
            selection: None,
            toast_message: None,
            toast_start: None,
        };

        app.raw_content = content;
        app.render_content();
        Ok(app)
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;

        let (event_handler, tx) = EventHandler::new(Duration::from_millis(250));

        let _watcher = if self.is_stdin {
            None
        } else {
            let canonical = self.file_path.canonicalize()
                .with_context(|| format!("Failed to canonicalize path: {}", self.file_path.display()))?;
            Some(watcher::setup_watcher(&canonical, tx, self.config.behavior.debounce_ms)?)
        };

        while !self.should_quit {
            terminal.draw(|frame| ui::render(frame, self))?;

            // Block for the first event, then drain all pending events before re-rendering
            let first = event_handler.next()?;
            self.process_event(first);
            while let Some(event) = event_handler.try_next() {
                self.process_event(event);
                if self.should_quit {
                    break;
                }
            }
        }

        crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;
        Ok(())
    }

    fn process_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Mouse(mouse) => self.handle_mouse(mouse),
            AppEvent::FileChanged => {
                if !self.is_stdin {
                    if let Err(e) = self.reload_file() {
                        self.status_message = format!("Error: {e}");
                    } else {
                        self.status_message = "Reloaded".to_string();
                    }
                }
            }
            AppEvent::Resize => {
                self.recompute_image_heights();
            }
            AppEvent::Tick => {
                if let Some(start) = self.toast_start {
                    if start.elapsed() >= Duration::from_secs(2) {
                        self.toast_message = None;
                        self.toast_start = None;
                    }
                }
            }
        }
    }

    fn reload_file(&mut self) -> anyhow::Result<()> {
        self.raw_content = std::fs::read_to_string(&self.file_path)
            .with_context(|| format!("Failed to read {}", self.file_path.display()))?;
        self.render_content();
        Ok(())
    }

    fn render_content(&mut self) {
        self.link_infos.clear();
        self.footnote_def_lines.clear();
        self.content_blocks.clear();
        self.frontmatter_entries.clear();
        self.frontmatter_popup_index = None;
        self.frontmatter_popup_scroll = 0;

        if self.is_markdown {
            let (blocks, links, footnote_defs, fm_entries) = markdown::render_markdown(&self.raw_content, &self.config.theme);
            self.content_blocks = blocks;
            self.link_infos = links;
            self.footnote_def_lines = footnote_defs;
            self.frontmatter_entries = fm_entries;
            self.load_images();
        } else {
            let text = if self.is_json {
                markdown::render_json(&self.raw_content, &self.config.theme)
            } else if self.is_mermaid {
                mermaid::render_mermaid(&self.raw_content, &self.config.theme)
            } else if self.is_d2 {
                d2::render_d2(&self.raw_content, &self.config.theme)
            } else {
                markdown::render_plain(&self.raw_content)
            };
            self.content_blocks = vec![ContentBlock::Text { lines: text.lines.into_iter().collect() }];
        }

        self.compute_total_lines();
        self.clamp_scroll();
    }

    fn load_images(&mut self) {
        for block in &mut self.content_blocks {
            if let ContentBlock::Image { source, protocol, error, display_height, .. } = block {
                match image_loader::load_image(source, &self.base_dir) {
                    Ok(img) => {
                        if let Some(ref mut picker) = self.picker {
                            let font_size = picker.font_size();
                            let cols = 80u16;
                            *display_height = image_loader::compute_display_height(&img, cols, font_size);
                            *protocol = Some(picker.new_resize_protocol(img));
                        } else {
                            *error = Some("No image protocol available".to_string());
                            *display_height = 1;
                        }
                    }
                    Err(e) => {
                        *error = Some(e);
                        *display_height = 1;
                    }
                }
            }
        }
    }

    fn recompute_image_heights(&mut self) {
        let font_size = match &self.picker {
            Some(p) => p.font_size(),
            None => return,
        };
        // Estimate available content columns (viewport width minus gutter)
        let cols = 80u16; // Will be updated in render
        for block in &mut self.content_blocks {
            if let ContentBlock::Image { source, display_height, protocol, .. } = block
                && protocol.is_some()
                && let Ok(img) = image_loader::load_image(source, &self.base_dir)
            {
                *display_height = image_loader::compute_display_height(&img, cols, font_size);
            }
        }
        self.compute_total_lines();
        self.clamp_scroll();
    }

    fn compute_total_lines(&mut self) {
        self.total_lines = 0;
        for block in &self.content_blocks {
            match block {
                ContentBlock::Text { lines } => self.total_lines += lines.len(),
                ContentBlock::Image { display_height, .. } => self.total_lines += *display_height as usize,
            }
        }
        self.gutter_width = if self.total_lines == 0 { 1 } else { self.total_lines.ilog10() as usize + 1 };
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};

        if self.search_mode {
            match key.code {
                KeyCode::Esc => {
                    self.search_mode = false;
                    self.search_query.clear();
                    self.search_matches.clear();
                    self.current_match = None;
                    self.status_message.clear();
                }
                KeyCode::Enter => {
                    self.search_mode = false;
                    if !self.search_query.is_empty() {
                        self.execute_search();
                        if self.search_matches.is_empty() {
                            self.status_message = format!("Pattern not found: {}", self.search_query);
                        }
                    }
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Char(c) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                        self.search_mode = false;
                        self.search_query.clear();
                        self.search_matches.clear();
                        self.current_match = None;
                        self.status_message.clear();
                    } else {
                        self.search_query.push(c);
                    }
                }
                _ => {}
            }
            return;
        }

        // Ctrl-C copies selection if active, instead of quitting
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('c')
            && self.selection.is_some()
        {
            self.copy_selection();
            return;
        }

        if self.frontmatter_popup_index.is_some() {
            self.frontmatter_popup_index = None;
            return;
        }

        let Some(action) = self.config.keybindings.resolve_action(&key) else {
            if self.show_help {
                self.show_help = false;
            }
            return;
        };

        if self.show_help {
            match action {
                Action::ToggleHelp | Action::Quit => {
                    self.show_help = false;
                    return;
                }
                _ => {
                    self.show_help = false;
                    return;
                }
            }
        }

        let scroll_speed = self.config.behavior.scroll_speed;

        match action {
            Action::Quit => self.should_quit = true,
            Action::ScrollDown => self.scroll_down(scroll_speed),
            Action::ScrollUp => self.scroll_up(scroll_speed),
            Action::HalfPageDown => {
                let half = (self.viewport_height / 2).max(1);
                self.scroll_down(half as usize);
            }
            Action::HalfPageUp => {
                let half = (self.viewport_height / 2).max(1);
                self.scroll_up(half as usize);
            }
            Action::PageDown => {
                self.scroll_down(self.viewport_height as usize);
            }
            Action::PageUp => {
                self.scroll_up(self.viewport_height as usize);
            }
            Action::Top => {
                self.scroll_offset = 0;
            }
            Action::Bottom => {
                self.scroll_to_bottom();
            }
            Action::ToggleHelp => {
                self.show_help = true;
            }
            Action::SearchForward => {
                self.search_mode = true;
                self.search_query.clear();
                self.search_matches.clear();
                self.current_match = None;
                self.status_message.clear();
            }
            Action::SearchNext => {
                self.jump_to_next_match();
            }
            Action::SearchPrev => {
                self.jump_to_prev_match();
            }
            Action::ToggleSplitView => {
                self.split_view = !self.split_view;
            }
        }
    }

    fn execute_search(&mut self) {
        self.search_matches.clear();
        self.current_match = None;

        if self.search_query.is_empty() {
            return;
        }

        let query_lower = self.search_query.to_lowercase();
        let mut row_offset = 0usize;

        for block in &self.content_blocks {
            match block {
                ContentBlock::Text { lines } => {
                    for (i, line) in lines.iter().enumerate() {
                        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                        if line_text.to_lowercase().contains(&query_lower) {
                            self.search_matches.push(row_offset + i);
                        }
                    }
                    row_offset += lines.len();
                }
                ContentBlock::Image { display_height, .. } => {
                    row_offset += *display_height as usize;
                }
            }
        }

        if !self.search_matches.is_empty() {
            let current_pos = self.scroll_offset as usize;
            let idx = self.search_matches
                .iter()
                .position(|&line| line >= current_pos)
                .unwrap_or(0);
            self.current_match = Some(idx);
            self.scroll_to_match(idx);
            self.update_search_status();
        }
    }

    fn jump_to_next_match(&mut self) {
        if self.search_matches.is_empty() {
            if !self.search_query.is_empty() {
                self.status_message = format!("Pattern not found: {}", self.search_query);
            }
            return;
        }
        let idx = match self.current_match {
            Some(i) => (i + 1) % self.search_matches.len(),
            None => 0,
        };
        self.current_match = Some(idx);
        self.scroll_to_match(idx);
        self.update_search_status();
    }

    fn jump_to_prev_match(&mut self) {
        if self.search_matches.is_empty() {
            if !self.search_query.is_empty() {
                self.status_message = format!("Pattern not found: {}", self.search_query);
            }
            return;
        }
        let idx = match self.current_match {
            Some(0) => self.search_matches.len() - 1,
            Some(i) => i - 1,
            None => self.search_matches.len() - 1,
        };
        self.current_match = Some(idx);
        self.scroll_to_match(idx);
        self.update_search_status();
    }

    fn scroll_to_match(&mut self, idx: usize) {
        let line = self.search_matches[idx] as u16;
        if line < self.scroll_offset || line >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = line.saturating_sub(self.viewport_height / 4);
            self.clamp_scroll();
        }
    }

    fn update_search_status(&mut self) {
        if let Some(idx) = self.current_match {
            self.status_message = format!(
                "/{} [{}/{}]",
                self.search_query,
                idx + 1,
                self.search_matches.len()
            );
        }
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseEventKind, MouseButton};

        // When frontmatter popup is open, capture scroll events for the popup
        if self.frontmatter_popup_index.is_some() {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.frontmatter_popup_scroll = self.frontmatter_popup_scroll.saturating_add(
                        self.config.behavior.scroll_speed as u16,
                    );
                }
                MouseEventKind::ScrollUp => {
                    self.frontmatter_popup_scroll = self.frontmatter_popup_scroll.saturating_sub(
                        self.config.behavior.scroll_speed as u16,
                    );
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    self.frontmatter_popup_index = None;
                    self.frontmatter_popup_scroll = 0;
                }
                _ => {}
            }
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let click_row = mouse.row;
                let click_col = mouse.column as usize;

                let gutter_total = self.gutter_width + 3;
                if click_col < gutter_total {
                    self.selection = None;
                    return;
                }
                let content_col = click_col - gutter_total;

                let content_line = match self.visual_row_to_line(click_row as usize) {
                    Some(line) => line,
                    None => {
                        self.selection = None;
                        return;
                    }
                };

                if let Some(url) = self.find_link_at(content_line, content_col) {
                    self.selection = None;
                    if let Some(label) = url.strip_prefix("#footnote:") {
                        if let Some(&target_line) = self.footnote_def_lines.get(label) {
                            self.scroll_offset = (target_line as u16).saturating_sub(self.viewport_height / 4);
                            self.clamp_scroll();
                            self.status_message = format!("Jumped to footnote [{label}]");
                        }
                    } else if let Some(idx_str) = url.strip_prefix("#frontmatter:") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if idx < self.frontmatter_entries.len() {
                                self.frontmatter_popup_index = Some(idx);
                                self.frontmatter_popup_scroll = 0;
                            }
                        }
                    } else {
                        self.status_message = format!("Opening: {url}");
                        let _ = open_url(&url);
                    }
                } else {
                    // Start text selection
                    self.selection = Some(Selection {
                        start: (content_line, content_col),
                        end: (content_line, content_col),
                    });
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let click_col = mouse.column as usize;
                let gutter_total = self.gutter_width + 3;
                let content_col = click_col.saturating_sub(gutter_total);

                if let Some(content_line) = self.visual_row_to_line(mouse.row as usize) {
                    if let Some(ref mut sel) = self.selection {
                        sel.end = (content_line, content_col);
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                self.copy_selection();
            }
            MouseEventKind::ScrollDown if self.config.behavior.mouse_scroll => {
                self.scroll_down(self.config.behavior.scroll_speed);
            }
            MouseEventKind::ScrollUp if self.config.behavior.mouse_scroll => {
                self.scroll_up(self.config.behavior.scroll_speed);
            }
            MouseEventKind::Moved => {
                self.hover_line = self.visual_row_to_line(mouse.row as usize);
            }
            _ => {}
        }
    }

    fn find_link_at(&self, line: usize, col: usize) -> Option<String> {
        for info in &self.link_infos {
            if info.line == line && col >= info.col_start && col < info.col_end {
                return Some(info.url.clone());
            }
        }
        None
    }

    fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines as u16);
        self.clamp_scroll();
    }

    fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines as u16);
    }

    fn scroll_to_bottom(&mut self) {
        if self.total_lines > 0 {
            self.scroll_offset = (self.total_lines - 1) as u16;
        } else {
            self.scroll_offset = 0;
        }
    }

    fn clamp_scroll(&mut self) {
        let max_scroll = if self.total_lines > 0 {
            (self.total_lines - 1) as u16
        } else {
            0
        };
        self.scroll_offset = self.scroll_offset.min(max_scroll);
    }

    // Accessors for ui.rs
    pub fn content_blocks(&self) -> &[ContentBlock] {
        &self.content_blocks
    }

    pub fn content_blocks_mut(&mut self) -> &mut [ContentBlock] {
        &mut self.content_blocks
    }

    pub fn scroll_offset(&self) -> u16 {
        self.scroll_offset
    }

    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    pub fn file_path_display(&self) -> String {
        self.file_path.display().to_string()
    }

    pub fn status_message(&self) -> &str {
        &self.status_message
    }

    pub fn set_viewport_height(&mut self, height: u16) {
        self.viewport_height = height;
    }

    pub fn show_help(&self) -> bool {
        self.show_help
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn search_mode(&self) -> bool {
        self.search_mode
    }

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    pub fn search_matches(&self) -> &[usize] {
        &self.search_matches
    }

    pub fn hover_line(&self) -> Option<usize> {
        self.hover_line
    }

    pub fn gutter_width(&self) -> usize {
        self.gutter_width
    }

    pub fn split_view(&self) -> bool {
        self.split_view
    }

    pub fn raw_content(&self) -> &str {
        &self.raw_content
    }

    pub fn set_visual_line_map(&mut self, map: Vec<usize>) {
        self.visual_line_map = map;
    }

    pub fn frontmatter_popup_index(&self) -> Option<usize> {
        self.frontmatter_popup_index
    }

    pub fn frontmatter_entries(&self) -> &[(String, String)] {
        &self.frontmatter_entries
    }

    pub fn frontmatter_popup_scroll(&self) -> u16 {
        self.frontmatter_popup_scroll
    }

    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    pub fn toast_message(&self) -> Option<&str> {
        self.toast_message.as_deref()
    }

    fn show_toast(&mut self, msg: String) {
        self.toast_message = Some(msg);
        self.toast_start = Some(Instant::now());
    }

    fn copy_selection(&mut self) {
        if self.selection.is_none() {
            return;
        }
        let text = self.extract_selected_text();
        if !text.is_empty() {
            match copy_to_clipboard(&text) {
                Ok(()) => {
                    self.show_toast(format!("Copied {} chars", text.len()));
                }
                Err(e) => {
                    self.show_toast(format!("Copy failed: {e}"));
                }
            }
        }
        self.selection = None;
    }

    fn extract_selected_text(&self) -> String {
        let sel = match &self.selection {
            Some(s) => s,
            None => return String::new(),
        };

        // Normalize so start <= end
        let (start, end) = if sel.start.0 < sel.end.0
            || (sel.start.0 == sel.end.0 && sel.start.1 <= sel.end.1)
        {
            (sel.start, sel.end)
        } else {
            (sel.end, sel.start)
        };

        let mut result = Vec::new();
        let mut line_idx = 0usize;

        for block in &self.content_blocks {
            match block {
                ContentBlock::Text { lines } => {
                    for line in lines {
                        if line_idx >= start.0 && line_idx <= end.0 {
                            let text: String =
                                line.spans.iter().map(|s| s.content.as_ref()).collect();

                            let col_start = if line_idx == start.0 { start.1 } else { 0 };
                            let col_end = if line_idx == end.0 {
                                end.1
                            } else {
                                // full line width
                                UnicodeWidthStr::width(text.as_str())
                            };

                            // Walk characters, accumulating display width
                            let mut current_col = 0usize;
                            let mut extracted = String::new();
                            for ch in text.chars() {
                                let w = UnicodeWidthStr::width(ch.to_string().as_str());
                                if current_col + w > col_start && current_col < col_end {
                                    extracted.push(ch);
                                }
                                current_col += w;
                                if current_col >= col_end {
                                    break;
                                }
                            }
                            result.push(extracted);
                        }
                        line_idx += 1;
                        if line_idx > end.0 {
                            break;
                        }
                    }
                }
                ContentBlock::Image { display_height, .. } => {
                    line_idx += *display_height as usize;
                }
            }
            if line_idx > end.0 {
                break;
            }
        }

        result.join("\n")
    }

    /// Map a screen row to a logical line index.
    /// The visual_line_map is indexed by screen row (built during rendering).
    fn visual_row_to_line(&self, screen_row: usize) -> Option<usize> {
        if self.visual_line_map.is_empty() {
            // No wrap mapping; fall back to 1:1
            let line = screen_row + self.scroll_offset as usize;
            if line < self.total_lines { Some(line) } else { None }
        } else {
            match self.visual_line_map.get(screen_row) {
                Some(&v) if v != usize::MAX => Some(v),
                _ => None,
            }
        }
    }
}

fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    use std::io::Write;

    #[cfg(target_os = "macos")]
    {
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
    }
    #[cfg(target_os = "linux")]
    {
        let mut child = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
    }
    #[cfg(target_os = "windows")]
    {
        let mut child = std::process::Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
    }
    Ok(())
}

fn open_url(url: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd").args(["/C", "start", url]).spawn()?;
    }
    Ok(())
}
