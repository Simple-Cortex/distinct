//! `distinct-rs` binary entry point.
//!
//! D-K: installs a counting global allocator so the render-worker
//! can be capped at 512 MiB without OS-level support on every platform.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingAllocator;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIMIT: AtomicUsize = AtomicUsize::new(usize::MAX);

/// Set the allocation limit. Call before any heavy work.
pub fn set_render_limit(bytes: usize) {
    LIMIT.store(bytes, Ordering::Relaxed);
}

/// True when growing live usage by `add` bytes would pass `limit`.
/// `usize::MAX` means no cap (every normal command).
fn exceeds(limit: usize, live: usize, add: usize) -> bool {
    limit != usize::MAX && live.saturating_add(add) > limit
}

/// Atomically reserve `add` bytes of live usage, failing when the cap would
/// be exceeded. The check and the increment are one compare-exchange loop, so
/// concurrent threads cannot jointly overshoot the limit.
fn reserve(add: usize) -> bool {
    let limit = LIMIT.load(Ordering::Relaxed);
    LIVE_BYTES
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |live| {
            if exceeds(limit, live, add) {
                None
            } else {
                Some(live.saturating_add(add))
            }
        })
        .is_ok()
}

// SAFETY: every method forwards to `System`, which upholds the `GlobalAlloc`
// contract. The counter only adds bookkeeping: bytes are reserved before the
// `System` call and rolled back if it returns null, and released on dealloc.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !reserve(layout.size()) {
            return std::ptr::null_mut();
        }
        // SAFETY: caller upholds `GlobalAlloc::alloc` preconditions for `layout`.
        let ptr = unsafe { System.alloc(layout) };
        if ptr.is_null() {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: `ptr`/`layout` come from a matching `alloc`/`realloc` call.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old = layout.size();
        let growth = new_size.saturating_sub(old);
        if !reserve(growth) {
            return std::ptr::null_mut();
        }
        // SAFETY: caller upholds `GlobalAlloc::realloc` preconditions.
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if new_ptr.is_null() {
            // Allocation failed; the old block is still live. Undo the growth.
            LIVE_BYTES.fetch_sub(growth, Ordering::Relaxed);
        } else if new_size < old {
            LIVE_BYTES.fetch_sub(old - new_size, Ordering::Relaxed);
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOC: CountingAllocator = CountingAllocator;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // D-K: render-worker sets the memory cap before doing anything else.
    if args.first().map(|s| s.as_str()) == Some("render-worker") {
        let limit = render_limit_from_env_or_default();
        set_render_limit(limit);
    }

    let code = distinct_cli::run(
        &args,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(3))
}

fn render_limit_from_env_or_default() -> usize {
    const DEFAULT_LIMIT: usize = 512 * 1024 * 1024; // 512 MiB

    #[cfg(debug_assertions)]
    {
        if let Ok(val) = std::env::var("DISTINCT_TEST_RENDER_LIMIT_BYTES") {
            if let Ok(n) = val.parse::<usize>() {
                return n;
            }
        }
    }

    DEFAULT_LIMIT
}

#[cfg(test)]
mod tests {
    use super::*;

    // The cap logic is tested as a pure function: flipping the global LIMIT
    // in a test would make allocations on other test threads fail.
    #[test]
    fn exceeds_respects_limit() {
        assert!(exceeds(1024, 0, 2048));
        assert!(exceeds(1024, 1000, 25));
        assert!(!exceeds(1024, 0, 512));
        assert!(!exceeds(1024, 512, 512));
        assert!(!exceeds(usize::MAX, usize::MAX - 1, 4096));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn env_override_parsed() {
        std::env::set_var("DISTINCT_TEST_RENDER_LIMIT_BYTES", "4096");
        let limit = render_limit_from_env_or_default();
        assert_eq!(limit, 4096);
        std::env::remove_var("DISTINCT_TEST_RENDER_LIMIT_BYTES");
    }
}
