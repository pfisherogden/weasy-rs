//! CSS Paged Media parser, selector matcher, cascade, and ComputedStyle resolver.

use crate::fonts::FontFamily;
use crate::html::ElementNode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub fn is_visible(&self) -> bool {
        self.a > 0.001
    }
}

pub fn parse_color(val: &str) -> Option<Color> {
    let s = val.trim().to_ascii_lowercase();
    if s.is_empty() || s == "transparent" || s == "none" {
        return Some(Color::TRANSPARENT);
    }
    match s.as_str() {
        "black" => return Some(Color::BLACK),
        "white" => return Some(Color::WHITE),
        "red" => return Some(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }),
        "gray" | "grey" => return Some(Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 }),
        "lightgray" | "lightgrey" => return Some(Color { r: 0.827, g: 0.827, b: 0.827, a: 1.0 }),
        "yellow" => return Some(Color { r: 1.0, g: 1.0, b: 0.0, a: 1.0 }),
        _ => {}
    }
    if let Some(hex) = s.strip_prefix('#') {
        let hex = hex.trim();
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            return Some(Color {
                r: (r * 17) as f32 / 255.0,
                g: (g * 17) as f32 / 255.0,
                b: (b * 17) as f32 / 255.0,
                a: 1.0,
            });
        } else if hex.len() >= 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
                a: 1.0,
            });
        }
    }
    if s.starts_with("rgb(") || s.starts_with("rgba(") {
        let inner = s
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() >= 3 {
            let r: f32 = parts[0].parse().ok()?;
            let g: f32 = parts[1].parse().ok()?;
            let b: f32 = parts[2].parse().ok()?;
            let a: f32 = if parts.len() >= 4 {
                parts[3].parse().unwrap_or(1.0)
            } else {
                1.0
            };
            return Some(Color {
                r: (r / 255.0).clamp(0.0, 1.0),
                g: (g / 255.0).clamp(0.0, 1.0),
                b: (b / 255.0).clamp(0.0, 1.0),
                a: a.clamp(0.0, 1.0),
            });
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dimension {
    Auto,
    Pt(f32),
    Percent(f32),
}

pub fn parse_length_pt(val: &str, base_font_size: f32) -> Option<f32> {
    let s = val.trim().to_ascii_lowercase();
    if s == "0" || s == "0.0" {
        return Some(0.0);
    }
    if let Some(num) = s.strip_suffix("pt") {
        return num.trim().parse::<f32>().ok();
    }
    if let Some(num) = s.strip_suffix("in") {
        return num.trim().parse::<f32>().ok().map(|v| v * 72.0);
    }
    if let Some(num) = s.strip_suffix("px") {
        return num.trim().parse::<f32>().ok().map(|v| v * 0.75);
    }
    if let Some(num) = s.strip_suffix("mm") {
        return num.trim().parse::<f32>().ok().map(|v| v * 2.8346457);
    }
    if let Some(num) = s.strip_suffix("cm") {
        return num.trim().parse::<f32>().ok().map(|v| v * 28.346457);
    }
    if let Some(num) = s.strip_suffix("rem") {
        return num.trim().parse::<f32>().ok().map(|v| v * 10.0);
    }
    if let Some(num) = s.strip_suffix("em") {
        return num.trim().parse::<f32>().ok().map(|v| v * base_font_size);
    }
    s.parse::<f32>().ok()
}

pub fn parse_dimension(val: &str, base_font_size: f32) -> Dimension {
    let s = val.trim().to_ascii_lowercase();
    if s == "auto" || s == "none" || s.is_empty() {
        return Dimension::Auto;
    }
    if let Some(pct) = s.strip_suffix('%') {
        if let Ok(v) = pct.trim().parse::<f32>() {
            return Dimension::Percent(v);
        }
    }
    if let Some(pt) = parse_length_pt(&s, base_font_size) {
        return Dimension::Pt(pt);
    }
    Dimension::Auto
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    Flex,
    Table,
    TableRow,
    TableCell,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Solid,
    Dotted,
    Dashed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderSide {
    pub width: f32,
    pub style: BorderStyle,
    pub color: Color,
}

impl BorderSide {
    pub const NONE: BorderSide = BorderSide {
        width: 0.0,
        style: BorderStyle::None,
        color: Color::BLACK,
    };

    pub fn is_visible(&self) -> bool {
        self.width > 0.0 && self.style != BorderStyle::None && self.color.is_visible()
    }
}

pub fn parse_border_shorthand(val: &str, base_font_size: f32) -> BorderSide {
    let s = val.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") || s == "0" {
        return BorderSide::NONE;
    }
    let mut width = 1.0;
    let mut style = BorderStyle::Solid;
    let mut color = Color::BLACK;

    for token in s.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            "solid" => style = BorderStyle::Solid,
            "dotted" => style = BorderStyle::Dotted,
            "dashed" => style = BorderStyle::Dashed,
            "none" | "hidden" => style = BorderStyle::None,
            "thin" => width = 0.5,
            "medium" => width = 1.5,
            "thick" => width = 2.5,
            _ => {
                if let Some(w) = parse_length_pt(&lower, base_font_size) {
                    width = w;
                } else if let Some(c) = parse_color(&lower) {
                    color = c;
                }
            }
        }
    }
    BorderSide { width, style, color }
}

