//! Lightweight, fault-tolerant HTML5 tokenizer and DOM builder for report templates.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum DomNode {
    Element(ElementNode),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct ElementNode {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attrs: HashMap<String, String>,
    pub inline_style: String,
    pub children: Vec<DomNode>,
}

impl ElementNode {
    pub fn new(tag: String, attrs: HashMap<String, String>) -> Self {
        let id = attrs.get("id").cloned();
        let classes = attrs
            .get("class")
            .map(|c| {
                c.split_whitespace()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();
        let inline_style = attrs.get("style").cloned().unwrap_or_default();
        Self {
            tag,
            id,
            classes,
            attrs,
            inline_style,
            children: Vec::new(),
        }
    }

    pub fn has_class(&self, cls: &str) -> bool {
        self.classes.iter().any(|c| c == cls)
    }

    pub fn text_content(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        for child in &self.children {
            match child {
                DomNode::Text(t) => out.push_str(t),
                DomNode::Element(el) => {
                    if el.tag == "br" {
                        out.push('\n');
                    } else {
                        el.collect_text(out);
                    }
                }
            }
        }
    }
}

pub struct ParsedDocument {
    pub root: ElementNode,
    pub style_sheets: Vec<String>,
    pub link_hrefs: Vec<String>,
}

fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub fn decode_html_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&' {
            let mut end = i + 1;
            while end < chars.len() && end - i <= 12 && chars[end] != ';' && chars[end] != '&' && !chars[end].is_whitespace() {
                end += 1;
            }
            if end < chars.len() && chars[end] == ';' {
                let ent: String = chars[i + 1..end].iter().collect();
                let decoded = match ent.as_str() {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" | "#39" => Some('\''),
                    "nbsp" | "#160" => Some('\u{00A0}'),
                    "ndash" | "#8211" => Some('\u{2013}'),
                    "mdash" | "#8212" => Some('\u{2014}'),
                    "bull" | "#8226" => Some('\u{2022}'),
                    "middot" | "#183" => Some('\u{00B7}'),
                    "copy" | "#169" => Some('\u{00A9}'),
                    "reg" | "#174" => Some('\u{00AE}'),
                    "deg" | "#176" => Some('\u{00B0}'),
                    "plusmn" | "#177" => Some('\u{00B1}'),
                    "times" | "#215" => Some('\u{00D7}'),
                    "rarr" | "#8594" => Some('-'),
                    "larr" | "#8592" => Some('-'),
                    _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                        u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32)
                    }
                    _ if ent.starts_with('#') => {
                        ent[1..].parse::<u32>().ok().and_then(char::from_u32)
                    }
                    _ => None,
                };
                if let Some(ch) = decoded {
                    out.push(ch);
                    i = end + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

pub fn parse_html(html: &str) -> ParsedDocument {
    let mut stack: Vec<ElementNode> = vec![ElementNode::new("document".to_string(), HashMap::new())];
    let mut style_sheets = Vec::new();
    let mut link_hrefs = Vec::new();

    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        if bytes[pos] == b'<' {
            // Check comment <!-- ... -->
            if pos + 4 <= len && &bytes[pos..pos + 4] == b"<!--" {
                if let Some(end) = html[pos + 4..].find("-->") {
                    pos += 4 + end + 3;
                } else {
                    break;
                }
                continue;
            }
            // Check <!DOCTYPE or <![CDATA[
            if pos + 2 <= len && bytes[pos + 1] == b'!' {
                if let Some(end) = html[pos + 2..].find('>') {
                    pos += 2 + end + 1;
                } else {
                    break;
                }
                continue;
            }
            // Check <?xml ... ?>
            if pos + 2 <= len && bytes[pos + 1] == b'?' {
                if let Some(end) = html[pos + 2..].find('>') {
                    pos += 2 + end + 1;
                } else {
                    break;
                }
                continue;
            }

            // Closing tag </tag>
            if pos + 2 <= len && bytes[pos + 1] == b'/' {
                if let Some(end) = html[pos + 2..].find('>') {
                    let close_tag = html[pos + 2..pos + 2 + end]
                        .trim()
                        .to_ascii_lowercase();
                    pos += 2 + end + 1;
                    // Find matching open tag in stack
                    if let Some(match_idx) = stack.iter().rposition(|el| el.tag == close_tag) {
                        if match_idx > 0 {
                            while stack.len() > match_idx {
                                let popped = stack.pop().unwrap();
                                if popped.tag == "style" {
                                    style_sheets.push(popped.text_content());
                                }
                                if let Some(parent) = stack.last_mut() {
                                    parent.children.push(DomNode::Element(popped));
                                }
                            }
                        }
                    }
                    continue;
                } else {
                    break;
                }
            }

            // Opening tag <tag attr="val" ...>
            if let Some((tag_name, attrs, self_closing, next_pos)) = parse_open_tag(html, pos) {
                pos = next_pos;
                if tag_name == "link" {
                    let rel = attrs.get("rel").map(|s| s.to_ascii_lowercase()).unwrap_or_default();
                    if rel.contains("stylesheet") {
                        if let Some(href) = attrs.get("href") {
                            link_hrefs.push(href.clone());
                        }
                    }
                }

                // Auto-close implicit tags in tables/lists if needed
                auto_close_implicit(&mut stack, &tag_name);

                let elem = ElementNode::new(tag_name.clone(), attrs);

                if tag_name == "style" || tag_name == "script" {
                    // Raw text element until </style> or </script>
                    let close_pat = format!("</{}", tag_name);
                    let rest = &html[pos..];
                    let lower_rest = rest.to_ascii_lowercase();
                    if let Some(close_rel) = lower_rest.find(&close_pat) {
                        let raw_content = &rest[..close_rel];
                        if tag_name == "style" {
                            style_sheets.push(raw_content.to_string());
                        }
                        pos += close_rel;
                        if let Some(gt) = html[pos..].find('>') {
                            pos += gt + 1;
                        }
                    } else {
                        pos = len;
                    }
                    continue;
                }

                if self_closing || is_void_element(&tag_name) {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(DomNode::Element(elem));
                    }
                } else {
                    stack.push(elem);
                }
                continue;
            }
        }

        // Plain text until next '<'
        let next_lt = html[pos + 1..].find('<').map(|i| pos + 1 + i).unwrap_or(len);
        let raw_text = &html[pos..next_lt];
        let decoded = decode_html_entities(raw_text);
        if !decoded.is_empty() {
            if let Some(parent) = stack.last_mut() {
                if let Some(DomNode::Text(existing)) = parent.children.last_mut() {
                    existing.push_str(&decoded);
                } else {
                    parent.children.push(DomNode::Text(decoded));
                }
            }
        }
        pos = next_lt;
    }

