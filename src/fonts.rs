//! Standard PDF Type1 Font Metrics (Helvetica, Courier, Times-Roman) and text wrapping utilities.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontFamily {
    Helvetica,
    Courier,
    Times,
}

impl FontFamily {
    pub fn from_css(val: &str) -> Self {
        let lower = val.to_ascii_lowercase();
        if lower.contains("courier") || lower.contains("mono") || lower.contains("consolas") {
            FontFamily::Courier
        } else if lower.contains("times") || lower.contains("georgia") || (lower.contains("serif") && !lower.contains("sans-serif")) {
            FontFamily::Times
        } else {
            FontFamily::Helvetica
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontSpec {
    pub family: FontFamily,
    pub bold: bool,
    pub italic: bool,
}

impl FontSpec {
    pub fn to_pdf_font(self) -> PdfFont {
        PdfFont::select(self.family, self.bold, self.italic)
    }

    pub fn pdf_resource_name(self) -> &'static str {
        self.to_pdf_font().pdf_resource_name()
    }

    pub fn pdf_base_font(self) -> &'static str {
        self.to_pdf_font().base_font_name()
    }

    pub fn ascent(self, font_size: f32) -> f32 {
        match self.family {
            FontFamily::Helvetica => font_size * 0.75,
            FontFamily::Courier => font_size * 0.68,
            FontFamily::Times => font_size * 0.72,
        }
    }
}

pub const ALL_FONTS: [FontSpec; 10] = [
    FontSpec { family: FontFamily::Helvetica, bold: false, italic: false },
    FontSpec { family: FontFamily::Helvetica, bold: true, italic: false },
    FontSpec { family: FontFamily::Helvetica, bold: false, italic: true },
    FontSpec { family: FontFamily::Helvetica, bold: true, italic: true },
    FontSpec { family: FontFamily::Courier, bold: false, italic: false },
    FontSpec { family: FontFamily::Courier, bold: true, italic: false },
    FontSpec { family: FontFamily::Times, bold: false, italic: false },
    FontSpec { family: FontFamily::Times, bold: true, italic: false },
    FontSpec { family: FontFamily::Times, bold: false, italic: true },
    FontSpec { family: FontFamily::Times, bold: true, italic: true },
];

pub fn measure_text(text: &str, font: FontSpec, font_size: f32) -> f32 {
    font.to_pdf_font().measure_text(text, font_size)
}

pub fn truncate_ellipsis(text: &str, font: FontSpec, font_size: f32, max_width: f32) -> String {
    font.to_pdf_font().truncate_ellipsis(text, font_size, max_width)
}

pub fn wrap_text(text: &str, font: FontSpec, font_size: f32, max_width: f32) -> Vec<String> {
    font.to_pdf_font().wrap_text(text, font_size, max_width)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PdfFont {
    Helvetica,
    HelveticaBold,
    HelveticaOblique,
    HelveticaBoldOblique,
    Courier,
    CourierBold,
    TimesRoman,
    TimesBold,
    TimesItalic,
    TimesBoldItalic,
}

impl PdfFont {
    pub fn select(family: FontFamily, bold: bool, italic: bool) -> Self {
        match (family, bold, italic) {
            (FontFamily::Helvetica, false, false) => PdfFont::Helvetica,
            (FontFamily::Helvetica, true, false) => PdfFont::HelveticaBold,
            (FontFamily::Helvetica, false, true) => PdfFont::HelveticaOblique,
            (FontFamily::Helvetica, true, true) => PdfFont::HelveticaBoldOblique,
            (FontFamily::Courier, false, _) => PdfFont::Courier,
            (FontFamily::Courier, true, _) => PdfFont::CourierBold,
            (FontFamily::Times, false, false) => PdfFont::TimesRoman,
            (FontFamily::Times, true, false) => PdfFont::TimesBold,
            (FontFamily::Times, false, true) => PdfFont::TimesItalic,
            (FontFamily::Times, true, true) => PdfFont::TimesBoldItalic,
        }
    }

    pub fn pdf_resource_name(self) -> &'static str {
        match self {
            PdfFont::Helvetica => "F1",
            PdfFont::HelveticaBold => "F2",
            PdfFont::HelveticaOblique => "F3",
            PdfFont::HelveticaBoldOblique => "F4",
            PdfFont::Courier => "F5",
            PdfFont::CourierBold => "F6",
            PdfFont::TimesRoman => "F7",
            PdfFont::TimesBold => "F8",
            PdfFont::TimesItalic => "F9",
            PdfFont::TimesBoldItalic => "F10",
        }
    }

    pub fn base_font_name(self) -> &'static str {
        match self {
            PdfFont::Helvetica => "Helvetica",
            PdfFont::HelveticaBold => "Helvetica-Bold",
            PdfFont::HelveticaOblique => "Helvetica-Oblique",
            PdfFont::HelveticaBoldOblique => "Helvetica-BoldOblique",
            PdfFont::Courier => "Courier",
            PdfFont::CourierBold => "Courier-Bold",
            PdfFont::TimesRoman => "Times-Roman",
            PdfFont::TimesBold => "Times-Bold",
            PdfFont::TimesItalic => "Times-Italic",
            PdfFont::TimesBoldItalic => "Times-BoldItalic",
        }
    }

