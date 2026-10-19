//! Paged Media, Multi-Column, CSS/HTML Table, Flexbox, and Inline Layout Engine.

use crate::css::{
    BorderSide, BorderStyle, Color, ComputedStyle, Dimension, Display, PageStyle, Stylesheet,
    TextAlign, TextTransform, VerticalAlign,
};
use crate::fonts::{measure_text, truncate_ellipsis, wrap_text, FontSpec};
use crate::html::{DomNode, ElementNode, ParsedDocument};
use crate::pdf::{PdfDocument, PdfPageCanvas};
use std::path::Path;

#[derive(Debug, Clone)]
pub enum DrawOp {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        color: Color,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        style: BorderStyle,
        color: Color,
    },
    Text {
        x: f32,
        baseline_y: f32,
        text: String,
        font: FontSpec,
        font_size: f32,
        color: Color,
    },
}

impl DrawOp {
    pub fn offset(&self, dx: f32, dy: f32) -> DrawOp {
        match self {
            DrawOp::Rect {
                x,
                y,
                w,
                h,
                radius,
                color,
            } => DrawOp::Rect {
                x: x + dx,
                y: y + dy,
                w: *w,
                h: *h,
                radius: *radius,
                color: *color,
            },
            DrawOp::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                style,
                color,
            } => DrawOp::Line {
                x1: x1 + dx,
                y1: y1 + dy,
                x2: x2 + dx,
                y2: y2 + dy,
                width: *width,
                style: *style,
                color: *color,
            },
            DrawOp::Text {
                x,
                baseline_y,
                text,
                font,
                font_size,
                color,
            } => DrawOp::Text {
                x: x + dx,
                baseline_y: baseline_y + dy,
                text: text.clone(),
                font: *font,
                font_size: *font_size,
                color: *color,
            },
        }
    }

    pub fn render(&self, canvas: &mut PdfPageCanvas, dx: f32, dy: f32) {
        match self {
            DrawOp::Rect {
                x,
                y,
                w,
                h,
                radius,
                color,
            } => {
                if *radius > 0.2 {
                    canvas.fill_rounded_rect(x + dx, y + dy, *w, *h, *radius, *color);
                } else {
                    canvas.fill_rect(x + dx, y + dy, *w, *h, *color);
                }
            }
            DrawOp::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                style,
                color,
            } => {
                canvas.stroke_line(
                    x1 + dx,
                    y1 + dy,
                    x2 + dx,
                    y2 + dy,
                    *width,
                    *style,
                    *color,
                );
            }
            DrawOp::Text {
                x,
                baseline_y,
                text,
                font,
                font_size,
                color,
            } => {
                canvas.draw_text(x + dx, baseline_y + dy, text, *font, *font_size, *color);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayoutBlock {
    pub width: f32,
    pub height: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub background_color: Color,
    pub border_top: BorderSide,
    pub border_right: BorderSide,
    pub border_bottom: BorderSide,
    pub border_left: BorderSide,
    pub border_radius: f32,
    pub break_inside_avoid: bool,
    pub break_after_avoid: bool,
    pub break_before_page: bool,
    pub break_after_page: bool,
    pub ops: Vec<DrawOp>,
    pub children: Vec<LayoutBlock>,
    pub repeat_header: Option<Box<LayoutBlock>>,
}

impl LayoutBlock {
    pub fn total_height(&self) -> f32 {
        self.margin_top + self.height + self.margin_bottom
    }

    /// Minimum vertical space required at the start of this block so that headers/orphans are never split.
    pub fn min_first_piece_height(&self) -> f32 {
        if self.children.is_empty() {
            return self.total_height();
        }
        let mut h = self.margin_top + self.padding_top + self.border_top.width;
        if let Some(ref rh) = self.repeat_header {
            h += rh.total_height();
        }
        h += keep_with_next_chain_height(&self.children, 0);
        h.min(self.total_height())
    }
}

fn keep_with_next_chain_height(children: &[LayoutBlock], start_idx: usize) -> f32 {
    if start_idx >= children.len() {
        return 0.0;
    }
    let mut sum = 0.0;
    for (idx, ch) in children.iter().enumerate().skip(start_idx) {
        if ch.break_after_avoid && idx + 1 < children.len() {
            sum += ch.total_height();
        } else {
            sum += ch.min_first_piece_height();
            break;
        }
    }
    sum
}

pub fn render_html_to_pdf(html: &str, base_url: Option<&str>) -> Vec<u8> {
    let parsed = crate::html::parse_html(html);
    let mut stylesheet = Stylesheet::default();

    // Load linked external stylesheets if base_url is provided
    if let Some(base) = base_url {
        let base_path = Path::new(base);
        let base_dir = if base_path.is_dir() {
            base_path
        } else {
            base_path.parent().unwrap_or(Path::new("."))
        };
        for href in &parsed.link_hrefs {
            let clean_href = href.trim_start_matches("file://");
            let candidate = Path::new(clean_href);
            let resolved = if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                base_dir.join(clean_href)
            };
            if let Ok(css_content) = std::fs::read_to_string(&resolved) {
                stylesheet.parse_and_merge(&css_content);
            }
        }
    }

    for css in &parsed.style_sheets {
        stylesheet.parse_and_merge(css);
    }

    let doc = layout_document(&parsed, &stylesheet);
    doc.to_bytes()
}

struct RunningHeaderTemplate<'a> {
    node: &'a ElementNode,
    style: ComputedStyle,
}

fn layout_document(parsed: &ParsedDocument, stylesheet: &Stylesheet) -> PdfDocument {
    let page_style = &stylesheet.page;
    let content_width =
        (page_style.width - page_style.margin_left - page_style.margin_right).max(72.0);
    let content_height =
        (page_style.height - page_style.margin_top - page_style.margin_bottom).max(72.0);

    // Find <body> or root
    let body_node = find_tag(&parsed.root, "body").unwrap_or(&parsed.root);
    let root_ancestors = vec![(body_node, 0, 1)];
    let root_default = ComputedStyle::default();
    let body_style = stylesheet.compute_style(&root_ancestors, &root_default);

    let mut running_header: Option<RunningHeaderTemplate> = None;
    let mut main_blocks: Vec<LayoutBlock> = Vec::new();
    let mut multi_col_spec: (usize, f32, BorderSide) = (
        body_style.column_count,
        body_style.column_gap,
        body_style.column_rule,
    );

    let elem_children: Vec<&ElementNode> = body_node
        .children
        .iter()
        .filter_map(|c| match c {
            DomNode::Element(e) => Some(e),
            _ => None,
        })
        .collect();
    let sibling_count = elem_children.len();

    for (idx, child_el) in elem_children.into_iter().enumerate() {
        let mut anc = root_ancestors.clone();
        anc.push((child_el, idx, sibling_count));
        let st = stylesheet.compute_style(&anc, &body_style);
        if st.display == Display::None {
            continue;
        }
        if st.position_running.is_some() || child_el.id.as_deref() == Some("header") {
            running_header = Some(RunningHeaderTemplate {
                node: child_el,
                style: st,
            });
            continue;
        }
        if st.column_count > 1 {
            multi_col_spec = (st.column_count, st.column_gap, st.column_rule);
            let col_w =
                (content_width - st.column_gap * (st.column_count as f32 - 1.0)) / st.column_count as f32;
            collect_flow_blocks(child_el, &anc, &st, stylesheet, col_w, &mut main_blocks);
        } else {
            let col_w = if multi_col_spec.0 > 1 {
                (content_width - multi_col_spec.1 * (multi_col_spec.0 as f32 - 1.0))
                    / multi_col_spec.0 as f32
            } else {
                content_width
            };
            collect_flow_blocks(child_el, &anc, &st, stylesheet, col_w, &mut main_blocks);
        }
    }

    // Paginate blocks into pages and columns
    let (col_count, col_gap, col_rule) = multi_col_spec;
    let col_width = if col_count > 1 {
        (content_width - col_gap * (col_count as f32 - 1.0)) / col_count as f32
    } else {
        content_width
    };

    let mut paginator = Paginator::new(
        page_style.clone(),
        col_count,
        col_width,
        col_gap,
        col_rule,
        content_height,
    );

    for block in &main_blocks {
        paginator.place_block(block);
    }

    paginator.finish(running_header.as_ref(), stylesheet)
}

fn find_tag<'a>(node: &'a ElementNode, target: &str) -> Option<&'a ElementNode> {
    if node.tag == target {
        return Some(node);
    }
    for ch in &node.children {
        if let DomNode::Element(el) = ch {
            if let Some(found) = find_tag(el, target) {
                return Some(found);
            }
        }
    }
    None
}

