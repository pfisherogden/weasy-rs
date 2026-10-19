"""Tests for weasy-rs Python drop-in HTML to PDF engine."""

from __future__ import annotations

import io
from pathlib import Path

from weasy_rs import HTML, is_rust_available, render_html_to_pdf, render_html_to_pdf_bytes


def test_is_rust_available() -> None:
    assert is_rust_available() is True


def test_html_write_pdf_bytes_and_file(tmp_path: Path) -> None:
    html_str = """
    <!DOCTYPE html>
    <html>
    <head>
        <style>
            @page { size: letter; margin: 0.5in; margin-top: 1.25in; }
            #header { position: running(header); border-bottom: 1pt solid #000; }
            .content-container { column-count: 2; column-gap: 30pt; column-rule: 1pt solid #ddd; }
            .event-header-box { background-color: #f0f0f0; font-weight: bold; padding: 2pt 4pt; border: 1pt solid #999; }
            .div-entry-row { display: table; width: 100%; max-width: 255pt; table-layout: fixed; }
            .div-col { display: table-cell; padding: 0 4pt; }
            .col-lane { width: 20pt; text-align: center; }
            .col-name { width: 88pt; }
            .col-time { width: 45pt; text-align: right; }
        </style>
    </head>
    <body>
        <div id="header">Tri-Valley Swim League - Page <span class="page-counter"></span></div>
        <div class="content-container">
            <div class="event-header-box">Event 1 Girls 6 &amp; under 100 Yard Medley Relay</div>
            <div class="div-entry-row">
                <div class="div-col col-lane">1</div>
                <div class="div-col col-name">SHRK</div>
                <div class="div-col col-time">2:08.09</div>
            </div>
        </div>
    </body>
    </html>
    """
    pdf_bytes = HTML(string=html_str).write_pdf()
    assert isinstance(pdf_bytes, bytes)
    assert pdf_bytes.startswith(b"%PDF-1.4")
    assert pdf_bytes.endswith(b"%%EOF\n")

    out_file = tmp_path / "test_out.pdf"
    res = HTML(string=html_str).write_pdf(out_file)
    assert res is None
    assert out_file.exists()
    assert out_file.read_bytes() == pdf_bytes

    buf = io.BytesIO()
    HTML(string=html_str).write_pdf(buf)
    assert buf.getvalue() == pdf_bytes

    html_file = tmp_path / "input.html"
    html_file.write_text(html_str, encoding="utf-8")
    assert HTML(filename=html_file).write_pdf() == pdf_bytes
    assert render_html_to_pdf_bytes(html_str) == pdf_bytes

    out_file2 = tmp_path / "test_out2.pdf"
    assert render_html_to_pdf(html_str, out_file2) is True
    assert out_file2.read_bytes() == pdf_bytes