    /// Returns the character advance width in 1/1000ths of an em (standard Adobe AFM units).
    pub fn char_width_1000(self, ch: char) -> u16 {
        match self {
            PdfFont::Courier | PdfFont::CourierBold => 600,
            PdfFont::Helvetica | PdfFont::HelveticaOblique => helvetica_width(ch),
            PdfFont::HelveticaBold | PdfFont::HelveticaBoldOblique => helvetica_bold_width(ch),
            PdfFont::TimesRoman | PdfFont::TimesItalic => times_roman_width(ch),
            PdfFont::TimesBold | PdfFont::TimesBoldItalic => times_bold_width(ch),
        }
    }

    /// Measure the width of `text` in points at `font_size`.
    pub fn measure_text(self, text: &str, font_size: f32) -> f32 {
        let total_1000: u32 = text.chars().map(|c| self.char_width_1000(c) as u32).sum();
        (total_1000 as f32) * font_size / 1000.0
    }

    /// Truncate `text` to fit within `max_width` by appending `...` if needed.
    pub fn truncate_ellipsis(self, text: &str, font_size: f32, max_width: f32) -> String {
        if max_width <= 0.0 {
            return String::new();
        }
        if self.measure_text(text, font_size) <= max_width {
            return text.to_string();
        }
        let dots = "...";
        let dots_w = self.measure_text(dots, font_size);
        let target = (max_width - dots_w).max(0.0);
        let mut acc = 0.0f32;
        let mut out = String::new();
        for ch in text.chars() {
            let cw = (self.char_width_1000(ch) as f32) * font_size / 1000.0;
            if acc + cw > target {
                break;
            }
            acc += cw;
            out.push(ch);
        }
        out.push_str(dots);
        out
    }

    /// Wrap `text` into lines that fit within `max_width` points.
    pub fn wrap_text(self, text: &str, font_size: f32, max_width: f32) -> Vec<String> {
        if text.is_empty() {
            return vec![String::new()];
        }
        if max_width <= 1.0 || self.measure_text(text, font_size) <= max_width + 0.5 {
            return vec![text.to_string()];
        }

        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut current_w = 0.0f32;
        let space_w = (self.char_width_1000(' ') as f32) * font_size / 1000.0;

        for word in text.split(' ') {
            if word.is_empty() {
                continue;
            }
            let word_w = self.measure_text(word, font_size);
            if current.is_empty() {
                current.push_str(word);
                current_w = word_w;
            } else if current_w + space_w + word_w <= max_width + 0.5 {
                current.push(' ');
                current.push_str(word);
                current_w += space_w + word_w;
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
                current_w = word_w;
            }
        }
        if !current.is_empty() || lines.is_empty() {
            lines.push(current);
        }
        lines
    }
}

