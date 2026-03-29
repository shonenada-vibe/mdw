use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Context;
use ratatui::DefaultTerminal;
use ratatui_image::picker::Picker;
use ratatui_image::thread::{ResizeRequest, ThreadProtocol};
use ratatui_image::{Resize, ResizeEncodeRender};

use unicode_width::UnicodeWidthStr;

use crate::config::{Action, Config};
use crate::content::{ContentBlock, ImageSource};
use crate::d2;
use crate::event::{AppEvent, CommandResult, EventHandler, ImageLoadResult, ImageResizeResult};
use crate::file_tree::FileTree;
use crate::image_loader;
use crate::markdown;
use crate::markdown::{CodeBlockInfo, LinkInfo};
use crate::markmap;
use crate::mermaid;
use crate::mindmap;
use crate::specstory;
use crate::syntax_highlight;
use crate::ui;
use crate::watcher;

const IMAGE_RENDER_DEBOUNCE_MS: u64 = 180;

#[derive(Clone, Debug, PartialEq)]
pub enum ConsoleStatus {
    Idle,
    Running,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct Selection {
    pub start: (usize, usize), // (logical_line, content_col)
    pub end: (usize, usize),
}

pub struct App {
    file_path: PathBuf,
    has_open_file: bool,
    is_stdin: bool,
    is_markdown: bool,
    is_json: bool,
    is_mermaid: bool,
    is_d2: bool,
    is_mindmap: bool,
    is_yaml: bool,
    is_image: bool,
    raw_content: String,
    content_blocks: Vec<ContentBlock>,
    total_lines: usize,
    scroll_offset: u16,
    viewport_height: u16,
    content_x: u16,
    content_width: u16,
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
    tree_root_dir: PathBuf,
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
    toast_is_error: bool,
    markmap_view: bool,
    collapsed_markmap_nodes: HashSet<String>,
    collapsed_json_nodes: HashSet<String>,
    file_tree_view: bool,
    file_tree: FileTree,
    file_tree_selected: usize,
    file_tree_scroll: usize,
    cursor_line: Option<usize>,
    cursor_col: usize,
    visual_mode: bool,
    visual_line_mode: bool,
    visual_anchor: (usize, usize),
    code_block_infos: Vec<CodeBlockInfo>,
    console_visible: bool,
    console_output: String,
    console_status: ConsoleStatus,
    console_scroll: u16,
    console_command: String,
    confirm_prompt: Option<String>,
    confirm_code_block_content: Option<String>,
    confirm_code_block_lang: Option<String>,
    event_tx: Option<std::sync::mpsc::Sender<AppEvent>>,
    defer_image_render_until: Option<Instant>,
    diagram_cache: HashMap<u64, (image::DynamicImage, u16)>,
    spec_history_view: bool,
    is_specstory: bool,
}

impl App {
    pub fn new(
        path: PathBuf,
        config: Config,
        picker: Option<Picker>,
        start_in_file_tree: bool,
    ) -> anyhow::Result<Self> {
        let is_directory = path.is_dir();
        let tree_root_dir = if is_directory {
            path.clone()
        } else {
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(|parent| parent.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        };
        let initial_file_path = if is_directory {
            tree_root_dir.clone()
        } else {
            path
        };
        let file_tree = if start_in_file_tree {
            FileTree::read(tree_root_dir.clone())
                .unwrap_or_else(|_| FileTree::empty(tree_root_dir.clone()))
        } else {
            FileTree::empty(tree_root_dir.clone())
        };

        let mut app = App {
            file_path: initial_file_path,
            has_open_file: !is_directory,
            is_stdin: false,
            is_markdown: false,
            is_json: false,
            is_mermaid: false,
            is_d2: false,
            is_mindmap: false,
            is_yaml: false,
            is_image: false,
            raw_content: String::new(),
            content_blocks: Vec::new(),
            total_lines: 0,
            scroll_offset: 0,
            viewport_height: 0,
            content_x: 0,
            content_width: 0,
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
            base_dir: tree_root_dir.clone(),
            tree_root_dir: tree_root_dir.clone(),
            split_view: false,
            visual_line_map: Vec::new(),
            frontmatter_entries: Vec::new(),
            frontmatter_popup_index: None,
            frontmatter_popup_scroll: 0,
            selection: None,
            toast_message: None,
            toast_start: None,
            toast_is_error: false,
            markmap_view: false,
            collapsed_markmap_nodes: HashSet::new(),
            collapsed_json_nodes: HashSet::new(),
            file_tree_view: start_in_file_tree,
            file_tree,
            file_tree_selected: 0,
            file_tree_scroll: 0,
            cursor_line: None,
            cursor_col: 0,
            visual_mode: false,
            visual_line_mode: false,
            visual_anchor: (0, 0),
            code_block_infos: Vec::new(),
            console_visible: false,
            console_output: String::new(),
            console_status: ConsoleStatus::Idle,
            console_scroll: 0,
            console_command: String::new(),
            confirm_prompt: None,
            confirm_code_block_content: None,
            confirm_code_block_lang: None,
            event_tx: None,
            defer_image_render_until: None,
            diagram_cache: HashMap::new(),
            spec_history_view: false,
            is_specstory: false,
        };

        if app.has_open_file {
            app.update_file_kind_flags();
            app.reload_file()?;
        } else {
            app.render_content();
        }
        app.select_active_file_in_tree();
        if app.file_tree_view {
            app.scroll_file_tree_to_top();
        }
        Ok(app)
    }

    pub fn from_stdin(
        content: String,
        config: Config,
        picker: Option<Picker>,
    ) -> anyhow::Result<Self> {
        let mut app = App {
            file_path: PathBuf::from("stdin"),
            has_open_file: true,
            is_stdin: true,
            is_markdown: true,
            is_json: false,
            is_mermaid: false,
            is_d2: false,
            is_mindmap: false,
            is_yaml: false,
            is_image: false,
            raw_content: String::new(),
            content_blocks: Vec::new(),
            total_lines: 0,
            scroll_offset: 0,
            viewport_height: 0,
            content_x: 0,
            content_width: 0,
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
            tree_root_dir: PathBuf::from("."),
            split_view: false,
            visual_line_map: Vec::new(),
            frontmatter_entries: Vec::new(),
            frontmatter_popup_index: None,
            frontmatter_popup_scroll: 0,
            selection: None,
            toast_message: None,
            toast_start: None,
            toast_is_error: false,
            markmap_view: false,
            collapsed_markmap_nodes: HashSet::new(),
            collapsed_json_nodes: HashSet::new(),
            file_tree_view: false,
            file_tree: FileTree::empty(PathBuf::from(".")),
            file_tree_selected: 0,
            file_tree_scroll: 0,
            cursor_line: None,
            cursor_col: 0,
            visual_mode: false,
            visual_line_mode: false,
            visual_anchor: (0, 0),
            code_block_infos: Vec::new(),
            console_visible: false,
            console_output: String::new(),
            console_status: ConsoleStatus::Idle,
            console_scroll: 0,
            console_command: String::new(),
            confirm_prompt: None,
            confirm_code_block_content: None,
            confirm_code_block_lang: None,
            event_tx: None,
            defer_image_render_until: None,
            diagram_cache: HashMap::new(),
            spec_history_view: false,
            is_specstory: false,
        };

        app.raw_content = content;
        app.detect_specstory();
        app.render_content();
        Ok(app)
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;

        let (event_handler, tx) = EventHandler::new(Duration::from_millis(250));
        self.event_tx = Some(tx.clone());
        self.start_image_loading();

        let mut watched_root = if self.is_stdin {
            None
        } else {
            let canonical = self.tree_root_dir.canonicalize().with_context(|| {
                format!(
                    "Failed to canonicalize path: {}",
                    self.tree_root_dir.display()
                )
            })?;
            Some(canonical)
        };
        let mut _watcher_handle = if let Some(path) = watched_root.as_ref() {
            Some(watcher::setup_watcher(
                path,
                tx.clone(),
                self.config.behavior.debounce_ms,
            )?)
        } else {
            None
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

            if !self.is_stdin {
                match self.tree_root_dir.canonicalize() {
                    Ok(current_root) => {
                        if watched_root.as_ref() != Some(&current_root) {
                            match watcher::setup_watcher(
                                &current_root,
                                tx.clone(),
                                self.config.behavior.debounce_ms,
                            ) {
                                Ok(handle) => {
                                    _watcher_handle = Some(handle);
                                    watched_root = Some(current_root);
                                }
                                Err(e) => {
                                    self.show_error_toast(format!("File watcher error: {e}"));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        self.show_error_toast(format!("Path error: {e}"));
                    }
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
                    if self.file_tree_view {
                        self.refresh_file_tree();
                    }
                    if self.has_open_file && !self.is_image {
                        if let Err(e) = self.reload_file() {
                            self.show_error_toast(format!("Reload failed: {e}"));
                        } else {
                            self.status_message = "Reloaded".to_string();
                        }
                    }
                }
            }
            AppEvent::Resize => {
                self.defer_image_render();
                self.recompute_image_heights();
            }
            AppEvent::CommandFinished(result) => {
                self.console_output = result.output;
                self.console_status = if result.success {
                    ConsoleStatus::Success
                } else {
                    ConsoleStatus::Error
                };
            }
            AppEvent::ImageLoaded(result) => {
                self.handle_image_loaded(result);
            }
            AppEvent::ImageResized(result) => {
                self.handle_image_resized(result);
            }
            AppEvent::Tick => {
                if self
                    .defer_image_render_until
                    .is_some_and(|until| Instant::now() >= until)
                {
                    self.defer_image_render_until = None;
                }
                if let Some(start) = self.toast_start {
                    let timeout = if self.toast_is_error { 5 } else { 2 };
                    if start.elapsed() >= Duration::from_secs(timeout) {
                        self.toast_message = None;
                        self.toast_start = None;
                        self.toast_is_error = false;
                    }
                }
            }
        }
    }

    fn reload_file(&mut self) -> anyhow::Result<()> {
        if !self.has_open_file {
            self.raw_content.clear();
            self.render_content();
            return Ok(());
        }
        if self.is_image {
            // Image files are binary; skip read_to_string
            self.raw_content.clear();
            self.render_content();
            return Ok(());
        }
        self.raw_content = std::fs::read_to_string(&self.file_path)
            .with_context(|| format!("Failed to read {}", self.file_path.display()))?;
        self.detect_specstory();
        self.render_content();
        Ok(())
    }

    fn detect_specstory(&mut self) {
        let detected = specstory::is_specstory(&self.raw_content);
        self.is_specstory = detected;
        if detected && !self.spec_history_view {
            self.spec_history_view = true;
        }
    }

    fn render_content(&mut self) {
        self.link_infos.clear();
        self.footnote_def_lines.clear();
        self.content_blocks.clear();
        self.frontmatter_entries.clear();
        self.frontmatter_popup_index = None;
        self.frontmatter_popup_scroll = 0;

        if !self.has_open_file && !self.is_stdin {
            self.raw_content.clear();
            self.content_blocks = vec![ContentBlock::Text {
                lines: markdown::render_plain(
                    "Select a file from the tree to open it.\n\nPress t to hide or show the file tree.",
                )
                .lines
                .into_iter()
                .collect(),
            }];
        } else if self.is_image {
            // Use the filename only — base_dir is already the parent directory,
            // so load_image will join them correctly (avoids double-nesting).
            let image_name = self
                .file_path
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| self.file_path.clone());
            let source = ImageSource::Local(image_name);
            let alt = self
                .file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            self.content_blocks = vec![ContentBlock::Image {
                alt_text: alt,
                display_height: 1,
                protocol: None,
                error: None,
                source,
                cached_image: None,
                loading: false,
            }];
            self.start_image_loading();
        } else if self.is_markdown && self.spec_history_view {
            // Subtract gutter (line numbers + separator) and scrollbar (1 col)
            let gutter_total = self.gutter_width as u16 + 3;
            let usable_width = self.content_width.saturating_sub(gutter_total + 1);
            self.content_blocks = specstory::render_specstory(
                &self.raw_content,
                &self.config.theme,
                usable_width,
            );
            self.link_infos = vec![];
        } else if self.is_markdown && self.markmap_view {
            let result = markmap::render_markmap(
                &self.raw_content,
                &self.config.theme,
                &self.collapsed_markmap_nodes,
                0,
            );
            self.content_blocks = vec![ContentBlock::Text {
                lines: result.text.lines.into_iter().collect(),
            }];
            self.link_infos = result.link_infos;
        } else if self.is_markdown {
            let (blocks, links, footnote_defs, fm_entries, cb_infos) = markdown::render_markdown(
                &self.raw_content,
                &self.config.theme,
                &self.collapsed_markmap_nodes,
                &self.config.diagrams,
            );
            self.content_blocks = blocks;
            self.link_infos = links;
            self.footnote_def_lines = footnote_defs;
            self.frontmatter_entries = fm_entries;
            self.code_block_infos = cb_infos;
            self.start_image_loading();
            self.fixup_code_block_offsets();
        } else {
            self.code_block_infos.clear();
            let text = if self.is_json {
                let result = markdown::render_json(
                    &self.raw_content,
                    &self.config.theme,
                    &self.collapsed_json_nodes,
                    0,
                );
                self.link_infos = result.link_infos;
                result.text
            } else if (self.is_mermaid || self.is_d2)
                && self.config.diagrams.render_mode == "image"
            {
                use std::hash::{Hash, Hasher};
                let lang = if self.is_mermaid { "mermaid" } else { "d2" };
                let tool_path = if self.is_mermaid {
                    self.config.diagrams.mermaid_path.clone()
                } else {
                    self.config.diagrams.d2_path.clone()
                };
                let mut hasher = std::hash::DefaultHasher::new();
                self.raw_content.hash(&mut hasher);
                let content_hash = hasher.finish();

                self.content_blocks = vec![ContentBlock::Image {
                    alt_text: format!("{lang} diagram"),
                    display_height: 3,
                    protocol: None,
                    error: None,
                    source: ImageSource::Diagram {
                        lang: lang.to_string(),
                        content: self.raw_content.clone(),
                        content_hash,
                        tool_path,
                        background: self.config.diagrams.background.clone(),
                        cli_theme: self.config.diagrams.cli_theme.clone(),
                    },
                    cached_image: None,
                    loading: false,
                }];
                self.start_image_loading();

                self.compute_total_lines();
                self.clamp_scroll();
                self.clamp_cursor();
                self.sync_visual_selection();
                return;
            } else if self.is_mermaid {
                mermaid::render_mermaid(&self.raw_content, &self.config.theme)
            } else if self.is_d2 {
                d2::render_d2(&self.raw_content, &self.config.theme)
            } else if self.is_mindmap {
                mindmap::render_mindmap(&self.raw_content, &self.config.theme)
            } else if self.is_yaml {
                render_syntax_highlighted(&self.raw_content, "yaml")
            } else {
                markdown::render_plain(&self.raw_content)
            };
            self.content_blocks = vec![ContentBlock::Text {
                lines: text.lines.into_iter().collect(),
            }];
        }

        self.compute_total_lines();
        self.clamp_scroll();
        self.clamp_cursor();
        self.sync_visual_selection();
    }

    fn update_file_kind_flags(&mut self) {
        let ext = self.file_path.extension().and_then(|e| e.to_str());
        self.is_markdown = ext.is_some_and(|e| matches!(e, "md" | "markdown" | "mdx"));
        self.is_json = ext.is_some_and(|e| e == "json");
        self.is_mermaid = ext.is_some_and(|e| e == "mermaid");
        self.is_d2 = ext.is_some_and(|e| e == "d2");
        self.is_mindmap = ext.is_some_and(|e| e == "mm");
        self.is_yaml = ext.is_some_and(|e| matches!(e, "yaml" | "yml"));
        self.is_image = ext.is_some_and(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "tiff" | "tif" | "webp" | "pnm"
                    | "pbm" | "pgm" | "ppm"
            )
        });
        self.base_dir = self
            .file_path
            .parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_else(|| self.tree_root_dir.clone());
        if !self.is_markdown {
            self.markmap_view = false;
        }
    }

    fn refresh_file_tree(&mut self) {
        if self.file_tree.is_empty() {
            self.file_tree = FileTree::read(self.tree_root_dir.clone())
                .unwrap_or_else(|_| FileTree::empty(self.tree_root_dir.clone()));
        } else {
            self.file_tree.refresh();
        }
        self.select_active_file_in_tree();
    }

    fn select_active_file_in_tree(&mut self) {
        let selected = if self.has_open_file {
            self.file_tree
                .find_path(&self.file_path)
                .or_else(|| self.file_tree.first_file_index())
        } else {
            self.file_tree.first_file_index().or(Some(0))
        };

        self.file_tree_selected = selected.unwrap_or(0);
        self.ensure_file_tree_selection_visible();
    }

    fn ensure_file_tree_selection_visible(&mut self) {
        if self.file_tree_selected < self.file_tree_scroll {
            self.file_tree_scroll = self.file_tree_selected;
            return;
        }

        let visible = self.viewport_height.max(1) as usize;
        if self.file_tree_selected >= self.file_tree_scroll + visible {
            self.file_tree_scroll = self.file_tree_selected + 1 - visible;
        }
    }

    fn scroll_file_tree_to_top(&mut self) {
        self.file_tree_scroll = 0;
    }

    fn go_to_parent_in_file_tree(&mut self) {
        if !self.file_tree_view {
            return;
        }

        let previous_root = self.tree_root_dir.clone();
        let Some(parent) = self.tree_root_dir.parent().map(PathBuf::from) else {
            self.status_message = "Already at filesystem root".to_string();
            return;
        };

        if parent == self.tree_root_dir {
            self.status_message = "Already at filesystem root".to_string();
            return;
        }

        self.tree_root_dir = parent;
        self.file_tree = FileTree::read(self.tree_root_dir.clone())
            .unwrap_or_else(|_| FileTree::empty(self.tree_root_dir.clone()));
        if let Some(index) = self.file_tree.find_path(&previous_root) {
            self.file_tree_selected = index;
        } else {
            self.select_active_file_in_tree();
        }
        self.scroll_file_tree_to_top();
        self.status_message = format!("Tree root: {}", self.tree_root_dir.display());
    }

    fn open_selected_tree_entry(&mut self) {
        let Some(entry) = self.file_tree.get(self.file_tree_selected) else {
            return;
        };

        if entry.is_dir {
            let path = entry.path.clone();
            self.file_tree.toggle_expand(&path);
            self.ensure_file_tree_selection_visible();
            return;
        }

        if let Err(error) = self.open_file(entry.path.clone()) {
            self.show_error_toast(format!("Error: {error}"));
        }
    }

    fn open_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        self.file_path = path;
        self.has_open_file = true;
        self.update_file_kind_flags();
        self.spec_history_view = false;
        self.is_specstory = false;
        self.search_matches.clear();
        self.current_match = None;
        self.selection = None;
        self.visual_mode = false;
        self.cursor_col = 0;
        self.hover_line = None;
        self.reload_file()?;
        self.select_active_file_in_tree();
        self.status_message = format!("Opened {}", self.file_path.display());
        Ok(())
    }

