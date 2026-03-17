# mdw — Project Context for AI Agents

## What is mdw?

`mdw` is a terminal-based Markdown viewer with live reload, built in Rust. It renders Markdown files in the terminal with rich formatting, image display, syntax highlighting, diagram rendering, and vim-style navigation.

## Tech Stack

- **Language:** Rust (2024 edition)
- **TUI Framework:** ratatui 0.30 + crossterm 0.29
- **Markdown Parsing:** pulldown-cmark
- **Syntax Highlighting:** syntect
- **Image Rendering:** ratatui-image (terminal image protocols)
- **File Watching:** notify + notify-debouncer-mini
- **CLI Parsing:** clap (derive)
- **Config:** TOML via serde + toml crate
- **Error Handling:** anyhow + color-eyre

## Build & Run

```sh
cargo build
cargo run -- <file.md>       # View a markdown file
cargo run -- --screenshot out.txt <file.md>  # Headless render
cargo run -- config setup    # Generate default config
cargo test
cargo clippy
```

Reads from stdin if no file is provided.

## Supported File Types

Markdown (.md), JSON, YAML, Mermaid (.mmd), D2 (.d2), Mindmap (.mm), and image files. File type is auto-detected from extension.

## Architecture Overview (~10k lines of Rust)

### Core Flow

1. `main.rs` parses CLI args, loads config, initializes terminal, creates `App`
2. `App::run()` enters the main event loop
3. `EventHandler` (background thread) polls crossterm for key/mouse/resize events and sends them via `mpsc::channel`
4. `App` processes events → resolves keybinding → dispatches `Action` → updates state
5. `ui::draw()` renders the current state to the terminal each frame

### Source Files

| File | Lines | Purpose |
|------|-------|---------|
| `app.rs` | ~2400 | Main `App` struct, event loop, state management, action dispatch |
| `markdown.rs` | ~1700 | Markdown → ratatui `Text` rendering (pulldown-cmark walker) |
| `mermaid.rs` | ~1800 | Mermaid diagram → ASCII art rendering |
| `ui.rs` | ~1200 | TUI layout & rendering (content panes, status bar, help, toast, console) |
| `config.rs` | ~770 | Config loading (TOML), `Action` enum, `KeyCombo` parsing, keybinding resolution |
| `d2.rs` | ~770 | D2 diagram rendering |
| `markmap.rs` | ~470 | Markmap (tree visualization from headings) |
| `main.rs` | ~250 | CLI entry point, stdin handling, screenshot mode |
| `file_tree.rs` | ~220 | File tree sidebar panel |
| `mindmap.rs` | ~120 | Mindmap (.mm) file parsing and rendering |
| `event.rs` | ~100 | Event types (`AppEvent`) and `EventHandler` |
| `syntax_highlight.rs` | ~70 | Code block syntax highlighting via syntect |
| `image_loader.rs` | ~55 | Async image loading (local + remote via ureq) |
| `watcher.rs` | ~35 | File watcher with debounce for live reload |
| `content.rs` | ~30 | `ContentBlock` enum (Text or Image) |

### Key Data Structures

```
App {
    file_path, raw_content,
    content_blocks: Vec<ContentBlock>,  // Rendered content
    scroll_offset, viewport_height,     // Viewport state
    visual_line_map: Vec<usize>,        // Screen row → logical line mapping
    config: Config,                     // User config + keybindings
    search_*, cursor_*, selection,      // Search & selection state
    file_tree: FileTree,                // Sidebar file browser
    console_*,                          // Embedded console for code execution
    ...
}

ContentBlock = Text { lines: Vec<Line<'static>> }
             | Image { alt_text, protocol, cached_image, source, ... }

ImageSource = Local(PathBuf) | Remote(String)

AppEvent = Key(KeyEvent) | Mouse(MouseEvent) | FileChanged
         | Tick | Resize | CommandFinished | ImageLoaded | ImageResized
```

### Action / Keybinding System

All user actions are defined in the `Action` enum (~25 variants):
- Navigation: `ScrollDown`, `ScrollUp`, `Top`, `Bottom`, `HalfPageDown/Up`, `PageDown/Up`
- Cursor: `CursorLeft/Right`, `CursorLineStart/End`, `CursorWordForward/Backward`
- Features: `ToggleHelp`, `SearchForward`, `SearchNext/Prev`, `ToggleSplitView`, `ToggleMarkmap`, `ToggleFileTree`, `ToggleVisualMode`, `ToggleConsole`
- Actions: `Activate` (follow link/open file), `RunCodeBlock`, `RunCodeBlockSh`, `Quit`

Keybindings are configured via TOML config. Default bindings are vim-style (j/k, g/G, etc.). Resolution: `KeyEvent` → `Config::resolve_action()` → `Option<Action>`.

### Config System

Config file location: `~/.config/mdw/config.toml`

Sections:
- `[keybindings]` — action-to-key mappings
- `[theme]` — colors for headings, links, code, etc.
- `[behavior]` — line_wrap, tab_width, gutter settings

### Rendering Pipeline

1. Raw file content is parsed based on file type
2. For Markdown: `markdown::render()` walks pulldown-cmark events → produces `Vec<ContentBlock>` + metadata (links, code blocks, footnotes)
3. For diagrams: respective modules (mermaid, d2, markmap, mindmap) convert to ASCII art → wrapped as `ContentBlock::Text`
4. `ui::draw()` renders content blocks into ratatui widgets, handling scroll, line wrap, selection highlighting, images

### Key Features

- **Live Reload:** File watcher triggers re-parse and re-render on save
- **Vim Navigation:** j/k scrolling, g/G jump, / search, visual mode with selection
- **Split View:** Side-by-side raw source + rendered view
- **Image Support:** Inline terminal images (local + remote URLs), async loading
- **Code Execution:** Run code blocks directly from the viewer (with confirmation prompt)
- **File Tree:** Sidebar file browser with expand/collapse
- **Syntax Highlighting:** Code blocks highlighted via syntect
- **Diagram Rendering:** Mermaid, D2, markmap, mindmap → ASCII art
- **Search:** Forward search with match highlighting and navigation
- **Selection & Copy:** Visual mode selection with clipboard copy
- **Frontmatter:** YAML/TOML frontmatter popup display
- **Mouse Support:** Scroll, click to navigate
- **Screenshot Mode:** Headless render to text file
- **Configurable:** TOML config for keybindings, theme, behavior

## Conventions

- Actions are the central abstraction for user interaction — add new features by adding an `Action` variant, keybinding, and handler in `App`
- Content is always `Vec<ContentBlock>` — text or images
- The `visual_line_map` bridges screen coordinates to logical content lines (important for wrapped text)
- State lives in `App`; rendering is stateless in `ui.rs`
- No async runtime — uses threads + `mpsc::channel` for concurrency
- Platform clipboard via shell commands (pbcopy/xclip/clip)
