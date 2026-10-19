"""weasy-rs: High-speed Rust HTML+CSS Paged Media to PDF renderer (WeasyPrint drop-in)."""

from .api import HTML, render_html_to_pdf, render_html_to_pdf_bytes
from .bridge import is_rust_available

__all__ = [
    "HTML",
    "is_rust_available",
    "render_html_to_pdf",
    "render_html_to_pdf_bytes",
]
