use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use ratatui::DefaultTerminal;
use ratatui::text::Text;

use crate::config::{Action, Config};
use crate::event::{AppEvent, EventHandler};
use crate::markdown;
use crate::mermaid;
use crate::ui;
use crate::watcher;

pub struct App {
    file_path: PathBuf,
    is_markdown: bool,
    is_json: bool,
    is_mermaid: bool,
    raw_content: String,
    rendered_content: Text<'static>,
    total_lines: usize,
    scroll_offset: u16,
    viewport_height: u16,
    should_quit: bool,
    show_help: bool,
    status_message: String,
    config: Config,
}

impl App {
    pub fn new(file_path: PathBuf, config: Config) -> anyhow::Result<Self> {
        let ext = file_path.extension().and_then(|e| e.to_str());
        let is_markdown = ext.is_some_and(|e| matches!(e, "md" | "markdown" | "mdx"));
        let is_json = ext.is_some_and(|e| e == "json");
        let is_mermaid = ext.is_some_and(|e| e == "mermaid");

        let mut app = App {
            file_path,
            is_markdown,
            is_json,
            is_mermaid,
            raw_content: String::new(),
            rendered_content: Text::default(),
            total_lines: 0,
            scroll_offset: 0,
            viewport_height: 0,
            should_quit: false,
            show_help: false,
            status_message: String::new(),
            config,
        };

        app.reload_file()?;
        Ok(app)
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        let (event_handler, tx) = EventHandler::new(Duration::from_millis(250));

        let canonical = self.file_path.canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {}", self.file_path.display()))?;
        let _watcher = watcher::setup_watcher(&canonical, tx, self.config.behavior.debounce_ms)?;

        while !self.should_quit {
            terminal.draw(|frame| ui::render(frame, self))?;

            match event_handler.next()? {
                AppEvent::Key(key) => self.handle_key(key),
                AppEvent::FileChanged => {
                    if let Err(e) = self.reload_file() {
                        self.status_message = format!("Error: {e}");
                    } else {
                        self.status_message = "Reloaded".to_string();
                    }
                }
                AppEvent::Resize => {
                    // viewport_height is updated in ui::render via set_viewport_height
                }
                AppEvent::Tick => {}
            }
        }

        Ok(())
    }

    fn reload_file(&mut self) -> anyhow::Result<()> {
        self.raw_content = std::fs::read_to_string(&self.file_path)
            .with_context(|| format!("Failed to read {}", self.file_path.display()))?;

        self.rendered_content = if self.is_markdown {
            markdown::render_markdown(&self.raw_content, &self.config.theme)
        } else if self.is_json {
            markdown::render_json(&self.raw_content, &self.config.theme)
        } else if self.is_mermaid {
            mermaid::render_mermaid(&self.raw_content, &self.config.theme)
        } else {
            markdown::render_plain(&self.raw_content)
        };

        self.total_lines = self.rendered_content.lines.len();
        self.clamp_scroll();
        Ok(())
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
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
        }
    }

    fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines as u16);
        self.clamp_scroll();
    }

    fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines as u16);
    }

    fn scroll_to_bottom(&mut self) {
        if self.total_lines as u16 > self.viewport_height {
            self.scroll_offset = self.total_lines as u16 - self.viewport_height;
        } else {
            self.scroll_offset = 0;
        }
    }

    fn clamp_scroll(&mut self) {
        let max_scroll = if self.total_lines as u16 > self.viewport_height {
            self.total_lines as u16 - self.viewport_height
        } else {
            0
        };
        self.scroll_offset = self.scroll_offset.min(max_scroll);
    }

    // Accessors for ui.rs
    pub fn rendered_content(&self) -> &Text<'static> {
        &self.rendered_content
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
}
