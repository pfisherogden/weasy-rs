"""Native ctypes C-ABI bridge for weasy-rs."""

from __future__ import annotations

import ctypes
import logging
import os
import shutil
import subprocess
import sys
from pathlib import Path

logger = logging.getLogger(__name__)

_lib: ctypes.CDLL | None = None
_lib_loaded = False


def _lib_filename() -> str:
    if sys.platform == "darwin":
        return "libweasy_rs.dylib"
    if sys.platform == "win32":
        return "weasy_rs.dll"
    return "libweasy_rs.so"


def _core_lib_filename() -> str:
    if sys.platform == "darwin":
        return "libmm_core_rs.dylib"
    if sys.platform == "win32":
        return "mm_core_rs.dll"
    return "libmm_core_rs.so"


def _find_or_build_lib() -> Path | None:
    lib_name = _lib_filename()
    core_lib_name = _core_lib_filename()
    pkg_dir = Path(__file__).resolve().parent
    repo_root = pkg_dir.parent.parent

    candidates = [
        pkg_dir / lib_name,
        repo_root / "target" / "release" / lib_name,
    ]
    if hasattr(sys, "_MEIPASS"):
        meipass = Path(sys._MEIPASS)
        candidates.insert(0, meipass / "weasy_rs" / lib_name)
        candidates.insert(0, meipass / lib_name)
        candidates.append(meipass / "mm_to_json" / "lib" / core_lib_name)
        candidates.append(meipass / core_lib_name)

    for cand in candidates:
        if cand.exists():
            return cand

    cargo_toml = pkg_dir / "_rust" / "Cargo.toml"
    if not cargo_toml.exists():
        cargo_toml = repo_root / "Cargo.toml"

    cargo_bin = shutil.which("cargo")
    if not cargo_bin:
        fallback = Path.home() / ".cargo" / "bin" / "cargo"
        if fallback.exists():
            cargo_bin = str(fallback)

    if cargo_toml.exists() and cargo_bin:
        try:
            target_dir = pkg_dir / "_rust_target"
            env = os.environ.copy()
            subprocess.run(
                [
                    cargo_bin,
                    "build",
                    "--release",
                    "--manifest-path",
                    str(cargo_toml),
                    "--target-dir",
                    str(target_dir),
                ],
                check=True,
                capture_output=True,
                env=env,
                timeout=120,
            )
            built = target_dir / "release" / lib_name
            if built.exists():
                dest = pkg_dir / lib_name
                try:
                    shutil.copy2(built, dest)
                    return dest
                except OSError:
                    return built
        except Exception as e:
            logger.debug("weasy-rs auto-build failed: %s", e)

    try:
        from mm_to_json.rust_bridge import _find_or_build_rust_lib

        core_path = _find_or_build_rust_lib()
        if core_path and core_path.exists():
            return core_path
    except Exception:
        pass

    return None


def _get_lib() -> ctypes.CDLL | None:
    global _lib, _lib_loaded
    if _lib_loaded:
        return _lib
    _lib_loaded = True

    lib_path = _find_or_build_lib()
    if not lib_path:
        return None

    try:
        dll = ctypes.CDLL(str(lib_path))
        dll.weasy_rs_render_pdf_buf.argtypes = [
            ctypes.c_char_p,
            ctypes.c_size_t,
            ctypes.c_char_p,
            ctypes.c_size_t,
            ctypes.POINTER(ctypes.c_size_t),
            ctypes.POINTER(ctypes.c_size_t),
        ]
        dll.weasy_rs_render_pdf_buf.restype = ctypes.c_void_p

        dll.weasy_rs_free_buf.argtypes = [
            ctypes.c_void_p,
            ctypes.c_size_t,
            ctypes.c_size_t,
        ]
        dll.weasy_rs_free_buf.restype = None

        dll.weasy_rs_render_pdf_to_file.argtypes = [
            ctypes.c_char_p,
            ctypes.c_size_t,
            ctypes.c_char_p,
            ctypes.c_size_t,
            ctypes.c_char_p,
            ctypes.c_size_t,
        ]
        dll.weasy_rs_render_pdf_to_file.restype = ctypes.c_int

        _lib = dll
        return _lib
    except Exception as e:
        logger.warning("Failed to load weasy-rs library at %s: %s", lib_path, e)
        return None


def is_rust_available() -> bool:
    """Return True if the native Rust weasy-rs library is available."""
    return _get_lib() is not None


def rust_render_pdf_bytes(html: str, base_url: str | None = None) -> bytes:
    """Render HTML + CSS string to PDF bytes using native Rust."""
    dll = _get_lib()
    if dll is None:
        raise RuntimeError("weasy-rs native library is not available")
    html_bytes = html.encode("utf-8")
    base_bytes = base_url.encode("utf-8") if base_url else b""
    out_len = ctypes.c_size_t(0)
    out_cap = ctypes.c_size_t(0)
    ptr = dll.weasy_rs_render_pdf_buf(
        html_bytes,
        len(html_bytes),
        base_bytes if base_bytes else None,
        len(base_bytes),
        ctypes.byref(out_len),
        ctypes.byref(out_cap),
    )
    if not ptr:
        raise RuntimeError("weasy-rs failed to render PDF bytes")
    try:
        return ctypes.string_at(ptr, out_len.value)
    finally:
        dll.weasy_rs_free_buf(ptr, out_len.value, out_cap.value)


def rust_render_pdf_to_file(
    html: str, output_path: str | Path, base_url: str | None = None
) -> bool:
    """Render HTML + CSS directly to a PDF file on disk using native Rust."""
    dll = _get_lib()
    if dll is None:
        raise RuntimeError("weasy-rs native library is not available")
    html_bytes = html.encode("utf-8")
    base_bytes = base_url.encode("utf-8") if base_url else b""
    out_bytes = str(output_path).encode("utf-8")
    rc = dll.weasy_rs_render_pdf_to_file(
        html_bytes,
        len(html_bytes),
        base_bytes if base_bytes else None,
        len(base_bytes),
        out_bytes,
        len(out_bytes),
    )
    return rc == 0
