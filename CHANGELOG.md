# Changelog

All notable changes to this project will be documented in this file.

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