#[derive(Debug, Clone)]
pub struct PageMarginBox {
    pub content: String,
    pub font_family: FontFamily,
    pub font_size: f32,
    pub color: Color,
}

impl Default for PageMarginBox {
    fn default() -> Self {
        Self {
            content: String::new(),
            font_family: FontFamily::Helvetica,
            font_size: 8.0,
            color: Color { r: 0.4, g: 0.4, b: 0.4, a: 1.0 },
        }
    }
}

#[derive(Debug, Clone)]
pub struct PageStyle {
    pub width: f32,
    pub height: f32,
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub bottom_left: Option<PageMarginBox>,
    pub bottom_right: Option<PageMarginBox>,
}

impl Default for PageStyle {
    fn default() -> Self {
        Self {
            width: 612.0,
            height: 792.0,
            margin_top: 90.0,    // 1.25in default
            margin_right: 36.0,  // 0.5in default
            margin_bottom: 36.0, // 0.5in default
            margin_left: 36.0,   // 0.5in default
            bottom_left: None,
            bottom_right: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub display: Display,
    pub position_running: Option<String>,
    pub font_family: FontFamily,
    pub font_size: f32,
    pub font_bold: bool,
    pub font_italic: bool,
    pub line_height: f32,
    pub color: Color,
    pub background_color: Color,
    pub text_align: TextAlign,
    pub vertical_align: VerticalAlign,
    pub text_transform: TextTransform,
    pub white_space_nowrap: bool,
    pub text_overflow_ellipsis: bool,
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub border_top: BorderSide,
    pub border_right: BorderSide,
    pub border_bottom: BorderSide,
    pub border_left: BorderSide,
    pub border_radius: f32,
    pub border_collapse: bool,
    pub width: Dimension,
    pub max_width: Dimension,
    pub height: Dimension,
    pub min_height: Dimension,
    pub column_count: usize,
    pub column_gap: f32,
    pub column_rule: BorderSide,
    pub break_inside_avoid: bool,
    pub break_after_avoid: bool,
    pub break_before_page: bool,
    pub break_after_page: bool,
    pub flex_justify_between: bool,
    pub flex_gap: f32,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::Block,
            position_running: None,
            font_family: FontFamily::Helvetica,
            font_size: 8.5,
            font_bold: false,
            font_italic: false,
            line_height: 1.15,
            color: Color::BLACK,
            background_color: Color::TRANSPARENT,
            text_align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
            text_transform: TextTransform::None,
            white_space_nowrap: false,
            text_overflow_ellipsis: false,
            margin_top: 0.0,
            margin_right: 0.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            padding_top: 0.0,
            padding_right: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
            border_top: BorderSide::NONE,
            border_right: BorderSide::NONE,
            border_bottom: BorderSide::NONE,
            border_left: BorderSide::NONE,
            border_radius: 0.0,
            border_collapse: false,
            width: Dimension::Auto,
            max_width: Dimension::Auto,
            height: Dimension::Auto,
            min_height: Dimension::Auto,
            column_count: 1,
            column_gap: 18.0,
            column_rule: BorderSide::NONE,
            break_inside_avoid: false,
            break_after_avoid: false,
            break_before_page: false,
            break_after_page: false,
            flex_justify_between: false,
            flex_gap: 0.0,
        }
    }
}

