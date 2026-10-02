//! Embedded developer session. No listener, shell, subprocess, or implicit pairing.
pub mod local_packets;
pub mod model;
mod session;
use session::{Request, SessionHandle};
use std::{
    collections::HashMap,
    ffi::{CString, c_char},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static HANDLES: OnceLock<Mutex<HashMap<u64, Arc<SessionHandle>>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("mobile runtime creation failed")
    })
}
fn handles() -> &'static Mutex<HashMap<u64, Arc<SessionHandle>>> {
    HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn output(value: serde_json::Value) -> *mut c_char {
    CString::new(value.to_string())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `data` must reference `length` readable bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn meridian_session_create(data: *const u8, length: usize) -> u64 {
    std::panic::catch_unwind(|| {
        if data.is_null() || length == 0 || length > 131072 {
            return 0;
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, length) };
        let Ok(config) = serde_json::from_slice::<model::Configuration>(bytes) else {
            return 0;
        };
        if config.endpoint().is_err() || config.validate_pairing().is_err() {
            return 0;
        }
        let handle = SessionHandle::new(config, runtime());
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut map) = handles().lock() {
            map.insert(id, handle);
            id
        } else {
            0
        }
    })
    .unwrap_or(0)
}

/// Synchronous bridge call; invoke from a Swift actor away from MainActor.
/// # Safety
/// `data` must reference `length` readable bytes. Free the result exactly once
/// using `meridian_string_free`; no Rust allocation may be freed by Swift.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn meridian_session_call(
    id: u64,
    data: *const u8,
    length: usize,
) -> *mut c_char {
    let result=std::panic::catch_unwind(|| {
        // Permit bounded road-route geometry as well as short control messages.
        if data.is_null() || length == 0 || length > 1_048_576 { return serde_json::json!({"ok":false,"error":{"code":"INVALID_REQUEST","message":"Invalid mobile request."}}); }
        let bytes=unsafe{std::slice::from_raw_parts(data,length)};
        let request=serde_json::from_slice::<Request>(bytes);
        let handle=handles().lock().ok().and_then(|m|m.get(&id).cloned());
        match (request,handle) {
            (Ok(request),Some(handle)) => match runtime().block_on(handle.call(request)) {
                Ok(status)=>serde_json::json!({"ok":true,"result":status}),
                Err(error)=>serde_json::json!({"ok":false,"error":error})
            },
            _=>serde_json::json!({"ok":false,"error":{"code":"INVALID_REQUEST","message":"The session handle or request is invalid."}})
        }
    }).unwrap_or_else(|_|serde_json::json!({"ok":false,"error":{"code":"INTERNAL_ERROR","message":"The native session failed. Reconnect and restore location."}}));
    output(result)
}

#[unsafe(no_mangle)]
pub extern "C" fn meridian_session_destroy(id: u64) {
    if let Ok(mut map) = handles().lock() {
        map.remove(&id);
    }
}
/// # Safety
/// Pass only a non-freed pointer returned by meridian_session_call, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn meridian_string_free(data: *mut c_char) {
    if !data.is_null() {
        drop(unsafe { CString::from_raw(data) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ffi_rejects_null_and_invalid_handles() {
        unsafe {
            assert_eq!(meridian_session_create(std::ptr::null(), 0), 0);
            let input = b"{\"method\":\"status\"}";
            let ptr = meridian_session_call(u64::MAX, input.as_ptr(), input.len());
            let value: serde_json::Value =
                serde_json::from_str(std::ffi::CStr::from_ptr(ptr).to_str().unwrap()).unwrap();
            assert_eq!(value["ok"], false);
            meridian_string_free(ptr);
        }
    }
}