    fn start_image_loading(&mut self) {
        let font_size = match &self.picker {
            Some(p) => p.font_size(),
            None => return,
        };
        let tx = match &self.event_tx {
            Some(tx) => tx.clone(),
            None => return,
        };
        let gutter_total = self.gutter_width as u16 + 3;
        let cols = if self.content_width > gutter_total {
            self.content_width - gutter_total
        } else {
            80u16
        };
        let base_dir = self.base_dir.clone();

        // Collect image blocks that need loading
        let mut to_load: Vec<(usize, ImageSource)> = Vec::new();
        for (idx, block) in self.content_blocks.iter_mut().enumerate() {
            if let ContentBlock::Image {
                source,
                loading,
                display_height,
                protocol,
                cached_image,
                error,
                ..
            } = block
            {
                // Check diagram cache first
                if let ImageSource::Diagram { content_hash, .. } = source
                    && let Some((img, height)) = self.diagram_cache.get(content_hash)
                {
                    *display_height = *height;
                    *protocol = Self::new_thread_protocol(
                        idx,
                        self.picker.as_ref(),
                        self.event_tx.as_ref(),
                        img.clone(),
                        cols,
                        *height,
                    );
                    *cached_image = Some(img.clone());
                    *error = None;
                    *loading = false;
                    continue;
                }

                let source_clone = match source {
                    ImageSource::Local(p) => ImageSource::Local(p.clone()),
                    ImageSource::Remote(u) => ImageSource::Remote(u.clone()),
                    ImageSource::Diagram {
                        lang,
                        content,
                        content_hash,
                        tool_path,
                        background,
                        cli_theme,
                    } => ImageSource::Diagram {
                        lang: lang.clone(),
                        content: content.clone(),
                        content_hash: *content_hash,
                        tool_path: tool_path.clone(),
                        background: background.clone(),
                        cli_theme: cli_theme.clone(),
                    },
                };
                *loading = true;
                to_load.push((idx, source_clone));
            }
        }

        if to_load.is_empty() {
            return;
        }

        for (block_index, source) in to_load {
            let tx = tx.clone();
            let base_dir = base_dir.clone();
            std::thread::spawn(move || {
                let result = match image_loader::load_image(&source, &base_dir) {
                    Ok(img) => {
                        let height = image_loader::compute_display_height(&img, cols, font_size);
                        Ok((img, height))
                    }
                    Err(e) => Err(e),
                };
                let _ = tx.send(AppEvent::ImageLoaded(ImageLoadResult {
                    block_index,
                    result,
                }));
            });
        }
    }

