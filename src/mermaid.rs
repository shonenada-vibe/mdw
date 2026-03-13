use std::collections::HashMap;
use std::collections::VecDeque;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::config::ThemeConfig;
use crate::markdown;

pub fn render_mermaid(input: &str, theme: &ThemeConfig) -> Text<'static> {
    let first_line = input
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with("%%"));

    let Some(first) = first_line else {
        return markdown::render_plain(input);
    };

    let lower = first.to_lowercase();
    if lower.starts_with("graph ") || lower.starts_with("flowchart ") {
        match FlowGraph::parse(input) {
            Some(g) => g.render(theme),
            None => markdown::render_plain(input),
        }
    } else if lower.starts_with("sequencediagram") {
        render_sequence(input, theme)
    } else if lower.starts_with("classdiagram") {
        render_class(input, theme)
    } else if lower.starts_with("gantt") {
        render_gantt(input, theme)
    } else if lower.starts_with("gitgraph") {
        render_git(input, theme)
    } else if lower.starts_with("journey") {
        render_journey(input, theme)
    } else {
        markdown::render_plain(input)
    }
}

// ===========================================================================
// Flowchart / Graph (existing)
// ===========================================================================

const DIR_UP: u8 = 1;
const DIR_DOWN: u8 = 2;
const DIR_LEFT: u8 = 4;
const DIR_RIGHT: u8 = 8;

#[derive(Clone, Copy)]
enum Direction {
    TopDown,
    LeftRight,
}

#[derive(Clone)]
enum Shape {
    Rectangle,
    Rounded,
    Diamond,
    Default,
}

struct FNode {
    label: String,
    shape: Shape,
}

struct FEdge {
    from: usize,
    to: usize,
    label: Option<String>,
}

struct FlowGraph {
    direction: Direction,
    nodes: Vec<FNode>,
    edges: Vec<FEdge>,
    node_map: HashMap<String, usize>,
}

impl FlowGraph {
    fn parse(input: &str) -> Option<Self> {
        let mut lines_iter = input.lines();
        let direction = loop {
            let line = lines_iter.next()?.trim();
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }
            break Self::parse_direction(line)?;
        };

        let mut graph = FlowGraph {
            direction,
            nodes: Vec::new(),
            edges: Vec::new(),
            node_map: HashMap::new(),
        };

        for line in lines_iter {
            let line = line.trim();
            if line.is_empty()
                || line.starts_with("%%")
                || line == "end"
                || line.starts_with("subgraph")
                || line.starts_with("classDef")
                || line.starts_with("class ")
                || line.starts_with("style ")
                || line.starts_with("click ")
                || line.starts_with("linkStyle")
            {
                continue;
            }
            graph.parse_line(line);
        }