fn helvetica_width(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00A0}' => 278,
        '!' => 278,
        '"' => 355,
        '#' => 556,
        '$' => 556,
        '%' => 889,
        '&' => 667,
        '\'' => 191,
        '(' | ')' => 333,
        '*' => 389,
        '+' => 584,
        ',' => 278,
        '-' => 333,
        '.' => 278,
        '/' => 278,
        '0'..='9' => 556,
        ':' | ';' => 278,
        '<' | '=' | '>' => 584,
        '?' => 556,
        '@' => 1015,
        'A' => 667,
        'B' => 667,
        'C' => 722,
        'D' => 722,
        'E' => 667,
        'F' => 611,
        'G' => 778,
        'H' => 722,
        'I' => 278,
        'J' => 500,
        'K' => 667,
        'L' => 556,
        'M' => 833,
        'N' => 722,
        'O' => 778,
        'P' => 667,
        'Q' => 778,
        'R' => 722,
        'S' => 667,
        'T' => 611,
        'U' => 722,
        'V' => 667,
        'W' => 944,
        'X' => 667,
        'Y' => 667,
        'Z' => 611,
        '[' | '\\' | ']' => 278,
        '^' => 469,
        '_' => 556,
        '`' => 333,
        'a' => 556,
        'b' => 556,
        'c' => 500,
        'd' => 556,
        'e' => 556,
        'f' => 278,
        'g' => 556,
        'h' => 556,
        'i' => 222,
        'j' => 222,
        'k' => 500,
        'l' => 222,
        'm' => 833,
        'n' => 556,
        'o' => 556,
        'p' => 556,
        'q' => 556,
        'r' => 333,
        's' => 500,
        't' => 278,
        'u' => 556,
        'v' => 500,
        'w' => 722,
        'x' => 500,
        'y' => 500,
        'z' => 500,
        '{' | '}' => 334,
        '|' => 260,
        '~' => 584,
        '–' => 556,
        '—' => 1000,
        '•' => 350,
        _ => 556,
    }
}

fn helvetica_bold_width(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00A0}' => 278,
        '!' => 333,
        '"' => 474,
        '#' => 556,
        '$' => 556,
        '%' => 889,
        '&' => 722,
        '\'' => 238,
        '(' | ')' => 333,
        '*' => 389,
        '+' => 584,
        ',' => 278,
        '-' => 333,
        '.' => 278,
        '/' => 278,
        '0'..='9' => 556,
        ':' | ';' => 333,
        '<' | '=' | '>' => 584,
        '?' => 611,
        '@' => 975,
        'A' => 722,
        'B' => 722,
        'C' => 722,
        'D' => 722,
        'E' => 667,
        'F' => 611,
        'G' => 778,
        'H' => 722,
        'I' => 278,
        'J' => 556,
        'K' => 722,
        'L' => 611,
        'M' => 833,
        'N' => 722,
        'O' => 778,
        'P' => 667,
        'Q' => 778,
        'R' => 722,
        'S' => 667,
        'T' => 611,
        'U' => 722,
        'V' => 667,
        'W' => 944,
        'X' => 667,
        'Y' => 667,
        'Z' => 611,
        '[' | ']' => 333,
        '\\' => 278,
        '^' => 584,
        '_' => 556,
        '`' => 333,
        'a' => 556,
        'b' => 611,
        'c' => 556,
        'd' => 611,
        'e' => 556,
        'f' => 333,
        'g' => 611,
        'h' => 611,
        'i' => 278,
        'j' => 278,
        'k' => 556,
        'l' => 278,
        'm' => 889,
        'n' => 611,
        'o' => 611,
        'p' => 611,
        'q' => 611,
        'r' => 389,
        's' => 556,
        't' => 333,
        'u' => 611,
        'v' => 556,
        'w' => 778,
        'x' => 556,
        'y' => 556,
        'z' => 500,
        '{' | '}' => 389,
        '|' => 280,
        '~' => 584,
        '–' => 556,
        '—' => 1000,
        '•' => 350,
        _ => 556,
    }
}

