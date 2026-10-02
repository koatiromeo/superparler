//! overlay_test — visual smoke test for the native Win32 recording overlay.
//!
//! Shows the floating pill (recording → transcribing → hide) without needing a
//! full dictation. Run: `cargo run --bin overlay_test` from `src-tauri/`.
fn main() {
    #[cfg(windows)]
    superparler_lib::overlay::demo();
    #[cfg(not(windows))]
    eprintln!("overlay is Windows-only");
}