        if graph.nodes.is_empty() {
            return None;
        }
        Some(graph)
    }

    fn parse_direction(line: &str) -> Option<Direction> {
        let lower = line.to_lowercase();
        let parts: Vec<&str> = lower.split_whitespace().collect();
        if parts.len() < 2 || (parts[0] != "graph" && parts[0] != "flowchart") {
            return None;
        }
        match parts[1] {
            "td" | "tb" | "bt" => Some(Direction::TopDown),
            "lr" | "rl" => Some(Direction::LeftRight),
            _ => None,
        }
    }

    fn ensure_node(&mut self, id: &str, label: Option<String>, shape: Shape) -> usize {
        if let Some(&idx) = self.node_map.get(id) {
            if let Some(l) = label {
                self.nodes[idx].label = l;
                self.nodes[idx].shape = shape;
            }
            return idx;
        }
        let idx = self.nodes.len();
        let lbl = label.unwrap_or_else(|| id.to_string());
        self.nodes.push(FNode { label: lbl, shape });
        self.node_map.insert(id.to_string(), idx);
        idx
    }

    fn parse_line(&mut self, line: &str) {
        let bytes = line.as_bytes();
        let mut pos = skip_ws(bytes, 0);
        let (id, label, shape, new_pos) = match parse_node_ref(bytes, pos) {
            Some(v) => v,
            None => return,
        };
        pos = new_pos;
        let mut from = self.ensure_node(&id, label, shape);

        loop {
            pos = skip_ws(bytes, pos);
            if pos >= bytes.len() {
                break;
            }
            let (edge_label, new_pos) = match parse_arrow(bytes, pos) {
                Some(v) => v,
                None => break,
            };
            pos = new_pos;
            pos = skip_ws(bytes, pos);
            let (id, label, shape, new_pos) = match parse_node_ref(bytes, pos) {
                Some(v) => v,
                None => break,
            };
            pos = new_pos;
            let to = self.ensure_node(&id, label, shape);
            self.edges.push(FEdge {
                from,
                to,
                label: edge_label,
            });
            from = to;
        }
    }

    fn assign_layers(&self) -> Vec<usize> {
        let n = self.nodes.len();
        if n == 0 {
            return vec![];
        }
        let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
        let mut in_deg = vec![0u32; n];
        for edge in &self.edges {
            adj[edge.from].push(edge.to);
            in_deg[edge.to] += 1;
        }
        let mut layers = vec![0usize; n];
        let mut queue: VecDeque<usize> = VecDeque::new();
        let mut processed = vec![false; n];
        for i in 0..n {
            if in_deg[i] == 0 {
                queue.push_back(i);
            }
        }
        while let Some(node) = queue.pop_front() {
            processed[node] = true;
            for &next in &adj[node] {
                layers[next] = layers[next].max(layers[node] + 1);
                in_deg[next] -= 1;
                if in_deg[next] == 0 {
                    queue.push_back(next);
                }
            }
        }
        let max_layer = layers.iter().copied().max().unwrap_or(0);
        for i in 0..n {
            if !processed[i] {
                layers[i] = max_layer + 1;
            }
        }
        layers
    }

    fn render(&self, theme: &ThemeConfig) -> Text<'static> {
        match self.direction {
            Direction::TopDown => self.render_td(theme),
            Direction::LeftRight => self.render_lr(theme),
        }
    }

    fn render_td(&self, theme: &ThemeConfig) -> Text<'static> {
        let layers = self.assign_layers();
        let max_layer = layers.iter().copied().max().unwrap_or(0);
        let mut layer_nodes: Vec<Vec<usize>> = vec![vec![]; max_layer + 1];
        for (i, &layer) in layers.iter().enumerate() {
            layer_nodes[layer].push(i);
        }
        let node_widths: Vec<usize> = self
            .nodes
            .iter()
            .map(|n| (n.label.chars().count() + 4).max(6))
            .collect();
        let node_height: usize = 3;
        let v_gap: usize = 3;
        let h_gap: usize = 4;
        let layer_widths: Vec<usize> = layer_nodes
            .iter()
            .map(|nodes| {
                if nodes.is_empty() {
                    return 0;
                }
                let total: usize = nodes.iter().map(|&i| node_widths[i]).sum();
                total + nodes.len().saturating_sub(1) * h_gap
            })
            .collect();
        let max_width = layer_widths.iter().copied().max().unwrap_or(0).max(1);
        let mut node_x = vec![0usize; self.nodes.len()];
        let mut node_y = vec![0usize; self.nodes.len()];
        for (li, nodes) in layer_nodes.iter().enumerate() {
            let lw = layer_widths[li];
            let start_x = max_width.saturating_sub(lw) / 2;
            let y = li * (node_height + v_gap);
            let mut x = start_x;
            for &ni in nodes {
                node_x[ni] = x;
                node_y[ni] = y;
                x += node_widths[ni] + h_gap;
            }
        }
        let grid_width = max_width + 4;
        let grid_height = (max_layer + 1) * (node_height + v_gap);
        let mut grid = Grid::new(grid_width, grid_height);
        let border_style = Style::default().fg(theme.mermaid_node_border.0);
        let text_style = Style::default().fg(theme.mermaid_node_text.0);
        let edge_style = Style::default().fg(theme.mermaid_edge.0);
        let label_style = Style::default().fg(theme.mermaid_edge_label.0);

        let mut children_map: HashMap<usize, Vec<(usize, Option<&str>)>> = HashMap::new();
        for edge in &self.edges {
            children_map
                .entry(edge.from)
                .or_default()
                .push((edge.to, edge.label.as_deref()));
        }
        for (&src, targets) in &children_map {
            let src_cx = node_x[src] + node_widths[src] / 2;
            let src_by = node_y[src] + node_height;
            let mut all_xs: Vec<usize> = targets
                .iter()
                .map(|&(to, _)| node_x[to] + node_widths[to] / 2)
                .collect();
            all_xs.push(src_cx);
            all_xs.sort();
            let min_x = *all_xs.iter().min().unwrap();
            let max_x = *all_xs.iter().max().unwrap();
            let branch_y = src_by + 1;
            grid.connect_v(src_cx, src_by, branch_y, edge_style);
            if min_x != max_x {
                grid.connect_h(branch_y, min_x, max_x, edge_style);
            }
            for &(to, elabel) in targets {
                let dst_cx = node_x[to] + node_widths[to] / 2;
                let dst_ty = node_y[to];
                if dst_ty > branch_y {
                    grid.connect_v(dst_cx, branch_y, dst_ty - 1, edge_style);
                    grid.set_char(dst_cx, dst_ty - 1, '\u{25BC}', edge_style);
                }
                if let Some(text) = elabel {
                    let lx = dst_cx + 2;
                    for (i, ch) in text.chars().enumerate() {
                        grid.set_char(lx + i, branch_y, ch, label_style);
                    }
                }
            }
        }
        for (i, node) in self.nodes.iter().enumerate() {
            grid.draw_box(
                node_x[i],
                node_y[i],
                node_widths[i],
                &node.label,
                &node.shape,
                border_style,
                text_style,
            );
            let cx = node_x[i] + node_widths[i] / 2;
            if children_map.contains_key(&i) {
                grid.set_char(cx, node_y[i] + 2, '\u{252C}', border_style);
            }
            if self.edges.iter().any(|e| e.to == i) {
                grid.set_char(cx, node_y[i], '\u{2534}', border_style);
            }
        }
        grid.to_text()
    }

    fn render_lr(&self, theme: &ThemeConfig) -> Text<'static> {
        let layers = self.assign_layers();
        let max_layer = layers.iter().copied().max().unwrap_or(0);
        let mut layer_nodes: Vec<Vec<usize>> = vec![vec![]; max_layer + 1];
        for (i, &layer) in layers.iter().enumerate() {
            layer_nodes[layer].push(i);
        }
        let node_widths: Vec<usize> = self
            .nodes
            .iter()
            .map(|n| (n.label.chars().count() + 4).max(6))
            .collect();
        let node_height: usize = 3;
        let v_gap: usize = 2;
        let h_gap: usize = 6;
        let col_widths: Vec<usize> = layer_nodes
            .iter()
            .map(|nodes| nodes.iter().map(|&i| node_widths[i]).max().unwrap_or(6))
            .collect();
        let max_npl = layer_nodes.iter().map(|n| n.len()).max().unwrap_or(1);
        let mut node_x = vec![0usize; self.nodes.len()];
        let mut node_y = vec![0usize; self.nodes.len()];
        let mut x = 0;
        for (li, nodes) in layer_nodes.iter().enumerate() {
            let total_h = nodes.len() * node_height + nodes.len().saturating_sub(1) * v_gap;
            let max_h = max_npl * node_height + max_npl.saturating_sub(1) * v_gap;
            let start_y = max_h.saturating_sub(total_h) / 2;
            let mut y = start_y;
            for &ni in nodes {
                node_x[ni] = x + (col_widths[li].saturating_sub(node_widths[ni])) / 2;
                node_y[ni] = y;
                y += node_height + v_gap;
            }
            x += col_widths[li] + h_gap;
        }
        let grid_width = x + 1;
        let grid_height = max_npl * node_height + max_npl.saturating_sub(1) * v_gap + 1;
        let mut grid = Grid::new(grid_width, grid_height);
        let border_style = Style::default().fg(theme.mermaid_node_border.0);
        let text_style = Style::default().fg(theme.mermaid_node_text.0);
        let edge_style = Style::default().fg(theme.mermaid_edge.0);
        let label_style = Style::default().fg(theme.mermaid_edge_label.0);

        let mut children_map: HashMap<usize, Vec<(usize, Option<&str>)>> = HashMap::new();
        for edge in &self.edges {
            children_map
                .entry(edge.from)
                .or_default()
                .push((edge.to, edge.label.as_deref()));
        }
        for (&src, targets) in &children_map {
            let src_rx = node_x[src] + node_widths[src];
            let src_cy = node_y[src] + node_height / 2;
            let mut all_ys: Vec<usize> = targets
                .iter()
                .map(|&(to, _)| node_y[to] + node_height / 2)
                .collect();
            all_ys.push(src_cy);
            all_ys.sort();
            let min_y = *all_ys.iter().min().unwrap();
            let max_y = *all_ys.iter().max().unwrap();
            let branch_x = src_rx + 1;
            grid.connect_h(src_cy, src_rx, branch_x, edge_style);
            if min_y != max_y {
                grid.connect_v(branch_x, min_y, max_y, edge_style);
            }
            for &(to, elabel) in targets {
                let dst_lx = node_x[to];
                let dst_cy = node_y[to] + node_height / 2;
                if dst_lx > branch_x {
                    grid.connect_h(dst_cy, branch_x, dst_lx - 1, edge_style);
                    grid.set_char(dst_lx - 1, dst_cy, '\u{25B6}', edge_style);
                }
                if let Some(text) = elabel {
                    let lx = branch_x + 1;
                    let ly = dst_cy.saturating_sub(1);
                    for (i, ch) in text.chars().enumerate() {
                        grid.set_char(lx + i, ly, ch, label_style);
                    }
                }
            }
        }
        for (i, node) in self.nodes.iter().enumerate() {
            grid.draw_box(
                node_x[i],
                node_y[i],
                node_widths[i],
                &node.label,
                &node.shape,
                border_style,
                text_style,
            );
            let cy = node_y[i] + node_height / 2;
            if children_map.contains_key(&i) {
                grid.set_char(node_x[i] + node_widths[i] - 1, cy, '\u{251C}', border_style);
            }
            if self.edges.iter().any(|e| e.to == i) {
                grid.set_char(node_x[i], cy, '\u{2524}', border_style);
            }
        }
        grid.to_text()
    }
}

