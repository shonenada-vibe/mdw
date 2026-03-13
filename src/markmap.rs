use std::collections::HashSet;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use unicode_width::UnicodeWidthStr;

use crate::config::ThemeConfig;
use crate::markdown::LinkInfo;

pub struct MarkmapRenderResult {
    pub text: Text<'static>,
    pub link_infos: Vec<LinkInfo>,
}

struct MarkmapNode {
    label: String,
    children: Vec<MarkmapNode>,
}

/// Parse markdown content into a tree structure based on heading hierarchy and list items.
fn parse_markmap(input: &str) -> Option<MarkmapNode> {
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(input, options);

    let mut root: Option<MarkmapNode> = None;
    // Stack of (heading_level, node) — heading_level 0 means root
    let mut stack: Vec<(usize, MarkmapNode)> = Vec::new();
    let mut current_text = String::new();
    let mut in_heading = false;
    let mut heading_level: usize = 0;
    let mut in_list_item = false;
    let mut list_depth: usize = 0;
    // Stack of list item nodes for nested lists
    let mut item_stack: Vec<MarkmapNode> = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                heading_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                current_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                let label = current_text.trim().to_string();
                current_text.clear();

                if label.is_empty() {
                    continue;
                }

                let node = MarkmapNode {
                    label,
                    children: Vec::new(),
                };

                // Pop stack until we find a parent with lower heading level
                while let Some((lvl, _)) = stack.last() {
                    if *lvl >= heading_level {
                        let (_, child) = stack.pop().unwrap();
                        if let Some((_, parent)) = stack.last_mut() {
                            parent.children.push(child);
                        } else if let Some(ref mut r) = root {
                            r.children.push(child);
                        }
                    } else {
                        break;
                    }
                }

                if heading_level == 1 && root.is_none() {
                    root = Some(node);
                } else {
                    stack.push((heading_level, node));
                }
            }
            Event::Start(Tag::List(_)) => {
                // If we're inside a list item and have accumulated text,
                // this means a nested list is starting — push current text as parent node
                if in_list_item && !current_text.trim().is_empty() {
                    let label = current_text.trim().to_string();
                    current_text.clear();
                    item_stack.push(MarkmapNode {
                        label,
                        children: Vec::new(),
                    });
                }
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
            }
            Event::Start(Tag::Item) => {
                in_list_item = true;
                current_text.clear();
            }
            Event::End(TagEnd::Item) => {
                in_list_item = false;

                // If there's accumulated text, create a node from it.
                // The text may have been flushed already when a nested list started,
                // in which case current_text is empty and we pop from item_stack.
                let label = current_text.trim().to_string();
                current_text.clear();

                if !label.is_empty() && item_stack.is_empty() {
                    // Simple item with no nested children
                    let node = MarkmapNode {
                        label,
                        children: Vec::new(),
                    };
                    // Attach to the most recent heading on the stack, or root
                    if let Some((_, parent)) = stack.last_mut() {
                        parent.children.push(node);
                    } else if let Some(ref mut r) = root {
                        r.children.push(node);
                    }
                } else if let Some(mut node) = item_stack.pop() {
                    // Item with nested children — the label was already set
                    // when the nested list started
                    if !label.is_empty() {
                        // Shouldn't happen, but handle gracefully
                        node.children.push(MarkmapNode {
                            label,
                            children: Vec::new(),
                        });
                    }
                    if let Some(parent) = item_stack.last_mut() {
                        parent.children.push(node);
                    } else if let Some((_, parent)) = stack.last_mut() {
                        parent.children.push(node);
                    } else if let Some(ref mut r) = root {
                        r.children.push(node);
                    }
                } else if !label.is_empty() {
                    let node = MarkmapNode {
                        label,
                        children: Vec::new(),
                    };
                    if let Some((_, parent)) = stack.last_mut() {
                        parent.children.push(node);
                    } else if let Some(ref mut r) = root {
                        r.children.push(node);
                    }
                }
            }
            Event::Start(Tag::Paragraph) => {
                // pulldown-cmark wraps list item text in paragraphs — ignore
            }
            Event::End(TagEnd::Paragraph) => {}
            Event::Text(text) => {
                if in_heading || in_list_item {
                    current_text.push_str(&text);
                } else if !text.trim().is_empty() && root.is_none() && stack.is_empty() {
                    root = Some(MarkmapNode {
                        label: text.trim().to_string(),
                        children: Vec::new(),
                    });
                }
            }
            Event::Code(code) => {
                if in_heading || in_list_item {
                    current_text.push_str(&code);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_heading || in_list_item {
                    current_text.push(' ');
                }
            }
            _ => {}
        }
    }

    // Flush remaining stack
    while let Some((_, child)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(child);
        } else if let Some(ref mut r) = root {
            r.children.push(child);
        } else {
            root = Some(child);
        }
    }

    root
}

