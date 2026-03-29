mod app;
mod config;
mod content;
mod d2;
mod event;
mod file_tree;
mod image_loader;
mod markdown;
mod markmap;
mod mermaid;
mod mindmap;
mod specstory;
mod syntax_highlight;
mod ui;
mod watcher;

use std::fs;
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

    /// Render to a text file instead of interactive mode (headless)
    #[arg(long)]
    screenshot: Option<PathBuf>,
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

    if let Some(output) = cli.screenshot {
        if is_stdin {
            anyhow::bail!("--screenshot does not support stdin input");
        }
        if !file.exists() {
            anyhow::bail!("File not found: {}", file.display());
        }
        return run_screenshot(file, config, output);
    }

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

        let start_in_file_tree = file.is_dir();

        // Init picker before ratatui takes over terminal (from_query_stdio needs raw stdio)
        let picker = init_picker();
        let mut terminal = ratatui::init();
        let result = app::App::new(file, config, picker, start_in_file_tree)?.run(&mut terminal);
        ratatui::restore();
        result
    }
}

fn init_picker() -> Option<ratatui_image::picker::Picker> {
    use ratatui_image::picker::ProtocolType;

    let mut picker = ratatui_image::picker::Picker::from_query_stdio()
        .unwrap_or_else(|_| ratatui_image::picker::Picker::halfblocks());

    // Like chafa, detect the best image protocol by inspecting the terminal
    // environment rather than relying solely on DA1 escape-sequence probing,
    // which can produce false positives (especially for Sixel).
    let detected = picker.protocol_type();
    let best = detect_best_protocol();

    if let Some(proto) = best {
        // Environment-based detection is more reliable than DA1 probing
        picker.set_protocol_type(proto);
    } else if detected == ProtocolType::Sixel {
        // Sixel via DA1 is unreliable; fall back to halfblocks
        picker.set_protocol_type(ProtocolType::Halfblocks);
    }

    Some(picker)
}

/// Detect the best image protocol based on terminal environment variables,
/// inspired by chafa's terminal detection strategy in chafa-term-db.c.
fn detect_best_protocol() -> Option<ratatui_image::picker::ProtocolType> {
    use ratatui_image::picker::ProtocolType;

    // Ghostty: supports Kitty graphics protocol
    // chafa checks: TERM=xterm-ghostty, TERM_PROGRAM=ghostty, GHOSTTY_BIN_DIR
    if std::env::var("GHOSTTY_BIN_DIR").is_ok() {
        return Some(ProtocolType::Kitty);
    }
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        if term_program.eq_ignore_ascii_case("ghostty") {
            return Some(ProtocolType::Kitty);
        }
    }

    // Kitty: check KITTY_WINDOW_ID, KITTY_PID, or TERM=xterm-kitty
    if std::env::var("KITTY_WINDOW_ID").is_ok() || std::env::var("KITTY_PID").is_ok() {
        return Some(ProtocolType::Kitty);
    }

    // TERM containing "kitty" or "ghostty" (e.g. xterm-kitty, xterm-ghostty)
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("kitty") || term.contains("ghostty") {
            return Some(ProtocolType::Kitty);
        }
    }

    // iTerm2 and compatible (WezTerm, mintty)
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        let lc = term_program.to_lowercase();
        if lc.contains("iterm") || lc.contains("wezterm") || lc.contains("mintty") {
            return Some(ProtocolType::Iterm2);
        }
    }

    // WezTerm also sets its own env var
    if std::env::var("WEZTERM_EXECUTABLE").is_ok() {
        return Some(ProtocolType::Iterm2);
    }

    // Sixel-capable terminals detected by environment
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        let lc = term_program.to_lowercase();
        if lc.contains("foot") || lc.contains("contour") || lc.contains("xterm") {
            return Some(ProtocolType::Sixel);
        }
    }

    // No confident match; let caller decide
    None
}

fn run_screenshot(file: PathBuf, config: config::Config, output: PathBuf) -> anyhow::Result<()> {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend)?;
    let mut app = app::App::new(file, config, None, false)?;

    terminal.draw(|frame| ui::render(frame, &mut app))?;

    // Extract text from the TestBackend's Display output, stripping the quote wrapping
    let display = terminal.backend().to_string();
    let mut lines = Vec::new();
    for line in display.lines() {
        if line.starts_with('"') && line.ends_with('"') {
            lines.push(&line[1..line.len() - 1]);
        } else if let Some(stripped) = line.strip_prefix('"') {
            // Line may have hidden multi-width symbol info after the closing quote
            if let Some(pos) = stripped.find('"') {
                lines.push(&stripped[..pos]);
            } else {
                lines.push(line);
            }
        } else {
            lines.push(line);
        }
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, lines.join("\n"))?;
    Ok(())
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
