use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::text::Text;

use crate::event::{AppEvent, EventHandler};
use crate::markdown;
use crate::ui;
use crate::watcher;

pub struct App {
    file_path: PathBuf,
    is_markdown: bool,
    raw_content: String,
    rendered_content: Text<'static>,
    total_lines: usize,
    scroll_offset: u16,
    viewport_height: u16,
    should_quit: bool,
    status_message: String,
}

impl App {
    pub fn new(file_path: PathBuf) -> anyhow::Result<Self> {
        let is_markdown = file_path
            .extension()
            .map(|ext| ext == "md" || ext == "markdown" || ext == "mdx")
            .unwrap_or(false);

        let mut app = App {
            file_path,
            is_markdown,
            raw_content: String::new(),
            rendered_content: Text::default(),
            total_lines: 0,
            scroll_offset: 0,
            viewport_height: 0,
            should_quit: false,
            status_message: String::new(),
        };

        app.reload_file()?;
        Ok(app)
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        let (event_handler, tx) = EventHandler::new(Duration::from_millis(250));

        let canonical = self.file_path.canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {}", self.file_path.display()))?;
        let _watcher = watcher::setup_watcher(&canonical, tx)?;

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
            markdown::render_markdown(&self.raw_content)
        } else {
            markdown::render_plain(&self.raw_content)
        };

        self.total_lines = self.rendered_content.lines.len();
        self.clamp_scroll();
        Ok(())
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        match (key.modifiers, key.code) {
            // Quit
            (_, KeyCode::Char('q')) => self.should_quit = true,
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => self.should_quit = true,

            // Scroll down one line
            (KeyModifiers::NONE, KeyCode::Char('j')) | (_, KeyCode::Down) => {
                self.scroll_down(1);
            }

            // Scroll up one line
            (KeyModifiers::NONE, KeyCode::Char('k')) | (_, KeyCode::Up) => {
                self.scroll_up(1);
            }

            // Half-page down
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                let half = (self.viewport_height / 2).max(1);
                self.scroll_down(half as usize);
            }

            // Half-page up
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                let half = (self.viewport_height / 2).max(1);
                self.scroll_up(half as usize);
            }

            // Full page down
            (KeyModifiers::CONTROL, KeyCode::Char('f')) | (_, KeyCode::PageDown) => {
                self.scroll_down(self.viewport_height as usize);
            }

            // Full page up
            (KeyModifiers::CONTROL, KeyCode::Char('b')) | (_, KeyCode::PageUp) => {
                self.scroll_up(self.viewport_height as usize);
            }

            // Go to top
            (KeyModifiers::NONE, KeyCode::Char('g')) | (_, KeyCode::Home) => {
                self.scroll_offset = 0;
            }

            // Go to bottom
            (KeyModifiers::SHIFT, KeyCode::Char('G')) | (_, KeyCode::End) => {
                self.scroll_to_bottom();
            }
            (KeyModifiers::NONE, KeyCode::Char('G')) => {
                self.scroll_to_bottom();
            }

            _ => {}
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
}