// ===========================================================================
// Sequence Diagram
// ===========================================================================

fn render_sequence(input: &str, theme: &ThemeConfig) -> Text<'static> {
    let border = Style::default().fg(theme.mermaid_node_border.0);
    let text_s = Style::default().fg(theme.mermaid_node_text.0);
    let edge_s = Style::default().fg(theme.mermaid_edge.0);
    let label_s = Style::default().fg(theme.mermaid_edge_label.0);
    let title_s = Style::default()
        .fg(theme.mermaid_node_text.0)
        .add_modifier(Modifier::BOLD);

    let mut participants: Vec<String> = Vec::new();
    let mut pmap: HashMap<String, usize> = HashMap::new();

    struct SeqMsg {
        from: usize,
        to: usize,
        label: String,
        dashed: bool,
    }
    struct SeqNote {
        at: usize,
        text: String,
    }
    enum SeqEvent {
        Msg(SeqMsg),
        Note(SeqNote),
        Loop(String),
        LoopEnd,
    }

    let mut events: Vec<SeqEvent> = Vec::new();

    let ensure_p = |name: &str, ps: &mut Vec<String>, pm: &mut HashMap<String, usize>| -> usize {
        if let Some(&i) = pm.get(name) {
            return i;
        }
        let i = ps.len();
        ps.push(name.to_string());
        pm.insert(name.to_string(), i);
        i
    };

    for line in input.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        if let Some(rest) = line.strip_prefix("participant ") {
            let name = rest.trim();
            ensure_p(name, &mut participants, &mut pmap);
            continue;
        }
        if let Some(rest) = line.strip_prefix("actor ") {
            let name = rest.trim();
            ensure_p(name, &mut participants, &mut pmap);
            continue;
        }

        if let Some(rest) = line.strip_prefix("loop ") {
            events.push(SeqEvent::Loop(rest.trim().to_string()));
            continue;
        }
        if line == "end" {
            events.push(SeqEvent::LoopEnd);
            continue;
        }

        if line.starts_with("Note ") {
            let rest = &line[5..];
            let text_start = rest.find(':').map(|i| i + 1);
            if let Some(start) = text_start {
                let text = rest[start..].trim().replace("<br/>", " ");
                let who_part = rest[..start - 1].trim();
                let who = who_part
                    .split_whitespace()
                    .last()
                    .unwrap_or("?")
                    .trim_end_matches(':');
                let idx = ensure_p(who, &mut participants, &mut pmap);
                events.push(SeqEvent::Note(SeqNote { at: idx, text }));
            }
            continue;
        }

        // Parse message: From ->>|-->>|->|--> To : label
        let arrows = ["-->>", "->>", "-->", "->"];
        let mut found = false;
        for arrow in &arrows {
            if let Some(ap) = line.find(arrow) {
                let from_str = line[..ap].trim();
                let after = &line[ap + arrow.len()..];
                let (to_str, label) = if let Some(cp) = after.find(':') {
                    (after[..cp].trim(), after[cp + 1..].trim().to_string())
                } else {
                    (after.trim(), String::new())
                };
                let fi = ensure_p(from_str, &mut participants, &mut pmap);
                let ti = ensure_p(to_str, &mut participants, &mut pmap);
                let dashed = arrow.contains("--");
                events.push(SeqEvent::Msg(SeqMsg {
                    from: fi,
                    to: ti,
                    label,
                    dashed,
                }));
                found = true;
                break;
            }
        }
        if !found {
            // skip unrecognized
        }
    }

    if participants.is_empty() {
        return markdown::render_plain(input);
    }

    let col_width = participants
        .iter()
        .map(|p| p.chars().count() + 4)
        .max()
        .unwrap_or(10)
        .max(16);
    let spacing = col_width;

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Title
    lines.push(Line::from(Span::styled(
        "Sequence Diagram".to_string(),
        title_s,
    )));
    lines.push(Line::from(""));

    // Participant boxes
    let mut header_spans: Vec<Span<'static>> = Vec::new();
    for (i, p) in participants.iter().enumerate() {
        let bw = p.chars().count() + 4;
        let cx = i * spacing + spacing / 2;
        let pad_before = if i == 0 {
            cx.saturating_sub(bw / 2)
        } else {
            let prev_end = header_spans
                .iter()
                .map(|s: &Span| s.content.chars().count())
                .sum::<usize>();
            (cx.saturating_sub(bw / 2)).saturating_sub(prev_end)
        };
        if pad_before > 0 {
            header_spans.push(Span::raw(" ".repeat(pad_before)));
        }
        let box_str = format!("\u{250C}{}\u{2510}", "\u{2500}".repeat(bw - 2));
        header_spans.push(Span::styled(box_str, border));
    }
    lines.push(Line::from(header_spans));

    let mut name_spans: Vec<Span<'static>> = Vec::new();
    for (i, p) in participants.iter().enumerate() {
        let bw = p.chars().count() + 4;
        let cx = i * spacing + spacing / 2;
        let pad_before = if i == 0 {
            cx.saturating_sub(bw / 2)
        } else {
            let prev_end = name_spans
                .iter()
                .map(|s: &Span| s.content.chars().count())
                .sum::<usize>();
            (cx.saturating_sub(bw / 2)).saturating_sub(prev_end)
        };
        if pad_before > 0 {
            name_spans.push(Span::raw(" ".repeat(pad_before)));
        }
        let name_str = format!("\u{2502} {} \u{2502}", p);
        name_spans.push(Span::styled(name_str, text_s));
    }
    lines.push(Line::from(name_spans));

    let mut bottom_spans: Vec<Span<'static>> = Vec::new();
    for (i, p) in participants.iter().enumerate() {
        let bw = p.chars().count() + 4;
        let cx = i * spacing + spacing / 2;
        let pad_before = if i == 0 {
            cx.saturating_sub(bw / 2)
        } else {
            let prev_end = bottom_spans
                .iter()
                .map(|s: &Span| s.content.chars().count())
                .sum::<usize>();
            (cx.saturating_sub(bw / 2)).saturating_sub(prev_end)
        };
        if pad_before > 0 {
            bottom_spans.push(Span::raw(" ".repeat(pad_before)));
        }
        let box_str = format!("\u{2514}{}\u{2518}", "\u{2500}".repeat(bw - 2));
        bottom_spans.push(Span::styled(box_str, border));
    }
    lines.push(Line::from(bottom_spans));

    // Lifelines + events
    let np = participants.len();
    let lifeline_char = '\u{2502}';

    let make_lifeline =
        |lines: &mut Vec<Line<'static>>, np: usize, spacing: usize, edge_s: Style| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            for i in 0..np {
                let cx = i * spacing + spacing / 2;
                let prev_end = spans
                    .iter()
                    .map(|s: &Span| s.content.chars().count())
                    .sum::<usize>();
                let pad = cx.saturating_sub(prev_end);
                if pad > 0 {
                    spans.push(Span::raw(" ".repeat(pad)));
                }
                spans.push(Span::styled(lifeline_char.to_string(), edge_s));
            }
            lines.push(Line::from(spans));
        };

    let mut indent_depth = 0usize;

    for event in &events {
        match event {
            SeqEvent::Loop(label) => {
                make_lifeline(&mut lines, np, spacing, edge_s);
                let indent = "  ".repeat(indent_depth);
                lines.push(Line::from(Span::styled(
                    format!("{indent}\u{250C}\u{2500} loop: {label} \u{2500}\u{2510}"),
                    border,
                )));
                indent_depth += 1;
            }
            SeqEvent::LoopEnd => {
                indent_depth = indent_depth.saturating_sub(1);
                let indent = "  ".repeat(indent_depth);
                lines.push(Line::from(Span::styled(
                    format!("{indent}\u{2514}\u{2500} end \u{2500}\u{2518}"),
                    border,
                )));
                make_lifeline(&mut lines, np, spacing, edge_s);
            }
            SeqEvent::Msg(msg) => {
                make_lifeline(&mut lines, np, spacing, edge_s);

                let from_cx = msg.from * spacing + spacing / 2;
                let to_cx = msg.to * spacing + spacing / 2;
                let (left, right) = if from_cx <= to_cx {
                    (from_cx, to_cx)
                } else {
                    (to_cx, from_cx)
                };
                let arrow_width = if right > left { right - left } else { 1 };
                let arrow_ch = if msg.dashed { '\u{2504}' } else { '\u{2500}' };
                let arrow_tip = if from_cx <= to_cx {
                    '\u{25B6}'
                } else {
                    '\u{25C0}'
                };

                let mut arrow_str = String::new();
                for _ in 0..arrow_width.saturating_sub(1) {
                    arrow_str.push(arrow_ch);
                }
                arrow_str.push(arrow_tip);

                let label_line = if !msg.label.is_empty() {
                    format!(" {}", msg.label)
                } else {
                    String::new()
                };

                let mut spans: Vec<Span<'static>> = Vec::new();
                let start = left;
                if start > 0 {
                    spans.push(Span::raw(" ".repeat(start)));
                }
                if from_cx <= to_cx {
                    spans.push(Span::styled(arrow_str, edge_s));
                } else {
                    // Build reversed arrow: tip first, then the shaft
                    let mut tip_first = String::new();
                    tip_first.push(arrow_tip);
                    for _ in 0..arrow_width.saturating_sub(1) {
                        tip_first.push(arrow_ch);
                    }
                    spans.push(Span::styled(tip_first, edge_s));
                }
                if !label_line.is_empty() {
                    spans.push(Span::styled(label_line, label_s));
                }
                lines.push(Line::from(spans));

                make_lifeline(&mut lines, np, spacing, edge_s);
            }
            SeqEvent::Note(note) => {
                make_lifeline(&mut lines, np, spacing, edge_s);
                let cx = note.at * spacing + spacing / 2;
                let note_w = note.text.chars().count() + 4;
                let start = cx + 2;
                let pad = " ".repeat(start);
                lines.push(Line::from(Span::styled(
                    format!("{pad}\u{250C}{}\u{2510}", "\u{2500}".repeat(note_w - 2)),
                    label_s,
                )));
                lines.push(Line::from(Span::styled(
                    format!("{pad}\u{2502} {} \u{2502}", note.text),
                    label_s,
                )));
                lines.push(Line::from(Span::styled(
                    format!("{pad}\u{2514}{}\u{2518}", "\u{2500}".repeat(note_w - 2)),
                    label_s,
                )));
                make_lifeline(&mut lines, np, spacing, edge_s);
            }
        }
    }

    // Final lifeline
    make_lifeline(&mut lines, np, spacing, edge_s);

    Text::from(lines)
}

