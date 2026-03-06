use std::fmt;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use serde::de::{self, Visitor};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// KeyCombo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombo {
    fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('+').collect();
        let mut modifiers = KeyModifiers::NONE;

        for &part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "ctrl" => modifiers |= KeyModifiers::CONTROL,
                "alt" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                other => return Err(format!("unknown modifier: {other}")),
            }
        }

        let key_str = parts[parts.len() - 1];
        let code = match key_str.to_lowercase().as_str() {
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "esc" | "escape" => KeyCode::Esc,
            "enter" | "return" => KeyCode::Enter,
            "space" => KeyCode::Char(' '),
            "tab" => KeyCode::Tab,
            "backspace" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            _ => {
                let chars: Vec<char> = key_str.chars().collect();
                if chars.len() == 1 {
                    let ch = chars[0];
                    if ch.is_ascii_uppercase() && !modifiers.contains(KeyModifiers::SHIFT) {
                        // Uppercase letter without explicit shift — store as-is
                        KeyCode::Char(ch)
                    } else if modifiers.contains(KeyModifiers::SHIFT) && ch.is_ascii_lowercase() {
                        // shift+g → 'G'
                        KeyCode::Char(ch.to_ascii_uppercase())
                    } else {
                        KeyCode::Char(ch)
                    }
                } else {
                    return Err(format!("unknown key: {key_str}"));
                }
            }
        };

        Ok(KeyCombo { code, modifiers })
    }

    pub fn matches(&self, event: &KeyEvent) -> bool {
        if self.code != event.code {
            return false;
        }
        // For character keys, crossterm may or may not include SHIFT in modifiers
        // depending on the terminal. We handle both cases.
        match self.code {
            KeyCode::Char(c) if c.is_ascii_uppercase() => {
                // Accept with or without SHIFT modifier for uppercase chars
                let base = self.modifiers - KeyModifiers::SHIFT;
                let event_base = event.modifiers - KeyModifiers::SHIFT;
                base == event_base
            }
            _ => self.modifiers == event.modifiers,
        }
    }
}

impl<'de> Deserialize<'de> for KeyCombo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        KeyCombo::parse(&s).map_err(de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// ThemeColor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ThemeColor(pub Color);

impl ThemeColor {
    fn parse(s: &str) -> Result<Self, String> {
        if let Some(hex) = s.strip_prefix('#') {
            if hex.len() != 6 {
                return Err(format!("invalid hex color: {s}"));
            }
            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| format!("invalid hex color: {s}"))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| format!("invalid hex color: {s}"))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| format!("invalid hex color: {s}"))?;
            return Ok(ThemeColor(Color::Rgb(r, g, b)));
        }

        let color = match s.to_lowercase().as_str() {
            "black" => Color::Black,
            "red" => Color::Red,
            "green" => Color::Green,
            "yellow" => Color::Yellow,
            "blue" => Color::Blue,
            "magenta" => Color::Magenta,
            "cyan" => Color::Cyan,
            "gray" | "grey" => Color::Gray,
            "darkgray" | "darkgrey" => Color::DarkGray,
            "lightred" => Color::LightRed,
            "lightgreen" => Color::LightGreen,
            "lightyellow" => Color::LightYellow,
            "lightblue" => Color::LightBlue,
            "lightmagenta" => Color::LightMagenta,
            "lightcyan" => Color::LightCyan,
            "white" => Color::White,
            other => return Err(format!("unknown color: {other}")),
        };
        Ok(ThemeColor(color))
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ThemeColorVisitor;

        impl<'de> Visitor<'de> for ThemeColorVisitor {
            type Value = ThemeColor;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a color name or #rrggbb hex string")
            }

            fn visit_str<E>(self, v: &str) -> Result<ThemeColor, E>
            where
                E: de::Error,
            {
                ThemeColor::parse(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(ThemeColorVisitor)
    }
}

// ---------------------------------------------------------------------------
// Action
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    ScrollDown,
    ScrollUp,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    Top,
    Bottom,
    ToggleHelp,
    SearchForward,
    SearchNext,
    SearchPrev,
}

