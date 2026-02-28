use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEventKind};

#[derive(Debug)]
pub enum AppEvent {
    Key(crossterm::event::KeyEvent),
    FileChanged,
    Tick,
    Resize,
}

pub struct EventHandler {
    rx: mpsc::Receiver<AppEvent>,
    _input_thread: thread::JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> (Self, mpsc::Sender<AppEvent>) {
        let (tx, rx) = mpsc::channel();
        let tx_input = tx.clone();

        let input_thread = thread::spawn(move || loop {
            if event::poll(tick_rate).unwrap_or(false) {
                match event::read() {
                    Ok(CrosstermEvent::Key(key)) => {
                        if key.kind == KeyEventKind::Press {
                            let _ = tx_input.send(AppEvent::Key(key));
                        }
                    }
                    Ok(CrosstermEvent::Resize(_, _)) => {
                        let _ = tx_input.send(AppEvent::Resize);
                    }
                    _ => {}
                }
            }
            let _ = tx_input.send(AppEvent::Tick);
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
}