    fn handle_image_loaded(&mut self, result: ImageLoadResult) {
        let idx = result.block_index;
        if idx >= self.content_blocks.len() {
            return;
        }
        let picker = self.picker.clone();
        let event_tx = self.event_tx.clone();
        let gutter_total = self.gutter_width as u16 + 3;
        let cols = if self.content_width > gutter_total {
            self.content_width - gutter_total
        } else {
            80u16
        };

        // Extract diagram info before mutable borrow for fallback
        let diagram_info = if let ContentBlock::Image {
            source: ImageSource::Diagram {
                lang,
                content,
                content_hash,
                ..
            },
            ..
        } = &self.content_blocks[idx]
        {
            Some((lang.clone(), content.clone(), *content_hash))
        } else {
            None
        };

        if let ContentBlock::Image {
            display_height,
            protocol,
            error,
            cached_image,
            loading,
            ..
        } = &mut self.content_blocks[idx]
        {
            *loading = false;
            match result.result {
                Ok((img, height)) => {
                    *display_height = height;
                    *protocol = Self::new_thread_protocol(
                        idx,
                        picker.as_ref(),
                        event_tx.as_ref(),
                        img.clone(),
                        cols,
                        height,
                    );
                    *cached_image = Some(img.clone());
                    *error = None;
                    // Cache diagram result
                    if let Some((_, _, hash)) = &diagram_info {
                        self.diagram_cache.insert(*hash, (img, height));
                    }
                }
                Err(e) => {
                    // For diagram sources, fall back to ASCII rendering
                    if let Some((lang, content, _)) = &diagram_info {
                        let tool = match lang.as_str() {
                            "mermaid" => &self.config.diagrams.mermaid_path,
                            "d2" => &self.config.diagrams.d2_path,
                            _ => "",
                        };
                        let tip = format!(
                            "Falling back to ASCII: `{tool}` not found or failed. Install it for image rendering."
                        );
                        let rendered = match lang.as_str() {
                            "mermaid" => {
                                mermaid::render_mermaid(content, &self.config.theme)
                            }
                            "d2" => d2::render_d2(content, &self.config.theme),
                            _ => {
                                *protocol = None;
                                *error = Some(e);
                                *display_height = 1;
                                self.fixup_code_block_offsets();
                                self.compute_total_lines();
                                return;
                            }
                        };
                        self.content_blocks[idx] = ContentBlock::Text {
                            lines: rendered.lines.into_iter().collect(),
                        };
                        self.show_toast(tip);
                    } else {
                        *protocol = None;
                        *error = Some(e);
                        *display_height = 1;
                    }
                }
            }
        }
        self.fixup_code_block_offsets();
        self.compute_total_lines();
    }