// ---------------------------------------------------------------------------
// KeybindingsConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub quit: Vec<KeyCombo>,
    pub scroll_down: Vec<KeyCombo>,
    pub scroll_up: Vec<KeyCombo>,
    pub half_page_down: Vec<KeyCombo>,
    pub half_page_up: Vec<KeyCombo>,
    pub page_down: Vec<KeyCombo>,
    pub page_up: Vec<KeyCombo>,
    pub top: Vec<KeyCombo>,
    pub bottom: Vec<KeyCombo>,
    pub toggle_help: Vec<KeyCombo>,
    pub search_forward: Vec<KeyCombo>,
    pub search_next: Vec<KeyCombo>,
    pub search_prev: Vec<KeyCombo>,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            quit: parse_combos(&["q", "ctrl+c"]),
            scroll_down: parse_combos(&["j", "down"]),
            scroll_up: parse_combos(&["k", "up"]),
            half_page_down: parse_combos(&["ctrl+d"]),
            half_page_up: parse_combos(&["ctrl+u"]),
            page_down: parse_combos(&["ctrl+f", "pagedown", "space"]),
            page_up: parse_combos(&["ctrl+b", "pageup"]),
            top: parse_combos(&["g", "home"]),
            bottom: parse_combos(&["shift+g", "G", "end"]),
            toggle_help: parse_combos(&["?"]),
            search_forward: parse_combos(&["/"]),
            search_next: parse_combos(&["n"]),
            search_prev: parse_combos(&["N", "shift+n"]),
        }
    }
}

fn parse_combos(specs: &[&str]) -> Vec<KeyCombo> {
    specs
        .iter()
        .map(|s| KeyCombo::parse(s).expect("invalid default key combo"))
        .collect()
}

impl KeybindingsConfig {
    pub fn resolve_action(&self, event: &KeyEvent) -> Option<Action> {
        let bindings: &[(Action, &[KeyCombo])] = &[
            (Action::Quit, &self.quit),
            (Action::ScrollDown, &self.scroll_down),
            (Action::ScrollUp, &self.scroll_up),
            (Action::HalfPageDown, &self.half_page_down),
            (Action::HalfPageUp, &self.half_page_up),
            (Action::PageDown, &self.page_down),
            (Action::PageUp, &self.page_up),
            (Action::Top, &self.top),
            (Action::Bottom, &self.bottom),
            (Action::ToggleHelp, &self.toggle_help),
            (Action::SearchForward, &self.search_forward),
            (Action::SearchNext, &self.search_next),
            (Action::SearchPrev, &self.search_prev),
        ];

        for (action, combos) in bindings {
            for combo in *combos {
                if combo.matches(event) {
                    return Some(*action);
                }
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// ThemeConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub heading1: ThemeColor,
    pub heading2: ThemeColor,
    pub heading3: ThemeColor,
    pub heading4: ThemeColor,
    pub heading5: ThemeColor,
    pub heading6: ThemeColor,
    pub code_block_fg: ThemeColor,
    pub code_block_bg: ThemeColor,
    pub inline_code_fg: ThemeColor,
    pub inline_code_bg: ThemeColor,
    pub blockquote: ThemeColor,
    pub link: ThemeColor,
    pub link_url: ThemeColor,
    pub horizontal_rule: ThemeColor,
    pub status_bar_bg: ThemeColor,
    pub status_bar_fg: ThemeColor,
    pub status_bar_message_fg: ThemeColor,
    pub json_key: ThemeColor,
    pub json_string: ThemeColor,
    pub json_number: ThemeColor,
    pub json_boolean: ThemeColor,
    pub json_null: ThemeColor,
    pub json_punctuation: ThemeColor,
    pub line_number: ThemeColor,
    pub mermaid_node_border: ThemeColor,
    pub mermaid_node_text: ThemeColor,
    pub mermaid_edge: ThemeColor,
    pub mermaid_edge_label: ThemeColor,
    pub search_match_fg: ThemeColor,
    pub search_match_bg: ThemeColor,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            heading1: ThemeColor(Color::Magenta),
            heading2: ThemeColor(Color::Cyan),
            heading3: ThemeColor(Color::Yellow),
            heading4: ThemeColor(Color::Green),
            heading5: ThemeColor(Color::Blue),
            heading6: ThemeColor(Color::White),
            code_block_fg: ThemeColor(Color::LightGreen),
            code_block_bg: ThemeColor(Color::Rgb(40, 40, 40)),
            inline_code_fg: ThemeColor(Color::LightYellow),
            inline_code_bg: ThemeColor(Color::DarkGray),
            blockquote: ThemeColor(Color::Green),
            link: ThemeColor(Color::Blue),
            link_url: ThemeColor(Color::DarkGray),
            horizontal_rule: ThemeColor(Color::DarkGray),
            status_bar_bg: ThemeColor(Color::DarkGray),
            status_bar_fg: ThemeColor(Color::White),
            status_bar_message_fg: ThemeColor(Color::Yellow),
            json_key: ThemeColor(Color::Cyan),
            json_string: ThemeColor(Color::Green),
            json_number: ThemeColor(Color::LightYellow),
            json_boolean: ThemeColor(Color::Yellow),
            json_null: ThemeColor(Color::DarkGray),
            json_punctuation: ThemeColor(Color::White),
            line_number: ThemeColor(Color::DarkGray),
            mermaid_node_border: ThemeColor(Color::Cyan),
            mermaid_node_text: ThemeColor(Color::White),
            mermaid_edge: ThemeColor(Color::DarkGray),
            mermaid_edge_label: ThemeColor(Color::Yellow),
            search_match_fg: ThemeColor(Color::Black),
            search_match_bg: ThemeColor(Color::Yellow),
        }
    }
}

// ---------------------------------------------------------------------------
// BehaviorConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub line_wrap: bool,
    pub debounce_ms: u64,
    pub scroll_speed: usize,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            line_wrap: true,
            debounce_ms: 200,
            scroll_speed: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub keybindings: KeybindingsConfig,
    pub theme: ThemeConfig,
    pub behavior: BehaviorConfig,
}