impl ComputedStyle {
    pub fn inherit_from(parent: &ComputedStyle, tag: &str) -> Self {
        let default_display = match tag {
            "span" | "b" | "strong" | "i" | "em" | "u" | "a" | "small" | "sub" | "sup" | "label" => {
                Display::Inline
            }
            "table" => Display::Table,
            "tr" => Display::TableRow,
            "td" | "th" => Display::TableCell,
            "head" | "style" | "script" | "meta" | "link" | "title" => Display::None,
            _ => Display::Block,
        };
        let mut s = Self {
            display: default_display,
            font_family: parent.font_family,
            font_size: parent.font_size,
            font_bold: parent.font_bold || matches!(tag, "b" | "strong" | "th" | "h1" | "h2" | "h3" | "h4"),
            font_italic: parent.font_italic || matches!(tag, "i" | "em"),
            line_height: parent.line_height,
            color: parent.color,
            text_align: if tag == "th" {
                TextAlign::Left
            } else {
                parent.text_align
            },
            vertical_align: if matches!(tag, "td" | "th") {
                VerticalAlign::Middle
            } else {
                VerticalAlign::Top
            },
            text_transform: parent.text_transform,
            white_space_nowrap: parent.white_space_nowrap,
            border_collapse: parent.border_collapse,
            ..Default::default()
        };
        match tag {
            "h1" => {
                s.font_size = parent.font_size * 1.5;
                s.margin_bottom = 4.0;
            }
            "h2" => {
                s.font_size = parent.font_size * 1.25;
                s.margin_bottom = 3.0;
            }
            "h3" => {
                s.font_size = parent.font_size * 1.1;
                s.margin_bottom = 2.0;
            }
            _ => {}
        }
        s
    }

