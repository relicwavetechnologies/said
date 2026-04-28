use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Cursor;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::config::{CHANNELS, MIN_DURATION_S, SAMPLE_RATE};

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
                println!("[rec] opened at {rate}Hz F32, will resample to {SAMPLE_RATE}Hz I16");
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
        println!("[rec] ⏹  {duration:.1}s recorded ({native_rate}Hz)");

        if duration < MIN_DURATION_S {
            println!("[rec] too short — ignored");
            return None;
        }

        // Resample from native_rate to SAMPLE_RATE and convert F32 → I16
        let samples_i16 = resample_f32_to_i16(&samples_f32, native_rate, SAMPLE_RATE);

        let mut buf = Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: CHANNELS,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut buf, spec).ok()?;
        for sample in &samples_i16 {
            writer.write_sample(*sample).ok()?;
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

fn resample_f32_to_i16(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<i16> {
    if from_rate == to_rate {
        return input.iter().map(|&s| f32_to_i16(s)).collect();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (input.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = src_pos - idx as f64;

        let sample = if idx + 1 < input.len() {
            input[idx] as f64 * (1.0 - frac) + input[idx + 1] as f64 * frac
        } else if idx < input.len() {
            input[idx] as f64
        } else {
            0.0
        };

        output.push(f32_to_i16(sample as f32));
    }

    output
}

fn f32_to_i16(s: f32) -> i16 {
    let clamped = s.clamp(-1.0, 1.0);
    (clamped * 32767.0) as i16
}
