use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

pub fn highlight_code(code: &str, lang: &str) -> Option<Vec<Line<'static>>> {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];

    let syntax = ss
        .find_syntax_by_token(lang)
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    for line in LinesWithEndings::from(code) {
        let regions = highlighter.highlight_line(line, &ss).ok()?;
        let spans: Vec<Span<'static>> = regions
            .into_iter()
            .map(|(style, text)| {
                let ratatui_style = translate_style(style);
                Span::styled(text.replace('\n', "").to_string(), ratatui_style)
            })
            .collect();
        lines.push(Line::from(spans));
    }

    Some(lines)
}

fn translate_style(style: syntect::highlighting::Style) -> Style {
    let mut ratatui_style = Style::default();

    if style.foreground.a > 0 {
        ratatui_style = ratatui_style.fg(Color::Rgb(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        ));
    }

    if style.background.a > 0 {
        ratatui_style = ratatui_style.bg(Color::Rgb(
            style.background.r,
            style.background.g,
            style.background.b,
        ));
    }

    let font = style.font_style;
    if font.contains(syntect::highlighting::FontStyle::BOLD) {
        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
    }
    if font.contains(syntect::highlighting::FontStyle::ITALIC) {
        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
    }
    if font.contains(syntect::highlighting::FontStyle::UNDERLINE) {
        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
    }

    ratatui_style
}