    pub fn apply_declaration(&mut self, prop: &str, raw_val: &str) {
        let val = raw_val
            .trim()
            .trim_end_matches("!important")
            .trim();
        let lower = val.to_ascii_lowercase();
        let fs = self.font_size;

        match prop {
            "display" => {
                self.display = match lower.as_str() {
                    "none" => Display::None,
                    "inline" => Display::Inline,
                    "inline-block" => Display::InlineBlock,
                    "flex" | "inline-flex" => Display::Flex,
                    "table" => Display::Table,
                    "table-row" => Display::TableRow,
                    "table-cell" => Display::TableCell,
                    _ => Display::Block,
                };
            }
            "position" => {
                if lower.starts_with("running(") && lower.ends_with(')') {
                    let name = lower
                        .trim_start_matches("running(")
                        .trim_end_matches(')')
                        .trim()
                        .to_string();
                    self.position_running = Some(name);
                }
            }
            "font-family" => {
                self.font_family = FontFamily::from_css(val);
            }
            "font-size" => {
                if let Some(pt) = parse_length_pt(&lower, fs) {
                    self.font_size = pt;
                }
            }
            "font-weight" => {
                self.font_bold = match lower.as_str() {
                    "bold" | "bolder" | "600" | "700" | "800" | "900" => true,
                    "normal" | "lighter" | "100" | "200" | "300" | "400" | "500" => false,
                    _ => self.font_bold,
                };
            }
            "font-style" => {
                self.font_italic = matches!(lower.as_str(), "italic" | "oblique");
            }
            "line-height" => {
                if lower.ends_with("pt") || lower.ends_with("px") || lower.ends_with("in") {
                    if let Some(pt) = parse_length_pt(&lower, fs) {
                        if fs > 0.0 {
                            self.line_height = pt / fs;
                        }
                    }
                } else if let Ok(mult) = lower.parse::<f32>() {
                    self.line_height = mult;
                }
            }
            "color" => {
                if let Some(c) = parse_color(&lower) {
                    self.color = c;
                }
            }
            "background-color" | "background" => {
                if let Some(c) = parse_color(&lower) {
                    self.background_color = c;
                } else {
                    for tok in lower.split_whitespace() {
                        if let Some(c) = parse_color(tok) {
                            if c.is_visible() {
                                self.background_color = c;
                                break;
                            }
                        }
                    }
                }
            }
            "text-align" => {
                self.text_align = match lower.as_str() {
                    "center" => TextAlign::Center,
                    "right" | "end" => TextAlign::Right,
                    _ => TextAlign::Left,
                };
            }
            "vertical-align" => {
                self.vertical_align = match lower.as_str() {
                    "middle" => VerticalAlign::Middle,
                    "bottom" => VerticalAlign::Bottom,
                    _ => VerticalAlign::Top,
                };
            }
            "text-transform" => {
                self.text_transform = match lower.as_str() {
                    "uppercase" => TextTransform::Uppercase,
                    "lowercase" => TextTransform::Lowercase,
                    _ => TextTransform::None,
                };
            }
            "white-space" => {
                self.white_space_nowrap = lower == "nowrap";
            }
            "text-overflow" => {
                self.text_overflow_ellipsis = lower == "ellipsis";
            }
            "margin" => {
                let (t, r, b, l) = parse_box_shorthand(&lower, fs);
                self.margin_top = t;
                self.margin_right = r;
                self.margin_bottom = b;
                self.margin_left = l;
            }
            "margin-top" => {
                if let Some(v) = parse_length_pt(&lower, fs) {
                    self.margin_top = v;
                }
            }
            "margin-right" => {
                if let Some(v) = parse_length_pt(&lower, fs) {
                    self.margin_right = v;
                }
            }
            "margin-bottom" => {
                if let Some(v) = parse_length_pt(&lower, fs) {
                    self.margin_bottom = v;
                }
            }
            "margin-left" => {
                if let Some(v) = parse_length_pt(&lower, fs) {
                    self.margin_left = v;
                }
            }
            "padding" => {
                let (t, r, b, l) = parse_box_shorthand(&lower, fs);
                self.padding_top = t;
                self.padding_right = r;
                self.padding_bottom = b;
                self.padding_left = l;
            }
            "padding-top" => {
                if let Some(v) = parse_length_pt(&lower, fs) {
                    self.padding_top = v;
                }
            }
            "padding-right" => {
                if let Some(v) = parse_length_pt(&lower, fs) {
                    self.padding_right = v;
                }
            }
            "padding-bottom" => {
                if let Some(v) = parse_length_pt(&lower, fs) {
                    self.padding_bottom = v;
                }
            }
            "padding-left" => {
                if let Some(v) = parse_length_pt(&lower, fs) {
                    self.padding_left = v;
                }
            }
            "border" => {
                let b = parse_border_shorthand(&lower, fs);
                self.border_top = b;
                self.border_right = b;
                self.border_bottom = b;
                self.border_left = b;
            }
            "border-top" => {
                self.border_top = parse_border_shorthand(&lower, fs);
            }
            "border-right" => {
                self.border_right = parse_border_shorthand(&lower, fs);
            }
            "border-bottom" => {
                self.border_bottom = parse_border_shorthand(&lower, fs);
            }
            "border-left" => {
                self.border_left = parse_border_shorthand(&lower, fs);
            }
            "border-color" => {
                if let Some(c) = parse_color(&lower) {
                    self.border_top.color = c;
                    self.border_right.color = c;
                    self.border_bottom.color = c;
                    self.border_left.color = c;
                }
            }
            "border-width" => {
                if let Some(w) = parse_length_pt(&lower, fs) {
                    self.border_top.width = w;
                    self.border_right.width = w;
                    self.border_bottom.width = w;
                    self.border_left.width = w;
                }
            }
            "border-radius" => {
                if lower.ends_with('%') {
                    self.border_radius = 999.0; // circle / pill
                } else if let Some(r) = parse_length_pt(&lower, fs) {
                    self.border_radius = r;
                }
            }
            "border-collapse" => {
                self.border_collapse = lower == "collapse";
            }
            "width" => {
                self.width = parse_dimension(&lower, fs);
            }
            "max-width" => {
                self.max_width = parse_dimension(&lower, fs);
            }
            "height" => {
                self.height = parse_dimension(&lower, fs);
            }
            "min-height" => {
                self.min_height = parse_dimension(&lower, fs);
            }
            "column-count" => {
                if let Ok(c) = lower.parse::<usize>() {
                    self.column_count = c.max(1);
                }
            }
            "column-gap" => {
                if let Some(g) = parse_length_pt(&lower, fs) {
                    self.column_gap = g;
                }
            }
            "column-rule" => {
                self.column_rule = parse_border_shorthand(&lower, fs);
            }
            "break-inside" | "page-break-inside" => {
                self.break_inside_avoid = lower == "avoid";
            }
            "break-after" | "page-break-after" => {
                if lower == "avoid" {
                    self.break_after_avoid = true;
                } else if lower == "page" || lower == "always" {
                    self.break_after_page = true;
                }
            }
            "break-before" | "page-break-before" => {
                self.break_before_page = matches!(lower.as_str(), "page" | "always");
            }
            "justify-content" => {
                self.flex_justify_between = lower == "space-between";
            }
            "gap" => {
                if let Some(g) = parse_length_pt(&lower, fs) {
                    self.flex_gap = g;
                }
            }
            _ => {}
        }
    }
}

