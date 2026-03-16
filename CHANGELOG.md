# Changelog

All notable changes to this project will be documented in this file.

## [v0.14.2] - 2026-03-16

https://github.com/shonenada-vibe/mdw/releases/tag/v0.14.2

### Fixed
- Move image decoding and terminal image resize/encode work off the UI thread so image-heavy files no longer block the whole app while loading
- Reduce scroll-time hangs by deferring expensive image redraw work until scrolling settles, improving responsiveness on large images and long markdown pages

## [v0.14.0] - 2026-03-16

https://github.com/shonenada-vibe/mdw/releases/tag/v0.14.0

### Features
- Add code block execution: press `r` to run code block under cursor, with language-aware runners
- Add `ctrl+r` shortcut to run any code block as shell script regardless of language tag
- Add console output panel (`ctrl+t` to toggle) showing execution results with success/error status
- Add configurable runners for Python, JavaScript, Ruby, Go, and Rust with `{file}`/`{out}` placeholders
- Add `confirm_before_run` option (default: true) to prompt before executing code blocks
- Add async image loading to avoid blocking the UI while images are decoded
- Add file tree expand/collapse: click or press Enter on directories to toggle, with ▸/▾ indicators
- Lazy file tree loading: only read directory contents when the panel is opened or a directory is expanded

### Improved
- File tree preserves expanded state on refresh
- Only refresh file tree on file change events when the panel is visible

## [v0.13.0] - 2026-03-16

https://github.com/shonenada-vibe/mdw/releases/tag/v0.13.0

### Features
- Add Ghostty terminal detection for Kitty graphics protocol image rendering
- Increase maximum image display height from 30 to 50 terminal rows
- Add image protocol and rendering debug details to the status bar for diagnostics

### Fixed
- Fix blank image display on Kitty-protocol terminals by keeping the cursor indicator out of image cells
- Skip file-watcher reload for image files to avoid breaking active image protocol state
- Fall back to an `8x16` terminal cell size when font-size detection returns zero values

## [v0.12.2] - 2026-03-15

https://github.com/shonenada-vibe/mdw/releases/tag/v0.12.2

### Features
- Support opening image files directly (`mdw photo.png`) with png, jpg, gif, bmp, webp, tiff, pnm support
- Add chafa-style terminal protocol detection via environment variables for Kitty, iTerm2, WezTerm, Sixel terminals
- Use actual terminal width for image sizing instead of hardcoded 80 columns
- Cache decoded images to avoid re-reading from disk/network on terminal resize

### Fixed
- Show red error toast (5s) instead of crashing on file watcher, reload, and file open failures
- Fix double-nesting path bug when opening image files (e.g. `assets/assets/screenshot.jpg`)

## [v0.12.1] - 2026-03-15

https://github.com/shonenada-vibe/mdw/releases/tag/v0.12.1

### Fixed
- Make navigation shortcuts fully configurable, including file tree navigation and activate/open actions

## [v0.12.0] - 2026-03-13

https://github.com/shonenada-vibe/mdw/releases/tag/v0.12.0

### Features
- Add file tree panel with `t` toggle, tree navigation, and Enter-to-open files
- Auto-open file tree when the CLI path is a directory, with welcome content before a file is selected
- Add cursor-first navigation with highlighted line and column cursor using `h`/`j`/`k`/`l` and arrow keys
- Add Enter activation for links and collapsible JSON/markmap/frontmatter nodes at the cursor position
- Add vim-style visual mode with `v` for keyboard text selection and copy
- Add `u` shortcut to move the file tree root to its parent directory

## [v0.11.0] - 2026-03-13

https://github.com/shonenada-vibe/mdw/releases/tag/v0.11.0

### Features
- Add markmap colored branches, node indicators, and click-to-collapse
- Add JSON click-to-collapse for objects and arrays: click `{`/`[` lines to toggle visibility, collapsed nodes show `{...}`/`[...]` placeholders
- Add mindmap file (`.mm`) rendering support
- Add YAML file (`.yaml`/`.yml`) syntax highlighting support

