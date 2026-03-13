use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::config::ThemeConfig;

struct MindmapNode {
    label: String,
    children: Vec<MindmapNode>,
}

/// Parse indentation-based mindmap syntax into a tree.
fn parse_mindmap(input: &str) -> Option<MindmapNode> {
    let mut lines: Vec<(usize, String)> = Vec::new();

    for raw in input.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == "mindmap" || trimmed.starts_with("%%") {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        let label = strip_shape(trimmed);
        lines.push((indent, label));
    }

    if lines.is_empty() {
        return None;
    }

    let (_, root_label) = lines.remove(0);
    let mut root = MindmapNode {
        label: root_label,
        children: Vec::new(),
    };
    parse_children(&mut root, &lines, 0, &mut 0);
    Some(root)
}

/// Recursively parse children based on indentation.
fn parse_children(
    parent: &mut MindmapNode,
    lines: &[(usize, String)],
    parent_indent: usize,
    idx: &mut usize,
) {
    while *idx < lines.len() {
        let (indent, _) = &lines[*idx];
        if *indent <= parent_indent {
            return;
        }
        let child_indent = *indent;
        let child_label = lines[*idx].1.clone();
        let mut child = MindmapNode {
            label: child_label,
            children: Vec::new(),
        };
        *idx += 1;
        parse_children(&mut child, lines, child_indent, idx);
        parent.children.push(child);
    }
}

/// Strip Mermaid node shape markers: `((text))`, `(text)`, `[text]`, `{text}`, etc.
fn strip_shape(s: &str) -> String {
    let s = s.trim();
    // Try double markers first: ((...)), [[...]], {{...}}
    for (open, close) in [("((", "))"), ("[[", "]]"), ("{{", "}}")] {
        if let Some(inner) = s.strip_prefix(open).and_then(|r| r.strip_suffix(close)) {
            return inner.trim().to_string();
        }
    }
    // Single markers: (...), [...], {...}
    for (open, close) in [("(", ")"), ("[", "]"), ("{", "}")] {
        if let Some(inner) = s.strip_prefix(open).and_then(|r| r.strip_suffix(close)) {
            return inner.trim().to_string();
        }
    }
    s.to_string()
}

pub fn render_mindmap(input: &str, theme: &ThemeConfig) -> Text<'static> {
    let Some(root) = parse_mindmap(input) else {
        return Text::raw(input.to_string());
    };

    let root_style = Style::default()
        .fg(theme.heading1.0)
        .add_modifier(Modifier::BOLD);
    let branch_style = Style::default().fg(theme.code_block_fg.0);

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(root.label.clone(), root_style)));

    render_children(&root.children, &mut lines, "", branch_style);

    Text::from(lines)
}

fn render_children(
    children: &[MindmapNode],
    lines: &mut Vec<Line<'static>>,
    prefix: &str,
    style: Style,
) {
    for (i, child) in children.iter().enumerate() {
        let is_last = i == children.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        lines.push(Line::from(Span::styled(
            format!("{prefix}{connector}{}", child.label),
            style,
        )));

        let new_prefix = format!("{prefix}{child_prefix}");
        render_children(&child.children, lines, &new_prefix, style);
    }
}
