//! Zero-dependency C-ABI / ctypes bridge for `weasy-rs`.
//! Uses pure C-ABI buffer functions so the shared library links cleanly on
//! Linux, macOS, and Windows without requiring `python3.lib` at compile time.

use std::ffi::c_int;
use std::slice;

unsafe fn slice_to_str<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    let bytes = slice::from_raw_parts(ptr, len);
    std::str::from_utf8(bytes).ok()
}

/// Renders HTML + CSS to an owned heap buffer and writes its length and capacity
/// to `out_len` and `out_cap`. The caller must free the returned pointer with
/// `weasy_rs_free_buf(ptr, len, cap)`.
#[no_mangle]
pub unsafe extern "C" fn weasy_rs_render_pdf_buf(
    html_ptr: *const u8,
    html_len: usize,
    base_url_ptr: *const u8,
    base_url_len: usize,
    out_len: *mut usize,
    out_cap: *mut usize,
) -> *mut u8 {
    let res = std::panic::catch_unwind(|| {
        if out_len.is_null() || out_cap.is_null() {
            return std::ptr::null_mut();
        }
        *out_len = 0;
        *out_cap = 0;
        let html = match slice_to_str(html_ptr, html_len) {
            Some(s) => s,
            None => return std::ptr::null_mut(),
        };
        let base_url = if base_url_ptr.is_null() || base_url_len == 0 {
            None
        } else {
            slice_to_str(base_url_ptr, base_url_len)
        };
        let mut pdf_bytes = crate::layout::render_html_to_pdf(html, base_url);
        let len = pdf_bytes.len();
        let cap = pdf_bytes.capacity();
        let ptr = pdf_bytes.as_mut_ptr();
        std::mem::forget(pdf_bytes);
        *out_len = len;
        *out_cap = cap;
        ptr
    });
    res.unwrap_or(std::ptr::null_mut())
}

/// Frees a buffer previously allocated by `weasy_rs_render_pdf_buf`.
#[no_mangle]
pub unsafe extern "C" fn weasy_rs_free_buf(ptr: *mut u8, len: usize, cap: usize) {
    if !ptr.is_null() && cap > 0 {
        let _ = Vec::from_raw_parts(ptr, len, cap);
    }
}

/// Renders HTML + CSS directly to a file path on disk, returning 0 on success or -1 on error.
#[no_mangle]
pub unsafe extern "C" fn weasy_rs_render_pdf_to_file(
    html_ptr: *const u8,
    html_len: usize,
    base_url_ptr: *const u8,
    base_url_len: usize,
    out_path_ptr: *const u8,
    out_path_len: usize,
) -> c_int {
    let res = std::panic::catch_unwind(|| {
        let html = match slice_to_str(html_ptr, html_len) {
            Some(s) => s,
            None => return -1,
        };
        let out_path = match slice_to_str(out_path_ptr, out_path_len) {
            Some(s) => s,
            None => return -1,
        };
        let base_url = if base_url_ptr.is_null() || base_url_len == 0 {
            None
        } else {
            slice_to_str(base_url_ptr, base_url_len)
        };
        let pdf_bytes = crate::layout::render_html_to_pdf(html, base_url);
        if let Some(parent) = std::path::Path::new(out_path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        match std::fs::write(out_path, &pdf_bytes) {
            Ok(_) => 0,
            Err(_) => -1,
        }
    });
    res.unwrap_or(-1)
}
