//! WebAssembly bindings for the Poly compiler.
//!
//! These exports let the web playground run the *real* Rust transpiler
//! instead of a JavaScript approximation.  The module is dependency-free:
//! strings cross the JS↔WASM boundary as `(pointer, length)` pairs over the
//! linear memory, and the playground reads the generated Rust back through
//! the exported buffer helpers.
//!
//! API (called from JS):
//! - `poly_alloc(len) -> ptr`            allocate a writable buffer
//! - `transpile(ptr, len) -> result_ptr` run the full pipeline
//! - `result_ptr(result) -> usize`        output buffer pointer
//! - `result_len(result) -> usize`        output byte length
//! - `result_is_error(result) -> i32`     1 when the result is an error
//! - `free_result(result)`                release a result block
//!
//! The result block is a small fixed layout in wasm memory:
//!   [0..PTR]  output pointer (usize)
//!   [PTR..2P] output length (usize)
//!   [2P..]    error flag (u32)
//! On error, the output buffer holds the human-readable error message.
//!
//! Note: the pointer and length fields are native `usize`, so the block is
//! 32-bit on wasm and 64-bit on native test builds.  The playground only
//! ever runs the wasm target, where JS reads the values as 32-bit integers.
//!
//! Note: exports are prefixed with `poly_` where they could collide with
//! libc symbols (`alloc`, `free`) so the crate also links cleanly as a
//! native test target.

use std::alloc::{alloc as raw_alloc, dealloc, Layout};
use std::slice;

/// Size of a native pointer (4 on wasm32, 8 on 64-bit hosts).
const PTR_SIZE: usize = std::mem::size_of::<usize>();
/// Result-block field offsets.
const RESULT_PTR_OFFSET: usize = 0;
const RESULT_LEN_OFFSET: usize = PTR_SIZE;
const RESULT_ERR_OFFSET: usize = PTR_SIZE * 2;
/// Total size of the result block in bytes (2 pointers + one u32).
const RESULT_BLOCK_SIZE: usize = PTR_SIZE * 2 + 4;

/// A contiguous writable region of the wasm heap.
struct Buffer {
    ptr: *mut u8,
    len: usize,
    cursor: usize,
}

impl Buffer {
    fn from_raw(ptr: *mut u8, len: usize) -> Self {
        Self {
            ptr,
            len,
            cursor: 0,
        }
    }

    /// Copy `bytes` into the buffer at the cursor, truncating if the buffer
    /// is too small, and advance the cursor.
    fn write(&mut self, bytes: &[u8]) {
        let count = bytes.len().min(self.len.saturating_sub(self.cursor));
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr.add(self.cursor), count);
        }
        self.cursor += count;
    }
}

/// Allocate `len` raw bytes and return the pointer (0 on failure).
fn raw_allocate(len: usize) -> *mut u8 {
    let layout = Layout::from_size_align(len.max(1), 1).expect("valid layout");
    unsafe { raw_alloc(layout) }
}

/// Copy a byte slice into a freshly allocated wasm buffer and return its
/// pointer.  The caller must free it with the crate-level `poly_free`.
fn into_wasm_memory(bytes: &[u8]) -> *mut u8 {
    let ptr = raw_allocate(bytes.len());
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
    }
    ptr
}

/// Allocate a writable buffer of `len` bytes for the JS side to fill with
/// source code.  Returns a pointer into wasm linear memory (0 on failure).
///
/// The buffer is uninitialized; the JS side must write `len` bytes before
/// calling `transpile`.
#[no_mangle]
pub extern "C" fn poly_alloc(len: usize) -> *mut u8 {
    raw_allocate(len)
}

/// Transpile Poly source held at `ptr`/`len`.  Returns a pointer to the
/// 12-byte result block described in the module docs.
///
/// # Safety
///
/// `ptr` must point to `len` readable bytes for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn transpile(ptr: *const u8, len: usize) -> *mut u8 {
    let source_bytes = if ptr.is_null() || len == 0 {
        &[][..]
    } else {
        unsafe { slice::from_raw_parts(ptr, len) }
    };
    let source = String::from_utf8_lossy(source_bytes);
    let (output, is_error) = match run_pipeline(&source) {
        Ok(rust_code) => (rust_code, false),
        Err(message) => (message, true),
    };

    let result = poly_alloc(RESULT_BLOCK_SIZE);
    if result.is_null() {
        return std::ptr::null_mut();
    }
    let output_ptr = into_wasm_memory(output.as_bytes());
    let mut result_block = Buffer::from_raw(result, RESULT_BLOCK_SIZE);
    // The buffer is a byte-region; write native-size pointer fields followed
    // by a little-endian u32 error flag.
    result_block.write(&(output_ptr as usize).to_ne_bytes());
    result_block.write(&output.len().to_ne_bytes());
    result_block.write(&(is_error as u32).to_le_bytes());
    result
}

