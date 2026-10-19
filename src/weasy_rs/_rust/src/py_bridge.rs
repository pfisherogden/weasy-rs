//! Zero-dependency CPython C-API / ctypes bridge for `weasy-rs`.

use std::ffi::{c_char, c_int, c_void};
use std::slice;

#[repr(C)]
pub struct PyObject {
    _private: [u8; 0],
}

#[cfg(unix)]
extern "C" {
    fn PyBytes_FromStringAndSize(v: *const c_char, len: isize) -> *mut PyObject;
    fn Py_IncRef(o: *mut PyObject);
    static mut _Py_NoneStruct: PyObject;
}

#[cfg(windows)]
#[link(name = "python3")]
extern "C" {
    fn PyBytes_FromStringAndSize(v: *const c_char, len: isize) -> *mut PyObject;
    fn Py_IncRef(o: *mut PyObject);
    static mut _Py_NoneStruct: PyObject;
}

#[inline]
unsafe fn py_none() -> *mut PyObject {
    let ptr = std::ptr::addr_of_mut!(_Py_NoneStruct);
    Py_IncRef(ptr);
    ptr
}

unsafe fn slice_to_str<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    let bytes = slice::from_raw_parts(ptr, len);
    std::str::from_utf8(bytes).ok()
}

/// Renders HTML + CSS directly to a Python `bytes` object containing the PDF document.
#[no_mangle]
pub unsafe extern "C" fn weasy_rs_render_pdf_bytes(
    html_ptr: *const u8,
    html_len: usize,
    base_url_ptr: *const u8,
    base_url_len: usize,
) -> *mut c_void {
    let res = std::panic::catch_unwind(|| {
        let html = match slice_to_str(html_ptr, html_len) {
            Some(s) => s,
            None => return py_none(),
        };
        let base_url = if base_url_ptr.is_null() || base_url_len == 0 {
            None
        } else {
            slice_to_str(base_url_ptr, base_url_len)
        };
        let pdf_bytes = crate::layout::render_html_to_pdf(html, base_url);
        PyBytes_FromStringAndSize(
            pdf_bytes.as_ptr() as *const c_char,
            pdf_bytes.len() as isize,
        )
    });
    match res {
        Ok(ptr) => ptr as *mut c_void,
        Err(_) => py_none() as *mut c_void,
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
