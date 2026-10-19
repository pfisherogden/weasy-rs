"""Drop-in `weasyprint.HTML` compatible API backed by native Rust `weasy-rs`."""

from __future__ import annotations

from pathlib import Path
from typing import IO, Any

from .bridge import rust_render_pdf_bytes, rust_render_pdf_to_file


class HTML:
    """Drop-in replacement for `weasyprint.HTML`."""

    def __init__(
        self,
        filename: str | Path | None = None,
         url: str | None = None,
        file_obj: IO[Any] | None = None,
        string: str | bytes | None = None,
        encoding: str | None = None,
        base_url: str | Path | None = None,
        **kwargs: Any,
    ) -> None:
        self._base_url: str | None = str(base_url) if base_url is not None else None
        if string is not None:
            if isinstance(string, bytes):
                self._html = string.decode(encoding or "utf-8", errors="replace")
            else:
                self._html = string
        elif filename is not None:
            p = Path(filename)
            self._html = p.read_text(encoding=encoding or "utf-8")
            if self._base_url is None:
                self._base_url = str(p.resolve().parent)
        elif file_obj is not None:
            raw = file_obj.read()
            if isinstance(raw, bytes):
                self._html = raw.decode(encoding or "utf-8", errors="replace")
            else:
                self._html = str(raw)
        elif url is not None:
            p = Path(url.removeprefix("file://"))
            self._html = p.read_text(encoding=encoding or "utf-8")
            if self._base_url is None:
                self._base_url = str(p.resolve().parent)
        else:
            raise TypeError(
                "HTML() requires one of 'string', 'filename', 'file_obj', or 'url'"
            )

    def write_pdf(
        self,
        target: str | Path | IO[bytes] | None = None,
        **kwargs: Any,
    ) -> bytes | None:
        """Render the HTML document to PDF.

        - If `target` is `None`, returns the rendered PDF as `bytes`.
        - If `target` is a `str` or `Path`, writes the PDF to disk and returns `None`.
        - If `target` is a file-like object with `.write()`, writes PDF bytes to it and returns `None`.
        """
        if target is None:
            return rust_render_pdf_bytes(self._html, self._base_url)
        if isinstance(target, (str, Path)):
            ok = rust_render_pdf_to_file(self._html, target, self._base_url)
            if not ok:
                raise RuntimeError(f"weasy-rs failed to write PDF to {target}")
            return None
        if hasattr(target, "write"):
            pdf_bytes = rust_render_pdf_bytes(self._html, self._base_url)
            target.write(pdf_bytes)
            return None
        raise TypeError(f"Unsupported write_pdf target: {type(target)!r}")


def render_html_to_pdf(
    html: str, output_path: str | Path, base_url: str | Path | None = None
) -> bool:
    """Convenience function to render HTML string directly to a PDF file."""
    return rust_render_pdf_to_file(
        html, output_path, str(base_url) if base_url is not None else None
    )


def render_html_to_pdf_bytes(
    html: str, base_url: str | Path | None = None
) -> bytes:
    """Convenience function to render HTML string directly to PDF bytes."""
    return rust_render_pdf_bytes(
        html, str(base_url) if base_url is not None else None
    )
