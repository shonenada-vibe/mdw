mod app;
mod event;
mod markdown;
mod ui;
mod watcher;

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "mdw", about = "Terminal markdown file viewer with live reload")]
struct Cli {
    /// Path to the file to view
    file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    color_eyre::install().ok();

    let cli = Cli::parse();

    if !cli.file.exists() {
        anyhow::bail!("File not found: {}", cli.file.display());
    }

    let mut terminal = ratatui::init();
    let result = app::App::new(cli.file)?.run(&mut terminal);
    ratatui::restore();
    result
}
