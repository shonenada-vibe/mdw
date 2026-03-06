use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::config::ThemeConfig;

#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub line: usize,
    pub col_start: usize,
    pub col_end: usize,
    pub url: String,
}

pub fn render_json(input: &str, theme: &ThemeConfig) -> Text<'static> {
    let pretty = match serde_json::from_str::<serde_json::Value>(input) {
        Ok(val) => serde_json::to_string_pretty(&val).unwrap_or_else(|_| input.to_string()),
        Err(_) => return render_plain(input),
    };

    let key_style = Style::default().fg(theme.json_key.0);
    let string_style = Style::default().fg(theme.json_string.0);
    let number_style = Style::default().fg(theme.json_number.0);
    let bool_style = Style::default().fg(theme.json_boolean.0);
    let null_style = Style::default().fg(theme.json_null.0);
    let punct_style = Style::default().fg(theme.json_punctuation.0);

    let lines: Vec<Line<'static>> = pretty
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let indent = &line[..line.len() - trimmed.len()];
            let mut spans: Vec<Span<'static>> = Vec::new();

            if !indent.is_empty() {
                spans.push(Span::raw(indent.to_string()));
            }

            let mut chars = trimmed.char_indices().peekable();
            while let Some(&(i, ch)) = chars.peek() {
                match ch {
                    '"' => {
                        // Find the end of the string (handling escapes)
                        let mut end = i + 1;
                        let bytes = trimmed.as_bytes();
                        while end < bytes.len() {
                            if bytes[end] == b'\\' {
                                end += 2; // skip escaped char
                            } else if bytes[end] == b'"' {
                                end += 1;
                                break;
                            } else {
                                end += 1;
                            }
                        }
                        let s = &trimmed[i..end];

                        // Check if this string is a key (followed by ':')
                        let rest = trimmed[end..].trim_start();
                        let style = if rest.starts_with(':') {
                            key_style
                        } else {
                            string_style
                        };

                        spans.push(Span::styled(s.to_string(), style));
                        // Advance the iterator past the string
                        while chars.peek().is_some_and(|&(j, _)| j < end) {
                            chars.next();
                        }
                    }
                    '{' | '}' | '[' | ']' | ':' | ',' => {
                        spans.push(Span::styled(ch.to_string(), punct_style));
                        chars.next();
                    }
                    _ if ch.is_ascii_digit() || ch == '-' => {
                        // Number
                        let start = i;
                        chars.next();
                        while chars
                            .peek()
                            .is_some_and(|&(_, c)| c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-')
                        {
                            chars.next();
                        }
                        let end = chars.peek().map_or(trimmed.len(), |&(j, _)| j);
                        let token = &trimmed[start..end];
                        // Verify it's actually a number (not just a '-' before something else)
                        if token.parse::<f64>().is_ok() {
                            spans.push(Span::styled(token.to_string(), number_style));
                        } else {
                            spans.push(Span::raw(token.to_string()));
                        }
                    }
                    't' if trimmed[i..].starts_with("true") => {
                        spans.push(Span::styled("true".to_string(), bool_style));
                        for _ in 0..4 { chars.next(); }
                    }
                    'f' if trimmed[i..].starts_with("false") => {
                        spans.push(Span::styled("false".to_string(), bool_style));
                        for _ in 0..5 { chars.next(); }
                    }
                    'n' if trimmed[i..].starts_with("null") => {
                        spans.push(Span::styled("null".to_string(), null_style));
                        for _ in 0..4 { chars.next(); }
                    }
                    _ => {
                        spans.push(Span::raw(ch.to_string()));
                        chars.next();
                    }
                }
            }

            Line::from(spans)
        })
        .collect();

    Text::from(lines)
}

pub fn render_plain(input: &str) -> Text<'static> {
    let lines: Vec<Line<'static>> = input
        .lines()
        .map(|l| Line::from(l.to_string()))
        .collect();
    Text::from(lines)
}

pub fn render_markdown(input: &str, theme: &ThemeConfig) -> (Text<'static>, Vec<LinkInfo>) {
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(input, options);
    let mut writer = MarkdownWriter::new(theme.clone());

    for event in parser {
        writer.handle_event(event);
    }

    writer.finish()
}

