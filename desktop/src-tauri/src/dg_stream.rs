//! Deepgram Live Streaming client (P5 speed optimisation).
//!
//! Opens a WebSocket to `wss://api.deepgram.com/v1/listen` at the START of a
//! recording session and forwards raw audio chunks as they arrive from the
//! microphone.  By the time the user releases Caps Lock, Deepgram has already
//! processed most of the audio in real-time, so its final transcript arrives in
//! ~100–200 ms instead of the ~1 200–2 000 ms of the batch HTTP API.

use std::sync::mpsc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
};
use tracing::{debug, info, warn};
use voice_polish_recorder::{resample_to_16k, ChunkReceiver, SAMPLE_RATE};

/// Resample raw audio chunks from the microphone, send them to Deepgram via WS,
/// and return the final transcript once the stream is closed.
///
/// - `chunk_recv.rx` is closed (channel disconnected) by `AudioRecorder::stop()`.
/// - `language`: pass an empty string for auto-detect / Hindi default.
///
/// Returns `None` if WS connection fails or Deepgram returns no transcript.
pub async fn stream_to_deepgram(
    chunk_recv:   ChunkReceiver,
    deepgram_key: &str,
    language:     &str,
) -> Option<String> {
    if deepgram_key.is_empty() {
        warn!("[dg_stream] no Deepgram API key — WS streaming disabled");
        return None;
    }

    let lang = if language.is_empty() || language == "auto" { "hi" } else { language };

    // Deepgram WS URL with encoding parameters for raw i16 PCM at 16 kHz.
    // utterance_end_ms is omitted — CloseStream alone triggers finalization.
    let url_str = format!(
        "wss://api.deepgram.com/v1/listen\
         ?model=nova-3\
         &language={lang}\
         &smart_format=true\
         &punctuate=true\
         &encoding=linear16\
         &sample_rate={SAMPLE_RATE}\
         &channels=1\
         &interim_results=true\
         &endpointing=300"
    );

    debug!("[dg_stream] connecting (lang={lang}, 16kHz), key_len={}", deepgram_key.len());

    let mut req = match url_str.into_client_request() {
        Ok(r)  => r,
        Err(e) => { warn!("[dg_stream] bad WS URL: {e}"); return None; }
    };
    let auth_value = match format!("Token {deepgram_key}").parse() {
        Ok(v)  => v,
        Err(e) => { warn!("[dg_stream] invalid auth header value: {e}"); return None; }
    };
    req.headers_mut().insert("Authorization", auth_value);

    // Hard 5-second timeout on the WebSocket upgrade.
    let connect_result = tokio::time::timeout(
        Duration::from_secs(5),
        connect_async(req),
    ).await;

    let (ws, _resp) = match connect_result {
        Err(_elapsed) => {
            warn!("[dg_stream] WS connect timed out after 5 s");
            return None;
        }
        Ok(Err(e)) => {
            warn!("[dg_stream] WS connect failed: {e}");
            return None;
        }
        Ok(Ok(pair)) => pair,
    };

    debug!("[dg_stream] connected to Deepgram WS");

    let (mut ws_tx, mut ws_rx) = ws.split();

    // ── Bridge: std::sync::mpsc (cpal audio thread) → tokio channel ──────────
    let (async_tx, mut async_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(256);
    let native_rate              = chunk_recv.native_rate;
    let sync_rx: mpsc::Receiver<Vec<f32>> = chunk_recv.rx;

    std::thread::spawn(move || {
        while let Ok(chunk_f32) = sync_rx.recv() {
            let resampled     = resample_to_16k(&chunk_f32, native_rate);
            let pcm_bytes: Vec<u8> = resampled
                .iter()
                .flat_map(|&s| {
                    let i16_val = (s.clamp(-1.0, 1.0) * 32_767.0) as i16;
                    i16_val.to_le_bytes()
                })
                .collect();
            if async_tx.blocking_send(pcm_bytes).is_err() {
                break;
            }
        }
        // sync_rx closed = recording stopped; async_tx drops here
    });

    // ── Main loop: send PCM chunks + receive Deepgram messages ───────────────
    let mut transcript_parts: Vec<String> = Vec::new();
    let mut chunks_sent = 0usize;

    // KeepAlive ticker: send KeepAlive if no audio for 8 s
    let mut keepalive_interval = tokio::time::interval(Duration::from_secs(8));
    keepalive_interval.tick().await; // consume the immediate first tick

    let exit_reason;
    loop {
        tokio::select! {
            chunk = async_rx.recv() => {
                match chunk {
                    Some(pcm) => {
                        chunks_sent += 1;
                        if ws_tx.send(Message::Binary(pcm)).await.is_err() {
                            warn!("[dg_stream] WS send error after {chunks_sent} chunks");
                            exit_reason = "send-error";
                            break;
                        }
                        keepalive_interval.reset();
                    }
                    None => {
                        // Audio channel closed → recording stopped
                        debug!("[dg_stream] audio ended ({chunks_sent} chunks sent) — sending CloseStream");
                        let close = r#"{"type":"CloseStream"}"#;
                        if let Err(e) = ws_tx.send(Message::Text(close.into())).await {
                            warn!("[dg_stream] CloseStream send failed: {e}");
                        }
                        exit_reason = "audio-ended";
                        break;
                    }
                }
            }

            _ = keepalive_interval.tick() => {
                debug!("[dg_stream] sending KeepAlive (no audio for 8 s)");
                let ka = r#"{"type":"KeepAlive"}"#;
                if ws_tx.send(Message::Text(ka.into())).await.is_err() {
                    warn!("[dg_stream] KeepAlive send failed");
                    exit_reason = "send-error";
                    break;
                }
            }

            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Log every Results message at debug so we can see what Deepgram sends
                        if let Ok(v) = serde_json::from_str::<Value>(&text) {
                            let msg_type = v["type"].as_str().unwrap_or("?");
                            if msg_type == "Results" {
                                let is_f   = v["is_final"].as_bool().unwrap_or(false);
                                let sp_f   = v["speech_final"].as_bool().unwrap_or(false);
                                let t      = v["channel"]["alternatives"][0]["transcript"]
                                    .as_str().unwrap_or("");
                                debug!("[dg_stream] Results is_final={is_f} speech_final={sp_f} transcript={t:?}");
                            } else {
                                debug!("[dg_stream] server msg type={msg_type}");
                            }
                        }
                        if let Some(fragment) = parse_speech_final(&text) {
                            debug!("[dg_stream] speech_final captured: {fragment:?}");
                            transcript_parts.push(fragment);
                        }
                    }
                    Some(Ok(Message::Close(frame))) => {
                        // Upgraded from debug → warn so it's always visible
                        warn!("[dg_stream] server closed WS during streaming: {:?}", frame);
                        exit_reason = "server-closed";
                        break;
                    }
                    Some(Err(e)) => {
                        warn!("[dg_stream] WS error during streaming: {e}");
                        exit_reason = "ws-error";
                        break;
                    }
                    None => { exit_reason = "stream-ended"; break; }
                    _ => {}
                }
            }
        }
    }

    info!(
        "[dg_stream] select loop done (reason={exit_reason}), parts so far: {}, chunks_sent: {chunks_sent}",
        transcript_parts.len()
    );

    // ── Drain remaining messages after CloseStream ────────────────────────────
    // Keep ws_tx alive here: dropping SplitSink before the drain could trigger
    // a WS close handshake, causing ws_rx.next() to return None immediately
    // and losing Deepgram's final is_final Results.
    let _keep_tx_alive = ws_tx;

    // Give Deepgram time to flush remaining utterances.
    // Scale by chunks sent: burst audio (all chunks arrive at once after TLS delay) needs
    // proportionally more time.  Formula: 12 ms/chunk, minimum 2500 ms.
    let drain_ms = (chunks_sent as u64 * 12).max(2500);
    debug!("[dg_stream] drain window: {drain_ms}ms for {chunks_sent} chunks");
    let drain_deadline = tokio::time::Instant::now() + Duration::from_millis(drain_ms);
    loop {
        let remaining = drain_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            debug!("[dg_stream] drain timeout reached");
            break;
        }

        match tokio::time::timeout(remaining, ws_rx.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                // Log every Results message during drain
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    let msg_type = v["type"].as_str().unwrap_or("?");
                    if msg_type == "Results" {
                        let is_f = v["is_final"].as_bool().unwrap_or(false);
                        let sp_f = v["speech_final"].as_bool().unwrap_or(false);
                        let t    = v["channel"]["alternatives"][0]["transcript"]
                            .as_str().unwrap_or("");
                        debug!("[dg_stream] drain Results is_final={is_f} speech_final={sp_f} transcript={t:?}");
                    } else {
                        debug!("[dg_stream] drain msg type={msg_type}");
                    }
                }
                if let Some(fragment) = parse_speech_final_or_final(&text) {
                    debug!("[dg_stream] drain captured: {fragment:?}");
                    transcript_parts.push(fragment);
                }
                if is_utterance_end(&text) {
                    debug!("[dg_stream] UtteranceEnd received — done");
                    break;
                }
            }
            Ok(Some(Ok(Message::Close(frame)))) => {
                debug!("[dg_stream] WS closed by server during drain: {:?}", frame);
                break;
            }
            Ok(None) => {
                debug!("[dg_stream] WS stream ended during drain");
                break;
            }
            Ok(Some(Err(e))) => { warn!("[dg_stream] drain WS error: {e}"); break; }
            Ok(Some(Ok(_))) => {} // ping/pong/binary — ignore
            Err(_) => { debug!("[dg_stream] drain timed out"); break; }
        }
    }

    // ws_tx drops here (after drain)
    drop(_keep_tx_alive);

    let full = transcript_parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if full.is_empty() {
        warn!("[dg_stream] no transcript received from WS stream (parts={}, chunks_sent={chunks_sent})",
              transcript_parts.len());
        None
    } else {
        debug!("[dg_stream] final transcript: {full:?}");
        Some(full)
    }
}

// ── Deepgram message parsers ─────────────────────────────────────────────────

fn parse_speech_final(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v["type"] != "Results" { return None; }
    if !v["speech_final"].as_bool().unwrap_or(false) { return None; }
    let t = v["channel"]["alternatives"][0]["transcript"].as_str()?.to_string();
    if t.is_empty() { None } else { Some(t) }
}

fn parse_speech_final_or_final(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v["type"] != "Results" { return None; }
    let speech_final = v["speech_final"].as_bool().unwrap_or(false);
    let is_final     = v["is_final"].as_bool().unwrap_or(false);
    if !speech_final && !is_final { return None; }
    let t = v["channel"]["alternatives"][0]["transcript"].as_str()?.to_string();
    if t.is_empty() { None } else { Some(t) }
}

fn is_utterance_end(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .map(|v| v["type"] == "UtteranceEnd")
        .unwrap_or(false)
}