pub fn parse_box_shorthand(val: &str, base_font_size: f32) -> (f32, f32, f32, f32) {
    let parts: Vec<f32> = val
        .split_whitespace()
        .map(|p| parse_length_pt(p, base_font_size).unwrap_or(0.0))
        .collect();
    match parts.len() {
        0 => (0.0, 0.0, 0.0, 0.0),
        1 => (parts[0], parts[0], parts[0], parts[0]),
        2 => (parts[0], parts[1], parts[0], parts[1]),
        3 => (parts[0], parts[1], parts[2], parts[1]),
        _ => (parts[0], parts[1], parts[2], parts[3]),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoClass {
    FirstChild,
    LastChild,
    NthChildEven,
    NthChildOdd,
}

#[derive(Debug, Clone)]
pub struct SimpleSelector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub pseudos: Vec<PseudoClass>,
}

impl SimpleSelector {
    pub fn specificity(&self) -> (u32, u32, u32) {
        let a = if self.id.is_some() { 1 } else { 0 };
        let b = (self.classes.len() + self.pseudos.len()) as u32;
        let c = if self.tag.is_some() { 1 } else { 0 };
        (a, b, c)
    }

    pub fn matches(&self, el: &ElementNode, child_idx: usize, sibling_count: usize) -> bool {
        if let Some(ref t) = self.tag {
            if t != "*" && &el.tag != t {
                return false;
            }
        }
        if let Some(ref id) = self.id {
            if el.id.as_ref() != Some(id) {
                return false;
            }
        }
        for cls in &self.classes {
            if !el.has_class(cls) {
                return false;
            }
        }
        for p in &self.pseudos {
            let one_based = child_idx + 1;
            match p {
                PseudoClass::FirstChild => {
                    if child_idx != 0 {
                        return false;
                    }
                }
                PseudoClass::LastChild => {
                    if child_idx + 1 != sibling_count {
                        return false;
                    }
                }
                PseudoClass::NthChildEven => {
                    if one_based % 2 != 0 {
                        return false;
                    }
                }
                PseudoClass::NthChildOdd => {
                    if one_based % 2 != 1 {
                        return false;
                    }
                }
            }
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
}

#[derive(Debug, Clone)]
pub struct Selector {
    pub parts: Vec<(Option<Combinator>, SimpleSelector)>,
    pub specificity: (u32, u32, u32),
}

impl Selector {
    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if s.is_empty() || s.starts_with('@') {
            return None;
        }
        // Normalize '>' with spaces around it
        let normalized = s.replace('>', " > ");
        let tokens: Vec<&str> = normalized.split_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        let mut pending_comb: Option<Combinator> = None;
        let mut spec = (0, 0, 0);

        for tok in tokens {
            if tok == ">" {
                pending_comb = Some(Combinator::Child);
                continue;
            }
            if tok == "+" || tok == "~" {
                return None;
            }
            let simple = parse_simple_selector(tok)?;
            let sp = simple.specificity();
            spec.0 += sp.0;
            spec.1 += sp.1;
            spec.2 += sp.2;
            let comb = if parts.is_empty() {
                None
            } else {
                Some(pending_comb.take().unwrap_or(Combinator::Descendant))
            };
            parts.push((comb, simple));
        }

        if parts.is_empty() {
            None
        } else {
            Some(Selector {
                parts,
                specificity: spec,
            })
        }
    }

    pub fn matches(&self, ancestors: &[(&ElementNode, usize, usize)]) -> bool {
        if ancestors.is_empty() || self.parts.is_empty() {
            return false;
        }
        let (target_el, target_idx, target_total) = ancestors.last().unwrap();
        let last_part = &self.parts.last().unwrap().1;
        if !last_part.matches(target_el, *target_idx, *target_total) {
            return false;
        }
        if self.parts.len() == 1 {
            return true;
        }

        let mut anc_pos = ancestors.len() - 1;
        for part_idx in (0..self.parts.len() - 1).rev() {
            let comb = self.parts[part_idx + 1].0.unwrap_or(Combinator::Descendant);
            let req = &self.parts[part_idx].1;
            if anc_pos == 0 {
                return false;
            }
            match comb {
                Combinator::Child => {
                    anc_pos -= 1;
                    let (el, idx, tot) = ancestors[anc_pos];
                    if !req.matches(el, idx, tot) {
                        return false;
                    }
                }
                Combinator::Descendant => {
                    let mut found = false;
                    while anc_pos > 0 {
                        anc_pos -= 1;
                        let (el, idx, tot) = ancestors[anc_pos];
                        if req.matches(el, idx, tot) {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return false;
                    }
                }
            }
        }
        true
    }
}

fn parse_simple_selector(tok: &str) -> Option<SimpleSelector> {
    let mut tag = None;
    let mut id = None;
    let mut classes = Vec::new();
    let mut pseudos = Vec::new();

    // Strip any ::after or ::before pseudo-elements except when checking
    if tok.contains("::") {
        return None;
    }

    // Split off pseudo-classes like :nth-child(even)
    let mut base = tok;
    while let Some(colon_idx) = base.rfind(':') {
        let pseudo_str = &base[colon_idx + 1..];
        base = &base[..colon_idx];
        match pseudo_str {
            "first-child" => pseudos.push(PseudoClass::FirstChild),
            "last-child" => pseudos.push(PseudoClass::LastChild),
            "nth-child(even)" | "nth-child(2n)" => pseudos.push(PseudoClass::NthChildEven),
            "nth-child(odd)" | "nth-child(2n+1)" => pseudos.push(PseudoClass::NthChildOdd),
            _ => return None,
        }
    }

    let chars: Vec<char> = base.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '#' {
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '.' && chars[i] != '#' {
                i += 1;
            }
            let val: String = chars[start..i].iter().collect();
            if !val.is_empty() {
                id = Some(val);
            }
        } else if c == '.' {
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '.' && chars[i] != '#' {
                i += 1;
            }
            let val: String = chars[start..i].iter().collect();
            if !val.is_empty() {
                classes.push(val);
            }
        } else {
            let start = i;
            while i < chars.len() && chars[i] != '.' && chars[i] != '#' {
                i += 1;
            }
            let val: String = chars[start..i].iter().collect();
            if !val.is_empty() {
                tag = Some(val.to_ascii_lowercase());
            }
        }
    }

    Some(SimpleSelector {
        tag,
        id,
        classes,
        pseudos,
    })
}

#[derive(Debug, Clone)]
pub struct CssRule {
    pub selector: Selector,
    pub declarations: Vec<(String, String)>,
    pub order: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    pub page: PageStyle,
    pub rules: Vec<CssRule>,
}

impl Stylesheet {
    pub fn parse_and_merge(&mut self, css: &str) {
        let clean = strip_css_comments(css);
        let bytes = clean.as_bytes();
        let len = bytes.len();
        let mut pos = 0;

        while pos < len {
            while pos < len && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos >= len {
                break;
            }

            if clean[pos..].starts_with("@page") {
                pos += 5;
                if let Some((block_content, next_pos)) = extract_braced_block(&clean, pos) {
                    self.parse_page_block(&block_content);
                    pos = next_pos;
                    continue;
                } else {
                    break;
                }
            }

            if bytes[pos] == b'@' {
                // Skip @media or @import or @font-face
                if let Some(open_brace) = clean[pos..].find('{') {
                    let header = clean[pos..pos + open_brace].trim().to_ascii_lowercase();
                    if let Some((block_content, next_pos)) =
                        extract_braced_block(&clean, pos + open_brace)
                    {
                        if header.starts_with("@media")
                            && (header.contains("print") || header.contains("all"))
                        {
                            self.parse_and_merge(&block_content);
                        }
                        pos = next_pos;
                        continue;
                    }
                }
                if let Some(semi) = clean[pos..].find(';') {
                    pos += semi + 1;
                    continue;
                }
                break;
            }

            if let Some(open_brace) = clean[pos..].find('{') {
                let selector_group = clean[pos..pos + open_brace].trim();
                if let Some((block_content, next_pos)) =
                    extract_braced_block(&clean, pos + open_brace)
                {
                    let decls = parse_declarations(&block_content);
                    if !decls.is_empty() {
                        for sel_raw in selector_group.split(',') {
                            if let Some(sel) = Selector::parse(sel_raw) {
                                let order = self.rules.len();
                                self.rules.push(CssRule {
                                    selector: sel,
                                    declarations: decls.clone(),
                                    order,
                                });
                            }
                        }
                    }
                    pos = next_pos;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn parse_page_block(&mut self, block: &str) {
        // A @page block can contain both top-level declarations and nested @top-center / @bottom-left / @bottom-right blocks
        let bytes = block.as_bytes();
        let len = bytes.len();
        let mut pos = 0;
        let mut top_level = String::new();

        while pos < len {
            if bytes[pos] == b'@' {
                let rest = &block[pos..];
                if let Some(open_b) = rest.find('{') {
                    let at_name = rest[..open_b].trim().to_ascii_lowercase();
                    if let Some((inner, next_p)) = extract_braced_block(block, pos + open_b) {
                        let decls = parse_declarations(&inner);
                        let mut mbox = PageMarginBox::default();
                        for (k, v) in decls {
                            match k.as_str() {
                                "content" => mbox.content = parse_margin_content(&v),
                                "font-family" => mbox.font_family = FontFamily::from_css(&v),
                                "font-size" => {
                                    if let Some(pt) = parse_length_pt(&v, 8.0) {
                                        mbox.font_size = pt;
                                    }
                                }
                                "color" => {
                                    if let Some(c) = parse_color(&v) {
                                        mbox.color = c;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if at_name == "@bottom-left" && !mbox.content.is_empty() {
                            self.page.bottom_left = Some(mbox);
                        } else if at_name == "@bottom-right" && !mbox.content.is_empty() {
                            self.page.bottom_right = Some(mbox);
                        }
                        pos = next_p;
                        continue;
                    }
                }
            }
            top_level.push(bytes[pos] as char);
            pos += 1;
        }

        for (k, v) in parse_declarations(&top_level) {
            let lower = v.to_ascii_lowercase();
            match k.as_str() {
                "size" => {
                    let is_landscape = lower.contains("landscape");
                    if lower.contains("a4") {
                        self.page.width = if is_landscape { 841.89 } else { 595.28 };
                        self.page.height = if is_landscape { 595.28 } else { 841.89 };
                    } else {
                        self.page.width = if is_landscape { 792.0 } else { 612.0 };
                        self.page.height = if is_landscape { 612.0 } else { 792.0 };
                    }
                }
                "margin" => {
                    let (t, r, b, l) = parse_box_shorthand(&lower, 10.0);
                    self.page.margin_top = t;
                    self.page.margin_right = r;
                    self.page.margin_bottom = b;
                    self.page.margin_left = l;
                }
                "margin-top" => {
                    if let Some(pt) = parse_length_pt(&lower, 10.0) {
                        self.page.margin_top = pt;
                    }
                }
                "margin-right" => {
                    if let Some(pt) = parse_length_pt(&lower, 10.0) {
                        self.page.margin_right = pt;
                    }
                }
                "margin-bottom" => {
                    if let Some(pt) = parse_length_pt(&lower, 10.0) {
                        self.page.margin_bottom = pt;
                    }
                }
                "margin-left" => {
                    if let Some(pt) = parse_length_pt(&lower, 10.0) {
                        self.page.margin_left = pt;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn compute_style(
        &self,
        ancestors: &[(&ElementNode, usize, usize)],
        parent_style: &ComputedStyle,
    ) -> ComputedStyle {
        let (el, _, _) = ancestors.last().unwrap();
        let mut style = ComputedStyle::inherit_from(parent_style, &el.tag);

        let mut matched: Vec<((u32, u32, u32), usize, &[(String, String)])> = Vec::new();
        for rule in &self.rules {
            if rule.selector.matches(ancestors) {
                matched.push((rule.selector.specificity, rule.order, &rule.declarations));
            }
        }
        matched.sort_by_key(|(spec, order, _)| (*spec, *order));

        // First pass: font-size declarations so em units resolve properly
        for (_, _, decls) in &matched {
            for (k, v) in *decls {
                if k == "font-size" {
                    style.apply_declaration(k, v);
                }
            }
        }
        for (_, _, decls) in &matched {
            for (k, v) in *decls {
                if k != "font-size" {
                    style.apply_declaration(k, v);
                }
            }
        }

        if !el.inline_style.is_empty() {
            for (k, v) in parse_declarations(&el.inline_style) {
                style.apply_declaration(&k, &v);
            }
        }

        style
    }
}

fn parse_margin_content(raw: &str) -> String {
    // Handles CSS content like: "Generated by MMTools" or "Page " counter(page) " of " counter(pages)
    let mut out = String::new();
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        if chars[i] == '"' || chars[i] == '\'' {
            let q = chars[i];
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != q {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            out.push_str(&s);
            if i < chars.len() {
                i += 1;
            }
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '"' && chars[i] != '\'' {
                i += 1;
            }
            let tok: String = chars[start..i].iter().collect();
            let lower = tok.to_ascii_lowercase();
            if lower == "counter(page)" {
                out.push_str("{page}");
            } else if lower == "counter(pages)" {
                out.push_str("{pages}");
            }
        }
    }
    out
}

pub fn parse_declarations(block: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in block.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(colon) = part.find(':') {
            let prop = part[..colon].trim().to_ascii_lowercase();
            let val = part[colon + 1..].trim().to_string();
            if !prop.is_empty() && !val.is_empty() {
                out.push((prop, val));
            }
        }
    }
    out
}

fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(len);
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn extract_braced_block(s: &str, start_pos: usize) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let open = s[start_pos..].find('{').map(|i| start_pos + i)?;
    let mut depth = 0usize;
    let mut i = open;
    while i < len {
        if bytes[i] == b'{' {
            depth += 1;
        } else if bytes[i] == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some((s[open + 1..i].to_string(), i + 1));
            }
        }
        i += 1;
    }
    None
}