fn times_roman_width(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00A0}' => 250,
        '!' => 333,
        '"' => 408,
        '#' => 500,
        '$' => 500,
        '%' => 833,
        '&' => 778,
        '\'' => 180,
        '(' | ')' => 333,
        '*' => 500,
        '+' => 564,
        ',' => 250,
        '-' => 333,
        '.' => 250,
        '/' => 278,
        '0'..='9' => 500,
        ':' | ';' => 278,
        '<' | '=' | '>' => 564,
        '?' => 444,
        '@' => 921,
        'A' => 722,
        'B' => 667,
        'C' => 667,
        'D' => 722,
        'E' => 611,
        'F' => 556,
        'G' => 722,
        'H' => 722,
        'I' => 333,
        'J' => 389,
        'K' => 722,
        'L' => 611,
        'M' => 889,
        'N' => 722,
        'O' => 722,
        'P' => 556,
        'Q' => 722,
        'R' => 667,
        'S' => 556,
        'T' => 611,
        'U' => 722,
        'V' => 722,
        'W' => 944,
        'X' => 722,
        'Y' => 722,
        'Z' => 611,
        'a' => 444,
        'b' => 500,
        'c' => 444,
        'd' => 500,
        'e' => 444,
        'f' => 333,
        'g' => 500,
        'h' => 500,
        'i' => 278,
        'j' => 278,
        'k' => 500,
        'l' => 278,
        'm' => 778,
        'n' => 500,
        'o' => 500,
        'p' => 500,
        'q' => 500,
        'r' => 333,
        's' => 389,
        't' => 278,
        'u' => 500,
        'v' => 500,
        'w' => 722,
        'x' => 500,
        'y' => 500,
        'z' => 444,
        _ => 500,
    }
}

fn times_bold_width(ch: char) -> u16 {
    match ch {
        ' ' | '\u{00A0}' => 250,
        '!' => 333,
        '"' => 555,
        '#' => 500,
        '$' => 500,
        '%' => 1000,
        '&' => 833,
        '\'' => 278,
        '(' | ')' => 333,
        '*' => 500,
        '+' => 570,
        ',' => 250,
        '-' => 333,
        '.' => 250,
        '/' => 278,
        '0'..='9' => 500,
        ':' | ';' => 333,
        '<' | '=' | '>' => 570,
        '?' => 500,
        '@' => 930,
        'A'..='D' => 722,
        'E' => 667,
        'F' => 611,
        'G' | 'H' => 778,
        'I' => 389,
        'J' => 500,
        'K' => 778,
        'L' => 667,
        'M' => 944,
        'N' => 722,
        'O' => 778,
        'P' => 611,
        'Q' => 778,
        'R' => 722,
        'S' => 556,
        'T' => 667,
        'U' | 'V' => 722,
        'W' => 1000,
        'X' | 'Y' => 722,
        'Z' => 667,
        'a' => 500,
        'b' => 556,
        'c' => 444,
        'd' => 556,
        'e' => 444,
        'f' => 333,
        'g' => 500,
        'h' => 556,
        'i' => 278,
        'j' => 333,
        'k' => 556,
        'l' => 278,
        'm' => 833,
        'n' => 556,
        'o' => 500,
        'p' | 'q' => 556,
        'r' => 444,
        's' => 389,
        't' => 333,
        'u' => 556,
        'v' => 500,
        'w' => 722,
        'x' | 'y' => 500,
        'z' => 444,
        _ => 500,
    }
}