/// Build a palette of branch colors from the theme's heading1..heading6 colors.
fn branch_palette(theme: &ThemeConfig) -> [Color; 6] {
    [
        theme.heading1.0,
        theme.heading2.0,
        theme.heading3.0,
        theme.heading4.0,
        theme.heading5.0,
        theme.heading6.0,
    ]
}

/// Count total descendants of a node (not including the node itself).
fn count_descendants(node: &MarkmapNode) -> usize {
    let mut count = node.children.len();
    for child in &node.children {
        count += count_descendants(child);
    }
    count
}

pub fn render_markmap(
    input: &str,
    theme: &ThemeConfig,
    collapsed: &HashSet<String>,
    base_line: usize,
) -> MarkmapRenderResult {
    let Some(root) = parse_markmap(input) else {
        return MarkmapRenderResult {
            text: Text::raw(input.to_string()),
            link_infos: Vec::new(),
        };
    };

    let palette = branch_palette(theme);

    let root_style = Style::default()
        .fg(theme.heading1.0)
        .add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut link_infos: Vec<LinkInfo> = Vec::new();

    // Root node with ◉ indicator
    lines.push(Line::from(Span::styled(
        format!("\u{25C9} {}", root.label),
        root_style,
    )));

    // Render each top-level child with its own branch color
    let child_count = root.children.len();
    for (i, child) in root.children.iter().enumerate() {
        let is_last = i == child_count - 1;
        let color = palette[i % palette.len()];
        render_children_styled(
            child,
            &mut lines,
            &mut link_infos,
            "",
            is_last,
            color,
            collapsed,
            &format!("{i}"),
            base_line,
        );
    }

    MarkmapRenderResult {
        text: Text::from(lines),
        link_infos,
    }
}

