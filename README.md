<p align="center">
  <img src="assets/logo.svg" alt="mdw logo" width="480">
</p>

<p align="center">
  A terminal markdown viewer with live reload, built with Rust.
</p>

---

## Screenshot

![screenshot](assets/screenshot.jpg)

## Features

- Syntax-highlighted markdown rendering in the terminal
- Live reload — file changes are reflected instantly
- Tables with box-drawing borders and column alignment
- Inline images (iTerm2 / Kitty protocols)
- Mermaid and D2 diagram rendering
- Search (`/`, `n`, `N`)
- Cursor-first navigation with vim-style keys (`h`/`j`/`k`/`l`, `g`/`G`, `ctrl+d`/`ctrl+u`)
- File tree panel with directory browsing (`t`, `u`, Enter)
- Visual selection and copy (`v`)
- Code block execution with console output panel (`r`, `ctrl+r`)
- Fully configurable keybindings, theme colors, and behavior via TOML
- Scrollbar, status bar, and help panel (`?`)

## Installation

### Homebrew

```sh
brew tap shonenada/tap
brew install shonenada/tap/mdw
```

### Shell script

```sh
curl -fsSL https://raw.githubusercontent.com/shonenada-vibe/mdw/main/scripts/install.sh | bash
```

To install a specific version:

```sh
curl -fsSL https://raw.githubusercontent.com/shonenada-vibe/mdw/main/scripts/install.sh | bash -s -- v0.1.0
```

To install to a custom directory:

```sh
MDW_INSTALL_DIR=~/.local/bin curl -fsSL https://raw.githubusercontent.com/shonenada-vibe/mdw/main/scripts/install.sh | bash
```

### From source

```sh
cargo install --path .
```

## Usage

```sh
# View a markdown file
mdw README.md

# Open a directory and browse files from the tree
mdw .

# Set up a config file
mdw config setup
```

## Configuration

Run `mdw config setup` to generate a default config at `~/.config/mdw/config.toml`.

All options are commented out by default — uncomment and edit to customize. mdw works out of the box with sensible defaults.

### Keybindings

```toml
[keybindings]
quit = ["q", "ctrl+c"]
scroll_down = ["j", "down"]
scroll_up = ["k", "up"]
cursor_left = ["h", "left"]
cursor_right = ["l", "right"]
cursor_line_start = ["^"]
cursor_line_end = ["$"]
cursor_word_forward = ["w"]
cursor_word_backward = ["b"]
half_page_down = ["ctrl+d"]
half_page_up = ["ctrl+u"]
page_down = ["ctrl+f", "pagedown"]
page_up = ["ctrl+b", "pageup"]
top = ["g", "home"]
bottom = ["shift+g", "G", "end"]
toggle_help = ["?"]
search_forward = ["/"]
search_next = ["n"]
search_prev = ["N", "shift+n"]
toggle_split_view = ["s"]
toggle_markmap = ["m"]
toggle_file_tree = ["t"]
file_tree_parent = ["u"]
activate = ["enter", "o"]
toggle_visual_mode = ["v"]
run_code_block = ["r"]
run_code_block_sh = ["ctrl+r"]
toggle_console = ["ctrl+t"]
```

Key format: `"key"`, `"ctrl+key"`, `"shift+key"`, `"alt+key"`. Special keys: `up`, `down`, `pageup`, `pagedown`, `home`, `end`, `esc`, `enter`, `space`, `tab`.

### Common shortcuts

| Shortcut | Action |
|---|---|
| `j` / `k` / `up` / `down` | Move cursor line |
| `h` / `l` / `left` / `right` | Move cursor column |
| `Enter` | Open link, toggle collapsible node, or open selected tree file |
| `t` | Toggle file tree |
| `u` | Move file tree root to parent directory |
| `v` | Toggle visual mode; press again to copy selection |
| `/`, `n`, `N` | Search and jump between matches |
| `r` | Run code block under cursor |
| `ctrl+r` | Run code block as shell |
| `ctrl+t` | Toggle console panel |
| `?` | Show help |

### Theme

```toml
[theme]
heading1 = "magenta"
heading2 = "cyan"
heading3 = "yellow"
heading4 = "green"
heading5 = "blue"
heading6 = "white"
code_block_fg = "lightgreen"
code_block_bg = "#282828"
inline_code_fg = "lightyellow"
inline_code_bg = "darkgray"
blockquote = "green"
link = "blue"
link_url = "darkgray"
horizontal_rule = "darkgray"
status_bar_bg = "darkgray"
status_bar_fg = "white"
status_bar_message_fg = "yellow"
table_border = "darkgray"
table_header_fg = "cyan"
```

Colors can be named (`red`, `lightblue`, `darkgray`, ...) or hex (`#rrggbb`).

### Behavior

```toml
[behavior]
line_wrap = true
debounce_ms = 200
scroll_speed = 1
```

| Option | Description | Default |
|---|---|---|
| `line_wrap` | Wrap long lines | `true` |
| `debounce_ms` | File watcher debounce interval (ms) | `200` |
| `scroll_speed` | Lines to scroll per `j`/`k` press | `1` |
| `mouse_scroll` | Enable mouse scroll | `true` |

### Runners

```toml
[runners]
confirm_before_run = true

[runners.runners]
python = "python3"
javascript = "node"
ruby = "ruby"
go = "go run {file}"
rust = "rustc {file} -o {out} && {out}"
```

| Option | Description | Default |
|---|---|---|
| `confirm_before_run` | Show confirmation prompt before executing | `true` |
| `runners.<lang>` | Custom command for a language | built-in defaults |

Supported languages: `sh`, `bash`, `python`/`py`, `javascript`/`js`, `ruby`/`rb`, `go`, `rust`.

For `go` and `rust` runners, use `{file}` and `{out}` placeholders for the temp source file and output binary paths.

## License

MIT