// ===========================================================================
// Class Diagram
// ===========================================================================

fn render_class(input: &str, theme: &ThemeConfig) -> Text<'static> {
    let border = Style::default().fg(theme.mermaid_node_border.0);
    let text_s = Style::default().fg(theme.mermaid_node_text.0);
    let edge_s = Style::default().fg(theme.mermaid_edge.0);
    let label_s = Style::default().fg(theme.mermaid_edge_label.0);
    let title_s = Style::default()
        .fg(theme.mermaid_node_text.0)
        .add_modifier(Modifier::BOLD);

    struct ClassInfo {
        name: String,
        members: Vec<String>,
    }

    let mut classes: HashMap<String, ClassInfo> = HashMap::new();

    struct ClassRel {
        from: String,
        to: String,
        rel_type: String,
        label: String,
    }

    let mut rels: Vec<ClassRel> = Vec::new();

    let ensure_class = |name: &str, classes: &mut HashMap<String, ClassInfo>| {
        if !classes.contains_key(name) {
            classes.insert(
                name.to_string(),
                ClassInfo {
                    name: name.to_string(),
                    members: Vec::new(),
                },
            );
        }
    };

    let rel_markers = [
        "<|--", "--|>", "*--", "--*", "o--", "--o", "..", "-->", "<-->", "--",
    ];

    for line in input.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        // Member: ClassName : member
        if let Some(colon) = line.find(" : ") {
            let class_name = line[..colon].trim();
            let member = line[colon + 3..].trim();
            ensure_class(class_name, &mut classes);
            classes
                .get_mut(class_name)
                .unwrap()
                .members
                .push(member.to_string());
            continue;
        }

        // Relationship
        let mut found = false;
        for marker in &rel_markers {
            if let Some(pos) = line.find(marker) {
                let from = line[..pos].trim().to_string();
                let after = &line[pos + marker.len()..];
                let (to, label) = if let Some(cp) = after.find(':') {
                    (
                        after[..cp].trim().to_string(),
                        after[cp + 1..].trim().to_string(),
                    )
                } else {
                    (after.trim().to_string(), String::new())
                };
                if !from.is_empty() && !to.is_empty() {
                    ensure_class(&from, &mut classes);
                    ensure_class(&to, &mut classes);
                    rels.push(ClassRel {
                        from,
                        to,
                        rel_type: marker.to_string(),
                        label,
                    });
                    found = true;
                }
                break;
            }
        }
        if !found {
            // Might be a class name alone
            if !line.contains(' ') && !line.contains(':') && !line.contains('{') {
                ensure_class(line, &mut classes);
            }
        }
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        "Class Diagram".to_string(),
        title_s,
    )));
    lines.push(Line::from(""));

    // Render each class as a box
    let class_names: Vec<String> = {
        let mut v: Vec<String> = classes.keys().cloned().collect();
        v.sort();
        v
    };

    for name in &class_names {
        let info = &classes[name];
        let max_w = std::iter::once(info.name.chars().count())
            .chain(info.members.iter().map(|m| m.chars().count()))
            .max()
            .unwrap_or(0);
        let box_w = max_w + 4;

        lines.push(Line::from(Span::styled(
            format!(
                "  \u{250C}{}\u{2510}",
                "\u{2500}".repeat(box_w.saturating_sub(2))
            ),
            border,
        )));
        let name_pad = box_w
            .saturating_sub(2)
            .saturating_sub(info.name.chars().count());
        let left_p = name_pad / 2;
        let right_p = name_pad - left_p;
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}", border),
            Span::styled(
                format!("{}{}{}", " ".repeat(left_p), info.name, " ".repeat(right_p)),
                Style::default()
                    .fg(theme.mermaid_node_text.0)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("\u{2502}", border),
        ]));

        if !info.members.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(
                    "  \u{251C}{}\u{2524}",
                    "\u{2500}".repeat(box_w.saturating_sub(2))
                ),
                border,
            )));
            for member in &info.members {
                let mpad = box_w
                    .saturating_sub(2)
                    .saturating_sub(member.chars().count());
                lines.push(Line::from(vec![
                    Span::styled("  \u{2502} ", border),
                    Span::styled(
                        format!("{}{}", member, " ".repeat(mpad.saturating_sub(1))),
                        text_s,
                    ),
                    Span::styled("\u{2502}", border),
                ]));
            }
        }

        lines.push(Line::from(Span::styled(
            format!(
                "  \u{2514}{}\u{2518}",
                "\u{2500}".repeat(box_w.saturating_sub(2))
            ),
            border,
        )));
        lines.push(Line::from(""));
    }

    // Relationships
    if !rels.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Relationships:".to_string(),
            title_s,
        )));
        for rel in &rels {
            let label = if rel.label.is_empty() {
                String::new()
            } else {
                format!(" : {}", rel.label)
            };
            lines.push(Line::from(vec![
                Span::styled("    ", edge_s),
                Span::styled(rel.from.clone(), text_s),
                Span::styled(format!(" {} ", rel.rel_type), edge_s),
                Span::styled(rel.to.clone(), text_s),
                Span::styled(label, label_s),
            ]));
        }
    }

    Text::from(lines)
}

