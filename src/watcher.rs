use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use crate::event::AppEvent;

pub fn setup_watcher(
    path: &Path,
    tx: mpsc::Sender<AppEvent>,
) -> anyhow::Result<Debouncer<notify::RecommendedWatcher>> {
    let mut debouncer = new_debouncer(
        Duration::from_millis(200),
        move |result: DebounceEventResult| {
            if let Ok(events) = result {
                if !events.is_empty() {
                    let _ = tx.send(AppEvent::FileChanged);
                }
            }
        },
    )?;

    debouncer.watcher().watch(path, RecursiveMode::NonRecursive)?;

    Ok(debouncer)
}
