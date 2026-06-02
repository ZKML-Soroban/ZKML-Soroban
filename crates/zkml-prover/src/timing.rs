//! Lightweight timing instrumentation.
//!
//! Enabled with the `timing` feature; otherwise `timed` simply runs the
//! closure with zero overhead.

/// Run `f`, printing its duration in milliseconds when the `timing` feature is
/// enabled.
pub fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    #[cfg(feature = "timing")]
    {
        use std::time::Instant;
        let start = Instant::now();
        let out = f();
        eprintln!("[timing] {label}: {} ms", start.elapsed().as_millis());
        out
    }
    #[cfg(not(feature = "timing"))]
    {
        let _ = label;
        f()
    }
}
