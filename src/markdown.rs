use std::path::PathBuf;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::config::ThemeConfig;
use crate::content::{ContentBlock, ImageSource};
use crate::d2;
use crate::mermaid;
use crate::syntax_highlight;

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
                        let mut end = i + 1;
                        let bytes = trimmed.as_bytes();
                        while end < bytes.len() {
                            if bytes[end] == b'\\' {
                                end += 2;
                            } else if bytes[end] == b'"' {
                                end += 1;
                                break;
                            } else {
                                end += 1;
                            }
                        }
                        let s = &trimmed[i..end];

                        let rest = trimmed[end..].trim_start();
                        let style = if rest.starts_with(':') {
                            key_style
                        } else {
                            string_style
                        };

                        spans.push(Span::styled(s.to_string(), style));
                        while chars.peek().is_some_and(|&(j, _)| j < end) {
                            chars.next();
                        }
                    }
                    '{' | '}' | '[' | ']' | ':' | ',' => {
                        spans.push(Span::styled(ch.to_string(), punct_style));
                        chars.next();
                    }
                    _ if ch.is_ascii_digit() || ch == '-' => {
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

pub fn render_markdown(input: &str, theme: &ThemeConfig) -> (Vec<ContentBlock>, Vec<LinkInfo>) {
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
    code_block_lang: Option<String>,
    code_block_lines: Vec<String>,
    in_blockquote: bool,
    link_url: Option<String>,
    link_text_start_col: usize,
    link_infos: Vec<LinkInfo>,
    theme: ThemeConfig,
    // Image support
    blocks: Vec<ContentBlock>,
    block_start_row: usize,
    in_image: bool,
    image_url: Option<String>,
    image_alt_parts: Vec<String>,
}

impl MarkdownWriter {
    fn new(theme: ThemeConfig) -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: Vec::new(),
            list_stack: Vec::new(),
            in_code_block: false,
            code_block_lang: None,
            code_block_lines: Vec::new(),
            in_blockquote: false,
            link_url: None,
            link_text_start_col: 0,
            link_infos: Vec::new(),
            theme,
            blocks: Vec::new(),
            block_start_row: 0,
            in_image: false,
            image_url: None,
            image_alt_parts: Vec::new(),
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

    fn flush_text_block(&mut self) {
        self.flush_line();
        if !self.lines.is_empty() {
            let lines: Vec<Line<'static>> = self.lines.drain(..).collect();
            let line_count = lines.len();
            self.blocks.push(ContentBlock::Text { lines });
            self.block_start_row += line_count;
        }
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
                if self.in_image {
                    self.image_alt_parts.push(text.to_string());
                } else if self.in_code_block {
                    self.code_block_lines.push(text.to_string());
                } else {
                    let style = self.current_style();
                    self.current_spans.push(Span::styled(text.to_string(), style));
                }
            }
            Event::Code(code) => {
                if self.in_image {
                    self.image_alt_parts.push(code.to_string());
                } else {
                    let style = Style::default()
                        .fg(self.theme.inline_code_fg.0)
                        .bg(self.theme.inline_code_bg.0);
                    self.current_spans.push(Span::styled(
                        format!(" {code} "),
                        style,
                    ));
                }
            }
            Event::SoftBreak => {
                if !self.in_image {
                    let style = self.current_style();
                    self.current_spans.push(Span::styled(" ", style));
                }
            }
            Event::HardBreak => {
                if !self.in_image {
                    self.flush_line();
                }
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
            Tag::CodeBlock(kind) => {
                self.flush_line();
                self.in_code_block = true;
                self.code_block_lang = match &kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang = lang.split_whitespace().next().unwrap_or("").to_lowercase();
                        if lang.is_empty() { None } else { Some(lang) }
                    }
                    CodeBlockKind::Indented => None,
                };
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
            Tag::Image { dest_url, .. } => {
                self.flush_text_block();
                self.in_image = true;
                self.image_url = Some(dest_url.to_string());
                self.image_alt_parts.clear();
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
                let lang = self.code_block_lang.take();
                let code_content: String = self.code_block_lines.drain(..).collect::<Vec<_>>().join("");
                let is_diagram = matches!(lang.as_deref(), Some("mermaid") | Some("d2"));

                if is_diagram {
                    let rendered = match lang.as_deref() {
                        Some("mermaid") => mermaid::render_mermaid(&code_content, &self.theme),
                        Some("d2") => d2::render_d2(&code_content, &self.theme),
                        _ => unreachable!(),
                    };
                    for line in rendered.lines {
                        self.lines.push(line);
                    }
                } else if let Some(ref lang_str) = lang {
                    if let Some(highlighted) = syntax_highlight::highlight_code(&code_content, lang_str) {
                        let bg = self.theme.code_block_bg.0;
                        for line in highlighted {
                            let mut indented_spans = vec![Span::styled("  ", Style::default().bg(bg))];
                            for span in line.spans {
                                let style = if span.style.bg.is_none() {
                                    span.style.bg(bg)
                                } else {
                                    span.style
                                };
                                indented_spans.push(Span::styled(span.content, style));
                            }
                            self.lines.push(Line::from(indented_spans));
                        }
                    } else {
                        let code_style = Style::default()
                            .fg(self.theme.code_block_fg.0)
                            .bg(self.theme.code_block_bg.0);
                        for line in code_content.split('\n') {
                            self.lines.push(Line::from(Span::styled(
                                format!("  {line}"),
                                code_style,
                            )));
                        }
                    }
                } else {
                    let code_style = Style::default()
                        .fg(self.theme.code_block_fg.0)
                        .bg(self.theme.code_block_bg.0);
                    for line in code_content.split('\n') {
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
                if let Some(ListKind::Ordered(_)) = self.list_stack.last() {
                }
                self.list_stack.pop();
                self.flush_line();
                if self.list_stack.is_empty() {
                    self.push_blank_line();
                }
            }
            TagEnd::Item => {
                self.flush_line();
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
                    let line_idx = self.block_start_row + self.lines.len();
                    self.link_infos.push(LinkInfo {
                        line: line_idx,
                        col_start: self.link_text_start_col,
                        col_end,
                        url,
                    });
                }
            }
            TagEnd::Image => {
                self.in_image = false;
                let alt_text = self.image_alt_parts.drain(..).collect::<String>();
                let url = self.image_url.take().unwrap_or_default();

                let source = if url.starts_with("https://") || url.starts_with("http://") {
                    ImageSource::Remote(url)
                } else {
                    ImageSource::Local(PathBuf::from(url))
                };

                self.blocks.push(ContentBlock::Image {
                    alt_text,
                    display_height: 1, // Will be computed later when terminal size is known
                    protocol: None,
                    error: None,
                    source,
                });
                self.block_start_row += 1; // Image takes at least 1 row initially
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

    fn finish(mut self) -> (Vec<ContentBlock>, Vec<LinkInfo>) {
        self.flush_text_block();
        (self.blocks, self.link_infos)
    }
}
