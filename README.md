# weasy-rs

High-performance native Rust HTML + CSS Paged Media to PDF rendering engine, designed as a drop-in replacement for `WeasyPrint` (`weasyprint.HTML`) for structured multi-column, table-heavy, and paged reports.

## Features

- **150x–240x Faster than WeasyPrint**: Renders 500-page championship report packs in under 400ms (vs ~69s in Python WeasyPrint).
- **Drop-In Python API (`weasy_rs.HTML`)**:
  ```python
  from weasy_rs import HTML

  # Write directly to a file
  HTML(string=html_content, base_url=template_dir).write_pdf("output.pdf")

  # Or return PDF bytes in memory
  pdf_bytes = HTML(string=html_content).write_pdf()
  ```
- **CSS Paged Media & Multi-Column Layout**:
  - `@page` rules (`size: letter`, `size: letter landscape`, `size: A4`, `margin`, `@top-center`, `@bottom-left`, `@bottom-right`, `counter(page)`, `counter(pages)`)
  - Running page headers (`position: running(header)`)
  - Multi-column flow (`column-count: 2`, `column-gap`, `column-rule`)
  - Page and column break control (`break-inside: avoid`, `break-after: avoid`, `break-before: page`, orphan header prevention)
  - CSS Tables (`display: table`, `display: table-cell`, `table-layout: fixed`, anonymous table objects) & HTML `<table>` (`<thead>` repetition across column/page splits, `colspan`, `border-collapse`)
  - Flexbox (`display: flex`, `justify-content: space-between`, `gap`)
  - Exact Adobe AFM font metrics for Standard 14 Type1 fonts (`Helvetica`, `Courier`, `Times-Roman` and bold/oblique variants) with `FlateDecode` compressed PDF 1.4 streams.