/// Checks if an element is a structural flow container whose children should be emitted into the flow
/// (either as a splittable group or direct stream).
fn collect_flow_blocks<'a>(
    el: &'a ElementNode,
    ancestors: &[(&'a ElementNode, usize, usize)],
    style: &ComputedStyle,
    stylesheet: &Stylesheet,
    avail_width: f32,
    out: &mut Vec<LayoutBlock>,
) {
    if style.display == Display::None || style.position_running.is_some() {
        return;
    }

    // If this element has column-count > 1 and is inside body, update avail_width for its children
    let effective_width = resolve_box_width(style, avail_width);

    // Pure transparent wrapper containers without borders/backgrounds/avoid-breaks (like .content-container, .event-section, .entries-list, .heat-block, .group-container)
    let is_pure_flow_wrapper = !style.break_inside_avoid
        && !style.break_after_avoid
        && !style.background_color.is_visible()
        && !style.border_top.is_visible()
        && !style.border_bottom.is_visible()
        && !style.border_left.is_visible()
        && !style.border_right.is_visible()
        && style.display == Display::Block
        && !matches!(el.tag.as_str(), "table" | "tr" | "td" | "th" | "p" | "h1" | "h2" | "h3" | "h4")
        && has_only_element_children(el)
        && !is_atomic_leaf_container(el);

    if is_pure_flow_wrapper {
        if style.break_before_page {
            out.push(LayoutBlock {
                width: effective_width,
                height: 0.0,
                margin_top: 0.0,
                margin_bottom: 0.0,
                padding_top: 0.0,
                padding_bottom: 0.0,
                padding_left: 0.0,
                background_color: Color::TRANSPARENT,
                border_top: BorderSide::NONE,
                border_right: BorderSide::NONE,
                border_bottom: BorderSide::NONE,
                border_left: BorderSide::NONE,
                border_radius: 0.0,
                break_inside_avoid: false,
                break_after_avoid: false,
                break_before_page: true,
                break_after_page: false,
                ops: Vec::new(),
                children: Vec::new(),
                repeat_header: None,
            });
        }

        let elem_children: Vec<&ElementNode> = el
            .children
            .iter()
            .filter_map(|c| match c {
                DomNode::Element(e) => Some(e),
                _ => None,
            })
            .collect();
        let total = elem_children.len();
        let start_len = out.len();
        for (i, ch) in elem_children.into_iter().enumerate() {
            let mut child_anc = ancestors.to_vec();
            child_anc.push((ch, i, total));
            let child_style = stylesheet.compute_style(&child_anc, style);
            collect_flow_blocks(ch, &child_anc, &child_style, stylesheet, effective_width, out);
        }
        if out.len() > start_len {
            out[start_len].margin_top += style.margin_top + style.padding_top;
            if let Some(last) = out.last_mut() {
                last.margin_bottom += style.margin_bottom + style.padding_bottom;
                if style.break_after_page {
                    last.break_after_page = true;
                }
            }
        }
        return;
    }

    let block = layout_element_block(el, ancestors, style, stylesheet, avail_width, None);
    out.push(block);
}

fn has_only_element_children(el: &ElementNode) -> bool {
    let mut has_el = false;
    for ch in &el.children {
        match ch {
            DomNode::Text(t) => {
                if !t.trim().is_empty() {
                    return false;
                }
            }
            DomNode::Element(_) => has_el = true,
        }
    }
    has_el
}

fn is_atomic_leaf_container(el: &ElementNode) -> bool {
    el.has_class("event-header-box")
        || el.has_class("heat-header-row")
        || el.has_class("group-header")
        || el.has_class("athlete-header")
        || el.has_class("entries-header-row")
        || el.has_class("div-entry-row")
        || el.has_class("div-relay-swimmers-row")
}

fn resolve_box_width(style: &ComputedStyle, avail_width: f32) -> f32 {
    let mut w = match style.width {
        Dimension::Auto => avail_width - style.margin_left - style.margin_right,
        Dimension::Pt(pt) => pt,
        Dimension::Percent(pct) => avail_width * (pct / 100.0),
    };
    match style.max_width {
        Dimension::Pt(mw) => w = w.min(mw),
        Dimension::Percent(pct) => w = w.min(avail_width * (pct / 100.0)),
        Dimension::Auto => {}
    }
    w.max(0.0)
}

fn layout_element_block<'a>(
    el: &'a ElementNode,
    ancestors: &[(&'a ElementNode, usize, usize)],
    style: &ComputedStyle,
    stylesheet: &Stylesheet,
    avail_width: f32,
    page_num_override: Option<(usize, usize)>,
) -> LayoutBlock {
    let box_width = resolve_box_width(style, avail_width);

    if el.tag == "table" {
        return layout_html_table(
            el,
            ancestors,
            style,
            stylesheet,
            box_width,
            page_num_override,
        );
    }

    if style.display == Display::Table
        || has_direct_table_cell_children(el, ancestors, style, stylesheet)
    {
        return layout_css_table(
            el,
            ancestors,
            style,
            stylesheet,
            box_width,
            page_num_override,
        );
    }

    if style.display == Display::Flex {
        return layout_flex_container(
            el,
            ancestors,
            style,
            stylesheet,
            box_width,
            page_num_override,
        );
    }

    let inner_width = (box_width
        - style.padding_left
        - style.padding_right
        - style.border_left.width
        - style.border_right.width)
        .max(0.0);

    let content_x = style.padding_left + style.border_left.width;
    let mut cur_y = style.padding_top + style.border_top.width;
    let mut ops: Vec<DrawOp> = Vec::new();
    let mut child_blocks: Vec<LayoutBlock> = Vec::new();

    if has_block_children(el, ancestors, style, stylesheet) {
        let elem_children: Vec<&ElementNode> = el
            .children
            .iter()
            .filter_map(|c| match c {
                DomNode::Element(e) => Some(e),
                _ => None,
            })
            .collect();
        let total = elem_children.len();

        // If this block is a splittable container (like .group-block, .athlete-block, .event-header-group, .entry-group)
        let mut sub_blocks = Vec::new();
        for (i, ch) in elem_children.into_iter().enumerate() {
            let mut child_anc = ancestors.to_vec();
            child_anc.push((ch, i, total));
            let mut child_style = stylesheet.compute_style(&child_anc, style);
            if ch.has_class("group-header")
                || ch.has_class("athlete-header")
                || ch.has_class("event-header-box")
                || ch.has_class("entries-header-row")
                || ch.has_class("heat-header-row")
                || matches!(ch.tag.as_str(), "h1" | "h2" | "h3" | "h4")
            {
                child_style.break_after_avoid = true;
            }
            if page_num_override.is_none() {
                collect_flow_blocks(
                    ch,
                    &child_anc,
                    &child_style,
                    stylesheet,
                    inner_width,
                    &mut sub_blocks,
                );
            } else {
                let b = layout_element_block(
                    ch,
                    &child_anc,
                    &child_style,
                    stylesheet,
                    inner_width,
                    page_num_override,
                );
                sub_blocks.push(b);
            }
        }

        for b in &sub_blocks {
            cur_y += b.margin_top;
            for op in &b.ops {
                ops.push(op.offset(content_x, cur_y));
            }
            cur_y += b.height + b.margin_bottom;
        }
        child_blocks = sub_blocks;
    } else {
        // Inline formatting context
        let inline_h = layout_inline_content(
            el,
            ancestors,
            style,
            stylesheet,
            content_x,
            cur_y,
            inner_width,
            &mut ops,
            page_num_override,
        );
        cur_y += inline_h;
    }

    cur_y += style.padding_bottom + style.border_bottom.width;

    // Enforce explicit height / min-height
    let mut final_height = cur_y;
    if let Dimension::Pt(h) = style.height {
        final_height = final_height.max(h);
    }
    if let Dimension::Pt(mh) = style.min_height {
        final_height = final_height.max(mh);
    }

    // Prepend background & borders so they draw behind content
    let mut final_ops = Vec::with_capacity(ops.len() + 5);
    emit_box_decorations(
        0.0,
        0.0,
        box_width,
        final_height,
        style,
        &mut final_ops,
    );
    final_ops.extend(ops);

    let break_after_avoid = style.break_after_avoid
        || el.has_class("heat-header-row")
        || el.has_class("event-header-box")
        || el.has_class("entries-header-row")
        || el.has_class("group-header")
        || el.has_class("athlete-header")
        || matches!(el.tag.as_str(), "h1" | "h2" | "h3" | "h4");

    LayoutBlock {
        width: box_width,
        height: final_height,
        margin_top: style.margin_top,
        margin_bottom: style.margin_bottom,
        padding_top: style.padding_top,
        padding_bottom: style.padding_bottom,
        padding_left: style.padding_left,
        background_color: style.background_color,
        border_top: style.border_top,
        border_right: style.border_right,
        border_bottom: style.border_bottom,
        border_left: style.border_left,
        border_radius: style.border_radius,
        break_inside_avoid: style.break_inside_avoid,
        break_after_avoid,
        break_before_page: style.break_before_page,
        break_after_page: style.break_after_page,
        ops: final_ops,
        children: child_blocks,
        repeat_header: None,
    }
}

fn has_direct_table_cell_children<'a>(
    el: &'a ElementNode,
    ancestors: &[(&'a ElementNode, usize, usize)],
    style: &ComputedStyle,
    stylesheet: &Stylesheet,
) -> bool {
    let elem_children: Vec<&ElementNode> = el
        .children
        .iter()
        .filter_map(|c| match c {
            DomNode::Element(e) => Some(e),
            _ => None,
        })
        .collect();
    let total = elem_children.len();
    for (i, ch) in elem_children.into_iter().enumerate() {
        let mut child_anc = ancestors.to_vec();
        child_anc.push((ch, i, total));
        let st = stylesheet.compute_style(&child_anc, style);
        if st.display == Display::TableCell {
            return true;
        }
    }
    false
}

fn has_block_children<'a>(
    el: &'a ElementNode,
    ancestors: &[(&'a ElementNode, usize, usize)],
    style: &ComputedStyle,
    stylesheet: &Stylesheet,
) -> bool {
    let elem_children: Vec<&ElementNode> = el
        .children
        .iter()
        .filter_map(|c| match c {
            DomNode::Element(e) => Some(e),
            _ => None,
        })
        .collect();
    let total = elem_children.len();
    for (i, ch) in elem_children.into_iter().enumerate() {
        if ch.tag == "br" {
            continue;
        }
        let mut child_anc = ancestors.to_vec();
        child_anc.push((ch, i, total));
        let st = stylesheet.compute_style(&child_anc, style);
        if matches!(
            st.display,
            Display::Block
                | Display::Table
                | Display::Flex
                | Display::TableRow
                | Display::TableCell
        ) {
            return true;
        }
    }
    false
}

fn emit_box_decorations(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    style: &ComputedStyle,
    ops: &mut Vec<DrawOp>,
) {
    if style.background_color.is_visible() {
        ops.push(DrawOp::Rect {
            x,
            y,
            w,
            h,
            radius: style.border_radius,
            color: style.background_color,
        });
    }
    if style.border_top.is_visible() {
        ops.push(DrawOp::Line {
            x1: x,
            y1: y + style.border_top.width * 0.5,
            x2: x + w,
            y2: y + style.border_top.width * 0.5,
            width: style.border_top.width,
            style: style.border_top.style,
            color: style.border_top.color,
        });
    }
    if style.border_bottom.is_visible() {
        ops.push(DrawOp::Line {
            x1: x,
            y1: y + h - style.border_bottom.width * 0.5,
            x2: x + w,
            y2: y + h - style.border_bottom.width * 0.5,
            width: style.border_bottom.width,
            style: style.border_bottom.style,
            color: style.border_bottom.color,
        });
    }
    if style.border_left.is_visible() {
        ops.push(DrawOp::Line {
            x1: x + style.border_left.width * 0.5,
            y1: y,
            x2: x + style.border_left.width * 0.5,
            y2: y + h,
            width: style.border_left.width,
            style: style.border_left.style,
            color: style.border_left.color,
        });
    }
    if style.border_right.is_visible() {
        ops.push(DrawOp::Line {
            x1: x + w - style.border_right.width * 0.5,
            y1: y,
            x2: x + w - style.border_right.width * 0.5,
            y2: y + h,
            width: style.border_right.width,
            style: style.border_right.style,
            color: style.border_right.color,
        });
    }
}

/// Layouts a `display: flex` container (used in celebration_style.css and points reports).
fn layout_flex_container<'a>(
    el: &'a ElementNode,
    ancestors: &[(&'a ElementNode, usize, usize)],
    style: &ComputedStyle,
    stylesheet: &Stylesheet,
    box_width: f32,
    page_num_override: Option<(usize, usize)>,
) -> LayoutBlock {
    let inner_width = (box_width
        - style.padding_left
        - style.padding_right
        - style.border_left.width
        - style.border_right.width)
        .max(0.0);
    let start_x = style.padding_left + style.border_left.width;
    let start_y = style.padding_top + style.border_top.width;

    let elem_children: Vec<&ElementNode> = el
        .children
        .iter()
        .filter_map(|c| match c {
            DomNode::Element(e) => Some(e),
            _ => None,
        })
        .collect();
    let total = elem_children.len();
    if total == 0 {
        return LayoutBlock {
            width: box_width,
            height: style.padding_top + style.padding_bottom,
            margin_top: style.margin_top,
            margin_bottom: style.margin_bottom,
            padding_top: style.padding_top,
            padding_bottom: style.padding_bottom,
            padding_left: style.padding_left,
            background_color: style.background_color,
            border_top: style.border_top,
            border_right: style.border_right,
            border_bottom: style.border_bottom,
            border_left: style.border_left,
            border_radius: style.border_radius,
            break_inside_avoid: style.break_inside_avoid,
            break_after_avoid: style.break_after_avoid,
            break_before_page: style.break_before_page,
            break_after_page: style.break_after_page,
            ops: Vec::new(),
            children: Vec::new(),
            repeat_header: None,
        };
    }

    let gap = style.flex_gap;
    let mut ops = Vec::new();

    // Check if children have explicit percentage widths > 40% that wrap (e.g. .relay-card width: 48%)
    let mut first_child_anc = ancestors.to_vec();
    first_child_anc.push((elem_children[0], 0, total));
    let first_st = stylesheet.compute_style(&first_child_anc, style);
    let is_two_col_wrap = matches!(first_st.width, Dimension::Percent(p) if p >= 45.0 && p <= 50.0) && total > 2;

    let mut cur_y = start_y;
    let mut row_blocks = Vec::new();

    if is_two_col_wrap {
        let item_w = (inner_width - gap) * 0.5;
        for pair in elem_children.chunks(2).enumerate() {
            let (chunk_idx, items) = pair;
            let mut row_h: f32 = 0.0;
            let mut row_ops = Vec::new();
            for (j, ch) in items.iter().enumerate() {
                let idx = chunk_idx * 2 + j;
                let mut child_anc = ancestors.to_vec();
                child_anc.push((ch, idx, total));
                let mut st = stylesheet.compute_style(&child_anc, style);
                st.width = Dimension::Pt(item_w);
                let b = layout_element_block(
                    ch,
                    &child_anc,
                    &st,
                    stylesheet,
                    item_w,
                    page_num_override,
                );
                let cx = start_x + j as f32 * (item_w + gap);
                for op in &b.ops {
                    row_ops.push(op.offset(cx, 0.0));
                }
                row_h = row_h.max(b.height);
            }
            for op in &row_ops {
                ops.push(op.offset(0.0, cur_y));
            }
            row_blocks.push(LayoutBlock {
                width: inner_width,
                height: row_h,
                margin_top: if chunk_idx == 0 { 0.0 } else { gap },
                margin_bottom: 0.0,
                padding_top: 0.0,
                padding_bottom: 0.0,
                padding_left: 0.0,
                background_color: Color::TRANSPARENT,
                border_top: BorderSide::NONE,
                border_right: BorderSide::NONE,
                border_bottom: BorderSide::NONE,
                border_left: BorderSide::NONE,
                border_radius: 0.0,
                break_inside_avoid: true,
                break_after_avoid: false,
                break_before_page: false,
                break_after_page: false,
                ops: row_ops,
                children: Vec::new(),
                repeat_header: None,
            });
            cur_y += row_h + gap;
        }
        if !row_blocks.is_empty() {
            cur_y -= gap;
        }
    } else if style.flex_justify_between && total == 2 {
        // e.g. .report-header with left title block and right badge/meta block
        let left_w = inner_width * 0.68;
        let right_w = inner_width * 0.30;
        let mut anc0 = ancestors.to_vec();
        anc0.push((elem_children[0], 0, 2));
        let st0 = stylesheet.compute_style(&anc0, style);
        let b0 = layout_element_block(
            elem_children[0],
            &anc0,
            &st0,
            stylesheet,
            left_w,
            page_num_override,
        );

        let mut anc1 = ancestors.to_vec();
        anc1.push((elem_children[1], 1, 2));
        let st1 = stylesheet.compute_style(&anc1, style);
        let b1 = layout_element_block(
            elem_children[1],
            &anc1,
            &st1,
            stylesheet,
            right_w,
            page_num_override,
        );

        let row_h = b0.height.max(b1.height);
        let y0 = start_y + (row_h - b0.height) * 0.5;
        let y1 = start_y + (row_h - b1.height) * 0.5;
        for op in &b0.ops {
            ops.push(op.offset(start_x, y0));
        }
        for op in &b1.ops {
            ops.push(op.offset(start_x + inner_width - b1.width, y1));
        }
        cur_y += row_h;
    } else {
        // Equal-width flex items (e.g. .summary-strip with .summary-card children)
        let item_w = (inner_width - gap * (total as f32 - 1.0)).max(0.0) / total as f32;
        let mut max_h: f32 = 0.0;
        let mut laid_out = Vec::with_capacity(total);
        for (i, ch) in elem_children.iter().enumerate() {
            let mut child_anc = ancestors.to_vec();
            child_anc.push((ch, i, total));
            let mut st = stylesheet.compute_style(&child_anc, style);
            st.width = Dimension::Pt(item_w);
            let b = layout_element_block(
                ch,
                &child_anc,
                &st,
                stylesheet,
                item_w,
                page_num_override,
            );
            max_h = max_h.max(b.height);
            laid_out.push((ch, child_anc, st, b));
        }
        for (i, (ch, child_anc, mut st, mut b)) in laid_out.into_iter().enumerate() {
            if b.height < max_h {
                st.height = Dimension::Pt(max_h);
                b = layout_element_block(
                    ch,
                    &child_anc,
                    &st,
                    stylesheet,
                    item_w,
                    page_num_override,
                );
            }
            let cx = start_x + i as f32 * (item_w + gap);
            for op in &b.ops {
                ops.push(op.offset(cx, start_y));
            }
        }
        cur_y += max_h;
    }

    let final_h = cur_y + style.padding_bottom + style.border_bottom.width;
    let mut final_ops = Vec::with_capacity(ops.len() + 5);
    emit_box_decorations(0.0, 0.0, box_width, final_h, style, &mut final_ops);
    final_ops.extend(ops);

    LayoutBlock {
        width: box_width,
        height: final_h,
        margin_top: style.margin_top,
        margin_bottom: style.margin_bottom,
        padding_top: style.padding_top,
        padding_bottom: style.padding_bottom,
        padding_left: style.padding_left,
        background_color: style.background_color,
        border_top: style.border_top,
        border_right: style.border_right,
        border_bottom: style.border_bottom,
        border_left: style.border_left,
        border_radius: style.border_radius,
        break_inside_avoid: style.break_inside_avoid,
        break_after_avoid: style.break_after_avoid,
        break_before_page: style.break_before_page,
        break_after_page: style.break_after_page,
        ops: final_ops,
        children: row_blocks,
        repeat_header: None,
    }
}

/// Layouts a CSS `display: table` element (used in `meet_program.j2` for `.entries-header-row`, `.div-entry-row`, `.div-relay-swimmers-row`, `.relay-grid`, `.relay-swimmer-row`).
fn layout_css_table<'a>(
    el: &'a ElementNode,
    ancestors: &[(&'a ElementNode, usize, usize)],
    style: &ComputedStyle,
    stylesheet: &Stylesheet,
    box_width: f32,
    page_num_override: Option<(usize, usize)>,
) -> LayoutBlock {
    let elem_children: Vec<&ElementNode> = el
        .children
        .iter()
        .filter_map(|c| match c {
            DomNode::Element(e) => Some(e),
            _ => None,
        })
        .collect();
    let total = elem_children.len();
    if total == 0 {
        return LayoutBlock {
            width: box_width,
            height: 0.0,
            margin_top: style.margin_top,
            margin_bottom: style.margin_bottom,
            padding_top: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
            background_color: style.background_color,
            border_top: style.border_top,
            border_right: style.border_right,
            border_bottom: style.border_bottom,
            border_left: style.border_left,
            border_radius: style.border_radius,
            break_inside_avoid: true,
            break_after_avoid: style.break_after_avoid,
            break_before_page: style.break_before_page,
            break_after_page: style.break_after_page,
            ops: Vec::new(),
            children: Vec::new(),
            repeat_header: None,
        };
    }

    // Collect cell styles and dimensions
    let mut cell_specs = Vec::with_capacity(total);
    let mut col_dims = Vec::with_capacity(total);
    for (i, ch) in elem_children.iter().enumerate() {
        let mut child_anc = ancestors.to_vec();
        child_anc.push((ch, i, total));
        let st = stylesheet.compute_style(&child_anc, style);
        if st.display == Display::None {
            continue;
        }
        col_dims.push(st.width);
        cell_specs.push((*ch, child_anc, st));
    }

    let col_widths = resolve_table_column_widths(&col_dims, box_width);
    let (row_h, mut cell_ops) = layout_table_row_cells(
        &cell_specs,
        &col_widths,
        style,
        stylesheet,
        page_num_override,
    );

    let final_h = row_h + style.padding_top + style.padding_bottom;
    let mut final_ops = Vec::with_capacity(cell_ops.len() + 5);
    emit_box_decorations(0.0, 0.0, box_width, final_h, style, &mut final_ops);
    for op in cell_ops.drain(..) {
        final_ops.push(op.offset(0.0, style.padding_top));
    }

    LayoutBlock {
        width: box_width,
        height: final_h,
        margin_top: style.margin_top,
        margin_bottom: style.margin_bottom,
        padding_top: style.padding_top,
        padding_bottom: style.padding_bottom,
        padding_left: style.padding_left,
        background_color: style.background_color,
        border_top: style.border_top,
        border_right: style.border_right,
        border_bottom: style.border_bottom,
        border_left: style.border_left,
        border_radius: style.border_radius,
        break_inside_avoid: true,
        break_after_avoid: style.break_after_avoid || el.has_class("entries-header-row"),
        break_before_page: style.break_before_page,
        break_after_page: style.break_after_page,
        ops: final_ops,
        children: Vec::new(),
        repeat_header: None,
    }
}

fn resolve_table_column_widths(dims: &[Dimension], table_width: f32) -> Vec<f32> {
    let n = dims.len();
    if n == 0 {
        return Vec::new();
    }
    let mut widths = vec![0.0f32; n];
    let mut auto_indices = Vec::new();
    let mut explicit_sum = 0.0f32;
    let mut all_explicit_pt = true;

    for (i, d) in dims.iter().enumerate() {
        match d {
            Dimension::Pt(pt) => {
                widths[i] = *pt;
                explicit_sum += *pt;
            }
            Dimension::Percent(pct) => {
                let w = table_width * (*pct / 100.0);
                widths[i] = w;
                explicit_sum += w;
                all_explicit_pt = false;
            }
            Dimension::Auto => {
                auto_indices.push(i);
            }
        }
    }

    if auto_indices.is_empty() {
        if explicit_sum > 0.0 && (explicit_sum - table_width).abs() > 0.1 {
            let scale = table_width / explicit_sum;
            for w in &mut widths {
                *w *= scale;
            }
        }
    } else if n >= 4 && all_explicit_pt && explicit_sum > table_width * 0.70 {
        // Multi-column data table with fixed pt columns + 1 auto Name column (e.g. psych_sheet.j2):
        // Give the auto column a natural minimum share (~35% of table_width) and scale proportionally.
        let natural_auto = (table_width * 0.36).max(88.0);
        for idx in auto_indices {
            widths[idx] = natural_auto;
            explicit_sum += natural_auto;
        }
        if explicit_sum > 0.0 {
            let scale = table_width / explicit_sum;
            for w in &mut widths {
                *w *= scale;
            }
        }
    } else if explicit_sum < table_width {
        let rem = table_width - explicit_sum;
        let each = rem / auto_indices.len() as f32;
        for idx in auto_indices {
            widths[idx] = each;
        }
    } else {
        // Explicit columns already exceed table_width; give auto columns a minimum share and scale
        let fallback_auto = (table_width * 0.12).max(20.0);
        for idx in auto_indices {
            widths[idx] = fallback_auto;
            explicit_sum += fallback_auto;
        }
        if explicit_sum > 0.0 {
            let scale = table_width / explicit_sum;
            for w in &mut widths {
                *w *= scale;
            }
        }
    }
    widths
}

fn layout_table_row_cells<'a>(
    cells: &[(&'a ElementNode, Vec<(&'a ElementNode, usize, usize)>, ComputedStyle)],
    col_widths: &[f32],
    row_style: &ComputedStyle,
    stylesheet: &Stylesheet,
    page_num_override: Option<(usize, usize)>,
) -> (f32, Vec<DrawOp>) {
    struct PreparedCell<'b> {
        style: ComputedStyle,
        x: f32,
        w: f32,
        content_h: f32,
        content_ops: Vec<DrawOp>,
        el: &'b ElementNode,
    }

    let mut prepared = Vec::with_capacity(cells.len());
    let mut col_idx = 0usize;
    let mut cur_x = 0.0f32;
    let mut max_row_h = match row_style.height {
        Dimension::Pt(h) => h,
        _ => 0.0,
    };
    if let Dimension::Pt(mh) = row_style.min_height {
        max_row_h = max_row_h.max(mh);
    }

    for (cell_el, cell_anc, cell_style) in cells {
        let colspan = cell_el
            .attrs
            .get("colspan")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1)
            .max(1);
        let mut cell_w = 0.0f32;
        for k in 0..colspan {
            if col_idx + k < col_widths.len() {
                cell_w += col_widths[col_idx + k];
            }
        }
        col_idx += colspan;

        if let Dimension::Pt(ch) = cell_style.height {
            max_row_h = max_row_h.max(ch);
        }

        let inner_w = (cell_w
            - cell_style.padding_left
            - cell_style.padding_right
            - cell_style.border_left.width
            - cell_style.border_right.width)
            .max(0.0);

        let mut content_ops = Vec::new();
        let content_h = if has_block_children(cell_el, cell_anc, cell_style, stylesheet) {
            let elem_children: Vec<&ElementNode> = cell_el
                .children
                .iter()
                .filter_map(|c| match c {
                    DomNode::Element(e) => Some(e),
                    _ => None,
                })
                .collect();
            let tot = elem_children.len();
            let mut cy = 0.0f32;
            for (i, ch) in elem_children.into_iter().enumerate() {
                let mut ch_anc = cell_anc.clone();
                ch_anc.push((ch, i, tot));
                let ch_st = stylesheet.compute_style(&ch_anc, cell_style);
                let b = layout_element_block(
                    ch,
                    &ch_anc,
                    &ch_st,
                    stylesheet,
                    inner_w,
                    page_num_override,
                );
                cy += b.margin_top;
                for op in &b.ops {
                    content_ops.push(op.offset(0.0, cy));
                }
                cy += b.height + b.margin_bottom;
            }
            cy
        } else {
            layout_inline_content(
                cell_el,
                cell_anc,
                cell_style,
                stylesheet,
                0.0,
                0.0,
                inner_w,
                &mut content_ops,
                page_num_override,
            )
        };

        let total_cell_h = content_h
            + cell_style.padding_top
            + cell_style.padding_bottom
            + cell_style.border_top.width
            + cell_style.border_bottom.width;
        max_row_h = max_row_h.max(total_cell_h);

        prepared.push(PreparedCell {
            style: cell_style.clone(),
            x: cur_x,
            w: cell_w,
            content_h,
            content_ops,
            el: cell_el,
        });
        cur_x += cell_w;
    }

    let mut row_ops = Vec::new();
    // Row background if visible
    if row_style.background_color.is_visible() {
        row_ops.push(DrawOp::Rect {
            x: 0.0,
            y: 0.0,
            w: cur_x,
            h: max_row_h,
            radius: 0.0,
            color: row_style.background_color,
        });
    }

    // Row top/bottom borders (drawn BEFORE cell contents so cell DQ underlines sit on top)
    if row_style.border_top.is_visible() {
        row_ops.push(DrawOp::Line {
            x1: 0.0,
            y1: row_style.border_top.width * 0.5,
            x2: cur_x,
            y2: row_style.border_top.width * 0.5,
            width: row_style.border_top.width,
            style: row_style.border_top.style,
            color: row_style.border_top.color,
        });
    }
    if row_style.border_bottom.is_visible() {
        row_ops.push(DrawOp::Line {
            x1: 0.0,
            y1: max_row_h - row_style.border_bottom.width * 0.5,
            x2: cur_x,
            y2: max_row_h - row_style.border_bottom.width * 0.5,
            width: row_style.border_bottom.width,
            style: row_style.border_bottom.style,
            color: row_style.border_bottom.color,
        });
    }

    for cell in prepared {
        // Cell background & borders
        emit_box_decorations(cell.x, 0.0, cell.w, max_row_h, &cell.style, &mut row_ops);

        let avail_inner_h = (max_row_h
            - cell.style.padding_top
            - cell.style.padding_bottom
            - cell.style.border_top.width
            - cell.style.border_bottom.width)
            .max(0.0);

        let y_off = match cell.style.vertical_align {
            VerticalAlign::Top => cell.style.padding_top + cell.style.border_top.width,
            VerticalAlign::Middle => {
                cell.style.padding_top
                    + cell.style.border_top.width
                    + ((avail_inner_h - cell.content_h).max(0.0) * 0.5)
            }
            VerticalAlign::Bottom => {
                max_row_h
                    - cell.style.padding_bottom
                    - cell.style.border_bottom.width
                    - cell.content_h
            }
        };
        let x_off = cell.x + cell.style.padding_left + cell.style.border_left.width;
        for op in cell.content_ops {
            row_ops.push(op.offset(x_off, y_off));
        }
        let _ = cell.el;
    }

    (max_row_h, row_ops)
}

/// Layouts an HTML `<table>` with `<thead>` and `<tbody>` rows.
fn layout_html_table<'a>(
    el: &'a ElementNode,
    ancestors: &[(&'a ElementNode, usize, usize)],
    style: &ComputedStyle,
    stylesheet: &Stylesheet,
    box_width: f32,
    page_num_override: Option<(usize, usize)>,
) -> LayoutBlock {
    // Collect thead rows and tbody rows
    let mut thead_rows: Vec<(
        &'a ElementNode,
        Vec<(&'a ElementNode, usize, usize)>,
        ComputedStyle,
    )> = Vec::new();
    let mut tbody_rows: Vec<(
        &'a ElementNode,
        Vec<(&'a ElementNode, usize, usize)>,
        ComputedStyle,
    )> = Vec::new();

    let elem_children: Vec<&ElementNode> = el
        .children
        .iter()
        .filter_map(|c| match c {
            DomNode::Element(e) => Some(e),
            _ => None,
        })
        .collect();
    let total = elem_children.len();

    for (i, ch) in elem_children.into_iter().enumerate() {
        let mut ch_anc = ancestors.to_vec();
        ch_anc.push((ch, i, total));
        let ch_st = stylesheet.compute_style(&ch_anc, style);
        match ch.tag.as_str() {
            "thead" => {
                let trs: Vec<&ElementNode> = ch
                    .children
                    .iter()
                    .filter_map(|c| match c {
                        DomNode::Element(e) if e.tag == "tr" => Some(e),
                        _ => None,
                    })
                    .collect();
                let tr_tot = trs.len();
                for (j, tr) in trs.into_iter().enumerate() {
                    let mut tr_anc = ch_anc.clone();
                    tr_anc.push((tr, j, tr_tot));
                    let tr_st = stylesheet.compute_style(&tr_anc, &ch_st);
                    thead_rows.push((tr, tr_anc, tr_st));
                }
            }
            "tbody" | "tfoot" => {
                let trs: Vec<&ElementNode> = ch
                    .children
                    .iter()
                    .filter_map(|c| match c {
                        DomNode::Element(e) if e.tag == "tr" => Some(e),
                        _ => None,
                    })
                    .collect();
                let tr_tot = trs.len();
                for (j, tr) in trs.into_iter().enumerate() {
                    let mut tr_anc = ch_anc.clone();
                    tr_anc.push((tr, j, tr_tot));
                    let tr_st = stylesheet.compute_style(&tr_anc, &ch_st);
                    tbody_rows.push((tr, tr_anc, tr_st));
                }
            }
            "tr" => {
                tbody_rows.push((ch, ch_anc, ch_st));
            }
            _ => {}
        }
    }

    // Determine column count and column widths from first non-colspan row
    let mut num_cols = 0usize;
    for (tr, _, _) in thead_rows.iter().chain(tbody_rows.iter()) {
        let mut c = 0usize;
        for node in &tr.children {
            if let DomNode::Element(cell) = node {
                if cell.tag == "td" || cell.tag == "th" {
                    let span = cell
                        .attrs
                        .get("colspan")
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    c += span;
                }
            }
        }
        num_cols = num_cols.max(c);
    }

    if num_cols == 0 {
        return LayoutBlock {
            width: box_width,
            height: 0.0,
            margin_top: style.margin_top,
            margin_bottom: style.margin_bottom,
            padding_top: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
            background_color: Color::TRANSPARENT,
            border_top: BorderSide::NONE,
            border_right: BorderSide::NONE,
            border_bottom: BorderSide::NONE,
            border_left: BorderSide::NONE,
            border_radius: 0.0,
            break_inside_avoid: false,
            break_after_avoid: false,
            break_before_page: false,
            break_after_page: false,
            ops: Vec::new(),
            children: Vec::new(),
            repeat_header: None,
        };
    }

    let mut col_dims = vec![Dimension::Auto; num_cols];
    for (tr, tr_anc, tr_st) in thead_rows.iter().chain(tbody_rows.iter()) {
        let cells: Vec<&ElementNode> = tr
            .children
            .iter()
            .filter_map(|c| match c {
                DomNode::Element(e) if e.tag == "td" || e.tag == "th" => Some(e),
                _ => None,
            })
            .collect();
        if cells.len() == num_cols {
            for (idx, cell) in cells.iter().enumerate() {
                if col_dims[idx] == Dimension::Auto {
                    let mut c_anc = tr_anc.clone();
                    c_anc.push((cell, idx, num_cols));
                    let c_st = stylesheet.compute_style(&c_anc, tr_st);
                    if c_st.width != Dimension::Auto {
                        col_dims[idx] = c_st.width;
                    } else if let Some(w_attr) = cell.attrs.get("width") {
                        col_dims[idx] = crate::css::parse_dimension(w_attr, c_st.font_size);
                    }
                }
            }
            if col_dims.iter().any(|d| *d != Dimension::Auto) {
                break;
            }
        }
    }

    let col_widths = resolve_table_column_widths(&col_dims, box_width);
    let mut row_blocks: Vec<LayoutBlock> = Vec::with_capacity(thead_rows.len() + tbody_rows.len());
    let mut repeat_header_block: Option<Box<LayoutBlock>> = None;

    // Layout thead rows
    let mut thead_ops = Vec::new();
    let mut thead_h = 0.0f32;
    for (tr, tr_anc, tr_st) in &thead_rows {
        let b = build_tr_block(
            tr,
            tr_anc,
            tr_st,
            &col_widths,
            box_width,
            true,
            stylesheet,
            page_num_override,
        );
        for op in &b.ops {
            thead_ops.push(op.offset(0.0, thead_h));
        }
        thead_h += b.height;
        row_blocks.push(b);
    }

    if thead_h > 0.0 {
        repeat_header_block = Some(Box::new(LayoutBlock {
            width: box_width,
            height: thead_h,
            margin_top: 0.0,
            margin_bottom: 0.0,
            padding_top: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
            background_color: Color::TRANSPARENT,
            border_top: BorderSide::NONE,
            border_right: BorderSide::NONE,
            border_bottom: BorderSide::NONE,
            border_left: BorderSide::NONE,
            border_radius: 0.0,
            break_inside_avoid: true,
            break_after_avoid: true,
            break_before_page: false,
            break_after_page: false,
            ops: thead_ops,
            children: Vec::new(),
            repeat_header: None,
        }));
    }

    // Layout tbody rows
    for (tr, tr_anc, tr_st) in &tbody_rows {
        let is_event_row = tr.has_class("event-row");
        let b = build_tr_block(
            tr,
            tr_anc,
            tr_st,
            &col_widths,
            box_width,
            is_event_row,
            stylesheet,
            page_num_override,
        );
        row_blocks.push(b);
    }

    let mut cur_y = 0.0f32;
    let mut all_ops = Vec::new();
    for rb in &row_blocks {
        for op in &rb.ops {
            all_ops.push(op.offset(0.0, cur_y));
        }
        cur_y += rb.height;
    }

    LayoutBlock {
        width: box_width,
        height: cur_y,
        margin_top: style.margin_top,
        margin_bottom: style.margin_bottom,
        padding_top: 0.0,
        padding_bottom: 0.0,
        padding_left: 0.0,
        background_color: style.background_color,
        border_top: style.border_top,
        border_right: style.border_right,
        border_bottom: style.border_bottom,
        border_left: style.border_left,
        border_radius: style.border_radius,
        break_inside_avoid: style.break_inside_avoid,
        break_after_avoid: style.break_after_avoid,
        break_before_page: style.break_before_page,
        break_after_page: style.break_after_page,
        ops: all_ops,
        children: row_blocks,
        repeat_header: repeat_header_block,
    }
}

fn build_tr_block<'a>(
    tr: &'a ElementNode,
    tr_anc: &[(&'a ElementNode, usize, usize)],
    tr_st: &ComputedStyle,
    col_widths: &[f32],
    box_width: f32,
    is_header_row: bool,
    stylesheet: &Stylesheet,
    page_num_override: Option<(usize, usize)>,
) -> LayoutBlock {
    let cell_els: Vec<&ElementNode> = tr
        .children
        .iter()
        .filter_map(|c| match c {
            DomNode::Element(e) if e.tag == "td" || e.tag == "th" => Some(e),
            _ => None,
        })
        .collect();
    let cell_tot = cell_els.len();
    let mut cell_specs = Vec::with_capacity(cell_tot);
    for (i, cell_el) in cell_els.into_iter().enumerate() {
        let mut c_anc = tr_anc.to_vec();
        c_anc.push((cell_el, i, cell_tot));
        let c_st = stylesheet.compute_style(&c_anc, tr_st);
        cell_specs.push((cell_el, c_anc, c_st));
    }

    let (row_h, row_ops) = layout_table_row_cells(
        &cell_specs,
        col_widths,
        tr_st,
        stylesheet,
        page_num_override,
    );

    LayoutBlock {
        width: box_width,
        height: row_h,
        margin_top: 0.0,
        margin_bottom: 0.0,
        padding_top: 0.0,
        padding_bottom: 0.0,
        padding_left: 0.0,
        background_color: tr_st.background_color,
        border_top: tr_st.border_top,
        border_right: BorderSide::NONE,
        border_bottom: tr_st.border_bottom,
        border_left: BorderSide::NONE,
        border_radius: 0.0,
        break_inside_avoid: true,
        break_after_avoid: is_header_row || tr_st.break_after_avoid,
        break_before_page: tr_st.break_before_page,
        break_after_page: tr_st.break_after_page,
        ops: row_ops,
        children: Vec::new(),
        repeat_header: None,
    }
}

#[derive(Debug, Clone)]
enum InlineItem {
    Text {
        text: String,
        font: FontSpec,
        font_size: f32,
        color: Color,
        bg_color: Color,
        pad_x: f32,
        pad_y: f32,
        radius: f32,
    },
    Box {
        w: f32,
        h: f32,
        margin_right: f32,
        bg_color: Color,
        border_bottom: BorderSide,
        radius: f32,
    },
    LineBreak,
}

fn collect_inline_items<'a>(
    el: &'a ElementNode,
    ancestors: &[(&'a ElementNode, usize, usize)],
    style: &ComputedStyle,
    stylesheet: &Stylesheet,
    avail_width: f32,
    page_num_override: Option<(usize, usize)>,
    out: &mut Vec<InlineItem>,
) {
    let mut elem_idx = 0usize;
    let elem_total = el
        .children
        .iter()
        .filter(|c| matches!(c, DomNode::Element(_)))
        .count();

    for child in &el.children {
        match child {
            DomNode::Text(raw) => {
                let norm = collapse_inline_whitespace(raw);
                if !norm.is_empty() {
                    let transformed = apply_text_transform(&norm, style.text_transform);
                    out.push(InlineItem::Text {
                        text: transformed,
                        font: FontSpec {
                            family: style.font_family,
                            bold: style.font_bold,
                            italic: style.font_italic,
                        },
                        font_size: style.font_size,
                        color: style.color,
                        bg_color: Color::TRANSPARENT,
                        pad_x: 0.0,
                        pad_y: 0.0,
                        radius: 0.0,
                    });
                }
            }
            DomNode::Element(ch_el) => {
                let cur_idx = elem_idx;
                elem_idx += 1;
                if ch_el.tag == "br" {
                    out.push(InlineItem::LineBreak);
                    continue;
                }
                let mut ch_anc = ancestors.to_vec();
                ch_anc.push((ch_el, cur_idx, elem_total));
                let ch_st = stylesheet.compute_style(&ch_anc, style);
                if ch_st.display == Display::None {
                    continue;
                }

                // Special handling for <span class="page-counter"></span>
                if ch_el.has_class("page-counter") {
                    let pg_str = page_num_override
                        .map(|(p, _)| p.to_string())
                        .unwrap_or_else(|| "1".to_string());
                    out.push(InlineItem::Text {
                        text: pg_str,
                        font: FontSpec {
                            family: ch_st.font_family,
                            bold: ch_st.font_bold,
                            italic: ch_st.font_italic,
                        },
                        font_size: ch_st.font_size,
                        color: ch_st.color,
                        bg_color: Color::TRANSPARENT,
                        pad_x: 0.0,
                        pad_y: 0.0,
                        radius: 0.0,
                    });
                    continue;
                }

                // Empty inline-block visual elements (e.g. .team-dot circle or .dq-line underline)
                if ch_el.children.is_empty()
                    && (ch_st.display == Display::InlineBlock
                        || ch_el.has_class("team-dot")
                        || ch_el.has_class("dq-line"))
                {
                    let w = match ch_st.width {
                        Dimension::Pt(pt) => pt,
                        Dimension::Percent(pct) => avail_width * (pct / 100.0),
                        Dimension::Auto => 5.0,
                    };
                    let h = match ch_st.height {
                        Dimension::Pt(pt) => pt,
                        _ => 5.0,
                    };
                    out.push(InlineItem::Box {
                        w,
                        h,
                        margin_right: ch_st.margin_right,
                        bg_color: ch_st.background_color,
                        border_bottom: ch_st.border_bottom,
                        radius: ch_st.border_radius,
                    });
                    continue;
                }

                // Inline badge with background (e.g. .time-improvement yellow pill in celebration report)
                if ch_st.background_color.is_visible() {
                    let txt = collapse_inline_whitespace(&ch_el.text_content());
                    let txt = apply_text_transform(&txt.trim(), ch_st.text_transform);
                    if !txt.is_empty() {
                        out.push(InlineItem::Text {
                            text: txt,
                            font: FontSpec {
                                family: ch_st.font_family,
                                bold: ch_st.font_bold,
                                italic: ch_st.font_italic,
                            },
                            font_size: ch_st.font_size,
                            color: ch_st.color,
                            bg_color: ch_st.background_color,
                            pad_x: ch_st.padding_left.max(ch_st.padding_right),
                            pad_y: ch_st.padding_top.max(ch_st.padding_bottom),
                            radius: ch_st.border_radius,
                        });
                        continue;
                    }
                }

                collect_inline_items(
                    ch_el,
                    &ch_anc,
                    &ch_st,
                    stylesheet,
                    avail_width,
                    page_num_override,
                    out,
                );
            }
        }
    }
}

fn layout_inline_content<'a>(
    el: &'a ElementNode,
    ancestors: &[(&'a ElementNode, usize, usize)],
    style: &ComputedStyle,
    stylesheet: &Stylesheet,
    start_x: f32,
    start_y: f32,
    avail_width: f32,
    ops: &mut Vec<DrawOp>,
    page_num_override: Option<(usize, usize)>,
) -> f32 {
    let mut items = Vec::new();
    collect_inline_items(
        el,
        ancestors,
        style,
        stylesheet,
        avail_width,
        page_num_override,
        &mut items,
    );

    if items.is_empty() {
        return 0.0;
    }

    // Split items into logical lines by explicit <br> first, then wrap if needed
    let mut raw_lines: Vec<Vec<InlineItem>> = vec![Vec::new()];
    for item in items {
        if matches!(item, InlineItem::LineBreak) {
            raw_lines.push(Vec::new());
        } else {
            raw_lines.last_mut().unwrap().push(item);
        }
    }

    let line_h = (style.font_size * style.line_height).max(style.font_size);
    let mut cur_y = start_y;

    for raw_line in raw_lines {
        if raw_line.is_empty() {
            continue;
        }
        // Check if this line is a simple single-font text or mixed inline items
        let has_boxes_or_badges = raw_line.iter().any(|it| match it {
            InlineItem::Box { .. } => true,
            InlineItem::Text { bg_color, .. } => bg_color.is_visible(),
            _ => false,
        });

        if !has_boxes_or_badges && raw_line.len() == 1 {
            if let InlineItem::Text {
                ref text,
                font,
                font_size,
                color,
                ..
            } = raw_line[0]
            {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let lh = (font_size * style.line_height).max(font_size);
                let lines = if style.white_space_nowrap {
                    if style.text_overflow_ellipsis {
                        vec![truncate_ellipsis(trimmed, font, font_size, avail_width)]
                    } else {
                        vec![trimmed.to_string()]
                    }
                } else {
                    wrap_text(trimmed, font, font_size, avail_width)
                };

                for ln in lines {
                    let tw = measure_text(&ln, font, font_size);
                    let tx = match style.text_align {
                        TextAlign::Left => start_x,
                        TextAlign::Center => start_x + ((avail_width - tw) * 0.5).max(0.0),
                        TextAlign::Right => start_x + (avail_width - tw).max(0.0),
                    };
                    let baseline_y = cur_y + (lh - font_size) * 0.5 + font.ascent(font_size);
                    ops.push(DrawOp::Text {
                        x: tx,
                        baseline_y,
                        text: ln,
                        font,
                        font_size,
                        color,
                    });
                    cur_y += lh;
                }
                continue;
            }
        }

        // Mixed inline line (e.g. .team-dot + team code, or .dq-line, or bold + regular spans, or yellow PR badge)
        let mut segments = Vec::new();
        let mut extra_wrapped_lines: Vec<(String, FontSpec, f32, Color)> = Vec::new();
        let mut total_w = 0.0f32;
        let mut max_lh = line_h;

        for (idx, it) in raw_line.iter().enumerate() {
            match it {
                InlineItem::Box {
                    w,
                    h,
                    margin_right,
                    bg_color,
                    border_bottom,
                    radius,
                } => {
                    segments.push((
                        total_w,
                        *w,
                        InlineItem::Box {
                            w: *w,
                            h: *h,
                            margin_right: *margin_right,
                            bg_color: *bg_color,
                            border_bottom: *border_bottom,
                            radius: *radius,
                        },
                    ));
                    total_w += *w + *margin_right;
                }
                InlineItem::Text {
                    text,
                    font,
                    font_size,
                    color,
                    bg_color,
                    pad_x,
                    pad_y,
                    radius,
                } => {
                    let mut t = text.clone();
                    if idx == 0 {
                        t = t.trim_start().to_string();
                    }
                    if idx + 1 == raw_line.len() {
                        t = t.trim_end().to_string();
                    }
                    if t.is_empty() {
                        continue;
                    }
                    let rem = (avail_width - total_w).max(0.0);
                    if style.white_space_nowrap && style.text_overflow_ellipsis {
                        t = truncate_ellipsis(&t, *font, *font_size, rem);
                    } else if !style.white_space_nowrap
                        && !bg_color.is_visible()
                        && idx + 1 == raw_line.len()
                        && measure_text(&t, *font, *font_size) > rem + 0.5
                    {
                        let wrapped = wrap_text(&t, *font, *font_size, rem);
                        if !wrapped.is_empty() {
                            t = wrapped[0].clone();
                            for cont in wrapped.into_iter().skip(1) {
                                extra_wrapped_lines.push((cont, *font, *font_size, *color));
                            }
                        }
                    }
                    let tw = measure_text(&t, *font, *font_size) + pad_x * 2.0;
                    max_lh = max_lh.max(*font_size * style.line_height + pad_y * 2.0);
                    segments.push((
                        total_w,
                        tw,
                        InlineItem::Text {
                            text: t,
                            font: *font,
                            font_size: *font_size,
                            color: *color,
                            bg_color: *bg_color,
                            pad_x: *pad_x,
                            pad_y: *pad_y,
                            radius: *radius,
                        },
                    ));
                    total_w += tw;
                }
                InlineItem::LineBreak => {}
            }
        }

        if segments.is_empty() {
            continue;
        }

        let base_x = match style.text_align {
            TextAlign::Left => start_x,
            TextAlign::Center => start_x + ((avail_width - total_w) * 0.5).max(0.0),
            TextAlign::Right => start_x + (avail_width - total_w).max(0.0),
        };

        for (rel_x, seg_w, seg) in segments {
            let sx = base_x + rel_x;
            match seg {
                InlineItem::Box {
                    w,
                    h,
                    bg_color,
                    border_bottom,
                    radius,
                    ..
                } => {
                    let by = cur_y + (max_lh - h) * 0.5;
                    if bg_color.is_visible() {
                        ops.push(DrawOp::Rect {
                            x: sx,
                            y: by,
                            w,
                            h,
                            radius,
                            color: bg_color,
                        });
                    }
                    if border_bottom.is_visible() {
                        ops.push(DrawOp::Line {
                            x1: sx,
                            y1: by + h,
                            x2: sx + w,
                            y2: by + h,
                            width: border_bottom.width,
                            style: border_bottom.style,
                            color: border_bottom.color,
                        });
                    }
                }
                InlineItem::Text {
                    text,
                    font,
                    font_size,
                    color,
                    bg_color,
                    pad_x,
                    pad_y,
                    radius,
                } => {
                    if bg_color.is_visible() {
                        let bh = font_size + pad_y * 2.0;
                        let by = cur_y + (max_lh - bh) * 0.5;
                        ops.push(DrawOp::Rect {
                            x: sx,
                            y: by,
                            w: seg_w,
                            h: bh,
                            radius,
                            color: bg_color,
                        });
                    }
                    let baseline_y = cur_y + (max_lh - font_size) * 0.5 + font.ascent(font_size);
                    ops.push(DrawOp::Text {
                        x: sx + pad_x,
                        baseline_y,
                        text,
                        font,
                        font_size,
                        color,
                    });
                }
                InlineItem::LineBreak => {}
            }
        }
        cur_y += max_lh;

        for (cont_text, font, font_size, color) in extra_wrapped_lines {
            let lh = (font_size * style.line_height).max(font_size);
            let tw = measure_text(&cont_text, font, font_size);
            let tx = match style.text_align {
                TextAlign::Left => start_x,
                TextAlign::Center => start_x + ((avail_width - tw) * 0.5).max(0.0),
                TextAlign::Right => start_x + (avail_width - tw).max(0.0),
            };
            let baseline_y = cur_y + (lh - font_size) * 0.5 + font.ascent(font_size);
            ops.push(DrawOp::Text {
                x: tx,
                baseline_y,
                text: cont_text,
                font,
                font_size,
                color,
            });
            cur_y += lh;
        }
    }

    cur_y - start_y
}

fn collapse_inline_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_space = false;
    for ch in s.chars() {
        if ch.is_ascii_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }
    out
}

fn apply_text_transform(s: &str, tf: TextTransform) -> String {
    match tf {
        TextTransform::None => s.to_string(),
        TextTransform::Uppercase => s.to_uppercase(),
        TextTransform::Lowercase => s.to_lowercase(),
    }
}

struct PageData {
    ops: Vec<DrawOp>,
    col_max_y: Vec<f32>,
}

struct Paginator {
    page_style: PageStyle,
    col_count: usize,
    col_width: f32,
    col_gap: f32,
    col_rule: BorderSide,
    col_height: f32,
    pages: Vec<PageData>,
    cur_col: usize,
    cur_y: f32,
}

impl Paginator {
    fn new(
        page_style: PageStyle,
        col_count: usize,
        col_width: f32,
        col_gap: f32,
        col_rule: BorderSide,
        col_height: f32,
    ) -> Self {
        let mut p = Self {
            page_style,
            col_count,
            col_width,
            col_gap,
            col_rule,
            col_height,
            pages: Vec::new(),
            cur_col: 0,
            cur_y: 0.0,
        };
        p.push_new_page();
        p
    }

    fn push_new_page(&mut self) {
        self.pages.push(PageData {
            ops: Vec::new(),
            col_max_y: vec![0.0; self.col_count],
        });
        self.cur_col = 0;
        self.cur_y = 0.0;
    }

    fn advance_column(&mut self) {
        if self.cur_col + 1 < self.col_count {
            self.cur_col += 1;
            self.cur_y = 0.0;
        } else {
            self.push_new_page();
        }
    }

    fn fnadvance_page(&mut self) {
        if self.cur_col > 0 || self.cur_y > 0.0 {
            self.push_new_page();
        }
    }

    fn col_x(&self, col: usize) -> f32 {
        self.page_style.margin_left + col as f32 * (self.col_width + self.col_gap)
    }

    fn content_top_y(&self) -> f32 {
        self.page_style.margin_top
    }

    fn emit_block_at_current(&mut self, block: &LayoutBlock, top_margin: f32) {
        self.cur_y += top_margin;
        let x = self.col_x(self.cur_col);
        let y = self.content_top_y() + self.cur_y;
        let page = self.pages.last_mut().unwrap();
        for op in &block.ops {
            page.ops.push(op.offset(x, y));
        }
        self.cur_y += block.height + block.margin_bottom;
        page.col_max_y[self.cur_col] = page.col_max_y[self.cur_col].max(self.cur_y);
    }

    fn place_block(&mut self, block: &LayoutBlock) {
        if block.break_before_page {
            self.fnadvance_page();
        }
        if block.height <= 0.0 && block.ops.is_empty() && block.children.is_empty() {
            if block.break_after_page {
                self.fnadvance_page();
            }
            return;
        }

        let top_m = if self.cur_y == 0.0 { 0.0 } else { block.margin_top };
        let needed = top_m + block.height;
        let rem = self.col_height - self.cur_y;

        // 1. Fits completely in the remaining height of the current column
        if needed <= rem + 0.5 {
            self.emit_block_at_current(block, top_m);
            if block.break_after_page {
                self.fnadvance_page();
            }
            return;
        }

        // 2. Does not fit in current column:
        // If block has break_inside_avoid AND fits inside an empty column, move to next column and emit intact!
        if block.break_inside_avoid && block.height <= self.col_height + 0.5 {
            if self.cur_y > 0.0 {
                self.advance_column();
            }
            self.emit_block_at_current(block, 0.0);
            if block.break_after_page {
                self.fnadvance_page();
            }
            return;
        }

        // 3. Block is either not break_inside_avoid, OR is taller than a full column and must be split across columns/pages
        if !block.children.is_empty() {
            // Check if the initial keep-together header + first item fits in remaining height
            let min_first = top_m + block.min_first_piece_height() - block.margin_top;
            if self.cur_y > 0.0 && min_first > rem + 0.5 {
                self.advance_column();
            }
            self.place_children_sequence(
                &block.children,
                block.repeat_header.as_deref(),
                block.padding_left,
            );
            self.cur_y = (self.cur_y + block.margin_bottom + block.padding_bottom)
                .min(self.col_height);
            if block.break_after_page {
                self.fnadvance_page();
            }
            return;
        }

        // 4. Atomic leaf block that didn't fit in remaining space
        if self.cur_y > 0.0 {
            self.advance_column();
        }
        self.emit_block_at_current(block, 0.0);
        if block.break_after_page {
            self.fnadvance_page();
        }
    }

    fn place_children_sequence(
        &mut self,
        children: &[LayoutBlock],
        repeat_header: Option<&LayoutBlock>,
        _x_indent: f32,
    ) {
        let mut i = 0usize;
        while i < children.len() {
            let ch = &children[i];
            if ch.break_before_page {
                self.fnadvance_page();
            }

            // If this child has break_after_avoid (e.g. .heat-header-row, .group-header, thead),
            // make sure the entire keep-with-next chain fits in the current column!
            let chain_h = keep_with_next_chain_height(children, i);
            let top_m = if self.cur_y == 0.0 { 0.0 } else { ch.margin_top };
            let rem = self.col_height - self.cur_y;

            if self.cur_y > 0.0 && (top_m + chain_h - ch.margin_top) > rem + 0.5 {
                self.advance_column();
                // If we broke inside a table that has a repeat_header (thead) and i > 0, emit the repeated thead!
                if i > 0 {
                    if let Some(rh) = repeat_header {
                        self.emit_block_at_current(rh, 0.0);
                    }
                }
            }

            // Now place child i (which might itself be a nested table or block)
            let rem_now = self.col_height - self.cur_y;
            let top_m_now = if self.cur_y == 0.0 { 0.0 } else { ch.margin_top };
            if top_m_now + ch.height <= rem_now + 0.5 {
                self.emit_block_at_current(ch, top_m_now);
            } else if !ch.children.is_empty() {
                self.place_block(ch);
            } else {
                if self.cur_y > 0.0 {
                    self.advance_column();
                    if i > 0 {
                        if let Some(rh) = repeat_header {
                            self.emit_block_at_current(rh, 0.0);
                        }
                    }
                }
                self.emit_block_at_current(ch, 0.0);
            }

            if ch.break_after_page {
                self.fnadvance_page();
            }
            i += 1;
        }
    }

    fn finish(
        mut self,
        running_header: Option<&RunningHeaderTemplate>,
        stylesheet: &Stylesheet,
    ) -> PdfDocument {
        // Trim trailing completely empty page if > 1 page
        if self.pages.len() > 1 && self.pages.last().unwrap().ops.is_empty() {
            self.pages.pop();
        }

        let total_pages = self.pages.len().max(1);
        let mut pdf = PdfDocument::new();
        let content_width =
            self.page_style.width - self.page_style.margin_left - self.page_style.margin_right;

        for (page_idx, page_data) in self.pages.into_iter().enumerate() {
            let page_num = page_idx + 1;
            let mut canvas = PdfPageCanvas::new(self.page_style.width, self.page_style.height);

            // 1. Render running #header in top margin area if present
            if let Some(rh) = running_header {
                let anc = vec![(rh.node, 0, 1)];
                let header_block = layout_element_block(
                    rh.node,
                    &anc,
                    &rh.style,
                    stylesheet,
                    content_width,
                    Some((page_num, total_pages)),
                );
                let header_y = (self.page_style.margin_top - header_block.height - 10.0).max(14.0);
                for op in &header_block.ops {
                    op.render(&mut canvas, self.page_style.margin_left, header_y);
                }
            }

            // 2. Render multi-column vertical rule if column_count > 1 and col_rule is visible
            if self.col_count > 1 && self.col_rule.is_visible() {
                let max_col_h = page_data
                    .col_max_y
                    .iter()
                    .copied()
                    .fold(0.0f32, f32::max);
                if max_col_h > 0.0 {
                    for c in 0..(self.col_count - 1) {
                        let rule_x = self.page_style.margin_left
                            + (c + 1) as f32 * self.col_width
                            + c as f32 * self.col_gap
                            + self.col_gap * 0.5;
                        canvas.stroke_line(
                            rule_x,
                            self.page_style.margin_top,
                            rule_x,
                            self.page_style.margin_top + max_col_h,
                            self.col_rule.width,
                            self.col_rule.style,
                            self.col_rule.color,
                        );
                    }
                }
            }

            // 3. Render page body operations
            for op in &page_data.ops {
                op.render(&mut canvas, 0.0, 0.0);
            }

            // 4. Render @bottom-left and @bottom-right margin boxes if present
            let footer_baseline_y =
                self.page_style.height - (self.page_style.margin_bottom * 0.45).max(12.0);
            if let Some(ref bl) = self.page_style.bottom_left {
                let txt = bl
                    .content
                    .replace("{page}", &page_num.to_string())
                    .replace("{pages}", &total_pages.to_string());
                let fspec = FontSpec {
                    family: bl.font_family,
                    bold: false,
                    italic: false,
                };
                canvas.draw_text(
                    self.page_style.margin_left,
                    footer_baseline_y,
                    &txt,
                    fspec,
                    bl.font_size,
                    bl.color,
                );
            }
            if let Some(ref br) = self.page_style.bottom_right {
                let txt = br
                    .content
                    .replace("{page}", &page_num.to_string())
                    .replace("{pages}", &total_pages.to_string());
                let fspec = FontSpec {
                    family: br.font_family,
                    bold: false,
                    italic: false,
                };
                let tw = measure_text(&txt, fspec, br.font_size);
                canvas.draw_text(
                    self.page_style.width - self.page_style.margin_right - tw,
                    footer_baseline_y,
                    &txt,
                    fspec,
                    br.font_size,
                    br.color,
                );
            }

            pdf.pages.push(canvas);
        }

        pdf
    }
}
