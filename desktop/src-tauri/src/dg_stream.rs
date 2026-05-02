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

/// Confidence threshold — words below this get [word?XX%] markers for the LLM.
const LOW_CONFIDENCE_THRESHOLD: f64 = 0.85;

/// Resample raw audio chunks from the microphone, send them to Deepgram via WS,
/// and return the final transcript once the stream is closed.
///
/// - `chunk_recv.rx` is closed (channel disconnected) by `AudioRecorder::stop()`.
/// - `language`: pass an empty string for auto-detect / Hindi default.
/// - `keyterms`: words/phrases to boost recognition for (the "right" spellings from
///   user's correction history). Appended as `&keyterm=` params (Nova-3 only).
///
/// Returns `None` if WS connection fails or Deepgram returns no transcript.
pub async fn stream_to_deepgram(
    chunk_recv:   ChunkReceiver,
    deepgram_key: &str,
    language:     &str,
    keyterms:     &[String],
) -> Option<String> {
    if deepgram_key.is_empty() {
        warn!("[dg_stream] no Deepgram API key — WS streaming disabled");
        return None;
    }

    let lang = if language.is_empty() || language == "auto" { "hi" } else { language };

    // Endpointing: how long Deepgram waits after silence before marking speech_final.
    // - multi: 100ms — Deepgram's recommended value for code-switching.
    // - hi/en: 300ms — balanced; 500ms was too slow (added 200ms to every recording).
    let endpointing = if lang == "multi" { 100 } else { 300 };

    // Build WS URL — keyterms appended once via urlencode (RFC-3986 safe).
    // Previously keyterms were appended twice (once via replace('+') and once via
    // urlencode loop), doubling the URL size with incorrectly-encoded duplicates.
    let mut url_str = format!(
        "wss://api.deepgram.com/v1/listen\
         ?model=nova-3\
         &language={lang}\
         &punctuate=true\
         &encoding=linear16\
         &sample_rate={SAMPLE_RATE}\
         &channels=1\
         &interim_results=true\
         &endpointing={endpointing}\
         &utterance_end_ms=1000"
    );
    let mut bias_count = 0usize;
    for term in keyterms.iter().take(100) {
        let cleaned = term.trim();
        if cleaned.is_empty() { continue; }
        url_str.push_str("&keyterm=");
        url_str.push_str(&urlencode(cleaned));
        bias_count += 1;
    }
    if bias_count > 0 {
        info!("[dg_stream] {} keyterm(s) for STT boost", bias_count);
    }

    info!("[dg_stream] connecting to Deepgram WS (lang={lang}, endpointing={endpointing}ms, bias={bias_count}), key_len={}", deepgram_key.len());

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

    info!("[dg_stream] ✓ connected to Deepgram WS");

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
                        if chunks_sent == 1 {
                            info!("[dg_stream] → first audio chunk sent to Deepgram ({} bytes)", pcm.len());
                        } else if chunks_sent % 50 == 0 {
                            info!("[dg_stream] → {chunks_sent} chunks sent so far");
                        }
                        if ws_tx.send(Message::Binary(pcm)).await.is_err() {
                            warn!("[dg_stream] WS send error after {chunks_sent} chunks");
                            exit_reason = "send-error";
                            break;
                        }
                        keepalive_interval.reset();
                    }
                    None => {
                        // Audio channel closed → recording stopped
                        info!("[dg_stream] audio ended ({chunks_sent} chunks sent) — sending CloseStream");
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
                                info!("[dg_stream] ← Results is_final={is_f} speech_final={sp_f} transcript={t:?}");

                                // Capture ALL is_final=true segments with confidence markers.
                                if is_f && !t.is_empty() {
                                    let enriched = enrich_from_words(&v["channel"]["alternatives"][0]["words"]);
                                    info!("[dg_stream] captured is_final segment: {enriched:?} (speech_final={sp_f})");
                                    transcript_parts.push(enriched);
                                }
                            } else {
                                info!("[dg_stream] ← server msg type={msg_type}");
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
    // IMPORTANT: Don't break on speech_final or UtteranceEnd — there may be multiple
    // utterances pending (the user paused briefly, creating separate segments).
    // Instead, after seeing any speech_final or UtteranceEnd, reset a 500ms timer.
    // If nothing arrives in 500ms, we're done.  This is enough for Deepgram to
    // send a second utterance's results if one exists.
    let drain_ms = (chunks_sent as u64 * 12).max(2500);
    info!("[dg_stream] drain budget={drain_ms}ms — will wait 500ms after last speech_final for additional segments");
    let drain_start    = tokio::time::Instant::now();
    let drain_deadline = drain_start + Duration::from_millis(drain_ms);

    // Track when we last saw a speech_final/UtteranceEnd — after 500ms with no more results, we exit
    let mut last_speech_final: Option<tokio::time::Instant> = None;

    loop {
        let remaining = drain_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            info!("[dg_stream] drain budget exhausted after {}ms",
                  drain_start.elapsed().as_millis());
            break;
        }

        // If we've seen a speech_final/UtteranceEnd, use a shorter timeout to catch stragglers.
        // 500ms is enough for Deepgram to send a second utterance's results if one exists.
        let effective_timeout = if let Some(sf_at) = last_speech_final {
            let since_sf = tokio::time::Instant::now().saturating_duration_since(sf_at);
            let sf_remaining = Duration::from_millis(500).saturating_sub(since_sf);
            if sf_remaining.is_zero() {
                info!("[dg_stream] 500ms after last speech_final/UtteranceEnd — drain done ({}ms total)",
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
                        info!("[dg_stream] ← drain Results is_final={is_f} speech_final={sp_f} transcript={t:?}");

                        // Capture any is_final fragment with confidence markers
                        if is_f && !t.is_empty() {
                            let enriched = enrich_from_words(&v["channel"]["alternatives"][0]["words"]);
                            info!("[dg_stream] drain captured: {enriched:?} (speech_final={sp_f})");
                            transcript_parts.push(enriched);
                        }

                        // Reset the timer — wait 500ms more for additional segments
                        if sp_f {
                            last_speech_final = Some(tokio::time::Instant::now());
                            info!("[dg_stream] speech_final at {}ms — waiting 500ms for more segments",
                                  drain_start.elapsed().as_millis());
                        }
                    } else if msg_type == "UtteranceEnd" {
                        // DON'T break here — UtteranceEnd fires per-utterance.
                        // If the user paused briefly, there may be a second utterance
                        // whose is_final/speech_final haven't arrived yet.
                        // Instead, treat it like a speech_final: reset the 500ms timer.
                        info!("[dg_stream] UtteranceEnd at {}ms — waiting for more segments",
                              drain_start.elapsed().as_millis());
                        last_speech_final = Some(tokio::time::Instant::now());
                    } else {
                        debug!("[dg_stream] drain msg type={msg_type}");
                    }
                }
            }
            Ok(Some(Ok(Message::Close(frame)))) => {
                info!("[dg_stream] WS closed by server during drain: {:?}", frame);
                break;
            }
            Ok(None) => {
                info!("[dg_stream] WS stream ended during drain");
                break;
            }
            Ok(Some(Err(e))) => { warn!("[dg_stream] drain WS error: {e}"); break; }
            Ok(Some(Ok(_))) => {} // ping/pong/binary — ignore
            Err(_) => {
                if last_speech_final.is_some() {
                    info!("[dg_stream] 500ms after last speech_final/UtteranceEnd — drain done ({}ms total)",
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

/// Build enriched text from Deepgram's `words` array inside a Results message.
/// Words with confidence < threshold are marked `[word?XX%]` so the LLM knows
/// which words to scrutinize for context-based correction.
///
/// Falls back to joining plain `punctuated_word`/`word` fields if parsing fails.
fn enrich_from_words(words_val: &Value) -> String {
    let Some(words) = words_val.as_array() else {
        return String::new();
    };
    if words.is_empty() {
        return String::new();
    }

    let mut parts = Vec::with_capacity(words.len());
    for w in words {
        let word = w["punctuated_word"]
            .as_str()
            .or_else(|| w["word"].as_str())
            .unwrap_or("");
        if word.is_empty() {
            continue;
        }

        let conf = w["confidence"].as_f64().unwrap_or(1.0);
        if conf < LOW_CONFIDENCE_THRESHOLD {
            parts.push(format!("[{}?{:.0}%]", word, conf * 100.0));
        } else {
            parts.push(word.to_string());
        }
    }

    parts.join(" ")
}

/// Minimal URL encoder for query-string values (RFC 3986 unreserved set).
/// Spaces become `%20` (not `+`).
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{:02X}", b);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::urlencode;

    #[test]
    fn urlencode_handles_jargon_and_special_chars() {
        assert_eq!(urlencode("n8n"),       "n8n");
        assert_eq!(urlencode("k8s"),       "k8s");
        assert_eq!(urlencode("hello"),     "hello");
        assert_eq!(urlencode("hi there"),  "hi%20there");
        assert_eq!(urlencode("a&b=c"),     "a%26b%3Dc");
    }
}

