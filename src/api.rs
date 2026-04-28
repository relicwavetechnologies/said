use reqwest::blocking::multipart;
use serde::Deserialize;

use crate::config;

#[derive(Deserialize)]
pub struct PolishResponse {
    pub transcript: String,
    pub polished: String,
    pub model: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub latency: Latency,
}

#[derive(Deserialize, Default)]
pub struct Latency {
    #[serde(default)]
    pub transcribe_ms: u64,
    #[serde(default)]
    pub polish_ms: u64,
}

pub fn process(wav_data: Vec<u8>) -> Result<String, String> {
    let mode = config::current_mode();
    println!("[voice] mode={}  sending to gateway…", mode.key);

    let form = multipart::Form::new()
        .text("mode", mode.key.to_string())
        .text("lang", "auto")
        .part(
            "audio",
            multipart::Part::bytes(wav_data)
                .file_name("recording.wav")
                .mime_str("audio/wav")
                .unwrap(),
        );

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(config::VOICE_URL)
        .header("X-API-Key", config::api_key())
        .multipart(form)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .map_err(|e| format!("gateway request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(format!("Gateway error {status}: {}", &body[..body.len().min(300)]));
    }

    let data: PolishResponse = resp
        .json()
        .map_err(|e| format!("failed to parse gateway response: {e}"))?;

    if data.polished.is_empty() {
        return Err("gateway returned empty polished text".into());
    }

    println!(
        "[voice] transcript ({:.2}): {}",
        data.confidence, data.transcript
    );
    println!("[voice] polished   [{}]: {}", data.model, data.polished);
    println!(
        "[voice] latency: stt={}ms  llm={}ms",
        data.latency.transcribe_ms, data.latency.polish_ms
    );

    Ok(data.polished)
}
