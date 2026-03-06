use std::collections::HashMap;
use std::collections::VecDeque;

use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};

use crate::config::ThemeConfig;
use crate::markdown;

pub fn render_d2(input: &str, theme: &ThemeConfig) -> Text<'static> {
    match D2Graph::parse(input) {
        Some(g) => g.render(theme),
        None => markdown::render_plain(input),
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

const DIR_UP: u8 = 1;
const DIR_DOWN: u8 = 2;
const DIR_LEFT: u8 = 4;
const DIR_RIGHT: u8 = 8;

#[derive(Clone, Copy)]
enum Direction {
    Down,
    Right,
}

#[derive(Clone)]
enum Shape {
    Rectangle,
    Diamond,
}

struct D2Node {
    label: String,
    shape: Shape,
}

struct D2Edge {
    from: usize,
    to: usize,
    label: Option<String>,
    bidirectional: bool,
}

struct D2Graph {
    direction: Direction,
    nodes: Vec<D2Node>,
    edges: Vec<D2Edge>,
    node_map: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

impl D2Graph {
    fn parse(input: &str) -> Option<Self> {
        let mut graph = D2Graph {
            direction: Direction::Down,
            nodes: Vec::new(),
            edges: Vec::new(),
            node_map: HashMap::new(),
        };

        let mut block_id: Option<String> = None;
        let mut block_label: Option<String> = None;
        let mut block_shape: Option<Shape> = None;
        let mut brace_depth: usize = 0;

        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Track brace depth for block definitions
            let open_braces = trimmed.chars().filter(|&c| c == '{').count();
            let close_braces = trimmed.chars().filter(|&c| c == '}').count();

            if brace_depth > 0 {
                // Inside a block definition
                if let Some(ref id) = block_id {
                    if trimmed.starts_with("label:") {
                        let val = extract_string_value(&trimmed[6..]);
                        block_label = Some(val);
                    } else if trimmed.starts_with("shape:") {
                        let val = trimmed[6..].trim().trim_matches('"');
                        if val == "diamond" {
                            block_shape = Some(Shape::Diamond);
                        }
                    }

                    brace_depth += open_braces;
                    brace_depth = brace_depth.saturating_sub(close_braces);

                    if brace_depth == 0 {
                        let shape = block_shape.take().unwrap_or(Shape::Rectangle);
                        let label = block_label.take().unwrap_or_else(|| id.clone());
                        graph.ensure_node(id, Some(label), shape);
                        block_id = None;
                    }
                } else {
                    brace_depth += open_braces;
                    brace_depth = brace_depth.saturating_sub(close_braces);
                }
                continue;
            }

            // direction: down / right
            if trimmed.starts_with("direction:") {
                let val = trimmed[10..].trim().to_lowercase();
                graph.direction = match val.as_str() {
                    "right" => Direction::Right,
                    _ => Direction::Down,
                };
                continue;
            }

            // Edge: A -> B: "label"  or  A -> B
            if let Some((from_id, to_id, label, bidi)) = parse_edge_line(trimmed) {
                let from = graph.ensure_node(&from_id, None, Shape::Rectangle);
                let to = graph.ensure_node(&to_id, None, Shape::Rectangle);
                graph.edges.push(D2Edge {
                    from,
                    to,
                    label,
                    bidirectional: bidi,
                });
                continue;
            }

            // Block start: NodeId: { or NodeId: { shape: ... }
            if let Some(colon_pos) = trimmed.find(':') {
                let id = trimmed[..colon_pos].trim().to_string();
                let after = trimmed[colon_pos + 1..].trim();

                if after.starts_with('{') {
                    block_id = Some(id);
                    block_label = None;
                    block_shape = None;
                    brace_depth = open_braces.saturating_sub(close_braces);

                    // Handle single-line block: Id: { shape: diamond, label: "..." }
                    if brace_depth == 0 && open_braces > 0 {
                        let inner = after.trim_start_matches('{').trim_end_matches('}').trim();
                        let mut shape = Shape::Rectangle;
                        let mut label = None;
                        for part in inner.split(',') {
                            let part = part.trim();
                            if part.starts_with("shape:") {
                                let v = part[6..].trim().trim_matches('"');
                                if v == "diamond" {
                                    shape = Shape::Diamond;
                                }
                            } else if part.starts_with("label:") {
                                label = Some(extract_string_value(&part[6..]));
                            }
                        }
                        let lbl = label.unwrap_or_else(|| block_id.as_ref().unwrap().clone());
                        graph.ensure_node(block_id.as_ref().unwrap(), Some(lbl), shape);
                        block_id = None;
                    }
                    continue;
                }

                // Simple node: NodeId: "label" or NodeId: label text
                if !id.is_empty() && !id.contains('>') && !id.contains('-') {
                    let label = extract_string_value(after);
                    graph.ensure_node(&id, Some(label), Shape::Rectangle);
                    continue;
                }
            }

            // Bare node reference (may contain spaces)
            let id = trimmed.trim_matches('"');
            if !id.is_empty() && !id.contains('{') && !id.contains('}') && !id.contains(':') {
                graph.ensure_node(id, None, Shape::Rectangle);
            }
        }

        if graph.nodes.is_empty() {
            return None;
        }
        Some(graph)
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
        self.nodes.push(D2Node { label: lbl, shape });
        self.node_map.insert(id.to_string(), idx);
        idx
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
        // Nodes with no edges get layer 0; unprocessed (cycles) go to end
        let max_layer = layers.iter().copied().max().unwrap_or(0);
        for i in 0..n {
            if !processed[i] && in_deg[i] > 0 {
                layers[i] = max_layer + 1;
            }
        }
        layers
    }

    // ------------------------------------------------------------------
    // Rendering
    // ------------------------------------------------------------------

    fn render(&self, theme: &ThemeConfig) -> Text<'static> {
        match self.direction {
            Direction::Down => self.render_down(theme),
            Direction::Right => self.render_right(theme),
        }
    }

    fn render_down(&self, theme: &ThemeConfig) -> Text<'static> {
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

        // Group edges by source
        let mut children_map: HashMap<usize, Vec<(usize, Option<&str>, bool)>> = HashMap::new();
        for edge in &self.edges {
            children_map
                .entry(edge.from)
                .or_default()
                .push((edge.to, edge.label.as_deref(), edge.bidirectional));
        }

        for (&src, targets) in &children_map {
            let src_cx = node_x[src] + node_widths[src] / 2;
            let src_by = node_y[src] + node_height;
            let mut all_xs: Vec<usize> = targets
                .iter()
                .map(|&(to, _, _)| node_x[to] + node_widths[to] / 2)
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

            for &(to, elabel, bidi) in targets {
                let dst_cx = node_x[to] + node_widths[to] / 2;
                let dst_ty = node_y[to];
                if dst_ty > branch_y {
                    grid.connect_v(dst_cx, branch_y, dst_ty - 1, edge_style);
                    grid.set_char(dst_cx, dst_ty - 1, '\u{25BC}', edge_style);
                }
                if bidi {
                    grid.set_char(src_cx, src_by, '\u{25B2}', edge_style);
                }
                if let Some(text) = elabel {
                    let lx = dst_cx + 2;
                    for (i, ch) in text.chars().enumerate() {
                        grid.set_char(lx + i, branch_y, ch, label_style);
                    }
                }
            }
        }

        // Draw nodes on top
        for (i, node) in self.nodes.iter().enumerate() {
            grid.draw_box(
                node_x[i], node_y[i], node_widths[i],
                &node.label, &node.shape, border_style, text_style,
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

    fn render_right(&self, theme: &ThemeConfig) -> Text<'static> {
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

        let mut children_map: HashMap<usize, Vec<(usize, Option<&str>, bool)>> = HashMap::new();
        for edge in &self.edges {
            children_map
                .entry(edge.from)
                .or_default()
                .push((edge.to, edge.label.as_deref(), edge.bidirectional));
        }

        for (&src, targets) in &children_map {
            let src_rx = node_x[src] + node_widths[src];
            let src_cy = node_y[src] + node_height / 2;
            let mut all_ys: Vec<usize> = targets
                .iter()
                .map(|&(to, _, _)| node_y[to] + node_height / 2)
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

            for &(to, elabel, bidi) in targets {
                let dst_lx = node_x[to];
                let dst_cy = node_y[to] + node_height / 2;
                if dst_lx > branch_x {
                    grid.connect_h(dst_cy, branch_x, dst_lx - 1, edge_style);
                    grid.set_char(dst_lx - 1, dst_cy, '\u{25B6}', edge_style);
                }
                if bidi {
                    grid.set_char(src_rx, src_cy, '\u{25C0}', edge_style);
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
                node_x[i], node_y[i], node_widths[i],
                &node.label, &node.shape, border_style, text_style,
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

// ---------------------------------------------------------------------------
// D2 parsing helpers
// ---------------------------------------------------------------------------

fn extract_string_value(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
    } else if s.starts_with('[') && s.ends_with(']') && s.len() >= 2 {
        s[1..s.len() - 1].trim_matches('"').to_string()
    } else {
        s.to_string()
    }
}

enum ArrowKind {
    Forward,
    Reverse,
    Bidirectional,
    Undirected,
}

fn parse_edge_line(line: &str) -> Option<(String, String, Option<String>, bool)> {
    // Arrow patterns ordered by length (longest first to avoid partial matches)
    let arrows: &[(&str, ArrowKind)] = &[
        (" <-> ", ArrowKind::Bidirectional),
        (" --> ", ArrowKind::Forward),
        (" <-- ", ArrowKind::Reverse),
        (" -> ", ArrowKind::Forward),
        (" <- ", ArrowKind::Reverse),
        (" -- ", ArrowKind::Undirected),
    ];

    for (arrow, kind) in arrows {
        if let Some(pos) = line.find(arrow) {
            let left = line[..pos].trim();
            let right_part = &line[pos + arrow.len()..];
            let (to_id, label) = split_node_and_label(right_part);
            let from_id = left.to_string();
            if !from_id.is_empty() && !to_id.is_empty() {
                return match kind {
                    ArrowKind::Reverse => Some((to_id, from_id, label, false)),
                    ArrowKind::Bidirectional => Some((from_id, to_id, label, true)),
                    _ => Some((from_id, to_id, label, false)),
                };
            }
        }
    }
    None
}

fn split_node_and_label(s: &str) -> (String, Option<String>) {
    let s = s.trim();
    // Look for ": " which separates the edge label from the target node.
    // But we need the *last* ": " that isn't inside the node name.
    // In D2, the edge label comes after the target: `A -> B: label`
    // So we search for ": " from the target side.
    if let Some(colon_pos) = s.find(':') {
        let to = s[..colon_pos].trim();
        let after = s[colon_pos + 1..].trim();
        if after.starts_with('{') {
            (s.to_string(), None)
        } else {
            let label = extract_string_value(after);
            (to.to_string(), Some(label))
        }
    } else {
        (s.to_string(), None)
    }
}

// ---------------------------------------------------------------------------
// Grid (same structure as mermaid.rs)
// ---------------------------------------------------------------------------

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
