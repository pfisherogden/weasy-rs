//! Direct PDF 1.4 writer with FlateDecode stream compression and Standard 14 Type1 fonts.

use crate::css::{BorderStyle, Color};
use crate::fonts::{FontSpec, ALL_FONTS};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fmt::Write as FmtWrite;
use std::io::Write;

pub struct PdfPageCanvas {
    pub width: f32,
    pub height: f32,
    stream: String,
}

impl PdfPageCanvas {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            stream: String::with_capacity(16384),
        }
    }

    #[inline]
    fn pdf_y(&self, top_y: f32) -> f32 {
        self.height - top_y
    }

    pub fn fill_rect(&mut self, x: f32, top_y: f32, w: f32, h: f32, color: Color) {
        if !color.is_visible() || w <= 0.0 || h <= 0.0 {
            return;
        }
        let by = self.pdf_y(top_y + h);
        let _ = writeln!(
            self.stream,
            "q {:.3} {:.3} {:.3} rg {:.2} {:.2} {:.2} {:.2} re f Q",
            color.r, color.g, color.b, x, by, w, h
        );
    }

    pub fn fill_rounded_rect(&mut self, x: f32, top_y: f32, w: f32, h: f32, radius: f32, color: Color) {
        if !color.is_visible() || w <= 0.0 || h <= 0.0 {
            return;
        }
        let r = radius.min(w * 0.5).min(h * 0.5);
        if r <= 0.2 {
            self.fill_rect(x, top_y, w, h, color);
            return;
        }
        let x0 = x;
        let x1 = x + w;
        let y1 = self.pdf_y(top_y);
        let y0 = self.pdf_y(top_y + h);
        let k = 0.55228475 * r;

        let _ = writeln!(
            self.stream,
            "q {:.3} {:.3} {:.3} rg {:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} l {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} l {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {:.2} {:.2} l {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c h f Q",
            color.r, color.g, color.b,
            x0 + r, y0,
            x1 - r, y0,
            x1 - r + k, y0, x1, y0 + r - k, x1, y0 + r,
            x1, y1 - r,
            x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1,
            x0 + r, y1,
            x0 + r - k, y1, x0, y1 - r + k, x0, y1 - r,
            x0, y0 + r,
            x0, y0 + r - k, x0 + r - k, y0, x0 + r, y0
        );
    }

    pub fn stroke_line(
        &mut self,
        x1: f32,
        top_y1: f32,
        x2: f32,
        top_y2: f32,
        width: f32,
        style: BorderStyle,
        color: Color,
    ) {
        if !color.is_visible() || width <= 0.0 || style == BorderStyle::None {
            return;
        }
        let y1 = self.pdf_y(top_y1);
        let y2 = self.pdf_y(top_y2);
        let dash = match style {
            BorderStyle::Solid | BorderStyle::None => "[] 0 d",
            BorderStyle::Dotted => "[1 2] 0 d",
            BorderStyle::Dashed => "[3 2] 0 d",
        };
        let _ = writeln!(
            self.stream,
            "q {:.3} {:.3} {:.3} RG {:.2} w {} {:.2} {:.2} m {:.2} {:.2} l S Q",
            color.r, color.g, color.b, width, dash, x1, y1, x2, y2
        );
    }

    pub fn draw_text(
        &mut self,
        x: f32,
        baseline_top_y: f32,
        text: &str,
        font: FontSpec,
        font_size: f32,
        color: Color,
    ) {
        if text.is_empty() || !color.is_visible() || font_size <= 0.0 {
            return;
        }
        let y = self.pdf_y(baseline_top_y);
        let escaped = encode_pdf_text(text);
        let _ = writeln!(
            self.stream,
            "BT /{} {:.2} Tf {:.3} {:.3} {:.3} rg {:.2} {:.2} Td ({}) Tj ET",
            font.pdf_resource_name(),
            font_size,
            color.r,
            color.g,
            color.b,
            x,
            y,
            escaped
        );
    }
}