impl Config {
    pub fn config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|d| d.join(".config").join("mdw").join("config.toml"))
    }

    pub fn load() -> anyhow::Result<Self> {
        let Some(path) = Self::config_path() else {
            return Ok(Config::default());
        };

        if !path.exists() {
            return Ok(Config::default());
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read config at {}: {e}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config at {}: {e}", path.display()))?;

        Ok(config)
    }

    pub fn write_default_config() -> anyhow::Result<PathBuf> {
        let Some(path) = Self::config_path() else {
            anyhow::bail!("Could not determine config directory");
        };

        if path.exists() {
            anyhow::bail!("Config file already exists at {}", path.display());
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&path, DEFAULT_CONFIG_TEMPLATE)?;
        Ok(path)
    }
}

// ---------------------------------------------------------------------------
// Default config template
// ---------------------------------------------------------------------------

const DEFAULT_CONFIG_TEMPLATE: &str = r##"# mdw configuration file
# Uncomment and modify options to customize behavior.
# All values shown are the defaults.

# [keybindings]
# Each key accepts an array of key combos.
# Format: "key", "ctrl+key", "shift+key", "alt+key"
# Special keys: up, down, left, right, pageup, pagedown, home, end,
#               esc, enter, space, tab, backspace, delete
#
# quit = ["q", "ctrl+c"]
# scroll_down = ["j", "down"]
# scroll_up = ["k", "up"]
# half_page_down = ["ctrl+d"]
# half_page_up = ["ctrl+u"]
# page_down = ["ctrl+f", "pagedown", "space"]
# page_up = ["ctrl+b", "pageup"]
# top = ["g", "home"]
# bottom = ["shift+g", "G", "end"]
# search_forward = ["/"]
# search_next = ["n"]
# search_prev = ["N", "shift+n"]

# [theme]
# Colors can be named colors or hex "#rrggbb".
# Named colors: black, red, green, yellow, blue, magenta, cyan, gray,
#               darkgray, lightred, lightgreen, lightyellow, lightblue,
#               lightmagenta, lightcyan, white
#
# heading1 = "magenta"
# heading2 = "cyan"
# heading3 = "yellow"
# heading4 = "green"
# heading5 = "blue"
# heading6 = "white"
# code_block_fg = "lightgreen"
# code_block_bg = "#282828"
# inline_code_fg = "lightyellow"
# inline_code_bg = "darkgray"
# blockquote = "green"
# link = "blue"
# link_url = "darkgray"
# horizontal_rule = "darkgray"
# status_bar_bg = "darkgray"
# status_bar_fg = "white"
# status_bar_message_fg = "yellow"
# json_key = "cyan"
# json_string = "green"
# json_number = "lightyellow"
# json_boolean = "yellow"
# json_null = "darkgray"
# json_punctuation = "white"
# line_number = "darkgray"
# mermaid_node_border = "cyan"
# mermaid_node_text = "white"
# mermaid_edge = "darkgray"
# mermaid_edge_label = "yellow"