    while stack.len() > 1 {
        let popped = stack.pop().unwrap();
        if let Some(parent) = stack.last_mut() {
            parent.children.push(DomNode::Element(popped));
        }
    }

    let root = stack.pop().unwrap();
    ParsedDocument {
        root,
        style_sheets,
        link_hrefs,
    }
}

fn auto_close_implicit(stack: &mut Vec<ElementNode>, new_tag: &str) {
    match new_tag {
        "tr" => {
            while stack.len() > 1 {
                let top = &stack.last().unwrap().tag;
                if top == "td" || top == "th" || top == "tr" {
                    let popped = stack.pop().unwrap();
                    stack.last_mut().unwrap().children.push(DomNode::Element(popped));
                } else {
                    break;
                }
            }
        }
        "td" | "th" => {
            while stack.len() > 1 {
                let top = &stack.last().unwrap().tag;
                if top == "td" || top == "th" {
                    let popped = stack.pop().unwrap();
                    stack.last_mut().unwrap().children.push(DomNode::Element(popped));
                } else {
                    break;
                }
            }
        }
        "tbody" | "thead" | "tfoot" => {
            while stack.len() > 1 {
                let top = &stack.last().unwrap().tag;
                if matches!(top.as_str(), "td" | "th" | "tr" | "thead" | "tbody" | "tfoot") {
                    let popped = stack.pop().unwrap();
                    stack.last_mut().unwrap().children.push(DomNode::Element(popped));
                } else {
                    break;
                }
            }
        }
        "li" => {
            if stack.last().map(|e| e.tag.as_str()) == Some("li") {
                let popped = stack.pop().unwrap();
                stack.last_mut().unwrap().children.push(DomNode::Element(popped));
            }
        }
        _ => {}
    }
}

fn parse_open_tag(
    html: &str,
    start: usize,
) -> Option<(String, HashMap<String, String>, bool, usize)> {
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut i = start + 1;
    if i >= len || (!bytes[i].is_ascii_alphabetic() && bytes[i] != b'_') {
        return None;
    }
    let tag_start = i;
    while i < len
        && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b'_' || bytes[i] == b':')
    {
        i += 1;
    }
    let tag_name = html[tag_start..i].to_ascii_lowercase();
    let mut attrs = HashMap::new();
    let mut self_closing = false;

    while i < len {
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }
        if bytes[i] == b'>' {
            i += 1;
            return Some((tag_name, attrs, self_closing, i));
        }
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'>' {
            self_closing = true;
            i += 2;
            return Some((tag_name, attrs, self_closing, i));
        }
        if bytes[i] == b'/' {
            i += 1;
            continue;
        }

        // Parse attribute name
        let attr_start = i;
        while i < len
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'>'
            && bytes[i] != b'/'
        {
            i += 1;
        }
        if i == attr_start {
            i += 1;
            continue;
        }
        let attr_name = html[attr_start..i].to_ascii_lowercase();
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < len && bytes[i] == b'=' {
            i += 1;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < len && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                let val_start = i;
                while i < len && bytes[i] != quote {
                    i += 1;
                }
                let val = decode_html_entities(&html[val_start..i]);
                if i < len {
                    i += 1;
                }
                attrs.insert(attr_name, val);
            } else {
                let val_start = i;
                while i < len && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                let val = decode_html_entities(&html[val_start..i]);
                attrs.insert(attr_name, val);
            }
        } else {
            attrs.insert(attr_name, String::new());
        }
    }
    Some((tag_name, attrs, self_closing, i))
}