/// Run the real Poly pipeline: lex → parse → type-check → generate Rust.
fn run_pipeline(source: &str) -> Result<String, String> {
    let transpiler = poly_transpiler::Transpiler::new();
    transpiler.transpile_checked(source)
}

/// Read the output pointer from a result block.
#[no_mangle]
pub extern "C" fn result_ptr(result: *const u8) -> usize {
    read_usize(result, RESULT_PTR_OFFSET)
}

/// Read the output length from a result block.
#[no_mangle]
pub extern "C" fn result_len(result: *const u8) -> usize {
    read_usize(result, RESULT_LEN_OFFSET)
}

/// Read the error flag from a result block (1 = error message, 0 = Rust code).
#[no_mangle]
pub extern "C" fn result_is_error(result: *const u8) -> i32 {
    read_u32(result, RESULT_ERR_OFFSET) as i32
}

/// Release the output buffer and the result block.  Must be called for every
/// successful `transpile` to avoid leaking the wasm heap.
#[no_mangle]
pub extern "C" fn free_result(result: *const u8) {
    if result.is_null() {
        return;
    }
    let output_ptr = result_ptr(result);
    let output_len = result_len(result);
    if output_ptr != 0 {
        unsafe { poly_free(output_ptr as *mut u8, output_len) };
    }
    unsafe { poly_free(result as *mut u8, RESULT_BLOCK_SIZE) };
}

/// Release a buffer allocated by `poly_alloc`.
///
/// # Safety
///
/// `ptr` must have been returned by `poly_alloc` with the matching `len`.
#[no_mangle]
pub unsafe extern "C" fn poly_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() {
        return;
    }
    let layout = Layout::from_size_align(len.max(1), 1).expect("valid layout");
    unsafe { dealloc(ptr, layout) };
}

/// Read a little-endian u32 at `offset` inside a raw block, safely.
fn read_u32(block: *const u8, offset: usize) -> u32 {
    if block.is_null() {
        return 0;
    }
    let bytes = unsafe { slice::from_raw_parts(block, RESULT_BLOCK_SIZE) };
    if offset + 4 > bytes.len() {
        return 0;
    }
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Read a native-size pointer or length at `offset` inside a raw block.
fn read_usize(block: *const u8, offset: usize) -> usize {
    if block.is_null() {
        return 0;
    }
    let bytes = unsafe { slice::from_raw_parts(block, RESULT_BLOCK_SIZE) };
    if offset + PTR_SIZE > bytes.len() {
        return 0;
    }
    let mut value = [0u8; PTR_SIZE];
    value.copy_from_slice(&bytes[offset..offset + PTR_SIZE]);
    usize::from_ne_bytes(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the full JS-visible cycle: write source into a `poly_alloc` buffer,
    /// transpile, read the output, then release both blocks.
    fn round_trip(source: &str) -> Result<(Vec<u8>, bool), String> {
        let src = source.as_bytes();
        let buf = poly_alloc(src.len());
        assert!(!buf.is_null(), "poly_alloc returned null");
        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), buf, src.len());
        }
        let result = unsafe { transpile(buf, src.len()) };
        unsafe { poly_free(buf, src.len()) };
        assert!(!result.is_null(), "transpile returned null");
        let out_ptr = result_ptr(result);
        let out_len = result_len(result);
        let is_error = result_is_error(result) != 0;
        let output = unsafe { slice::from_raw_parts(out_ptr as *const u8, out_len) }.to_vec();
        free_result(result);
        Ok((output, is_error))
    }

    #[test]
    fn round_trip_generates_rust() {
        let (output, is_error) = round_trip("put \"Hello, wasm!\"").unwrap();
        let text = String::from_utf8_lossy(&output);
        assert!(!is_error, "unexpected error: {text}");
        assert!(text.contains("println!"), "missing Rust output: {text}");
    }

    #[test]
    fn round_trip_reports_type_errors() {
        let (output, is_error) = round_trip("var x i32 := \"oops\"").unwrap();
        assert!(is_error, "type mismatch should be reported as an error");
        assert!(String::from_utf8_lossy(&output).contains("expected i32"));
    }

    #[test]
    fn round_trip_handles_unicode_source() {
        let (output, is_error) = round_trip("put unicode \"héllo wörld\"").unwrap();
        assert!(!is_error);
        let text = String::from_utf8_lossy(&output);
        assert!(text.contains("héllo wörld"), "unicode round trip: {text}");
    }
}
