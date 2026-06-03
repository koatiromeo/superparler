/// record_test — quick audio smoke-test.
/// Records 3 seconds from the default microphone, resamples to 16 kHz mono,
/// and writes /tmp/superparler_test.wav (or %TEMP%\superparler_test.wav on Windows).
///
/// Usage:
///   cargo run --bin record_test --manifest-path src-tauri/Cargo.toml
///
/// Verify output:
///   ffprobe /tmp/superparler_test.wav          # should show: 16000 Hz, mono, pcm_f32le
///   mpv /tmp/superparler_test.wav              # listen back
use std::time::Duration;

fn main() {
    // Basic tracing to stderr so we don't confuse stdout
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let out_path = std::env::temp_dir().join("superparler_test.wav");

    println!("SuperParler record_test");
    println!("Recording for 3 seconds — speak now...");

    // Start capture
    let recorder = match superparler_lib::audio::capture::AudioRecorder::start() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("ERROR: could not open microphone: {e}");
            eprintln!("       On Windows: check Privacy → Microphone in Settings.");
            eprintln!("       On macOS: grant Microphone access in System Settings.");
            std::process::exit(1);
        }
    };

    // Wait 3 seconds
    std::thread::sleep(Duration::from_secs(3));

    // Stop and get 16 kHz mono samples
    let samples = match recorder.stop() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR: failed to stop recorder: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "Captured {} samples at 16 kHz mono ({:.1} s)",
        samples.len(),
        samples.len() as f32 / 16_000.0
    );

    if samples.is_empty() {
        eprintln!("ERROR: no audio captured — is the microphone muted?");
        std::process::exit(1);
    }

    // Write WAV
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = match hound::WavWriter::create(&out_path, spec) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("ERROR: could not create WAV at {}: {e}", out_path.display());
            std::process::exit(1);
        }
    };

    for sample in &samples {
        if let Err(e) = writer.write_sample(*sample) {
            eprintln!("ERROR: WAV write failed: {e}");
            std::process::exit(1);
        }
    }

    if let Err(e) = writer.finalize() {
        eprintln!("ERROR: WAV finalize failed: {e}");
        std::process::exit(1);
    }

    println!("WAV written → {}", out_path.display());
    println!();
    println!("Verify:");
    println!("  ffprobe \"{}\"", out_path.display());
    println!("  mpv \"{}\"", out_path.display());
}
