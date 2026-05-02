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
    // utterance_end_ms=1000: Deepgram emits UtteranceEnd ~1 s after speech stops,
    // which lets us break the drain loop immediately instead of waiting the full window.
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
         &endpointing=300\
         &utterance_end_ms=1000"
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
                        if let Ok(v) = serde_json::from_str::<Value>(&text) {
                            let msg_type = v["type"].as_str().unwrap_or("?");
                            if msg_type == "Results" {
                                let is_f   = v["is_final"].as_bool().unwrap_or(false);
                                let sp_f   = v["speech_final"].as_bool().unwrap_or(false);
                                let t      = v["channel"]["alternatives"][0]["transcript"]
                                    .as_str().unwrap_or("");
                                debug!("[dg_stream] Results is_final={is_f} speech_final={sp_f} transcript={t:?}");

                                // Capture ALL is_final=true segments — not just speech_final.
                                // Each is_final covers a distinct audio chunk; speech_final
                                // only fires after endpointing silence (300ms).  Without this,
                                // continuous speech produces 0 parts during streaming and the
                                // entire transcript comes from a single drain speech_final
                                // (often just the LAST segment, losing everything before it).
                                if is_f && !t.is_empty() {
                                    info!("[dg_stream] captured is_final segment: {t:?} (speech_final={sp_f})");
                                    transcript_parts.push(t.to_string());
                                }
                            } else {
                                debug!("[dg_stream] server msg type={msg_type}");
                            }
                        }
                    }
                    Some(Ok(Message::Close(frame))) => {
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

    let parts_from_streaming = transcript_parts.len();
    info!(
        "[dg_stream] select loop done (reason={exit_reason}), parts from streaming: {parts_from_streaming}, chunks_sent: {chunks_sent}",
    );

    // ── Drain remaining messages after CloseStream ────────────────────────────
    // Keep ws_tx alive here: dropping SplitSink before the drain could trigger
    // a WS close handshake, causing ws_rx.next() to return None immediately
    // and losing Deepgram's final is_final Results.
    let _keep_tx_alive = ws_tx;

    // Give Deepgram time to flush remaining utterances after CloseStream.
    //
    // IMPORTANT: Don't break on the FIRST speech_final — there may be multiple
    // utterances pending (the user paused briefly, creating separate segments).
    // Instead, after seeing the first speech_final, set a short secondary timeout
    // (200ms) to catch any additional segments.  If nothing arrives in 200ms after
    // the last speech_final, we're done.
    let drain_ms = (chunks_sent as u64 * 12).max(800);
    info!("[dg_stream] drain budget={drain_ms}ms — will wait 200ms after last speech_final for additional segments");
    let drain_start    = tokio::time::Instant::now();
    let drain_deadline = drain_start + Duration::from_millis(drain_ms);

    // Track when we last saw a speech_final — after 200ms with no more results, we exit
    let mut last_speech_final: Option<tokio::time::Instant> = None;

    loop {
        let remaining = drain_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            info!("[dg_stream] drain budget exhausted after {}ms",
                  drain_start.elapsed().as_millis());
            break;
        }

        // If we've seen a speech_final, use a shorter timeout (200ms) to catch stragglers
        let effective_timeout = if let Some(sf_at) = last_speech_final {
            let since_sf = tokio::time::Instant::now().saturating_duration_since(sf_at);
            let sf_remaining = Duration::from_millis(200).saturating_sub(since_sf);
            if sf_remaining.is_zero() {
                info!("[dg_stream] 200ms after last speech_final — drain done ({}ms total)",
                      drain_start.elapsed().as_millis());
                break;
            }
            remaining.min(sf_remaining)
        } else {
            remaining
        };

        match tokio::time::timeout(effective_timeout, ws_rx.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    let msg_type = v["type"].as_str().unwrap_or("?");
                    if msg_type == "Results" {
                        let is_f = v["is_final"].as_bool().unwrap_or(false);
                        let sp_f = v["speech_final"].as_bool().unwrap_or(false);
                        let t    = v["channel"]["alternatives"][0]["transcript"]
                            .as_str().unwrap_or("");
                        debug!("[dg_stream] drain Results is_final={is_f} speech_final={sp_f} transcript={t:?}");

                        // Capture any is_final fragment (same rule as main loop)
                        if is_f && !t.is_empty() {
                            info!("[dg_stream] drain captured: {t:?} (speech_final={sp_f})");
                            transcript_parts.push(t.to_string());
                        }

                        // Reset the speech_final timer — wait 200ms more for additional segments
                        if sp_f {
                            last_speech_final = Some(tokio::time::Instant::now());
                            info!("[dg_stream] speech_final at {}ms — waiting 200ms for more segments",
                                  drain_start.elapsed().as_millis());
                        }
                    } else if msg_type == "UtteranceEnd" {
                        info!("[dg_stream] UtteranceEnd at {}ms — drain done",
                              drain_start.elapsed().as_millis());
                        break;
                    } else {
                        debug!("[dg_stream] drain msg type={msg_type}");
                    }
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
            Err(_) => {
                if last_speech_final.is_some() {
                    info!("[dg_stream] 200ms after last speech_final — drain done ({}ms total)",
                          drain_start.elapsed().as_millis());
                } else {
                    info!("[dg_stream] drain timed out after {}ms (no speech_final arrived)",
                          drain_start.elapsed().as_millis());
                }
                break;
            }
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

    let parts_from_drain = transcript_parts.len() - parts_from_streaming;
    info!(
        "[dg_stream] transcript assembled: {} parts ({} streaming + {} drain), {} chars",
        transcript_parts.len(), parts_from_streaming, parts_from_drain, full.len()
    );

    if full.is_empty() {
        warn!("[dg_stream] no transcript received from WS stream (parts={}, chunks_sent={chunks_sent})",
              transcript_parts.len());
        None
    } else {
        info!("[dg_stream] final transcript: {full:?}");
        Some(full)
    }
}

// (parse_speech_final removed — main loop now captures all is_final=true events inline)

