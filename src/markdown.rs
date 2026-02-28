use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

pub fn render_plain(input: &str) -> Text<'static> {
    let lines: Vec<Line<'static>> = input
        .lines()
        .map(|l| Line::from(l.to_string()))
        .collect();
    Text::from(lines)
}

pub fn render_markdown(input: &str) -> Text<'static> {
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(input, options);
    let mut writer = MarkdownWriter::new();

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
}

impl MarkdownWriter {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: Vec::new(),
            list_stack: Vec::new(),
            in_code_block: false,
            code_block_lines: Vec::new(),
            in_blockquote: false,
            link_url: None,
        }
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
                    .fg(Color::LightYellow)
                    .bg(Color::DarkGray);
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
                self.push_blank_line();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(80),
                    Style::default().fg(Color::DarkGray),
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
                self.push_blank_line();
                let (color, prefix) = match level {
                    HeadingLevel::H1 => (Color::Magenta, "# "),
                    HeadingLevel::H2 => (Color::Cyan, "## "),
                    HeadingLevel::H3 => (Color::Yellow, "### "),
                    HeadingLevel::H4 => (Color::Green, "#### "),
                    HeadingLevel::H5 => (Color::Blue, "##### "),
                    HeadingLevel::H6 => (Color::White, "###### "),
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
                self.style_stack.push(Style::default().fg(Color::Green));
                self.current_spans.push(Span::styled(
                    "│ ".to_string(),
                    Style::default().fg(Color::Green),
                ));
            }
            Tag::CodeBlock(_) => {
                self.flush_line();
                self.push_blank_line();
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
                self.style_stack.push(
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                );
            }
            Tag::Table(_) => {
                self.flush_line();
                self.push_blank_line();
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
                    .fg(Color::LightGreen)
                    .bg(Color::Rgb(40, 40, 40));
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
                    self.current_spans.push(Span::styled(
                        format!(" ({url})"),
                        Style::default().fg(Color::DarkGray),
                    ));
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
                    Style::default().fg(Color::DarkGray),
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

    fn finish(mut self) -> Text<'static> {
        self.flush_line();
        Text::from(self.lines)
    }
}