fn render_children_styled(
    node: &MarkmapNode,
    lines: &mut Vec<Line<'static>>,
    link_infos: &mut Vec<LinkInfo>,
    prefix: &str,
    is_last: bool,
    branch_color: Color,
    collapsed: &HashSet<String>,
    path: &str,
    base_line: usize,
) {
    let connector = if is_last {
        "\u{2514}\u{2500}\u{2500} "
    } else {
        "\u{251C}\u{2500}\u{2500} "
    };
    let child_prefix = if is_last { "    " } else { "\u{2502}   " };

    let has_children = !node.children.is_empty();
    let is_collapsed = collapsed.contains(path);

    let tree_style = Style::default().fg(branch_color);

    if has_children {
        // Node with children: use ● (expanded) or ○ (collapsed), bold label
        let indicator = if is_collapsed {
            "\u{25CB} "
        } else {
            "\u{25CF} "
        };
        let label_style = Style::default()
            .fg(branch_color)
            .add_modifier(Modifier::BOLD);

        let line_idx = base_line + lines.len();

        let mut spans = vec![
            Span::styled(format!("{prefix}{connector}"), tree_style),
            Span::styled(indicator.to_string(), label_style),
            Span::styled(node.label.clone(), label_style),
        ];

        if is_collapsed {
            let desc_count = count_descendants(node);
            spans.push(Span::styled(
                format!(" [{desc_count}]"),
                Style::default().fg(branch_color),
            ));
        }

        // Compute col_start/col_end for the clickable area
        let prefix_width = UnicodeWidthStr::width(prefix) + UnicodeWidthStr::width(connector);
        let indicator_width = UnicodeWidthStr::width(indicator);
        let label_width = UnicodeWidthStr::width(node.label.as_str());
        let col_start = prefix_width;
        let col_end = prefix_width + indicator_width + label_width;

        link_infos.push(LinkInfo {
            line: line_idx,
            col_start,
            col_end,
            url: format!("#markmap:{path}"),
        });

        lines.push(Line::from(spans));

        if !is_collapsed {
            let new_prefix = format!("{prefix}{child_prefix}");
            let num_children = node.children.len();
            for (i, child) in node.children.iter().enumerate() {
                let child_is_last = i == num_children - 1;
                let child_path = format!("{path}.{i}");
                render_children_styled(
                    child,
                    lines,
                    link_infos,
                    &new_prefix,
                    child_is_last,
                    branch_color,
                    collapsed,
                    &child_path,
                    base_line,
                );
            }
        }
    } else {
        // Leaf node: use ─ indicator, normal weight
        let leaf_style = Style::default().fg(branch_color);
        lines.push(Line::from(vec![
            Span::styled(format!("{prefix}{connector}"), tree_style),
            Span::styled("\u{2500} ", leaf_style),
            Span::styled(node.label.clone(), leaf_style),
        ]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_theme() -> ThemeConfig {
        ThemeConfig::default()
    }

    #[test]
    fn test_basic_markmap() {
        let input = "# Project\n## Frontend\n- React\n- Vue\n## Backend\n- Node.js\n";
        let result = render_markmap(input, &test_theme(), &HashSet::new(), 0);
        let text: Vec<String> = result
            .text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect();

        assert!(text[0].contains("Project"));
        assert!(text[0].contains('\u{25C9}')); // root indicator
        assert!(text.iter().any(|l| l.contains("Frontend")));
        assert!(text.iter().any(|l| l.contains("React")));
        assert!(text.iter().any(|l| l.contains("Vue")));
        assert!(text.iter().any(|l| l.contains("Backend")));
        assert!(text.iter().any(|l| l.contains("Node.js")));
    }

    #[test]
    fn test_nested_list() {
        let input = "# Root\n- Parent\n  - Child\n";
        let result = render_markmap(input, &test_theme(), &HashSet::new(), 0);
        let text: Vec<String> = result
            .text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect();

        assert!(text[0].contains("Root"));
        assert!(text.iter().any(|l| l.contains("Parent")));
        assert!(text.iter().any(|l| l.contains("Child")));
    }

    #[test]
    fn test_empty_input() {
        let input = "";
        let result = render_markmap(input, &test_theme(), &HashSet::new(), 0);
        assert!(result.text.lines.is_empty() || result.text.lines[0].spans.is_empty());
    }

    #[test]
    fn test_collapsed_node() {
        let input = "# Project\n## Frontend\n- React\n- Vue\n## Backend\n- Node.js\n";
        let mut collapsed = HashSet::new();
        collapsed.insert("0".to_string()); // Collapse "Frontend" (first top-level child)

        let result = render_markmap(input, &test_theme(), &collapsed, 0);
        let text: Vec<String> = result
            .text
            .lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect();

        // Frontend should show as collapsed with descendant count
        let frontend_line = text.iter().find(|l| l.contains("Frontend")).unwrap();
        assert!(frontend_line.contains('\u{25CB}')); // collapsed indicator
        assert!(frontend_line.contains("[2]")); // 2 descendants

        // React and Vue should NOT appear (collapsed)
        assert!(!text.iter().any(|l| l.contains("React")));
        assert!(!text.iter().any(|l| l.contains("Vue")));

        // Backend should still appear
        assert!(text.iter().any(|l| l.contains("Backend")));
        assert!(text.iter().any(|l| l.contains("Node.js")));
    }

    #[test]
    fn test_link_infos_generated() {
        let input = "# Project\n## Frontend\n- React\n## Backend\n- Node.js\n";
        let result = render_markmap(input, &test_theme(), &HashSet::new(), 0);

        // Should have link infos for nodes with children (Frontend, Backend)
        assert!(result.link_infos.len() >= 2);
        assert!(result.link_infos.iter().any(|l| l.url == "#markmap:0"));
        assert!(result.link_infos.iter().any(|l| l.url == "#markmap:1"));
    }
}