# [behavior]
# line_wrap = true
# debounce_ms = 200
# scroll_speed = 1
"##;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_keycombo_parse_simple_char() {
        let combo = KeyCombo::parse("j").unwrap();
        assert!(matches!(combo.code, KeyCode::Char('j')));
        assert_eq!(combo.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_keycombo_parse_uppercase() {
        let combo = KeyCombo::parse("G").unwrap();
        assert!(matches!(combo.code, KeyCode::Char('G')));
    }

    #[test]
    fn test_keycombo_parse_shift_g() {
        let combo = KeyCombo::parse("shift+g").unwrap();
        assert!(matches!(combo.code, KeyCode::Char('G')));
        assert!(combo.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn test_keycombo_parse_ctrl() {
        let combo = KeyCombo::parse("ctrl+d").unwrap();
        assert!(matches!(combo.code, KeyCode::Char('d')));
        assert!(combo.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_keycombo_parse_special() {
        let combo = KeyCombo::parse("pagedown").unwrap();
        assert!(matches!(combo.code, KeyCode::PageDown));
    }

    #[test]
    fn test_keycombo_matches() {
        let combo = KeyCombo::parse("ctrl+d").unwrap();
        assert!(combo.matches(&key_event(KeyCode::Char('d'), KeyModifiers::CONTROL)));
        assert!(!combo.matches(&key_event(KeyCode::Char('d'), KeyModifiers::NONE)));
    }

    #[test]
    fn test_keycombo_matches_uppercase() {
        let combo = KeyCombo::parse("G").unwrap();
        // Should match 'G' with NONE or SHIFT
        assert!(combo.matches(&key_event(KeyCode::Char('G'), KeyModifiers::NONE)));
        assert!(combo.matches(&key_event(KeyCode::Char('G'), KeyModifiers::SHIFT)));
    }

    #[test]
    fn test_theme_color_named() {
        let tc = ThemeColor::parse("magenta").unwrap();
        assert!(matches!(tc.0, Color::Magenta));
    }

    #[test]
    fn test_theme_color_hex() {
        let tc = ThemeColor::parse("#282828").unwrap();
        assert!(matches!(tc.0, Color::Rgb(40, 40, 40)));
    }

    #[test]
    fn test_theme_color_invalid() {
        assert!(ThemeColor::parse("#zzzzzz").is_err());
        assert!(ThemeColor::parse("notacolor").is_err());
    }

    #[test]
    fn test_empty_config_parses() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.behavior.scroll_speed, 1);
        assert!(config.behavior.line_wrap);
    }

    #[test]
    fn test_partial_config_parses() {
        let config: Config = toml::from_str(
            r#"
            [behavior]
            scroll_speed = 3
            "#,
        )
        .unwrap();
        assert_eq!(config.behavior.scroll_speed, 3);
        assert!(config.behavior.line_wrap); // default preserved
    }

    #[test]
    fn test_resolve_action() {
        let kb = KeybindingsConfig::default();
        let event = key_event(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(kb.resolve_action(&event), Some(Action::ScrollDown));

        let event = key_event(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(kb.resolve_action(&event), Some(Action::Quit));

        let event = key_event(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(kb.resolve_action(&event), None);
    }

    #[test]
    fn test_custom_keybinding_config() {
        let config: Config = toml::from_str(
            r#"
            [keybindings]
            scroll_down = ["s", "ctrl+n"]
            "#,
        )
        .unwrap();
        let event = key_event(KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(
            config.keybindings.resolve_action(&event),
            Some(Action::ScrollDown)
        );
        // Original 'j' should no longer work since we replaced the binding
        let event = key_event(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(config.keybindings.resolve_action(&event), None);
    }
}