enum ListKind {
    Unordered,
    Ordered(u64),
}

struct MarkdownWriter {
    lines: Vec<Line<'static>>,
    current_spans: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    list_stack: Vec<ListKind>,
    in_code_block: bool,
    code_block_lines: Vec<String>,
    in_blockquote: bool,
    link_url: Option<String>,
    link_text_start_col: usize,
    link_infos: Vec<LinkInfo>,
    theme: ThemeConfig,
}

impl MarkdownWriter {
    fn new(theme: ThemeConfig) -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: Vec::new(),
            list_stack: Vec::new(),
            in_code_block: false,
            code_block_lines: Vec::new(),
            in_blockquote: false,
            link_url: None,
            link_text_start_col: 0,
            link_infos: Vec::new(),
            theme,
        }
    }

    fn current_col(&self) -> usize {
        self.current_spans.iter().map(|s| s.content.len()).sum()
    }

    fn current_style(&self) -> Style {
        let mut style = Style::default();
        for s in &self.style_stack {
            style = style.patch(*s);
        }
        style
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            let spans: Vec<Span<'static>> = self.current_spans.drain(..).collect();
            self.lines.push(Line::from(spans));
        }
    }

    fn push_blank_line(&mut self) {
        self.lines.push(Line::from(""));
    }

    fn list_prefix(&self) -> String {
        if self.list_stack.is_empty() {
            return String::new();
        }
        let indent = "  ".repeat(self.list_stack.len() - 1);
        match self.list_stack.last() {
            Some(ListKind::Unordered) => format!("{indent}  * "),
            Some(ListKind::Ordered(n)) => format!("{indent}  {n}. "),
            None => String::new(),
        }
    }

    fn handle_event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.handle_start(tag),
            Event::End(tag) => self.handle_end(tag),
            Event::Text(text) => {
                if self.in_code_block {
                    self.code_block_lines.push(text.to_string());
                } else {
                    let style = self.current_style();
                    self.current_spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::Code(code) => {
                let style = Style::default()
                    .fg(self.theme.inline_code_fg.0)
                    .bg(self.theme.inline_code_bg.0);
                self.current_spans.push(Span::styled(
                    format!(" {code} "),
                    style,
                ));
            }
            Event::SoftBreak => {
                let style = self.current_style();
                self.current_spans.push(Span::styled(" ", style));
            }
            Event::HardBreak => {
                self.flush_line();
            }
            Event::Rule => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(80),
                    Style::default().fg(self.theme.horizontal_rule.0),
                )));
                self.push_blank_line();
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "[x] " } else { "[ ] " };
                let style = self.current_style();
                self.current_spans.push(Span::styled(marker.to_string(), style));
            }
            _ => {}
        }
    }

    fn handle_start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush_line();
                let (color, prefix) = match level {
                    HeadingLevel::H1 => (self.theme.heading1.0, "# "),
                    HeadingLevel::H2 => (self.theme.heading2.0, "## "),
                    HeadingLevel::H3 => (self.theme.heading3.0, "### "),
                    HeadingLevel::H4 => (self.theme.heading4.0, "#### "),
                    HeadingLevel::H5 => (self.theme.heading5.0, "##### "),
                    HeadingLevel::H6 => (self.theme.heading6.0, "###### "),
                };
                let style = Style::default()
                    .fg(color)
                    .add_modifier(Modifier::BOLD);
                self.style_stack.push(style);
                self.current_spans.push(Span::styled(prefix.to_string(), style));
            }
            Tag::Paragraph => {
                // If we're inside a list item, don't add blank line
                if self.list_stack.is_empty() {
                    self.flush_line();
                }
            }
            Tag::BlockQuote(_) => {
                self.flush_line();
                self.in_blockquote = true;
                let bq_color = self.theme.blockquote.0;
                self.style_stack.push(Style::default().fg(bq_color));
                self.current_spans.push(Span::styled(
                    "│ ".to_string(),
                    Style::default().fg(bq_color),
                ));
            }
            Tag::CodeBlock(_) => {
                self.flush_line();
                self.in_code_block = true;
                self.code_block_lines.clear();
            }
            Tag::List(first_item) => {
                self.flush_line();
                match first_item {
                    Some(start) => self.list_stack.push(ListKind::Ordered(start)),
                    None => self.list_stack.push(ListKind::Unordered),
                }
            }
            Tag::Item => {
                self.flush_line();
                let prefix = self.list_prefix();
                let style = self.current_style();
                self.current_spans.push(Span::styled(prefix, style));
            }
            Tag::Emphasis => {
                self.style_stack.push(Style::default().add_modifier(Modifier::ITALIC));
            }
            Tag::Strong => {
                self.style_stack.push(Style::default().add_modifier(Modifier::BOLD));
            }
            Tag::Strikethrough => {
                self.style_stack.push(Style::default().add_modifier(Modifier::CROSSED_OUT));
            }
            Tag::Link { dest_url, .. } => {
                self.link_url = Some(dest_url.to_string());
                self.link_text_start_col = self.current_col();
                self.style_stack.push(
                    Style::default()
                        .fg(self.theme.link.0)
                        .add_modifier(Modifier::UNDERLINED),
                );
            }
            Tag::Table(_) => {
                self.flush_line();
            }
            Tag::TableHead => {}
            Tag::TableRow => {
                self.flush_line();
                let style = self.current_style();
                self.current_spans.push(Span::styled("│ ", style));
            }
            Tag::TableCell => {}
            _ => {}
        }
    }

    fn handle_end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                self.style_stack.pop();
                self.flush_line();
                self.push_blank_line();
            }
            TagEnd::Paragraph => {
                self.flush_line();
                if self.list_stack.is_empty() {
                    self.push_blank_line();
                }
            }
            TagEnd::BlockQuote(_) => {
                self.in_blockquote = false;
                self.style_stack.pop();
                self.flush_line();
                self.push_blank_line();
            }
            TagEnd::CodeBlock => {
                let code_style = Style::default()
                    .fg(self.theme.code_block_fg.0)
                    .bg(self.theme.code_block_bg.0);
                for code_line in self.code_block_lines.drain(..) {
                    // Split by newlines within the code text
                    for line in code_line.split('\n') {
                        self.lines.push(Line::from(Span::styled(
                            format!("  {line}"),
                            code_style,
                        )));
                    }
                }
                self.in_code_block = false;
                self.push_blank_line();
            }
            TagEnd::List(_) => {
                // Increment ordered list counter if applicable
                if let Some(ListKind::Ordered(_)) = self.list_stack.last() {
                    // counter is incremented per-item in Item end
                }
                self.list_stack.pop();
                self.flush_line();
                if self.list_stack.is_empty() {
                    self.push_blank_line();
                }
            }
            TagEnd::Item => {
                self.flush_line();
                // Increment ordered list counter
                if let Some(ListKind::Ordered(n)) = self.list_stack.last_mut() {
                    *n += 1;
                }
            }
            TagEnd::Emphasis => {
                self.style_stack.pop();
            }
            TagEnd::Strong => {
                self.style_stack.pop();
            }
            TagEnd::Strikethrough => {
                self.style_stack.pop();
            }
            TagEnd::Link => {
                self.style_stack.pop();
                if let Some(url) = self.link_url.take() {
                    let url_display = format!(" ({url})");
                    self.current_spans.push(Span::styled(
                        url_display.clone(),
                        Style::default().fg(self.theme.link_url.0),
                    ));
                    let col_end = self.current_col();
                    let line_idx = self.lines.len();
                    self.link_infos.push(LinkInfo {
                        line: line_idx,
                        col_start: self.link_text_start_col,
                        col_end,
                        url,
                    });
                }
            }
            TagEnd::Table => {
                self.flush_line();
                self.push_blank_line();
            }
            TagEnd::TableHead => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(80),
                    Style::default().fg(self.theme.horizontal_rule.0),
                )));
            }
            TagEnd::TableRow => {
                let style = self.current_style();
                self.current_spans.push(Span::styled(" │", style));
                self.flush_line();
            }
            TagEnd::TableCell => {
                let style = self.current_style();
                self.current_spans.push(Span::styled(" │ ", style));
            }
            _ => {}
        }
    }

    fn finish(mut self) -> (Text<'static>, Vec<LinkInfo>) {
        self.flush_line();
        (Text::from(self.lines), self.link_infos)
    }
}
