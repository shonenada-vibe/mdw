# Changelog

All notable changes to this project will be documented in this file.

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
