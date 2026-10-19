"""Comprehensive test suite for weasy-rs Python drop-in HTML + CSS Paged Media to PDF engine."""

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


def test_multicol_pagination_and_anonymous_css_table_rows() -> None:
    """Verify 2-column overflow across multiple pages and CSS 2.1 anonymous table rows (2x2 relay grid)."""
    events_html = []
    for ev_num in range(1, 35):
        events_html.append(f"""
        <div class="event-container">
            <div class="event-header-box">Event {ev_num} Boys 9-10 100 Yard Medley Relay</div>
            <div class="heat-container">
                <div class="heat-header">Heat 1 of 1 Finals</div>
                <div class="div-entry-group">
                    <div class="div-entry-row">
                        <div class="div-col col-lane">1</div>
                        <div class="div-col col-name">Del Prado Stingrays</div>
                        <div class="div-col col-relay-team">A</div>
                        <div class="div-col col-time">1:12.34</div>
                    </div>
                    <div class="div-relay-swimmers-row">
                        <div class="div-col col-relay-swimmer">1) Swimmer One 10</div>
                        <div class="div-col col-relay-swimmer">2) Swimmer Two 9</div>
                    </div>
                    <div class="div-relay-swimmers-row">
                        <div class="div-col col-relay-swimmer">3) Swimmer Three 10</div>
                        <div class="div-col col-relay-swimmer">4) Swimmer Four 10</div>
                    </div>
                </div>
            </div>
        </div>
        """)

    html_str = f"""
    <!DOCTYPE html>
    <html>
    <head>
        <style>
            @page {{
                size: letter;
                margin: 0.5in;
                margin-top: 1.25in;
                @bottom-left {{ content: "MeetManager-Tools"; font-size: 8pt; }}
                @bottom-right {{ content: "HY-TEK's MEET MANAGER 8.0"; font-size: 8pt; }}
            }}
            #header {{ position: running(header); border-bottom: 1.5pt solid #000; }}
            .content-container {{ column-count: 2; column-gap: 30pt; column-rule: 1pt solid #ddd; }}
            .event-container {{ break-inside: avoid; margin-bottom: 8pt; }}
            .event-header-box {{ background-color: #f0f0f0; font-weight: bold; padding: 2pt 4pt; border: 1pt solid #999; }}
            .div-entry-row {{ display: table; width: 100%; table-layout: fixed; }}
            .div-relay-swimmers-row {{ width: 100%; padding-left: 16pt; font-style: italic; }}
            .div-col {{ display: table-cell; padding: 0 3pt; }}
            .col-lane {{ width: 20pt; }}
            .col-name {{ width: 130pt; }}
            .col-relay-team {{ width: 20pt; }}
            .col-time {{ width: 45pt; text-align: right; }}
            .col-relay-swimmer {{ width: 50%; }}
        </style>
    </head>
    <body>
        <div id="header">2026 TVSL Championships - Page <span class="page-counter"></span></div>
        <div class="content-container">
            {"".join(events_html)}
        </div>
    </body>
    </html>
    """
    pdf_bytes = HTML(string=html_str).write_pdf()
    assert pdf_bytes.startswith(b"%PDF-1.4")
    # Should have generated multiple pages (/Type /Page entries)
    assert pdf_bytes.count(b"/Type /Page /Parent") >= 2


def test_html_table_thead_repetition_and_badges() -> None:
    """Verify HTML <table> with repeating <thead>, team dots, yellow badges, and landscape @page."""
    rows_html = []
    for i in range(1, 60):
        rows_html.append(f"""
        <tr>
            <td>{i}</td>
            <td><span class="team-dot" style="background-color: #d97706;"></span>Swimmer {i}, Test</td>
            <td>10</td>
            <td>DP</td>
            <td><span class="pr-badge">32.{i:02d} (-1.15)</span></td>
        </tr>
        """)
    html_str = f"""
    <!DOCTYPE html>
    <html>
    <head>
        <style>
            @page {{ size: letter landscape; margin: 0.5in; }}
            table {{ width: 100%; border-collapse: collapse; }}
            th {{ background-color: #111827; color: #ffffff; padding: 4pt; text-align: left; }}
            td {{ padding: 3pt 4pt; border-bottom: 0.5pt solid #e5e7eb; }}
            .team-dot {{ display: inline-block; width: 6pt; height: 6pt; border-radius: 50%; margin-right: 4pt; }}
            .pr-badge {{ background-color: #fef08a; padding: 1pt 4pt; border-radius: 3pt; font-weight: bold; }}
        </style>
    </head>
    <body>
        <table>
            <thead>
                <tr>
                    <th>Place</th>
                    <th>Athlete</th>
                    <th>Age</th>
                    <th>Team</th>
                    <th>Finals Time</th>
                </tr>
            </thead>
            <tbody>
                {"".join(rows_html)}
            </tbody>
        </table>
    </body>
    </html>
    """
    pdf_bytes = HTML(string=html_str).write_pdf()
    assert pdf_bytes.startswith(b"%PDF-1.4")
    assert b"/MediaBox [0 0 792.00 612.00]" in pdf_bytes
    assert pdf_bytes.count(b"/Type /Page /Parent") >= 2