// ===========================================================================
// Gantt Chart
// ===========================================================================

fn render_gantt(input: &str, theme: &ThemeConfig) -> Text<'static> {
    let border = Style::default().fg(theme.mermaid_node_border.0);
    let text_s = Style::default().fg(theme.mermaid_node_text.0);
    let edge_s = Style::default().fg(theme.mermaid_edge.0);
    let label_s = Style::default().fg(theme.mermaid_edge_label.0);
    let title_s = Style::default()
        .fg(theme.mermaid_node_text.0)
        .add_modifier(Modifier::BOLD);

    let done_style = Style::default().fg(theme.mermaid_node_border.0);
    let active_style = Style::default().fg(theme.mermaid_edge_label.0);
    let future_style = Style::default().fg(theme.mermaid_edge.0);

    struct Task {
        name: String,
        section: String,
        status: String,
    }

    let mut title = String::from("Gantt Chart");
    let mut current_section = String::new();
    let mut tasks: Vec<Task> = Vec::new();

    for line in input.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("title ") {
            title = rest.trim().to_string();
            continue;
        }
        if line.starts_with("dateFormat")
            || line.starts_with("excludes")
            || line.starts_with("axisFormat")
        {
            continue;
        }
        if let Some(rest) = line.strip_prefix("section ") {
            current_section = rest.trim().to_string();
            continue;
        }

        // Task line: name : status?, id?, start, duration
        if let Some(colon) = line.find(':') {
            let name = line[..colon].trim().to_string();
            let rest = line[colon + 1..].trim();
            let parts: Vec<&str> = rest.split(',').map(|s| s.trim()).collect();
            let status = if parts.iter().any(|p| *p == "done") {
                "done".to_string()
            } else if parts.iter().any(|p| *p == "active") {
                "active".to_string()
            } else {
                "future".to_string()
            };
            tasks.push(Task {
                name,
                section: current_section.clone(),
                status,
            });
        }
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(title.clone(), title_s)));
    lines.push(Line::from(""));

    let max_name = tasks
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(10);
    let bar_width: usize = 30;

    let mut prev_section = String::new();
    for (i, task) in tasks.iter().enumerate() {
        if task.section != prev_section {
            if !task.section.is_empty() {
                if i > 0 {
                    lines.push(Line::from(""));
                }
                lines.push(Line::from(Span::styled(
                    format!("  \u{25B6} {}", task.section),
                    label_s,
                )));
            }
            prev_section = task.section.clone();
        }

        let name_pad = max_name.saturating_sub(task.name.chars().count());
        let (bar_ch, bar_style) = match task.status.as_str() {
            "done" => ('\u{2588}', done_style),
            "active" => ('\u{2593}', active_style),
            _ => ('\u{2591}', future_style),
        };

        let bar_len = match task.status.as_str() {
            "done" => bar_width,
            "active" => bar_width * 2 / 3,
            _ => bar_width / 2,
        };
        let bar = bar_ch.to_string().repeat(bar_len);

        lines.push(Line::from(vec![
            Span::styled(format!("  {}{} ", task.name, " ".repeat(name_pad)), text_s),
            Span::styled("\u{2502} ", border),
            Span::styled(bar, bar_style),
        ]));
    }

    // Bottom axis
    lines.push(Line::from(""));
    let axis_pad = max_name + 4;
    lines.push(Line::from(Span::styled(
        format!(
            "{}{}",
            " ".repeat(axis_pad),
            "\u{2500}".repeat(bar_width + 2)
        ),
        edge_s,
    )));

    Text::from(lines)
}

// ===========================================================================
// Git Graph
// ===========================================================================