## [v0.10.0] - 2026-03-12

https://github.com/shonenada-vibe/mdw/releases/tag/v0.10.0

### Features
- Add mouse text selection: click and drag to select text with visual highlighting
- Add right-click to copy selected text to clipboard
- Add Ctrl-C to copy selected text when selection is active
- Add toast notification at top of screen on copy success, auto-dismisses after 2 seconds
- Add configurable selection theme colors (`selection_fg`, `selection_bg`)

### Fixed
- Fix scroll lag caused by rendering after every individual mouse event; drain event queue before re-rendering

## [v0.9.0] - 2026-03-12

https://github.com/shonenada-vibe/mdw/releases/tag/v0.9.0

### Features
- Click frontmatter property row to show full untruncated value in a popup overlay
- Scrollable frontmatter popup with mouse scroll support
- Dismiss popup with any key or left click

## [v0.8.2] - 2026-03-12

https://github.com/shonenada-vibe/mdw/releases/tag/v0.8.2

### Features
- Add mouse scroll (ScrollDown/ScrollUp) support
- Add YAML frontmatter rendering as compact Obsidian-style "Properties" block

### Fixed
- Fix scroll not reaching bottom by allowing scroll up to total_lines-1
- Split gutter/content rendering so wrapped lines don't overlay line numbers
- Add unicode-width for correct CJK column calculation in click detection,
table alignment, and current_col()
- Add visual-line-map so mouse hover/click works correctly after wrapped lines

## [v0.8.1] - 2026-03-11

https://github.com/shonenada-vibe/mdw/releases/tag/v0.8.1

### Fixed
Failed to render seqeuence diagram

## [v0.8.0] - 2026-03-10

https://github.com/shonenada-vibe/mdw/releases/tag/v0.8.0

### Added
- Split view mode for side-by-side source and rendered markdown

## [v0.7.0] - 2026-03-10

https://github.com/shonenada-vibe/mdw/releases/tag/v0.7.0

### Added
- Table support for markdown

## [v0.6.0] - 2026-03-09

https://github.com/shonenada-vibe/mdw/releases/tag/v0.6.0

### Added
- Syntax highlighting for fenced code blocks

## [v0.5.0] - 2026-03-09

https://github.com/shonenada-vibe/mdw/releases/tag/v0.5.0

### Added
- Parse mermaid and D2 diagrams
- Image support

### Fixed
- Init picker before terminal and fall back from sixel to halfblocks

## [v0.4.0] - 2026-03-06

https://github.com/shonenada-vibe/mdw/releases/tag/v0.4.0

### Added
- Mouse hover line highlighting
- Click-to-open links in markdown viewer
- Vim-style search with `/` prompt, `n`/`N` navigation, and match highlighting
- Space for next page by default
- Stdin pipe support for reading content via `mdw -`
- Homebrew tap update PR workflow

## [v0.3.0] - 2026-03-06

https://github.com/shonenada-vibe/mdw/releases/tag/v0.3.0

### Added
- D2 diagram rendering with bidirectional arrow support
- Mermaid diagram rendering and help panel
- Line numbers and compact rendering
- Compact mode

## [v0.2.0] - 2026-03-01

https://github.com/shonenada-vibe/mdw/releases/tag/v0.2.0

### Added
- Syntax-colored JSON rendering with configurable theme
- Release with Homebrew

## [v0.1.0] - 2026-03-01

https://github.com/shonenada-vibe/mdw/releases/tag/v0.1.0

### Added
- Initial implementation of mdw terminal markdown viewer
- Config module with types, parsing, loading, and tests
- Config setup subcommand and wire config loading into CLI
- Configurable debounce_ms in setup_watcher
- ThemeConfig support in markdown renderer
- Config integration into App with configurable keybindings and scroll speed
- Theme colors for status bar and configurable line wrap
- SVG logo for mdw
- README with logo, usage, and configuration docs
- Release workflow