    fn new_thread_protocol(
        block_index: usize,
        picker: Option<&Picker>,
        event_tx: Option<&std::sync::mpsc::Sender<AppEvent>>,
        img: image::DynamicImage,
        cols: u16,
        height: u16,
    ) -> Option<ThreadProtocol> {
        let (picker, event_tx) = match (picker, event_tx) {
            (Some(picker), Some(event_tx)) => (picker, event_tx.clone()),
            _ => return None,
        };

        let (tx_worker, rx_worker) = mpsc::channel::<ResizeRequest>();
        std::thread::spawn(move || {
            while let Ok(request) = rx_worker.recv() {
                let result = request.resize_encode().map_err(|e| e.to_string());
                let _ = event_tx.send(AppEvent::ImageResized(ImageResizeResult {
                    block_index,
                    result,
                }));
            }
        });

        let mut protocol = ThreadProtocol::new(tx_worker, Some(picker.new_resize_protocol(img)));
        protocol.resize_encode(
            &Resize::Crop(None),
            ratatui::layout::Rect::new(0, 0, cols.max(1), height.max(1)),
        );
        Some(protocol)
    }

    fn handle_image_resized(&mut self, result: ImageResizeResult) {
        let idx = result.block_index;
        if idx >= self.content_blocks.len() {
            return;
        }
        if let ContentBlock::Image {
            protocol, error, ..
        } = &mut self.content_blocks[idx]
        {
            match result.result {
                Ok(response) => {
                    if let Some(protocol) = protocol.as_mut() {
                        let _ = protocol.update_resized_protocol(response);
                    }
                    *error = None;
                }
                Err(e) => {
                    *error = Some(format!("Failed to render image: {e}"));
                }
            }
        }
    }

    fn fixup_code_block_offsets(&mut self) {
        if self.code_block_infos.is_empty() {
            return;
        }
        // Recompute absolute line indices for each code block by walking the
        // content blocks and matching code block content against rendered lines.
        // This handles image display_height changes after start_image_loading / resize.
        let mut abs_row: usize = 0;
        let mut cb_idx = 0;
        for block in &self.content_blocks {
            match block {
                ContentBlock::Text { lines } => {
                    if cb_idx < self.code_block_infos.len() {
                        // Check if any code block content appears in this text block
                        for (i, line) in lines.iter().enumerate() {
                            let text: String =
                                line.spans.iter().map(|s| s.content.as_ref()).collect();
                            // Code blocks are rendered with 2-space indent
                            let trimmed = text.strip_prefix("  ").unwrap_or(&text);
                            if cb_idx < self.code_block_infos.len() {
                                let cb = &self.code_block_infos[cb_idx];
                                let first_line = cb.content.lines().next().unwrap_or("");
                                if !first_line.is_empty() && trimmed == first_line {
                                    let content_lines = cb.content.lines().count().max(1);
                                    // code_content ends with \n, so split('\n') adds an
                                    // extra trailing empty item — mirror what the renderer does.
                                    let rendered_lines = cb.content.split('\n').count();
                                    let line_count = rendered_lines.max(content_lines);
                                    self.code_block_infos[cb_idx].start_line = abs_row + i;
                                    self.code_block_infos[cb_idx].end_line =
                                        abs_row + i + line_count - 1;
                                    cb_idx += 1;
                                }
                            }
                        }
                    }
                    abs_row += lines.len();
                }
                ContentBlock::Image { display_height, .. } => {
                    abs_row += *display_height as usize;
                }
            }
        }
    }

    fn recompute_image_heights(&mut self) {
        let font_size = match &self.picker {
            Some(p) => p.font_size(),
            None => return,
        };
        let gutter_total = self.gutter_width as u16 + 3;
        let cols = if self.content_width > gutter_total {
            self.content_width - gutter_total
        } else {
            80u16
        };
        for block in &mut self.content_blocks {
            if let ContentBlock::Image {
                display_height,
                cached_image,
                ..
            } = block
            {
                if let Some(img) = cached_image.as_ref() {
                    *display_height =
                        image_loader::compute_display_height(img, cols, font_size);
                }
            }
        }
        self.compute_total_lines();
        self.clamp_scroll();
        self.clamp_cursor();
        self.sync_visual_selection();
        self.fixup_code_block_offsets();
    }

    fn compute_total_lines(&mut self) {
        self.total_lines = 0;
        for block in &self.content_blocks {
            match block {
                ContentBlock::Text { lines } => self.total_lines += lines.len(),
                ContentBlock::Image { display_height, .. } => {
                    self.total_lines += *display_height as usize
                }
            }
        }
        self.gutter_width = if self.total_lines == 0 {
            1
        } else {
            self.total_lines.ilog10() as usize + 1
        };
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};

        let before_scroll = self.scroll_offset;

