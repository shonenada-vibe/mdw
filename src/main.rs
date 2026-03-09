mod app;
mod config;
mod content;
mod d2;
mod event;
mod image_loader;
mod markdown;
mod mermaid;
mod syntax_highlight;
mod ui;
mod watcher;

use std::io::{self, Read as _};
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mdw", about = "Terminal markdown file viewer with live reload")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to the file to view
    file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Generate a default config file and show setup guide
    Setup,
}

fn main() -> anyhow::Result<()> {
    color_eyre::install().ok();

    let cli = Cli::parse();

    if let Some(Command::Config { action }) = cli.command {
        match action {
            ConfigAction::Setup => return run_config_setup(),
        }
    }

    let Some(file) = cli.file else {
        anyhow::bail!("A file path is required. Usage: mdw <FILE> or mdw -");
    };

    let config = config::Config::load()?;

    let is_stdin = file.as_os_str() == "-";

    if is_stdin {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;

        // Init picker after reading stdin but before ratatui takes over terminal
        let picker = init_picker();
        let mut terminal = ratatui::init();
        let result = app::App::from_stdin(content, config, picker)?.run(&mut terminal);
        ratatui::restore();
        result
    } else {
        if !file.exists() {
            anyhow::bail!("File not found: {}", file.display());
        }

        // Init picker before ratatui takes over terminal (from_query_stdio needs raw stdio)
        let picker = init_picker();
        let mut terminal = ratatui::init();
        let result = app::App::new(file, config, picker)?.run(&mut terminal);
        ratatui::restore();
        result
    }
}

fn init_picker() -> Option<ratatui_image::picker::Picker> {
    let mut picker = ratatui_image::picker::Picker::from_query_stdio()
        .unwrap_or_else(|_| ratatui_image::picker::Picker::halfblocks());

    // Sixel detection via DA1 can produce false positives on terminals that
    // report the capability flag but don't actually render sixel images.
    // Fall back to halfblocks which works universally.
    if picker.protocol_type() == ratatui_image::picker::ProtocolType::Sixel {
        picker.set_protocol_type(ratatui_image::picker::ProtocolType::Halfblocks);
    }

    Some(picker)
}

fn run_config_setup() -> anyhow::Result<()> {
    let config_path = config::Config::config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.config/mdw/config.toml".to_string());

    println!("mdw configuration setup");
    println!("=======================");
    println!();
    println!("Config file location: {config_path}");
    println!();

    match config::Config::write_default_config() {
        Ok(path) => {
            println!("Created default config at {}", path.display());
        }
        Err(e) => {
            println!("{e}");
        }
    }

    println!();
    println!("The config file uses TOML format with three sections:");
    println!();
    println!("  [keybindings]  Customize key bindings (e.g. scroll_down = [\"s\", \"down\"])");
    println!("  [theme]        Customize colors using names or #rrggbb hex values");
    println!("  [behavior]     Set line_wrap, debounce_ms, and scroll_speed");
    println!();
    println!("All options are commented out by default. Uncomment and edit to customize.");
    println!("mdw works without a config file — defaults are used for any missing options.");

    Ok(())
}
