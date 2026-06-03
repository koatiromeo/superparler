/// inject_test — standalone injection smoke-test.
///
/// Places a countdown in the terminal, then injects "Bonjour depuis SuperParler ✓"
/// into whatever application currently has keyboard focus.
///
/// Usage:
///   cargo run --bin inject_test --manifest-path src-tauri/Cargo.toml
///
/// Then: click into any text editor or browser URL bar and watch the text appear.
///
/// How it works (mirrors the production inject_text() flow):
///   1. Save current clipboard via arboard
///   2. Write the test text via arboard
///   3. Simulate Ctrl+V (or Cmd+V on macOS) via the platform inject module
///   4. Restore original clipboard via arboard
use std::time::Duration;

const TEST_TEXT: &str = "Bonjour depuis SuperParler ✓";
const COUNTDOWN_SECS: u64 = 3;

fn main() {
    println!("SuperParler inject_test");
    println!("=======================");
    println!("Text to inject: «{TEST_TEXT}»");
    println!();
    println!("Place your cursor in a text editor (Notepad, VS Code, browser…)");
    println!();

    // Countdown
    for i in (1..=COUNTDOWN_SECS).rev() {
        println!("Injecting in {i}…");
        std::thread::sleep(Duration::from_secs(1));
    }
    println!("Injecting now!");

    // ── Save clipboard via arboard ───────────────────────────────────────────
    let mut arboard = match arboard::Clipboard::new() {
        Ok(cb) => cb,
        Err(e) => {
            eprintln!("ERROR: cannot open clipboard: {e}");
            std::process::exit(1);
        }
    };

    let previous_text = arboard.get_text().ok();
    if previous_text.is_some() {
        println!("Saved previous clipboard content.");
    }

    // ── Write test text to clipboard ─────────────────────────────────────────
    if let Err(e) = arboard.set_text(TEST_TEXT) {
        eprintln!("ERROR: clipboard write failed: {e}");
        std::process::exit(1);
    }

    // Brief delay so the OS propagates the clipboard update
    std::thread::sleep(Duration::from_millis(80));

    // ── Simulate paste shortcut ───────────────────────────────────────────────
    let result = do_paste();
    if let Err(ref e) = result {
        eprintln!("ERROR: paste simulation failed: {e}");
    } else {
        println!("Paste simulated successfully.");
    }

    // Wait for the target app to consume the paste event
    std::thread::sleep(Duration::from_millis(250));

    // ── Restore previous clipboard ────────────────────────────────────────────
    match previous_text {
        Some(prev) => {
            if let Err(e) = arboard.set_text(&prev) {
                eprintln!("WARNING: clipboard restore failed: {e}");
            } else {
                println!("Previous clipboard content restored.");
            }
        }
        None => {
            println!("Clipboard was empty before; left with injected text.");
        }
    }

    if result.is_ok() {
        println!();
        println!("✓ Done. Check your text editor — «{TEST_TEXT}» should have appeared.");
    } else {
        std::process::exit(1);
    }
}

#[cfg(target_os = "windows")]
fn do_paste() -> superparler_lib::error::Result<()> {
    superparler_lib::inject::windows::paste()
}

#[cfg(target_os = "macos")]
fn do_paste() -> superparler_lib::error::Result<()> {
    superparler_lib::inject::macos::paste()
}

#[cfg(target_os = "linux")]
fn do_paste() -> superparler_lib::error::Result<()> {
    superparler_lib::inject::linux::paste()
}