        if self.confirm_prompt.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.confirm_prompt = None;
                    let content = self.confirm_code_block_content.take();
                    let lang = self.confirm_code_block_lang.take();
                    if let Some(content) = content {
                        self.execute_code(lang.as_deref(), &content);
                    }
                }
                _ => {
                    self.confirm_prompt = None;
                    self.confirm_code_block_content = None;
                    self.confirm_code_block_lang = None;
                    self.show_toast("Cancelled".to_string());
                }
            }
            return;
        }

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
                            self.status_message =
                                format!("Pattern not found: {}", self.search_query);
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

        if key.code == KeyCode::Esc {
            if self.console_visible {
                self.console_visible = false;
                return;
            }
            if self.visual_mode {
                self.clear_visual_mode();
                return;
            }
            if self.show_help {
                self.show_help = false;
                return;
            }
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
            Action::ScrollDown => {
                if self.file_tree_view {
                    self.move_file_tree_selection(scroll_speed as isize);
                } else {
                    self.move_cursor_vertical(scroll_speed as isize);
                }
            }
            Action::ScrollUp => {
                if self.file_tree_view {
                    self.move_file_tree_selection(-(scroll_speed as isize));
                } else {
                    self.move_cursor_vertical(-(scroll_speed as isize));
                }
            }
            Action::CursorLeft => self.move_cursor_horizontal(-1),
            Action::CursorRight => self.move_cursor_horizontal(1),
            Action::CursorLineStart => self.move_cursor_to_line_start(),
            Action::CursorLineEnd => self.move_cursor_to_line_end(),
            Action::CursorWordForward => self.move_cursor_to_next_word(),
            Action::CursorWordBackward => self.move_cursor_to_previous_word(),
            Action::HalfPageDown => {
                let half = (self.viewport_height / 2).max(1);
                self.move_cursor_vertical(half as isize);
            }
            Action::HalfPageUp => {
                let half = (self.viewport_height / 2).max(1);
                self.move_cursor_vertical(-(half as isize));
            }
            Action::PageDown => {
                self.move_cursor_vertical(self.viewport_height as isize);
            }
            Action::PageUp => {
                self.move_cursor_vertical(-(self.viewport_height as isize));
            }
            Action::Top => {
                self.move_cursor_to_line(0);
            }
            Action::Bottom => {
                self.move_cursor_to_line(self.total_lines.saturating_sub(1));
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
            Action::ToggleMarkmap => {
                self.markmap_view = !self.markmap_view;
                self.render_content();
            }
            Action::ToggleFileTree => {
                self.file_tree_view = !self.file_tree_view;
                if self.file_tree_view {
                    self.refresh_file_tree();
                    self.scroll_file_tree_to_top();
                }
            }
            Action::FileTreeParent => {
                self.go_to_parent_in_file_tree();
            }
            Action::Activate => {
                if self.file_tree_view {
                    self.open_selected_tree_entry();
                } else {
                    self.activate_cursor();
                }
            }
            Action::ToggleVisualMode => {
                self.toggle_visual_mode();
            }
            Action::ToggleVisualLineMode => {
                self.toggle_visual_line_mode();
            }
            Action::RunCodeBlock => {
                self.run_code_block();
            }
            Action::RunCodeBlockSh => {
                self.run_code_block_as_sh();
            }
            Action::ToggleConsole => {
                self.console_visible = !self.console_visible;
            }
            Action::ToggleSpecHistory => {
                self.spec_history_view = !self.spec_history_view;
                self.render_content();
            }
            Action::ToggleDiagramMode => {
                if self.config.diagrams.render_mode == "image" {
                    self.config.diagrams.render_mode = "ascii".to_string();
                    self.show_toast("Diagram mode: ASCII".to_string());
                } else {
                    self.config.diagrams.render_mode = "image".to_string();
                    self.show_toast("Diagram mode: Image".to_string());
                }
                self.diagram_cache.clear();
                self.render_content();
            }
        }

        if self.scroll_offset != before_scroll {
            self.defer_image_render();
        }
    }

    fn move_file_tree_selection(&mut self, delta: isize) {
        if self.file_tree.is_empty() {
            return;
        }

        let max_index = self.file_tree.len().saturating_sub(1) as isize;
        let next = (self.file_tree_selected as isize + delta).clamp(0, max_index) as usize;
        self.file_tree_selected = next;
        self.ensure_file_tree_selection_visible();
    }

    fn move_cursor_vertical(&mut self, delta: isize) {
        if self.total_lines == 0 {
            self.cursor_line = None;
            return;
        }

        let current = self.cursor_line.unwrap_or(0) as isize;
        let max_line = self.total_lines.saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max_line) as usize;
        self.cursor_line = Some(next);
        self.clamp_cursor();
        self.ensure_cursor_visible();
        self.sync_visual_selection();
    }

    fn move_cursor_horizontal(&mut self, delta: isize) {
        let Some(line) = self.cursor_line else {
            return;
        };

        let width = self.line_width(line) as isize;
        let next = (self.cursor_col as isize + delta).clamp(0, width.max(0)) as usize;
        self.cursor_col = next;
        self.sync_visual_selection();
    }

    fn move_cursor_to_line_start(&mut self) {
        if self.cursor_line.is_none() {
            return;
        }

        self.cursor_col = 0;
        self.sync_visual_selection();
    }

    fn move_cursor_to_line_end(&mut self) {
        let Some(line) = self.cursor_line else {
            return;
        };

        self.cursor_col = self.line_end_cursor_col(line);
        self.sync_visual_selection();
    }

    fn move_cursor_to_next_word(&mut self) {
        let Some((line, col)) = self.next_word_position() else {
            return;
        };

        self.cursor_line = Some(line);
        self.cursor_col = col;
        self.ensure_cursor_visible();
        self.sync_visual_selection();
    }

    fn move_cursor_to_previous_word(&mut self) {
        let Some((line, col)) = self.previous_word_position() else {
            return;
        };

        self.cursor_line = Some(line);
        self.cursor_col = col;
        self.ensure_cursor_visible();
        self.sync_visual_selection();
    }

    fn move_cursor_to_line(&mut self, line: usize) {
        if self.total_lines == 0 {
            self.cursor_line = None;
            self.cursor_col = 0;
            return;
        }

        self.cursor_line = Some(line.min(self.total_lines - 1));
        self.clamp_cursor();
        self.ensure_cursor_visible();
        self.sync_visual_selection();
    }

    fn ensure_cursor_visible(&mut self) {
        let Some(line) = self.cursor_line else {
            return;
        };

        let viewport = self.viewport_height.max(1) as usize;
        if line < self.scroll_offset as usize {
            self.scroll_offset = line as u16;
        } else if line >= self.scroll_offset as usize + viewport {
            self.scroll_offset = line.saturating_sub(viewport.saturating_sub(1)) as u16;
        }
        self.clamp_scroll();
    }

    fn clamp_cursor(&mut self) {
        if self.total_lines == 0 {
            self.cursor_line = None;
            self.cursor_col = 0;
            return;
        }

        let line = self.cursor_line.unwrap_or(0).min(self.total_lines - 1);
        self.cursor_line = Some(line);
        self.cursor_col = self.cursor_col.min(self.line_width(line));
    }

    fn toggle_visual_mode(&mut self) {
        let Some(line) = self.cursor_line else {
            return;
        };

        if self.visual_mode {
            self.copy_selection();
            self.visual_mode = false;
            self.visual_line_mode = false;
            self.selection = None;
            return;
        }

        self.visual_mode = true;
        self.visual_anchor = (line, self.cursor_col);
        self.sync_visual_selection();
    }

    fn toggle_visual_line_mode(&mut self) {
        let Some(line) = self.cursor_line else {
            return;
        };

        if self.visual_mode {
            self.copy_selection();
            self.visual_mode = false;
            self.visual_line_mode = false;
            self.selection = None;
            return;
        }

        self.visual_mode = true;
        self.visual_line_mode = true;
        self.visual_anchor = (line, 0);
        self.sync_visual_selection();
    }

    fn clear_visual_mode(&mut self) {
        self.visual_mode = false;
        self.visual_line_mode = false;
        self.selection = None;
    }

    fn sync_visual_selection(&mut self) {
        if !self.visual_mode {
            return;
        }

        let Some(line) = self.cursor_line else {
            self.selection = None;
            return;
        };

        if self.visual_line_mode {
            let anchor_line = self.visual_anchor.0;
            let (start_line, end_line) = if line < anchor_line {
                (line, anchor_line)
            } else {
                (anchor_line, line)
            };
            let end_col = self.line_width(end_line);
            self.selection = Some(Selection {
                start: (start_line, 0),
                end: (end_line, end_col),
            });
        } else {
            let cursor = (line, self.cursor_col);
            self.selection = Some(self.build_visual_selection(self.visual_anchor, cursor));
        }
    }

    fn build_visual_selection(&self, anchor: (usize, usize), cursor: (usize, usize)) -> Selection {
        let cursor_end = (cursor.0, self.selection_end_col(cursor.0, cursor.1));
        let anchor_end = (anchor.0, self.selection_end_col(anchor.0, anchor.1));

        if cursor < anchor {
            Selection {
                start: cursor,
                end: anchor_end,
            }
        } else {
            Selection {
                start: anchor,
                end: cursor_end,
            }
        }
    }

    fn selection_end_col(&self, line: usize, col: usize) -> usize {
        let line_width = self.line_width(line);
        if col >= line_width {
            return line_width;
        }

        self.char_width_at(line, col)
            .map(|width| (col + width).min(line_width))
            .unwrap_or((col + 1).min(line_width))
    }

    fn activate_cursor(&mut self) {
        let Some(line) = self.cursor_line else {
            return;
        };

        if let Some(url) = self.find_link_at(line, self.cursor_col) {
            self.activate_url(url);
        }
    }

    fn next_word_position(&self) -> Option<(usize, usize)> {
        let start_line = self.cursor_line?;

        for line in start_line..self.total_lines {
            let starts = self.word_start_cols(line);
            if line == start_line {
                if let Some(&col) = starts.iter().find(|&&col| col > self.cursor_col) {
                    return Some((line, col));
                }
            } else if let Some(&col) = starts.first() {
                return Some((line, col));
            }
        }

        None
    }

    fn previous_word_position(&self) -> Option<(usize, usize)> {
        let start_line = self.cursor_line?;

        for line in (0..=start_line).rev() {
            let starts = self.word_start_cols(line);
            if line == start_line {
                if let Some(&col) = starts.iter().rev().find(|&&col| col < self.cursor_col) {
                    return Some((line, col));
                }
            } else if let Some(&col) = starts.last() {
                return Some((line, col));
            }
        }

        None
    }

    fn activate_url(&mut self, url: String) {
        self.selection = None;
        if let Some(label) = url.strip_prefix("#footnote:") {
            if let Some(&target_line) = self.footnote_def_lines.get(label) {
                self.move_cursor_to_line(target_line);
                self.status_message = format!("Jumped to footnote [{label}]");
            }
        } else if let Some(json_path) = url.strip_prefix("#json:") {
            let path = json_path.to_string();
            if self.collapsed_json_nodes.contains(&path) {
                self.collapsed_json_nodes.remove(&path);
            } else {
                self.collapsed_json_nodes.insert(path);
            }
            self.render_content();
        } else if let Some(node_path) = url.strip_prefix("#markmap:") {
            let path = node_path.to_string();
            if self.collapsed_markmap_nodes.contains(&path) {
                self.collapsed_markmap_nodes.remove(&path);
            } else {
                self.collapsed_markmap_nodes.insert(path);
            }
            self.render_content();
        } else if let Some(idx_str) = url.strip_prefix("#frontmatter:") {
            if let Ok(idx) = idx_str.parse::<usize>()
                && idx < self.frontmatter_entries.len()
            {
                self.frontmatter_popup_index = Some(idx);
                self.frontmatter_popup_scroll = 0;
            }
        } else {
            self.status_message = format!("Opening: {url}");
            let _ = open_url(&url);
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
                        let line_text: String =
                            line.spans.iter().map(|s| s.content.as_ref()).collect();
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
            let current_pos = self.cursor_line.unwrap_or(self.scroll_offset as usize);
            let idx = self
                .search_matches
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
        let line = self.search_matches[idx];
        self.cursor_line = Some(line);
        self.cursor_col = 0;
        self.ensure_cursor_visible();
        self.sync_visual_selection();
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
        use crossterm::event::{MouseButton, MouseEventKind};

        // When frontmatter popup is open, capture scroll events for the popup
        if self.frontmatter_popup_index.is_some() {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.frontmatter_popup_scroll = self
                        .frontmatter_popup_scroll
                        .saturating_add(self.config.behavior.scroll_speed as u16);
                }
                MouseEventKind::ScrollUp => {
                    self.frontmatter_popup_scroll = self
                        .frontmatter_popup_scroll
                        .saturating_sub(self.config.behavior.scroll_speed as u16);
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
                if mouse.column < self.content_x
                    || mouse.column >= self.content_x + self.content_width
                {
                    self.selection = None;
                    return;
                }
                let click_col = (mouse.column - self.content_x) as usize;

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
                self.cursor_line = Some(content_line);
                self.cursor_col = content_col.min(self.line_width(content_line));
                self.sync_visual_selection();

                if let Some(url) = self.find_link_at(content_line, content_col) {
                    self.activate_url(url);
                } else {
                    // Start text selection
                    self.selection = Some(Selection {
                        start: (content_line, content_col),
                        end: (content_line, content_col),
                    });
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if mouse.column < self.content_x
                    || mouse.column >= self.content_x + self.content_width
                {
                    return;
                }
                let click_col = (mouse.column - self.content_x) as usize;
                let gutter_total = self.gutter_width + 3;
                let content_col = click_col.saturating_sub(gutter_total);

                if let Some(content_line) = self.visual_row_to_line(mouse.row as usize) {
                    if let Some(ref mut sel) = self.selection {
                        sel.end = (content_line, content_col);
                    }
                    self.cursor_line = Some(content_line);
                    self.cursor_col = content_col.min(self.line_width(content_line));
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
                self.hover_line = if mouse.column < self.content_x
                    || mouse.column >= self.content_x + self.content_width
                {
                    None
                } else {
                    self.visual_row_to_line(mouse.row as usize)
                };
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
        let before = self.scroll_offset;
        self.scroll_offset = self.scroll_offset.saturating_add(lines as u16);
        self.clamp_scroll();
        if self.scroll_offset != before {
            self.defer_image_render();
        }
    }

    fn scroll_up(&mut self, lines: usize) {
        let before = self.scroll_offset;
        self.scroll_offset = self.scroll_offset.saturating_sub(lines as u16);
        if self.scroll_offset != before {
            self.defer_image_render();
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

    fn find_code_block_at_cursor(&self) -> Option<&CodeBlockInfo> {
        let line = self.cursor_line?;
        self.code_block_infos
            .iter()
            .find(|cb| line >= cb.start_line && line <= cb.end_line)
    }

    fn run_code_block(&mut self) {
        let Some(cb) = self.find_code_block_at_cursor().cloned() else {
            self.show_toast("Not inside a code block".to_string());
            return;
        };

        let lang = cb.lang.as_deref().unwrap_or("");
        if !Self::is_supported_lang(lang) {
            self.show_error_toast(format!("Unsupported language: {}", if lang.is_empty() { "(none)" } else { lang }));
            return;
        }

        if self.config.runners.confirm_before_run {
            let display_lang = if lang.is_empty() { "code" } else { lang };
            self.confirm_prompt = Some(format!("Run this {} code? [y/N]", display_lang));
            self.confirm_code_block_content = Some(cb.content.clone());
            self.confirm_code_block_lang = cb.lang.clone();
            return;
        }

        self.execute_code(cb.lang.as_deref(), &cb.content);
    }

    fn run_code_block_as_sh(&mut self) {
        // If in visual mode with a selection, run the selected text as bash
        if self.visual_mode && self.selection.is_some() {
            let text = self.extract_selected_text();
            if text.is_empty() {
                self.show_toast("Selection is empty".to_string());
                return;
            }
            self.visual_mode = false;
            self.visual_line_mode = false;
            self.selection = None;

            if self.config.runners.confirm_before_run {
                self.confirm_prompt = Some("Run selection as bash? [y/N]".to_string());
                self.confirm_code_block_content = Some(text);
                self.confirm_code_block_lang = Some("bash".to_string());
                return;
            }

            self.execute_code(Some("bash"), &text);
            return;
        }

        let Some(cb) = self.find_code_block_at_cursor().cloned() else {
            self.show_toast("Not inside a code block".to_string());
            return;
        };

        if self.config.runners.confirm_before_run {
            self.confirm_prompt = Some("Run this code as bash? [y/N]".to_string());
            self.confirm_code_block_content = Some(cb.content.clone());
            self.confirm_code_block_lang = Some("bash".to_string());
            return;
        }

        self.execute_code(Some("bash"), &cb.content);
    }

    fn is_supported_lang(lang: &str) -> bool {
        matches!(
            lang,
            "sh" | "bash" | "shell" | "python" | "py" | "javascript" | "js" | "ruby" | "rb" | "go" | "rust"
        )
    }

    fn truncate_preview(s: &str, max_len: usize) -> String {
        let first_line = s.lines().next().unwrap_or("");
        if first_line.len() <= max_len {
            format!("{first_line} ...")
        } else {
            format!("{}...", &first_line[..max_len])
        }
    }

    fn execute_code(&mut self, lang: Option<&str>, content: &str) {
        let lang = match lang.unwrap_or("bash") {
            "shell" => "bash",
            other => other,
        };
        self.console_status = ConsoleStatus::Running;
        self.console_output = "Running...".to_string();
        self.console_visible = true;
        self.console_scroll = 0;

        let Some(tx) = self.event_tx.clone() else {
            self.show_error_toast("No event channel available".to_string());
            self.console_status = ConsoleStatus::Error;
            return;
        };

        let user_runners = self.config.runners.runners.clone();
        let lang = lang.to_string();
        let content = content.to_string();

        // Build command description for the console header
        let code_preview = content.trim();
        self.console_command = match lang.as_str() {
            "sh" | "bash" => {
                let cmd = user_runners.get(&lang).cloned().unwrap_or_else(|| "bash".into());
                if code_preview.contains('\n') {
                    format!("{cmd} -c '{}'", Self::truncate_preview(code_preview, 120))
                } else {
                    format!("{cmd} -c '{code_preview}'")
                }
            }
            "python" | "py" => {
                let cmd = user_runners.get(&lang)
                    .or_else(|| user_runners.get("python"))
                    .cloned()
                    .unwrap_or_else(|| "python3".into());
                if code_preview.contains('\n') {
                    format!("{cmd} -c '{}'", Self::truncate_preview(code_preview, 120))
                } else {
                    format!("{cmd} -c '{code_preview}'")
                }
            }
            "javascript" | "js" => {
                let cmd = user_runners.get(&lang)
                    .or_else(|| user_runners.get("javascript"))
                    .cloned()
                    .unwrap_or_else(|| "node".into());
                if code_preview.contains('\n') {
                    format!("{cmd} -e '{}'", Self::truncate_preview(code_preview, 120))
                } else {
                    format!("{cmd} -e '{code_preview}'")
                }
            }
            "ruby" | "rb" => {
                let cmd = user_runners.get(&lang)
                    .or_else(|| user_runners.get("ruby"))
                    .cloned()
                    .unwrap_or_else(|| "ruby".into());
                if code_preview.contains('\n') {
                    format!("{cmd} -e '{}'", Self::truncate_preview(code_preview, 120))
                } else {
                    format!("{cmd} -e '{code_preview}'")
                }
            }
            "go" => "go run <temp file>".into(),
            "rust" => "rustc <temp file> && <output>".into(),
            _ => lang.clone(),
        };

        std::thread::spawn(move || {
            let result = Self::run_command_blocking(&lang, &content, &user_runners);
            let _ = tx.send(AppEvent::CommandFinished(result));
        });
    }

    fn run_command_blocking(
        lang: &str,
        content: &str,
        user_runners: &std::collections::HashMap<String, String>,
    ) -> CommandResult {
        let lang = if lang == "shell" { "bash" } else { lang };
        let output_result = match lang {
            "sh" | "bash" => {
                let cmd = user_runners.get(lang).map(|s| s.as_str()).unwrap_or("bash");
                std::process::Command::new(cmd)
                    .arg("-c")
                    .arg(content)
                    .stdin(std::process::Stdio::null())
                    .output()
            }
            "python" | "py" => {
                let cmd = user_runners.get(lang)
                    .or_else(|| user_runners.get("python"))
                    .map(|s| s.as_str())
                    .unwrap_or("python3");
                std::process::Command::new(cmd)
                    .arg("-c")
                    .arg(content)
                    .stdin(std::process::Stdio::null())
                    .output()
            }
            "javascript" | "js" => {
                let cmd = user_runners.get(lang)
                    .or_else(|| user_runners.get("javascript"))
                    .map(|s| s.as_str())
                    .unwrap_or("node");
                std::process::Command::new(cmd)
                    .arg("-e")
                    .arg(content)
                    .stdin(std::process::Stdio::null())
                    .output()
            }
            "ruby" | "rb" => {
                let cmd = user_runners.get(lang)
                    .or_else(|| user_runners.get("ruby"))
                    .map(|s| s.as_str())
                    .unwrap_or("ruby");
                std::process::Command::new(cmd)
                    .arg("-e")
                    .arg(content)
                    .stdin(std::process::Stdio::null())
                    .output()
            }
            "go" => {
                return Self::run_tempfile_blocking(content, "go", ".go", &["go", "run"], user_runners);
            }
            "rust" => {
                return Self::run_rust_blocking(content, user_runners);
            }
            _ => {
                return CommandResult {
                    output: format!("Unsupported language: {lang}"),
                    success: false,
                };
            }
        };

        Self::process_output(output_result)
    }

    fn run_tempfile_blocking(
        content: &str,
        lang: &str,
        ext: &str,
        cmd_parts: &[&str],
        user_runners: &std::collections::HashMap<String, String>,
    ) -> CommandResult {
        let tmp_dir = std::env::temp_dir();
        let filename = format!("mdw_run_{}{}", std::process::id(), ext);
        let tmp_file = tmp_dir.join(&filename);

        if let Err(e) = std::fs::write(&tmp_file, content) {
            return CommandResult {
                output: format!("Failed to write temp file: {e}"),
                success: false,
            };
        }

        let result = if let Some(custom_cmd) = user_runners.get(lang) {
            let expanded = custom_cmd
                .replace("{file}", &tmp_file.display().to_string())
                .replace("{out}", &tmp_file.with_extension("").display().to_string());
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&expanded)
                .stdin(std::process::Stdio::null())
                .output()
        } else {
            let mut cmd = std::process::Command::new(cmd_parts[0]);
            for part in &cmd_parts[1..] {
                cmd.arg(part);
            }
            cmd.stdin(std::process::Stdio::null());
            cmd.arg(&tmp_file).output()
        };

        let _ = std::fs::remove_file(&tmp_file);
        Self::process_output(result)
    }

    fn run_rust_blocking(
        content: &str,
        user_runners: &std::collections::HashMap<String, String>,
    ) -> CommandResult {
        let tmp_dir = std::env::temp_dir();
        let src_file = tmp_dir.join(format!("mdw_run_{}.rs", std::process::id()));
        let out_file = tmp_dir.join(format!("mdw_run_{}", std::process::id()));

        if let Err(e) = std::fs::write(&src_file, content) {
            return CommandResult {
                output: format!("Failed to write temp file: {e}"),
                success: false,
            };
        }

        let result = if let Some(custom_cmd) = user_runners.get("rust") {
            let expanded = custom_cmd
                .replace("{file}", &src_file.display().to_string())
                .replace("{out}", &out_file.display().to_string());
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&expanded)
                .stdin(std::process::Stdio::null())
                .output()
        } else {
            let compile = std::process::Command::new("rustc")
                .arg(&src_file)
                .arg("-o")
                .arg(&out_file)
                .stdin(std::process::Stdio::null())
                .output();

            match compile {
                Ok(compile_out) if compile_out.status.success() => {
                    std::process::Command::new(&out_file)
                        .stdin(std::process::Stdio::null())
                        .output()
                }
                Ok(compile_out) => Ok(compile_out),
                Err(e) => Err(e),
            }
        };

        let _ = std::fs::remove_file(&src_file);
        let _ = std::fs::remove_file(&out_file);
        Self::process_output(result)
    }

    fn process_output(result: std::io::Result<std::process::Output>) -> CommandResult {
        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let mut combined = String::new();
                if !stdout.is_empty() {
                    combined.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&stderr);
                }
                if combined.is_empty() {
                    combined = "(no output)".to_string();
                }
                CommandResult {
                    output: combined,
                    success: output.status.success(),
                }
            }
            Err(e) => CommandResult {
                output: format!("Failed to execute: {e}"),
                success: false,
            },
        }
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
        if self.has_open_file || self.is_stdin {
            self.file_path.display().to_string()
        } else {
            self.tree_root_dir.display().to_string()
        }
    }

    pub fn status_message(&self) -> &str {
        &self.status_message
    }

    pub fn is_image(&self) -> bool {
        self.is_image
    }

    pub fn picker_info(&self) -> String {
        match &self.picker {
            Some(p) => format!("{:?} {:?}", p.protocol_type(), p.font_size()),
            None => "no picker".to_string(),
        }
    }

    pub fn image_debug_info(&self) -> String {
        if !self.is_image {
            return String::new();
        }
        let block_count = self.content_blocks.len();
        let block_info = if let Some(ContentBlock::Image {
            display_height,
            protocol,
            error,
            ..
        }) = self.content_blocks.first()
        {
            format!(
                "h={} proto={} err={}",
                display_height,
                if protocol.is_some() { "yes" } else { "no" },
                error.as_deref().unwrap_or("none"),
            )
        } else {
            format!("no-image-block(count={})", block_count)
        };
        format!(
            " [IMG {} | blocks={} | cw={} gw={}]",
            block_info, block_count, self.content_width, self.gutter_width,
        )
    }

    pub fn set_viewport_height(&mut self, height: u16) {
        self.viewport_height = height;
        self.ensure_file_tree_selection_visible();
    }

    pub fn set_content_area(&mut self, x: u16, width: u16) {
        let old_width = self.content_width;
        self.content_x = x;
        self.content_width = width;
        // Re-render specstory bubbles when width changes (they depend on terminal width)
        if self.spec_history_view && width != old_width && width > 0 {
            self.render_content();
        }
    }

    pub fn defer_image_render(&mut self) {
        self.defer_image_render_until =
            Some(Instant::now() + Duration::from_millis(IMAGE_RENDER_DEBOUNCE_MS));
    }

    pub fn should_defer_image_render(&self) -> bool {
        self.defer_image_render_until
            .is_some_and(|until| Instant::now() < until)
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

    pub fn spec_history_view(&self) -> bool {
        self.spec_history_view
    }

    pub fn raw_content(&self) -> &str {
        &self.raw_content
    }

    pub fn file_tree_view(&self) -> bool {
        self.file_tree_view
    }

    pub fn file_tree(&self) -> &FileTree {
        &self.file_tree
    }

    pub fn file_tree_selected(&self) -> usize {
        self.file_tree_selected
    }

    pub fn file_tree_scroll(&self) -> usize {
        self.file_tree_scroll
    }

    pub fn cursor_line(&self) -> Option<usize> {
        self.cursor_line
    }

    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    pub fn visual_mode(&self) -> bool {
        self.visual_mode
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

    pub fn toast_is_error(&self) -> bool {
        self.toast_is_error
    }

    pub fn console_visible(&self) -> bool {
        self.console_visible
    }

    pub fn console_output(&self) -> &str {
        &self.console_output
    }

    pub fn console_status(&self) -> &ConsoleStatus {
        &self.console_status
    }

    pub fn console_command(&self) -> &str {
        &self.console_command
    }

    pub fn confirm_prompt(&self) -> Option<&str> {
        self.confirm_prompt.as_deref()
    }

    fn line_width(&self, target_line: usize) -> usize {
        let mut line_idx = 0usize;

        for block in &self.content_blocks {
            match block {
                ContentBlock::Text { lines } => {
                    for line in lines {
                        if line_idx == target_line {
                            return line
                                .spans
                                .iter()
                                .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                                .sum();
                        }
                        line_idx += 1;
                    }
                }
                ContentBlock::Image { display_height, .. } => {
                    let height = *display_height as usize;
                    if target_line < line_idx + height {
                        return 0;
                    }
                    line_idx += height;
                }
            }
        }

        0
    }

    fn line_text(&self, target_line: usize) -> Option<String> {
        let mut line_idx = 0usize;

        for block in &self.content_blocks {
            match block {
                ContentBlock::Text { lines } => {
                    for line in lines {
                        if line_idx == target_line {
                            return Some(
                                line.spans
                                    .iter()
                                    .map(|span| span.content.as_ref())
                                    .collect(),
                            );
                        }
                        line_idx += 1;
                    }
                }
                ContentBlock::Image { display_height, .. } => {
                    let height = *display_height as usize;
                    if target_line < line_idx + height {
                        return Some(String::new());
                    }
                    line_idx += height;
                }
            }
        }

        None
    }

    fn line_end_cursor_col(&self, target_line: usize) -> usize {
        let Some(text) = self.line_text(target_line) else {
            return 0;
        };

        let mut current_col = 0usize;
        let mut last_col = 0usize;

        for ch in text.chars() {
            last_col = current_col;
            current_col += UnicodeWidthStr::width(ch.to_string().as_str()).max(1);
        }

        if text.is_empty() { 0 } else { last_col }
    }

    fn word_start_cols(&self, target_line: usize) -> Vec<usize> {
        let Some(text) = self.line_text(target_line) else {
            return Vec::new();
        };

        let mut starts = Vec::new();
        let mut current_col = 0usize;
        let mut prev_is_word = false;

        for ch in text.chars() {
            let is_word = is_word_char(ch);
            if is_word && !prev_is_word {
                starts.push(current_col);
            }
            prev_is_word = is_word;
            current_col += UnicodeWidthStr::width(ch.to_string().as_str()).max(1);
        }

        starts
    }

    fn char_width_at(&self, target_line: usize, target_col: usize) -> Option<usize> {
        let mut line_idx = 0usize;

        for block in &self.content_blocks {
            match block {
                ContentBlock::Text { lines } => {
                    for line in lines {
                        if line_idx == target_line {
                            let text: String = line
                                .spans
                                .iter()
                                .map(|span| span.content.as_ref())
                                .collect();
                            let mut current_col = 0usize;
                            for ch in text.chars() {
                                let width = UnicodeWidthStr::width(ch.to_string().as_str()).max(1);
                                if current_col == target_col {
                                    return Some(width);
                                }
                                current_col += width;
                                if current_col > target_col {
                                    return Some(width);
                                }
                            }
                            return None;
                        }
                        line_idx += 1;
                    }
                }
                ContentBlock::Image { display_height, .. } => {
                    let height = *display_height as usize;
                    if target_line < line_idx + height {
                        return None;
                    }
                    line_idx += height;
                }
            }
        }

        None
    }

    fn show_toast(&mut self, msg: String) {
        self.toast_message = Some(msg);
        self.toast_start = Some(Instant::now());
        self.toast_is_error = false;
    }

    fn show_error_toast(&mut self, msg: String) {
        self.toast_message = Some(msg);
        self.toast_start = Some(Instant::now());
        self.toast_is_error = true;
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
        let (start, end) =
            if sel.start.0 < sel.end.0 || (sel.start.0 == sel.end.0 && sel.start.1 <= sel.end.1) {
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
            if line < self.total_lines {
                Some(line)
            } else {
                None
            }
        } else {
            match self.visual_line_map.get(screen_row) {
                Some(&v) if v != usize::MAX => Some(v),
                _ => None,
            }
        }
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
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

fn render_syntax_highlighted(content: &str, lang: &str) -> ratatui::text::Text<'static> {
    if let Some(lines) = syntax_highlight::highlight_code(content, lang) {
        ratatui::text::Text::from(lines)
    } else {
        markdown::render_plain(content)
    }
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
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()?;
    }
    Ok(())
}
