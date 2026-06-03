use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

use crate::error::{AppError, Result};

pub struct AudioRecorder {
    stream: Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

impl AudioRecorder {
    pub fn start() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| AppError::AudioDeviceNotFound("no default input device".to_string()))?;

        tracing::info!(device = %device.name().unwrap_or_default(), "starting audio capture");

        let config = device
            .default_input_config()
            .map_err(|e| AppError::AudioDeviceNotFound(e.to_string()))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let buffer_clone = buffer.clone();

        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                let cfg: StreamConfig = config.into();
                device.build_input_stream(
                    &cfg,
                    move |data: &[f32], _| {
                        // SAFETY: callback is sync, no await, lock is briefly held
                        if let Ok(mut buf) = buffer_clone.lock() {
                            buf.extend_from_slice(data);
                        }
                    },
                    |err| tracing::error!("cpal stream error: {err}"),
                    None,
                )
            }
            SampleFormat::I16 => {
                let cfg: StreamConfig = config.into();
                let buf_clone2 = buffer_clone.clone();
                device.build_input_stream(
                    &cfg,
                    move |data: &[i16], _| {
                        if let Ok(mut buf) = buf_clone2.lock() {
                            buf.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
                        }
                    },
                    |err| tracing::error!("cpal stream error: {err}"),
                    None,
                )
            }
            fmt => {
                return Err(AppError::AudioStream(format!("unsupported sample format: {fmt:?}")));
            }
        }
        .map_err(|e| AppError::AudioStream(e.to_string()))?;

        stream.play().map_err(|e| AppError::AudioStream(e.to_string()))?;

        Ok(Self { stream, buffer, sample_rate, channels })
    }

    /// Stop recording and return raw samples (interleaved, original sample rate)
    pub fn stop(self) -> Result<(Vec<f32>, u32, u16)> {
        drop(self.stream); // stops the stream
        let samples = self
            .buffer
            .lock()
            .map_err(|_| AppError::AudioStream("buffer lock poisoned".to_string()))?
            .clone();
        tracing::info!(samples = samples.len(), sample_rate = self.sample_rate, "audio capture stopped");
        Ok((samples, self.sample_rate, self.channels))
    }
}
