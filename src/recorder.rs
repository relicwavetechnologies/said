use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Cursor;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::config::{CHANNELS, MIN_DURATION_S, SAMPLE_RATE};

enum RecCmd {
    Stop(mpsc::Sender<Vec<i16>>),
}

pub struct AudioRecorder {
    cmd_tx: Option<mpsc::Sender<RecCmd>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self { cmd_tx: None }
    }

    pub fn start(&mut self) -> Result<(), String> {
        // Verify device exists before spawning thread
        let host = cpal::default_host();
        let _device = host
            .default_input_device()
            .ok_or("no input device found")?;

        let frames: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
        let frames_for_reply = Arc::clone(&frames);

        let (cmd_tx, cmd_rx) = mpsc::channel::<RecCmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        // Build and own the stream on a dedicated thread (cpal::Stream is !Send)
        std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    let _ = ready_tx.send(Err("no input device found".into()));
                    return;
                }
            };

            let config = cpal::StreamConfig {
                channels: CHANNELS,
                sample_rate: cpal::SampleRate(SAMPLE_RATE),
                buffer_size: cpal::BufferSize::Default,
            };

            let frames_cb = Arc::clone(&frames_for_reply);
            let stream = match device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
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

            let _ = ready_tx.send(Ok(()));

            // Wait for stop command, keeping stream alive
            if let Ok(RecCmd::Stop(reply)) = cmd_rx.recv() {
                let data = frames_for_reply.lock().unwrap().clone();
                let _ = reply.send(data);
            }
            // stream drops here
        });

        // Wait for stream to start
        match ready_rx.recv() {
            Ok(Ok(())) => {}
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
        let samples = reply_rx.recv().ok()?;

        if samples.is_empty() {
            println!("[rec] no audio captured");
            return None;
        }

        let duration = samples.len() as f32 / SAMPLE_RATE as f32;
        println!("[rec] ⏹  {duration:.1}s recorded");

        if duration < MIN_DURATION_S {
            println!("[rec] too short — ignored");
            return None;
        }

        let mut buf = Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: CHANNELS,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut buf, spec).ok()?;
        for &sample in samples.iter() {
            writer.write_sample(sample).ok()?;
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
