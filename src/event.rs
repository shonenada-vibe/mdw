use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEventKind, MouseEvent};
use image::DynamicImage;
use ratatui_image::thread::ResizeResponse;

#[derive(Debug)]
pub enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Mouse(MouseEvent),
    FileChanged,
    Tick,
    Resize,
    CommandFinished(CommandResult),
    ImageLoaded(ImageLoadResult),
    ImageResized(ImageResizeResult),
}

#[derive(Debug)]
pub struct CommandResult {
    pub output: String,
    pub success: bool,
}

pub struct ImageLoadResult {
    pub block_index: usize,
    pub result: Result<(DynamicImage, u16), String>,
}

impl std::fmt::Debug for ImageLoadResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageLoadResult")
            .field("block_index", &self.block_index)
            .field("result", &self.result.as_ref().map(|(_, h)| h).map_err(|e| e.clone()))
            .finish()
    }
}

pub struct ImageResizeResult {
    pub block_index: usize,
    pub result: Result<ResizeResponse, String>,
}

impl std::fmt::Debug for ImageResizeResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageResizeResult")
            .field("block_index", &self.block_index)
            .field("result", &self.result.as_ref().map(|_| "ok").map_err(|e| e.clone()))
            .finish()
    }
}

pub struct EventHandler {
    rx: mpsc::Receiver<AppEvent>,
    _input_thread: thread::JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> (Self, mpsc::Sender<AppEvent>) {
        let (tx, rx) = mpsc::channel();
        let tx_input = tx.clone();

        let input_thread = thread::spawn(move || {
            loop {
                if event::poll(tick_rate).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            if key.kind == KeyEventKind::Press {
                                let _ = tx_input.send(AppEvent::Key(key));
                            }
                        }
                        Ok(CrosstermEvent::Mouse(mouse)) => {
                            let _ = tx_input.send(AppEvent::Mouse(mouse));
                        }
                        Ok(CrosstermEvent::Resize(_, _)) => {
                            let _ = tx_input.send(AppEvent::Resize);
                        }
                        _ => {}
                    }
                }
                let _ = tx_input.send(AppEvent::Tick);
            }
        });

        let handler = EventHandler {
            rx,
            _input_thread: input_thread,
        };
        (handler, tx)
    }

    pub fn next(&self) -> anyhow::Result<AppEvent> {
        Ok(self.rx.recv()?)
    }

    pub fn try_next(&self) -> Option<AppEvent> {
        self.rx.try_recv().ok()
    }
}
