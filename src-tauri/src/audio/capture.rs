/// audio/capture.rs — cpal input capture confined to a dedicated OS thread.
///
/// cpal::Stream is !Send (cpal::platform::NotSendSyncAcrossAllPlatforms on Windows).
/// Strategy: create AND use Device + Stream entirely inside a std::thread so nothing
/// crosses thread boundaries. AudioRecorder itself only holds mpsc channels → is Send.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

use super::resample;
use crate::error::{AppError, Result};

/// Handle returned to the caller after the recording thread is ready.
/// Send: only holds mpsc channel endpoints, not the cpal Stream.
pub struct AudioRecorder {
    stop_tx: std::sync::mpsc::SyncSender<()>,
    samples_rx: std::sync::mpsc::Receiver<Result<Vec<f32>>>,
}

impl AudioRecorder {
    /// Spawn a dedicated OS thread that opens and runs the cpal stream.
    /// Blocks until the thread confirms the stream is playing (or returns an error).
    pub fn start() -> Result<Self> {
        let (stop_tx, stop_rx) = std::sync::mpsc::sync_channel::<()>(1);
        let (samples_tx, samples_rx) = std::sync::mpsc::channel::<Result<Vec<f32>>>();
        // Oneshot: thread signals "stream ready" (Ok) or "init failed" (Err).
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();

        std::thread::Builder::new()
            .name("superparler-audio".to_string())
            .spawn(move || {
                // Everything created in THIS thread to respect WASAPI COM apartments.
                let host = cpal::default_host();
                let device = match host.default_input_device() {
                    Some(d) => d,
                    None => {
                        ready_tx
                            .send(Err(AppError::AudioDeviceNotFound(
                                "no default input device".to_string(),
                            )))
                            .ok();
                        return;
                    }
                };

                let name = device.name().unwrap_or_else(|_| "<unknown>".to_string());
                let config = match device.default_input_config() {
                    Ok(c) => c,
                    Err(e) => {
                        ready_tx
                            .send(Err(AppError::AudioDeviceNotFound(format!(
                                "default config: {e}"
                            ))))
                            .ok();
                        return;
                    }
                };

                let sample_rate = config.sample_rate().0;
                let channels = config.channels();
                tracing::info!(device = %name, sample_rate, channels, "audio thread: starting capture");

                let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(
                    sample_rate as usize * channels as usize * 10,
                )));

                let stream = match build_stream(&device, config, buffer.clone()) {
                    Ok(s) => s,
                    Err(e) => {
                        ready_tx.send(Err(e)).ok();
                        return;
                    }
                };

                if let Err(e) = stream.play() {
                    ready_tx
                        .send(Err(AppError::AudioStream(format!("play: {e}"))))
                        .ok();
                    return;
                }

                // Signal ready — caller can return AudioRecorder now.
                ready_tx.send(Ok(())).ok();

                // Block until stop signal.
                let _ = stop_rx.recv();
                drop(stream);

                let raw = match buffer.lock() {
                    Ok(b) => b.clone(),
                    Err(_) => {
                        samples_tx
                            .send(Err(AppError::AudioStream("buffer mutex poisoned".to_string())))
                            .ok();
                        return;
                    }
                };

                tracing::info!(
                    raw_samples = raw.len(),
                    sample_rate,
                    channels,
                    "audio thread: capture stopped"
                );

                let result = if raw.is_empty() {
                    Ok(Vec::new())
                } else {
                    resample::to_16khz_mono(&raw, sample_rate, channels)
                };

                samples_tx.send(result).ok();
            })
            .map_err(|e| AppError::AudioStream(format!("spawn audio thread: {e}")))?;

        // Wait for the thread to confirm the stream is running.
        ready_rx
            .recv()
            .map_err(|_| AppError::AudioStream("audio thread died before ready".to_string()))??;

        Ok(Self { stop_tx, samples_rx })
    }

    /// Stop the stream and return 16 kHz mono f32 samples (blocking, waits for thread).
    pub fn stop(self) -> Result<Vec<f32>> {
        self.stop_tx
            .send(())
            .map_err(|_| AppError::AudioStream("audio thread already gone".to_string()))?;
        self.samples_rx
            .recv()
            .map_err(|_| AppError::AudioStream("samples channel closed before result".to_string()))?
    }
}

fn build_stream(
    device: &cpal::Device,
    config: cpal::SupportedStreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<Stream> {
    let fmt = config.sample_format();
    let cfg: StreamConfig = config.into();
    let err_fn = |e| tracing::error!("cpal stream error: {e}");

    match fmt {
        SampleFormat::F32 => device
            .build_input_stream(
                &cfg,
                move |data: &[f32], _| push_f32(&buffer, data),
                err_fn,
                None,
            )
            .map_err(|e| AppError::AudioStream(format!("build F32 stream: {e}"))),

        SampleFormat::I16 => device
            .build_input_stream(
                &cfg,
                move |data: &[i16], _| {
                    let f: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    push_f32(&buffer, &f);
                },
                err_fn,
                None,
            )
            .map_err(|e| AppError::AudioStream(format!("build I16 stream: {e}"))),

        SampleFormat::U16 => device
            .build_input_stream(
                &cfg,
                move |data: &[u16], _| {
                    let f: Vec<f32> =
                        data.iter().map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0).collect();
                    push_f32(&buffer, &f);
                },
                err_fn,
                None,
            )
            .map_err(|e| AppError::AudioStream(format!("build U16 stream: {e}"))),

        other => Err(AppError::AudioStream(format!(
            "unsupported sample format {other:?} — device must use F32/I16/U16"
        ))),
    }
}

fn push_f32(buffer: &Mutex<Vec<f32>>, data: &[f32]) {
    if let Ok(mut buf) = buffer.lock() {
        buf.extend_from_slice(data);
    } else {
        tracing::warn!("audio buffer mutex poisoned — dropping frame");
    }
}

#[cfg(test)]
mod tests {
    /// No mic in CI → typed error is acceptable. Important: no panic.
    #[test]
    fn start_stop_no_panic() {
        match super::AudioRecorder::start() {
            Ok(rec) => {
                let samples = rec.stop().expect("stop should succeed after start");
                eprintln!("captured {} 16kHz mono samples", samples.len());
            }
            Err(e) => eprintln!("no audio device (expected in headless CI): {e}"),
        }
    }
}
