//! Browser shell for fondue-core. No wasm-bindgen: a small C ABI that passes
//! JSON strings through linear memory.
//!
//!   fd_alloc(len) -> ptr        JS writes a UTF-8 command at ptr
//!   fd_call(ptr, len) -> out    out points at [u32 little-endian length][bytes]
//!   fd_free(ptr, len)           release either buffer (out uses len + 4)
//!
//! Every buffer is a boxed slice of exactly the length the caller is told, so
//! freeing with that length is always correct.
//!
//! Release builds use panic=abort (wasm32-unknown-unknown cannot unwind). A
//! panic traps the instance; the page recovers by re-instantiating and
//! replaying its saved log.

use fondue_core::api::Engine;
use std::cell::RefCell;

thread_local! {
    static ENGINE: RefCell<Engine> = RefCell::new(Engine::default());
}

fn leak(bytes: Box<[u8]>) -> *mut u8 {
    Box::into_raw(bytes) as *mut u8
}

fn framed(payload: &[u8]) -> *const u8 {
    let mut v = Vec::with_capacity(payload.len() + 4);
    v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    v.extend_from_slice(payload);
    leak(v.into_boxed_slice())
}

#[no_mangle]
pub extern "C" fn fd_alloc(len: usize) -> *mut u8 {
    leak(vec![0u8; len].into_boxed_slice())
}

/// # Safety
/// `ptr` and `len` must come from `fd_alloc`, or from a buffer returned by
/// `fd_call` / `fd_seed` with `len` = payload length + 4.
#[no_mangle]
pub unsafe extern "C" fn fd_free(ptr: *mut u8, len: usize) {
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
}

/// # Safety
/// `ptr` must point at `len` bytes of UTF-8 written into a buffer from `fd_alloc`.
#[no_mangle]
pub unsafe extern "C" fn fd_call(ptr: *const u8, len: usize) -> *const u8 {
    let input = std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).unwrap_or("{}");
    let out = ENGINE.with(|e| e.borrow_mut().call(input));
    framed(out.as_bytes())
}

/// The built-in seed log, so the page can boot without fetching anything.
#[no_mangle]
pub extern "C" fn fd_seed() -> *const u8 {
    framed(fondue_core::SEED.as_bytes())
}
