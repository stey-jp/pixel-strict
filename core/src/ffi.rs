//! C ABI ownership: input allocations belong to the caller; result views belong to
//! PsResult and remain valid until ps_result_free. Never unwind across the ABI.
use crate::{MAX_INPUT_BYTES, Options, convert};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice,
};

pub struct PsResult {
    json: Vec<u8>,
    png: Vec<u8>,
}
fn error(message: &str) -> PsResult {
    PsResult {
        json: serde_json::to_vec(&serde_json::json!({"error":message})).unwrap_or_default(),
        png: Vec::new(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ps_alloc(len: usize) -> *mut u8 {
    if len == 0 || len > MAX_INPUT_BYTES {
        return ptr::null_mut();
    }
    Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
}

/// # Safety
/// `data` must come from ps_alloc(len), and be released exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps_dealloc(data: *mut u8, len: usize) {
    if !data.is_null() {
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(data, len)));
        }
    }
}

/// # Safety
/// Input pointers must reference readable buffers of their respective lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps_convert(
    data: *const u8,
    len: usize,
    settings: *const u8,
    settings_len: usize,
) -> *mut PsResult {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if data.is_null()
            || settings.is_null()
            || len == 0
            || len > MAX_INPUT_BYTES
            || settings_len == 0
            || settings_len > 4096
        {
            return error("Invalid input buffers");
        }
        let bytes = unsafe { slice::from_raw_parts(data, len) };
        let json = unsafe { slice::from_raw_parts(settings, settings_len) };
        let options: Options = match serde_json::from_slice(json) {
            Ok(v) => v,
            Err(e) => return error(&format!("Invalid settings: {e}")),
        };
        match convert(bytes, &options) {
            Ok(v) => PsResult {
                json: serde_json::to_vec(&v.report).unwrap_or_default(),
                png: v.png,
            },
            Err(e) => error(&e),
        }
    }))
    .unwrap_or_else(|_| error("Internal conversion error"));
    Box::into_raw(Box::new(result))
}

/// # Safety
/// A non-null result must be live and returned by ps_convert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps_result_json(result: *const PsResult) -> *const u8 {
    unsafe { result.as_ref() }.map_or(ptr::null(), |r| r.json.as_ptr())
}
/// # Safety
/// A non-null result must be live and returned by ps_convert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps_result_json_len(result: *const PsResult) -> usize {
    unsafe { result.as_ref() }.map_or(0, |r| r.json.len())
}
/// # Safety
/// A non-null result must be live and returned by ps_convert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps_result_png(result: *const PsResult) -> *const u8 {
    unsafe { result.as_ref() }.map_or(ptr::null(), |r| r.png.as_ptr())
}
/// # Safety
/// A non-null result must be live and returned by ps_convert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps_result_png_len(result: *const PsResult) -> usize {
    unsafe { result.as_ref() }.map_or(0, |r| r.png.len())
}
/// # Safety
/// A non-null result must be live, returned by ps_convert, and freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ps_result_free(result: *mut PsResult) {
    if !result.is_null() {
        unsafe {
            drop(Box::from_raw(result));
        }
    }
}
