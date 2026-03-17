# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

mdw is a terminal Markdown viewer with live reload, built in Rust (2024 edition) using ratatui + crossterm. It supports Markdown, JSON, YAML, Mermaid, D2, and mindmap files with vim-style navigation, inline images, syntax highlighting, and code execution.

## Build & Test Commands

```sh
cargo build                    # Build
cargo run -- <file.md>         # Run with a file
cargo run -- .                 # Browse directory with file tree
cargo test                     # Run all tests
cargo test --lib config        # Run tests in a specific module
cargo clippy                   # Lint
```

## Architecture

### Event-Driven TUI Loop

`main.rs` → parses CLI (clap) → loads `Config` → creates `App` → `App::run()` enters event loop.

`EventHandler` spawns a background thread polling crossterm, sending `AppEvent` variants (Key, Mouse, FileChanged, Tick, Resize, CommandFinished, ImageLoaded, ImageResized) over `mpsc::channel`. The main loop receives events, resolves keybindings to `Action` variants, and dispatches them.

### Action System (config.rs)

The `Action` enum defines all user actions (~25 variants). `KeybindingsConfig` maps each action to `Vec<KeyCombo>`. Resolution: `KeyEvent` → `Config::resolve_action()` → `Option<Action>`. To add a new action:
1. Add variant to `Action` enum
2. Add field to `KeybindingsConfig` with default keybinding in `impl Default`
3. Add match arm in `resolve_action()`
4. Handle the action in `App`'s event dispatch

### Content Model (content.rs, markdown.rs)

Content is `Vec<ContentBlock>` where each block is either `Text { lines: Vec<Line<'static>> }` or `Image { ... }`. The `visual_line_map: Vec<usize>` maps screen rows to logical line indices (critical for line-wrap, selection, and click handling).

`markdown::render()` walks pulldown-cmark events and produces content blocks + metadata (link positions, code block info, footnotes). Diagram modules (mermaid.rs, d2.rs, markmap.rs, mindmap.rs) convert their formats to ASCII art wrapped as `ContentBlock::Text`.

### Rendering (ui.rs)

`ui::draw()` is stateless — it reads `App` state and renders to ratatui widgets. Layout includes: optional file tree sidebar, content pane(s) (split view shows raw + rendered), status bar, optional console panel, and overlays (help, frontmatter popup, toast, confirmation prompt).

### Concurrency

No async runtime. Uses `std::thread` + `mpsc::channel`:
- Input thread polls crossterm events
- File watcher (notify) sends `FileChanged`
- Image loading runs on background threads
- Code execution runs on a background thread, result sent as `CommandFinished`

## Key Patterns

- All state lives in the `App` struct; `ui.rs` is a pure rendering function
- Config is loaded from `~/.config/mdw/config.toml` (TOML with serde)
- Clipboard uses platform commands (pbcopy/xclip/clip)
- Images are loaded async, cached as `DynamicImage`, rendered via ratatui-image protocols
- Tests exist in config.rs (14 tests), markdown.rs (3), markmap.rs (5) — mostly unit tests for parsing logic
