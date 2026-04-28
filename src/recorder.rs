use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Cursor;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::config::{CHANNELS, MIN_DURATION_S};

enum RecCmd {
    Stop(mpsc::Sender<(Vec<f32>, u32)>),
}

pub struct AudioRecorder {
    cmd_tx: Option<mpsc::Sender<RecCmd>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self { cmd_tx: None }
    }

    pub fn start(&mut self) -> Result<(), String> {
        let host = cpal::default_host();
        let _device = host
            .default_input_device()
            .ok_or("no input device found")?;

        let frames: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let frames_for_reply = Arc::clone(&frames);

        let (cmd_tx, cmd_rx) = mpsc::channel::<RecCmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();

        std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    let _ = ready_tx.send(Err("no input device found".into()));
                    return;
                }
            };

            let default_config = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("no default input config: {e}")));
                    return;
                }
            };

            let native_rate = default_config.sample_rate().0;
            let config = cpal::StreamConfig {
                channels: CHANNELS,
                sample_rate: cpal::SampleRate(native_rate),
                buffer_size: cpal::BufferSize::Default,
            };

            let frames_cb = Arc::clone(&frames_for_reply);
            let stream = match device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    frames_cb.lock().unwrap().extend_from_slice(data);
                },
                |err| eprintln!("[rec] ⚠ stream error: {err}"),
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("failed to open audio stream: {e}")));
                    return;
                }
            };

            if let Err(e) = stream.play() {
                let _ = ready_tx.send(Err(format!("failed to start stream: {e}")));
                return;
            }

            let _ = ready_tx.send(Ok(native_rate));

            if let Ok(RecCmd::Stop(reply)) = cmd_rx.recv() {
                let data = frames_for_reply.lock().unwrap().clone();
                let _ = reply.send((data, native_rate));
            }
        });

        match ready_rx.recv() {
            Ok(Ok(rate)) => {
                println!("[rec] opened at {rate}Hz F32");
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("recording thread died".into()),
        }

        self.cmd_tx = Some(cmd_tx);
        println!("[rec] 🎤  recording … press hotkey again to stop");
        Ok(())
    }

    pub fn stop(&mut self) -> Option<Vec<u8>> {
        let cmd_tx = self.cmd_tx.take()?;
        let (reply_tx, reply_rx) = mpsc::channel();
        let _ = cmd_tx.send(RecCmd::Stop(reply_tx));
        let (samples_f32, native_rate) = reply_rx.recv().ok()?;

        if samples_f32.is_empty() {
            println!("[rec] no audio captured");
            return None;
        }

        let duration = samples_f32.len() as f32 / native_rate as f32;
        let max_amp = samples_f32.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        println!("[rec] ⏹  {duration:.1}s recorded ({native_rate}Hz, peak={max_amp:.4})");

        if max_amp < 0.0001 {
            eprintln!("[rec] ⚠ audio is silence — microphone permission not granted");
            eprintln!("[rec]   System Settings → Privacy & Security → Microphone → enable for Voice Polish");
            return None;
        }

        if duration < MIN_DURATION_S {
            println!("[rec] too short — ignored");
            return None;
        }

        // Send at native sample rate — Deepgram handles any rate
        // Convert F32 → I16 without resampling for maximum quality
        let mut buf = Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: CHANNELS,
            sample_rate: native_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut buf, spec).ok()?;
        for &sample in &samples_f32 {
            let clamped = sample.clamp(-1.0, 1.0);
            writer.write_sample((clamped * 32767.0) as i16).ok()?;
        }
        writer.finalize().ok()?;

        Some(buf.into_inner())
    }

    pub fn preflight() -> Result<String, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no input device found — check microphone connection")?;
        let name = device.name().unwrap_or_else(|_| "unknown".into());
        Ok(name)
    }
}