fn render_git(input: &str, theme: &ThemeConfig) -> Text<'static> {
    let _border = Style::default().fg(theme.mermaid_node_border.0);
    let text_s = Style::default().fg(theme.mermaid_node_text.0);
    let edge_s = Style::default().fg(theme.mermaid_edge.0);
    let label_s = Style::default().fg(theme.mermaid_edge_label.0);
    let title_s = Style::default()
        .fg(theme.mermaid_node_text.0)
        .add_modifier(Modifier::BOLD);

    struct Branch {
        name: String,
        col: usize,
    }

    let mut branches: Vec<Branch> = Vec::new();
    let mut current_branch = "main".to_string();
    let mut commit_num: usize = 0;

    enum GitEvent {
        Commit {
            branch: String,
            id: usize,
        },
        #[allow(dead_code)]
        Branch {
            name: String,
            from: String,
        },
        Checkout {
            name: String,
        },
        Merge {
            from: String,
            into: String,
        },
    }

    let mut events: Vec<GitEvent> = Vec::new();

    branches.push(Branch {
        name: "main".to_string(),
        col: 0,
    });

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("%%")
            || line.starts_with("gitGraph")
            || line.starts_with("gitgraph")
        {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "commit" => {
                commit_num += 1;
                events.push(GitEvent::Commit {
                    branch: current_branch.clone(),
                    id: commit_num,
                });
            }
            "branch" if parts.len() >= 2 => {
                let name = parts[1].to_string();
                let col = branches.len();
                branches.push(Branch {
                    name: name.clone(),
                    col,
                });
                events.push(GitEvent::Branch {
                    name: name.clone(),
                    from: current_branch.clone(),
                });
            }
            "checkout" if parts.len() >= 2 => {
                let name = parts[1].to_string();
                events.push(GitEvent::Checkout { name: name.clone() });
                current_branch = name;
            }
            "merge" if parts.len() >= 2 => {
                let from = parts[1].to_string();
                events.push(GitEvent::Merge {
                    from: from.clone(),
                    into: current_branch.clone(),
                });
            }
            _ => {}
        }
    }

    let branch_col = |name: &str, bs: &[Branch]| -> usize {
        bs.iter().find(|b| b.name == name).map_or(0, |b| b.col)
    };

    let num_cols = branches.len().max(1);
    let col_spacing = 4;

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled("Git Graph".to_string(), title_s)));
    lines.push(Line::from(""));

    // Branch legend
    for b in &branches {
        let col_marker = format!("  {} ", "\u{25CF}");
        let colors = [
            theme.mermaid_node_border.0,
            theme.mermaid_edge_label.0,
            theme.mermaid_node_text.0,
            theme.mermaid_edge.0,
        ];
        let color = colors[b.col % colors.len()];
        lines.push(Line::from(vec![
            Span::styled(col_marker, Style::default().fg(color)),
            Span::styled(
                b.name.clone(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    lines.push(Line::from(""));

    let make_rail = |active_cols: &[bool],
                     num_cols: usize,
                     col_spacing: usize,
                     theme: &ThemeConfig|
     -> Vec<Span<'static>> {
        let colors = [
            theme.mermaid_node_border.0,
            theme.mermaid_edge_label.0,
            theme.mermaid_node_text.0,
            theme.mermaid_edge.0,
        ];
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::raw("  ".to_string()));
        for c in 0..num_cols {
            let color = colors[c % colors.len()];
            if c < active_cols.len() && active_cols[c] {
                spans.push(Span::styled("\u{2502}", Style::default().fg(color)));
            } else {
                spans.push(Span::raw(" "));
            }
            if c < num_cols - 1 {
                spans.push(Span::raw(" ".repeat(col_spacing - 1)));
            }
        }
        spans
    };

    let mut active_cols = vec![true; num_cols];
    for c in 1..num_cols {
        active_cols[c] = false;
    }

    for event in &events {
        match event {
            GitEvent::Commit { branch, id } => {
                let col = branch_col(branch, &branches);
                let colors = [
                    theme.mermaid_node_border.0,
                    theme.mermaid_edge_label.0,
                    theme.mermaid_node_text.0,
                    theme.mermaid_edge.0,
                ];
                let color = colors[col % colors.len()];

                let mut rail = make_rail(&active_cols, num_cols, col_spacing, theme);
                // Replace the column char with commit marker
                let span_idx = 1 + col * 2; // accounting for initial "  " and spacing spans
                if span_idx < rail.len() {
                    rail[span_idx] = Span::styled("\u{25CF}", Style::default().fg(color));
                }
                // Add commit label
                rail.push(Span::styled(format!("  #{id}"), text_s));
                lines.push(Line::from(rail));
            }
            GitEvent::Branch { name, from: _ } => {
                let col = branch_col(name, &branches);
                if col < active_cols.len() {
                    active_cols[col] = true;
                }
                let rail = make_rail(&active_cols, num_cols, col_spacing, theme);
                lines.push(Line::from(rail));
            }
            GitEvent::Checkout { name } => {
                let _col = branch_col(name, &branches);
                let rail = make_rail(&active_cols, num_cols, col_spacing, theme);
                lines.push(Line::from(rail));
            }
            GitEvent::Merge { from, into } => {
                let from_col = branch_col(from, &branches);
                let into_col = branch_col(into, &branches);
                let colors = [
                    theme.mermaid_node_border.0,
                    theme.mermaid_edge_label.0,
                    theme.mermaid_node_text.0,
                    theme.mermaid_edge.0,
                ];

                // Draw merge arrow
                let mut spans: Vec<Span<'static>> = Vec::new();
                spans.push(Span::raw("  ".to_string()));
                let (left, right) = if from_col < into_col {
                    (from_col, into_col)
                } else {
                    (into_col, from_col)
                };
                for c in 0..num_cols {
                    let color = colors[c % colors.len()];
                    if c >= left && c <= right {
                        if c == into_col {
                            spans.push(Span::styled("\u{25CF}", Style::default().fg(color)));
                        } else if c == from_col {
                            spans.push(Span::styled("\u{25CB}", Style::default().fg(color)));
                        } else {
                            spans.push(Span::styled("\u{2500}", edge_s));
                        }
                    } else if c < active_cols.len() && active_cols[c] {
                        spans.push(Span::styled("\u{2502}", Style::default().fg(color)));
                    } else {
                        spans.push(Span::raw(" "));
                    }
                    if c < num_cols - 1 {
                        if c >= left && c < right {
                            spans.push(Span::styled("\u{2500}".repeat(col_spacing - 1), edge_s));
                        } else {
                            spans.push(Span::raw(" ".repeat(col_spacing - 1)));
                        }
                    }
                }
                spans.push(Span::styled(
                    format!("  merge {} \u{2192} {}", from, into),
                    label_s,
                ));
                lines.push(Line::from(spans));
            }
        }
    }

    Text::from(lines)
}

// ===========================================================================
// User Journey
// ===========================================================================

fn render_journey(input: &str, theme: &ThemeConfig) -> Text<'static> {
    let border = Style::default().fg(theme.mermaid_node_border.0);
    let text_s = Style::default().fg(theme.mermaid_node_text.0);
    let label_s = Style::default().fg(theme.mermaid_edge_label.0);
    let title_s = Style::default()
        .fg(theme.mermaid_node_text.0)
        .add_modifier(Modifier::BOLD);

    struct Step {
        name: String,
        score: u8,
        actors: String,
        section: String,
    }

    let mut title = String::from("User Journey");
    let mut current_section = String::new();
    let mut steps: Vec<Step> = Vec::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") || line == "journey" {
            continue;
        }
        if let Some(rest) = line.strip_prefix("title ") {
            title = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("section ") {
            current_section = rest.trim().to_string();
            continue;
        }

        // Step: name: score: actors
        if let Some(first_colon) = line.find(':') {
            let name = line[..first_colon].trim().to_string();
            let rest = line[first_colon + 1..].trim();
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            let score = parts
                .first()
                .and_then(|s| s.trim().parse::<u8>().ok())
                .unwrap_or(3);
            let actors = parts
                .get(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            steps.push(Step {
                name,
                score,
                actors,
                section: current_section.clone(),
            });
        }
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(title, title_s)));
    lines.push(Line::from(""));

    let max_name = steps
        .iter()
        .map(|s| s.name.chars().count())
        .max()
        .unwrap_or(10);
    let bar_max: usize = 20;

    let score_to_face = |s: u8| -> &'static str {
        match s {
            5 => "\u{1F600}", // very happy (or fallback)
            4 => ":)",
            3 => ":|",
            2 => ":(",
            1 => ":(",
            _ => ":|",
        }
    };

    let score_to_style = |s: u8, theme: &ThemeConfig| -> Style {
        match s {
            5 => Style::default().fg(theme.mermaid_node_border.0),
            4 => Style::default().fg(theme.mermaid_node_border.0),
            3 => Style::default().fg(theme.mermaid_edge_label.0),
            2 => Style::default().fg(theme.mermaid_edge.0),
            _ => Style::default().fg(theme.mermaid_edge.0),
        }
    };

    let mut prev_section = String::new();
    for (i, step) in steps.iter().enumerate() {
        if step.section != prev_section {
            if !step.section.is_empty() {
                if i > 0 {
                    lines.push(Line::from(""));
                }
                lines.push(Line::from(Span::styled(
                    format!("  \u{2501}\u{2501} {} \u{2501}\u{2501}", step.section),
                    label_s,
                )));
                lines.push(Line::from(""));
            }
            prev_section = step.section.clone();
        }

        let name_pad = max_name.saturating_sub(step.name.chars().count());
        let bar_len = (step.score as usize * bar_max) / 5;
        let bar = "\u{2588}".repeat(bar_len);
        let empty_bar = "\u{2591}".repeat(bar_max.saturating_sub(bar_len));
        let face = score_to_face(step.score);
        let bar_style = score_to_style(step.score, theme);

        let actors_str = if step.actors.is_empty() {
            String::new()
        } else {
            format!("  ({})", step.actors)
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {}{} ", step.name, " ".repeat(name_pad)), text_s),
            Span::styled(format!("{face} "), bar_style),
            Span::styled("\u{2502}", border),
            Span::styled(bar, bar_style),
            Span::styled(empty_bar, Style::default().fg(theme.mermaid_edge.0)),
            Span::styled("\u{2502}", border),
            Span::styled(format!(" {}/5", step.score), text_s),
            Span::styled(actors_str, label_s),
        ]));
    }

    Text::from(lines)
}