fn encode_pdf_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        let code: u8 = match ch {
            '\u{00A0}' => 160, // nbsp -> space
            '\u{2013}' => 150, // en-dash in WinAnsi
            '\u{2014}' => 151, // em-dash in WinAnsi
            '\u{2018}' => 145, // left single quote
            '\u{2019}' => 146, // right single quote
            '\u{201C}' => 147, // left double quote
            '\u{201D}' => 148, // right double quote
            '\u{2022}' => 149, // bullet
            '\u{2026}' => 133, // ellipsis
            c if (c as u32) >= 32 && (c as u32) <= 126 => c as u8,
            c if (c as u32) >= 161 && (c as u32) <= 255 => c as u8,
            _ => b'?',
        };
        match code {
            b'(' => out.push_str("\\("),
            b')' => out.push_str("\\)"),
            b'\\' => out.push_str("\\\\"),
            c if (32..=126).contains(&c) => out.push(c as char),
            c => {
                let _ = write!(out, "\\{:03o}", c);
            }
        }
    }
    out
}

pub struct PdfDocument {
    pub pages: Vec<PdfPageCanvas>,
}

impl PdfDocument {
    pub fn new() -> Self {
        Self { pages: Vec::new() }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::with_capacity(65536);
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

        // Object numbering:
        // 1: Catalog
        // 2: Pages
        // 3..=12: 10 Standard Type1 Fonts (F1..F10)
        // 13..: For each page i (0..N):
        //   page_obj = 13 + i * 2
        //   content_obj = 14 + i * 2
        let num_pages = self.pages.len().max(1);
        let total_objs = 12 + num_pages * 2;
        let mut offsets = vec![0usize; total_objs + 1];

        // 1: Catalog
        offsets[1] = out.len();
        out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        // 2: Pages
        offsets[2] = out.len();
        let mut kids = String::new();
        for i in 0..num_pages {
            let page_obj = 13 + i * 2;
            let _ = write!(kids, "{} 0 R ", page_obj);
        }
        let pages_obj = format!(
            "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj\n",
            kids.trim(),
            num_pages
        );
        out.extend_from_slice(pages_obj.as_bytes());

        // 3..=12: Fonts F1..F10
        let mut font_res_entries = String::new();
        for (idx, font_spec) in ALL_FONTS.iter().enumerate() {
            let obj_id = 3 + idx;
            offsets[obj_id] = out.len();
            let font_obj = format!(
                "{} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>\nendobj\n",
                obj_id,
                font_spec.pdf_base_font()
            );
            out.extend_from_slice(font_obj.as_bytes());
            let _ = write!(font_res_entries, "/{} {} 0 R ", font_spec.pdf_resource_name(), obj_id);
        }

        // Pages + Content Streams
        if self.pages.is_empty() {
            let page_obj = 13;
            let content_obj = 14;
            offsets[page_obj] = out.len();
            let p_str = format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << {} >> >> /Contents {} 0 R >>\nendobj\n",
                page_obj, font_res_entries, content_obj
            );
            out.extend_from_slice(p_str.as_bytes());
            offsets[content_obj] = out.len();
            out.extend_from_slice(b"14 0 obj\n<< /Length 0 >>\nstream\n\nendstream\nendobj\n");
        } else {
            for (i, page) in self.pages.iter().enumerate() {
                let page_obj = 13 + i * 2;
                let content_obj = 14 + i * 2;

                offsets[page_obj] = out.len();
                let p_str = format!(
                    "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Resources << /Font << {} >> >> /Contents {} 0 R >>\nendobj\n",
                    page_obj, page.width, page.height, font_res_entries, content_obj
                );
                out.extend_from_slice(p_str.as_bytes());

                // Compress content stream with zlib (FlateDecode)
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                let _ = encoder.write_all(page.stream.as_bytes());
                let compressed = encoder.finish().unwrap_or_default();

                offsets[content_obj] = out.len();
                let c_header = format!(
                    "{} 0 obj\n<< /Filter /FlateDecode /Length {} >>\nstream\n",
                    content_obj,
                    compressed.len()
                );
                out.extend_from_slice(c_header.as_bytes());
                out.extend_from_slice(&compressed);
                out.extend_from_slice(b"\nendstream\nendobj\n");
            }
        }

        // Xref table
        let xref_offset = out.len();
        let xref_head = format!("xref\n0 {}\n0000000000 65535 f \n", total_objs + 1);
        out.extend_from_slice(xref_head.as_bytes());
        for offset in offsets.iter().skip(1) {
            let line = format!("{:010} 00000 n \n", offset);
            out.extend_from_slice(line.as_bytes());
        }

        // Trailer
        let trailer = format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            total_objs + 1,
            xref_offset
        );
        out.extend_from_slice(trailer.as_bytes());
        out
    }
}
