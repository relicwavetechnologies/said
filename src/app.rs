use crate::api;
use crate::menubar;
use crate::paster;
use crate::recorder::AudioRecorder;

#[derive(PartialEq)]
enum State {
    Idle,
    Recording,
    Processing,
}

pub struct AppCore {
    state: State,
    recorder: AudioRecorder,
}

impl AppCore {
    pub fn new() -> Self {
        Self {
            state: State::Idle,
            recorder: AudioRecorder::new(),
        }
    }

    pub fn toggle(&mut self) {
        match self.state {
            State::Idle => self.start(),
            State::Recording => self.stop(),
            State::Processing => {
                println!("[app] busy — waiting for gateway response");
            }
        }
    }

    pub fn reset_to_idle(&mut self) {
        self.state = State::Idle;
    }

    fn start(&mut self) {
        match self.recorder.start() {
            Ok(()) => {
                self.state = State::Recording;
                dispatch::Queue::main().exec_async(|| menubar::set_title("🔴"));
            }
            Err(e) => {
                eprintln!("[app] ✗ failed to start recording: {e}");
                self.state = State::Idle;
                dispatch::Queue::main().exec_async(|| menubar::set_title("❌"));
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    dispatch::Queue::main().exec_async(|| menubar::set_title("●"));
                });
            }
        }
    }

    fn stop(&mut self) {
        self.state = State::Processing;
        dispatch::Queue::main().exec_async(|| menubar::set_title("⏳"));

        let wav_data = self.recorder.stop();

        std::thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let data = wav_data.ok_or("no audio data")?;
                println!("[app] ── sending to gateway ─────────────────────");
                let polished = api::process(data)?;
                paster::paste(&polished).map_err(|e| format!("[paste] ✗ {e}"))?;
                println!("[app] ✓ pasted");
                println!("[app] ─────────────────────────────────────────");
                Ok(())
            })();

            match result {
                Ok(()) => {
                    dispatch::Queue::main().exec_async(|| menubar::set_title("✅"));
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                }
                Err(e) => {
                    eprintln!("[app] ✗ {e}");
                    dispatch::Queue::main().exec_async(|| menubar::set_title("❌"));
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }

            dispatch::Queue::main().exec_async(|| {
                menubar::set_title("●");
                menubar::reset_core_state();
            });
        });
    }
}