// ===========================================================================
// Flowchart parsing helpers
// ===========================================================================

fn skip_ws(s: &[u8], mut pos: usize) -> usize {
    while pos < s.len() && s[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}

fn parse_node_ref(s: &[u8], pos: usize) -> Option<(String, Option<String>, Shape, usize)> {
    if pos >= s.len() {
        return None;
    }
    let mut end = pos;
    while end < s.len() && (s[end].is_ascii_alphanumeric() || s[end] == b'_') {
        end += 1;
    }
    if end == pos {
        return None;
    }
    let id = String::from_utf8_lossy(&s[pos..end]).to_string();
    if end >= s.len() {
        return Some((id, None, Shape::Default, end));
    }
    match s[end] {
        b'[' => {
            if end + 1 < s.len() && s[end + 1] == b'[' {
                if let Some(c) = find_byte(s, end + 2, b']') {
                    if c + 1 < s.len() && s[c + 1] == b']' {
                        let l = String::from_utf8_lossy(&s[end + 2..c]).to_string();
                        return Some((id, Some(l), Shape::Rectangle, c + 2));
                    }
                }
            }
            let c = find_byte(s, end + 1, b']')?;
            let l = String::from_utf8_lossy(&s[end + 1..c]).to_string();
            Some((id, Some(l), Shape::Rectangle, c + 1))
        }
        b'(' => {
            if end + 1 < s.len() && s[end + 1] == b'(' {
                if let Some(c) = find_byte(s, end + 2, b')') {
                    if c + 1 < s.len() && s[c + 1] == b')' {
                        let l = String::from_utf8_lossy(&s[end + 2..c]).to_string();
                        return Some((id, Some(l), Shape::Rounded, c + 2));
                    }
                }
            }
            let c = find_byte(s, end + 1, b')')?;
            let l = String::from_utf8_lossy(&s[end + 1..c]).to_string();
            Some((id, Some(l), Shape::Rounded, c + 1))
        }
        b'{' => {
            if end + 1 < s.len() && s[end + 1] == b'{' {
                if let Some(c) = find_byte(s, end + 2, b'}') {
                    if c + 1 < s.len() && s[c + 1] == b'}' {
                        let l = String::from_utf8_lossy(&s[end + 2..c]).to_string();
                        return Some((id, Some(l), Shape::Diamond, c + 2));
                    }
                }
            }
            let c = find_byte(s, end + 1, b'}')?;
            let l = String::from_utf8_lossy(&s[end + 1..c]).to_string();
            Some((id, Some(l), Shape::Diamond, c + 1))
        }
        b'>' => {
            let c = find_byte(s, end + 1, b']')?;
            let l = String::from_utf8_lossy(&s[end + 1..c]).to_string();
            Some((id, Some(l), Shape::Rectangle, c + 1))
        }
        _ => Some((id, None, Shape::Default, end)),
    }
}

fn find_byte(s: &[u8], start: usize, byte: u8) -> Option<usize> {
    for i in start..s.len() {
        if s[i] == byte {
            return Some(i);
        }
    }
    None
}

fn find_bytes(s: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || s.len() < start + needle.len() {
        return None;
    }
    for i in start..=s.len() - needle.len() {
        if &s[i..i + needle.len()] == needle {
            return Some(i);
        }
    }
    None
}

fn parse_arrow(s: &[u8], pos: usize) -> Option<(Option<String>, usize)> {
    let len = s.len();
    if pos + 2 > len {
        return None;
    }
    if pos + 4 <= len && &s[pos..pos + 4] == b"-.->" {
        return Some(parse_arrow_label(s, pos + 4));
    }
    if pos + 3 <= len && &s[pos..pos + 3] == b"==>" {
        return Some(parse_arrow_label(s, pos + 3));
    }
    if pos + 3 <= len && &s[pos..pos + 3] == b"-->" {
        return Some(parse_arrow_label(s, pos + 3));
    }
    if pos + 3 <= len && &s[pos..pos + 3] == b"---" {
        let mut p = pos + 3;
        while p < len && s[p] == b'-' {
            p += 1;
        }
        if p < len && s[p] == b'>' {
            p += 1;
        }
        return Some((None, p));
    }
    if pos + 2 <= len && &s[pos..pos + 2] == b"--" {
        if let Some(ap) = find_bytes(s, pos + 2, b"-->") {
            let t = String::from_utf8_lossy(&s[pos + 2..ap]).trim().to_string();
            let label = if t.is_empty() { None } else { Some(t) };
            return Some((label, ap + 3));
        }
    }
    None
}

fn parse_arrow_label(s: &[u8], pos: usize) -> (Option<String>, usize) {
    let p = skip_ws(s, pos);
    if p < s.len() && s[p] == b'|' {
        if let Some(close) = find_byte(s, p + 1, b'|') {
            let t = String::from_utf8_lossy(&s[p + 1..close]).trim().to_string();
            let label = if t.is_empty() { None } else { Some(t) };
            return (label, close + 1);
        }
    }
    (None, pos)
}

// ===========================================================================
// Grid (for flowchart)
// ===========================================================================

fn dirs_to_char(d: u8) -> char {
    match (
        d & DIR_UP != 0,
        d & DIR_DOWN != 0,
        d & DIR_LEFT != 0,
        d & DIR_RIGHT != 0,
    ) {
        (true, true, true, true) => '\u{253C}',
        (false, true, true, true) => '\u{252C}',
        (true, false, true, true) => '\u{2534}',
        (true, true, false, true) => '\u{251C}',
        (true, true, true, false) => '\u{2524}',
        (false, true, false, true) => '\u{250C}',
        (false, true, true, false) => '\u{2510}',
        (true, false, false, true) => '\u{2514}',
        (true, false, true, false) => '\u{2518}',
        (true, true, false, false) => '\u{2502}',
        (false, false, true, true) => '\u{2500}',
        (true, false, false, false) => '\u{2502}',
        (false, true, false, false) => '\u{2502}',
        (false, false, true, false) => '\u{2500}',
        (false, false, false, true) => '\u{2500}',
        _ => ' ',
    }
}

struct Grid {
    cells: Vec<Vec<GridCell>>,
    width: usize,
    height: usize,
}

#[derive(Clone)]
struct GridCell {
    dirs: u8,
    explicit: Option<char>,
    style: Style,
    edge_style: Style,
}

impl GridCell {
    fn empty() -> Self {
        GridCell {
            dirs: 0,
            explicit: None,
            style: Style::default(),
            edge_style: Style::default(),
        }
    }
}

impl Grid {
    fn new(width: usize, height: usize) -> Self {
        Grid {
            cells: vec![vec![GridCell::empty(); width]; height],
            width,
            height,
        }
    }

    fn in_bounds(&self, x: usize, y: usize) -> bool {
        y < self.height && x < self.width
    }

    fn set_char(&mut self, x: usize, y: usize, ch: char, style: Style) {
        if self.in_bounds(x, y) {
            self.cells[y][x].explicit = Some(ch);
            self.cells[y][x].style = style;
        }
    }

    fn add_dir(&mut self, x: usize, y: usize, dir: u8, style: Style) {
        if self.in_bounds(x, y) {
            self.cells[y][x].dirs |= dir;
            self.cells[y][x].edge_style = style;
        }
    }

    fn connect_v(&mut self, x: usize, y1: usize, y2: usize, style: Style) {
        let (start, end) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
        for y in start..=end {
            if y > start {
                self.add_dir(x, y, DIR_UP, style);
            }
            if y < end {
                self.add_dir(x, y, DIR_DOWN, style);
            }
        }
    }

    fn connect_h(&mut self, y: usize, x1: usize, x2: usize, style: Style) {
        let (start, end) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        for x in start..=end {
            if x > start {
                self.add_dir(x, y, DIR_LEFT, style);
            }
            if x < end {
                self.add_dir(x, y, DIR_RIGHT, style);
            }
        }
    }

    fn draw_box(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        label: &str,
        shape: &Shape,
        border_style: Style,
        text_style: Style,
    ) {
        let (tl, tr, bl, br) = match shape {
            Shape::Rounded => ('\u{256D}', '\u{256E}', '\u{2570}', '\u{256F}'),
            Shape::Diamond => ('/', '\\', '\\', '/'),
            _ => ('\u{250C}', '\u{2510}', '\u{2514}', '\u{2518}'),
        };
        self.set_char(x, y, tl, border_style);
        for i in 1..width.saturating_sub(1) {
            self.set_char(x + i, y, '\u{2500}', border_style);
        }
        if width > 1 {
            self.set_char(x + width - 1, y, tr, border_style);
        }
        self.set_char(x, y + 1, '\u{2502}', border_style);
        for i in 1..width.saturating_sub(1) {
            self.set_char(x + i, y + 1, ' ', text_style);
        }
        if width > 1 {
            self.set_char(x + width - 1, y + 1, '\u{2502}', border_style);
        }
        let label_len = label.chars().count();
        let inner = width.saturating_sub(2);
        let left_pad = inner.saturating_sub(label_len) / 2;
        for (i, ch) in label.chars().enumerate() {
            let px = x + 1 + left_pad + i;
            if px < x + width.saturating_sub(1) {
                self.set_char(px, y + 1, ch, text_style);
            }
        }
        self.set_char(x, y + 2, bl, border_style);
        for i in 1..width.saturating_sub(1) {
            self.set_char(x + i, y + 2, '\u{2500}', border_style);
        }
        if width > 1 {
            self.set_char(x + width - 1, y + 2, br, border_style);
        }
    }

    fn to_text(&self) -> Text<'static> {
        let lines: Vec<Line<'static>> = self
            .cells
            .iter()
            .map(|row| {
                let mut spans: Vec<Span<'static>> = Vec::new();
                let mut buf = String::new();
                let mut cur_style = Style::default();
                let mut first = true;
                for cell in row {
                    let (ch, style) = if let Some(c) = cell.explicit {
                        (c, cell.style)
                    } else if cell.dirs != 0 {
                        (dirs_to_char(cell.dirs), cell.edge_style)
                    } else {
                        (' ', Style::default())
                    };
                    if first {
                        cur_style = style;
                        buf.push(ch);
                        first = false;
                    } else if style == cur_style {
                        buf.push(ch);
                    } else {
                        spans.push(Span::styled(std::mem::take(&mut buf), cur_style));
                        cur_style = style;
                        buf.push(ch);
                    }
                }
                if !buf.is_empty() {
                    spans.push(Span::styled(buf, cur_style));
                }
                Line::from(spans)
            })
            .collect();

        let lines: Vec<Line<'static>> = lines
            .into_iter()
            .rev()
            .skip_while(|l| l.spans.iter().all(|s| s.content.trim().is_empty()))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        Text::from(lines)
    }
}
