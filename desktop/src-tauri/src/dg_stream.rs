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

/// One transcription update from Deepgram during a streaming session.
/// Emitted for both interim (in-progress) and final (committed) results so
/// the UI can show live text as the user speaks.
///
/// While `is_final` is false the `text` may be revised by subsequent updates
/// for the same segment. Once `is_final` is true the segment is committed
/// and a new segment may begin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterimUpdate {
    /// Recognized text for this segment, with `[word?XX%]` markers for
    /// low-confidence words. Plain text — no JSON wrapping.
    pub text: String,
    /// True once Deepgram has committed this segment (text won't revise).
    pub is_final: bool,
    /// True when Deepgram detected an utterance-final pause. Implies
    /// `is_final`; useful as an early "user paused" signal for downstream
    /// work (e.g. starting polish before drain finishes).
    pub speech_final: bool,
}

/// Sender half of an interim-update channel. Pass `Some(tx)` to
/// `stream_to_deepgram` to receive live transcription updates while audio
/// streams. Use an unbounded channel so the WS task never blocks on a slow
/// consumer.
pub type InterimSender = tokio::sync::mpsc::UnboundedSender<InterimUpdate>;

/// Parse a Deepgram WS Results message into an [`InterimUpdate`].
///
/// Returns `None` for any non-Results message (`KeepAlive`, `Metadata`,
/// `UtteranceEnd`, etc) and for Results messages with empty alternatives.
/// This is a pure function so it can be unit-tested without a live WS.
fn parse_results_message(v: &Value) -> Option<InterimUpdate> {
    if v["type"].as_str() != Some("Results") {
        return None;
    }
    let text = enrich_from_words(&v["channel"]["alternatives"][0]["words"]);
    if text.is_empty() {
        return None;
    }
    Some(InterimUpdate {
        text,
        is_final:     v["is_final"].as_bool().unwrap_or(false),
        speech_final: v["speech_final"].as_bool().unwrap_or(false),
    })
}

/// A pre-warmed Deepgram WebSocket connection ready to start receiving audio.
/// Stored in `PrewarmedWsState` between recordings to eliminate the TLS handshake
/// from the hot path (~150ms saved, up to 3s saved under rapid use).
pub struct PrewarmedWs {
    pub ws:         tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    pub language:   String,
    pub keyterms:   Vec<String>,
    pub created_at: std::time::Instant,
}

/// Deepgram closes idle sockets after ~10s with no audio or KeepAlive.
/// Discard pre-warms older than this to avoid handing a dead socket to streaming.
const PREWARM_MAX_AGE: Duration = Duration::from_secs(8);

/// Build the Deepgram WS URL for the given language and keyterms.
fn build_ws_url(lang: &str, keyterms: &[String]) -> (String, usize) {
    let endpointing = if lang == "multi" { 100 } else { 500 };
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
    (url_str, bias_count)
}

/// Open a fresh Deepgram WebSocket connection and return it ready for audio.
/// Called both for cold-start and for pre-warming the next recording's connection.
pub async fn connect_ws(deepgram_key: &str, language: &str, keyterms: &[String]) -> Option<PrewarmedWs> {
    if deepgram_key.is_empty() { return None; }
    let lang = if language.is_empty() || language == "auto" { "hi" } else { language };
    let (url_str, bias_count) = build_ws_url(lang, keyterms);

    let mut req = match url_str.into_client_request() {
        Ok(r)  => r,
        Err(e) => { warn!("[dg_stream] bad WS URL: {e}"); return None; }
    };
    let auth_value = match format!("Token {deepgram_key}").parse() {
        Ok(v)  => v,
        Err(e) => { warn!("[dg_stream] invalid auth header value: {e}"); return None; }
    };
    req.headers_mut().insert("Authorization", auth_value);

    let start = tokio::time::Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(5), connect_async(req)).await;
    let ms = start.elapsed().as_millis();

    match result {
        Err(_) => { warn!("[dg_stream] WS connect timed out"); None }
        Ok(Err(e)) => { warn!("[dg_stream] WS connect failed: {e}"); None }
        Ok(Ok((ws, _))) => {
            info!("[dg_stream] ✓ WS connected in {ms}ms (lang={lang} keyterms={bias_count})");
            Some(PrewarmedWs {
                ws,
                language:   lang.to_string(),
                keyterms:   keyterms.to_vec(),
                created_at: std::time::Instant::now(),
            })
        }
    }
}

/// Stream audio to Deepgram and return the final transcript.
///
/// `prewarmed`: if Some and params match, uses the pre-established connection
/// (eliminates TLS handshake from hot path). Falls back to fresh connect if None
/// or if language/keyterms changed.
///
/// `interim_tx`: if Some, every Deepgram Results message (interim AND final)
/// is forwarded to this channel as an [`InterimUpdate`]. Send failures (closed
/// receiver) are ignored — the WS task never blocks on a slow consumer.
pub async fn stream_to_deepgram(
    chunk_recv:   ChunkReceiver,
    deepgram_key: &str,
    language:     &str,
    keyterms:     &[String],
    pre_embed:    Option<(&str, &str)>,
    prewarmed:    Option<PrewarmedWs>,
    interim_tx:   Option<InterimSender>,
) -> Option<String> {
    if deepgram_key.is_empty() {
        warn!("[dg_stream] no Deepgram API key — WS streaming disabled");
        return None;
    }

    let lang = if language.is_empty() || language == "auto" { "hi" } else { language };

    // Use pre-warmed WS if params match AND it's still fresh enough.
    let ws = if let Some(pw) = prewarmed {
        let age = pw.created_at.elapsed();
        if pw.language != lang || pw.keyterms != keyterms {
            info!("[dg_stream] pre-warm params mismatch — connecting fresh");
            connect_ws(deepgram_key, lang, keyterms).await?.ws
        } else if age > PREWARM_MAX_AGE {
            info!("[dg_stream] pre-warm stale (age={}ms) — connecting fresh", age.as_millis());
            connect_ws(deepgram_key, lang, keyterms).await?.ws
        } else {
            info!("[dg_stream] ✓ using pre-warmed WS (0ms connect, age={}ms)", age.as_millis());
            pw.ws
        }
    } else {
        info!("[dg_stream] no pre-warm — connecting fresh");
        connect_ws(deepgram_key, lang, keyterms).await?.ws
    };

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
    // Track whether speech_final arrived during streaming so the drain loop
    // can start its 500ms timer immediately instead of waiting the full 2500ms.
    let mut got_speech_final_during_stream = false;

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
                        let close = r#"{"type":"CloseStream"}"#;
                        if let Err(e) = ws_tx.send(Message::Text(close.into())).await {
                            warn!("[dg_stream] CloseStream send failed: {e}");
                        }
                        // ── Speculative pre-embed ──────────────────────────────
                        // Fire before the 500ms drain window so Gemini embedding
                        // is cached by the time /v1/voice/polish is called.
                        if let Some((url, secret)) = pre_embed {
                            let plain = plain_for_embed(&transcript_parts);
                            if !plain.is_empty() {
                                let url    = url.to_string();
                                let secret = secret.to_string();
                                debug!("[dg_stream] firing pre-embed ({} chars)", plain.len());
                                tokio::spawn(async move {
                                    let client = reqwest::Client::new();
                                    let body   = serde_json::json!({"text": plain});
                                    if let Err(e) = client
                                        .post(&url)
                                        .header("Authorization", format!("Bearer {secret}"))
                                        .json(&body)
                                        .timeout(std::time::Duration::from_secs(5))
                                        .send()
                                        .await
                                    {
                                        debug!("[dg_stream] pre-embed request failed: {e}");
                                    }
                                });
                            }
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
                            if let Some(update) = parse_results_message(&v) {
                                // Forward to UI consumer (live preview) regardless
                                // of is_final. Send failure = receiver dropped;
                                // ignore so the WS task never blocks on the UI.
                                if let Some(tx) = &interim_tx {
                                    let _ = tx.send(update.clone());
                                }
                                if update.speech_final {
                                    got_speech_final_during_stream = true;
                                }
                                if update.is_final {
                                    info!("[dg_stream] segment: {:?} (speech_final={})",
                                        update.text, update.speech_final);
                                    transcript_parts.push(update.text);
                                }
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

    // ── Drain remaining messages after CloseStream ────────────────────────────
    // Keep ws_tx alive here: dropping SplitSink before the drain could trigger
    // a WS close handshake, causing ws_rx.next() to return None immediately
    // and losing Deepgram's final is_final Results.
    let _keep_tx_alive = ws_tx;

    // Give Deepgram time to flush remaining utterances after CloseStream.
    //
    // Fast path: if speech_final already arrived during streaming (common for short
    // clips), pre-seed last_speech_final so the drain exits after just 500ms instead
    // of waiting the full 2500ms fallback budget.
    //
    // Fallback: 2500ms flat ceiling for the rare case where Deepgram sends nothing
    // during drain (e.g. very short clip fully processed before CloseStream).
    // Previously this was `chunks_sent * 12` which scaled to 10s+ for long recordings
    // — a flat ceiling is sufficient since the speech_final+500ms logic handles timing.
    let drain_ms       = 2500_u64;
    let drain_start    = tokio::time::Instant::now();
    let drain_deadline = drain_start + Duration::from_millis(drain_ms);

    let mut last_speech_final: Option<tokio::time::Instant> = if got_speech_final_during_stream {
        Some(tokio::time::Instant::now())
    } else {
        None
    };

    loop {
        let remaining = drain_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() { break; }

        let effective_timeout = if let Some(sf_at) = last_speech_final {
            let since_sf = tokio::time::Instant::now().saturating_duration_since(sf_at);
            let sf_remaining = Duration::from_millis(500).saturating_sub(since_sf);
            if sf_remaining.is_zero() { break; }
            remaining.min(sf_remaining)
        } else {
            remaining
        };

        match tokio::time::timeout(effective_timeout, ws_rx.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    if v["type"].as_str() == Some("UtteranceEnd") {
                        last_speech_final = Some(tokio::time::Instant::now());
                    } else if let Some(update) = parse_results_message(&v) {
                        if let Some(tx) = &interim_tx {
                            let _ = tx.send(update.clone());
                        }
                        if update.speech_final {
                            last_speech_final = Some(tokio::time::Instant::now());
                        }
                        if update.is_final {
                            transcript_parts.push(update.text);
                        }
                    }
                }
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
            Ok(Some(Err(e))) => { warn!("[dg_stream] drain error: {e}"); break; }
            Ok(Some(Ok(_))) => {}
            Err(_) => break,
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

    let drain_ms = drain_start.elapsed().as_millis();
    if full.is_empty() {
        warn!("[dg_stream] no transcript — chunks={chunks_sent} drain={drain_ms}ms");
        None
    } else {
        info!("[dg_stream] ✓ transcript ready — drain={}ms chunks={} parts={} : {full:?}",
            drain_ms, chunks_sent, transcript_parts.len());
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

/// Strip `[word?XX%]` confidence markers and join transcript parts into plain
/// text suitable for embedding.  The embedding cache key is SHA256 of the plain
/// transcript, so this must match what voice.rs does in `strip_confidence_markers`.
fn plain_for_embed(parts: &[String]) -> String {
    let joined = parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    // Replace each [word?XX%] marker with just the word before the '?'
    let mut result = String::with_capacity(joined.len());
    let mut chars  = joined.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            let mut inner = String::new();
            let mut closed = false;
            for ic in chars.by_ref() {
                if ic == ']' { closed = true; break; }
                inner.push(ic);
            }
            if closed {
                if let Some(qpos) = inner.rfind('?') {
                    let after = &inner[qpos + 1..];
                    if after.ends_with('%') && after[..after.len()-1].parse::<f64>().is_ok() {
                        result.push_str(&inner[..qpos]);
                        continue;
                    }
                }
                result.push('[');
                result.push_str(&inner);
                result.push(']');
            } else {
                result.push('[');
                result.push_str(&inner);
            }
        } else {
            result.push(c);
        }
    }
    result
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
    use super::*;
    use serde_json::json;

    #[test]
    fn urlencode_handles_jargon_and_special_chars() {
        assert_eq!(urlencode("n8n"),       "n8n");
        assert_eq!(urlencode("k8s"),       "k8s");
        assert_eq!(urlencode("hello"),     "hello");
        assert_eq!(urlencode("hi there"),  "hi%20there");
        assert_eq!(urlencode("a&b=c"),     "a%26b%3Dc");
    }

    // ── parse_results_message ─────────────────────────────────────────────────
    // These payloads mirror the exact shape Deepgram sends over the WS so the
    // parser is exercised against the real protocol, not a stubbed
    // approximation. Sample shapes pulled from Deepgram's WebSocket reference.

    fn results(words: Vec<(&str, f64)>, is_final: bool, speech_final: bool) -> serde_json::Value {
        let words_json: Vec<_> = words
            .into_iter()
            .map(|(w, c)| json!({"word": w, "punctuated_word": w, "confidence": c}))
            .collect();
        json!({
            "type": "Results",
            "channel": { "alternatives": [{ "words": words_json }] },
            "is_final": is_final,
            "speech_final": speech_final
        })
    }

    #[test]
    fn parses_interim_high_confidence() {
        let p = results(vec![("hello", 0.95), ("world", 0.92)], false, false);
        let r = parse_results_message(&p).expect("interim parses");
        assert_eq!(r.text, "hello world");
        assert!(!r.is_final);
        assert!(!r.speech_final);
    }

    #[test]
    fn parses_final_marks_low_confidence_words() {
        // 0.50 < LOW_CONFIDENCE_THRESHOLD (0.85) so "the" gets a marker.
        let p = results(vec![("send", 0.99), ("the", 0.50), ("report", 0.95)], true, true);
        let r = parse_results_message(&p).unwrap();
        assert_eq!(r.text, "send [the?50%] report");
        assert!(r.is_final);
        assert!(r.speech_final);
    }

    #[test]
    fn returns_none_for_non_results_messages() {
        // KeepAlive, Metadata, UtteranceEnd are all non-Results — the caller
        // handles UtteranceEnd separately via msg_type, the others are no-ops.
        assert!(parse_results_message(&json!({"type": "KeepAlive"})).is_none());
        assert!(parse_results_message(&json!({"type": "Metadata", "request_id": "x"})).is_none());
        assert!(parse_results_message(&json!({"type": "UtteranceEnd", "last_word_end": 1.2})).is_none());
    }

    #[test]
    fn returns_none_for_empty_alternatives() {
        // Deepgram sometimes ships Results with no words during silence —
        // emitting an InterimUpdate with empty text would clobber UI state.
        let p = json!({
            "type": "Results",
            "channel": { "alternatives": [{ "words": [] }] },
            "is_final": false,
            "speech_final": false
        });
        assert!(parse_results_message(&p).is_none());
    }

    #[test]
    fn falls_back_to_word_when_punctuated_word_missing() {
        // Some language models omit punctuated_word — must not crash and must
        // recover the plain `word` field.
        let p = json!({
            "type": "Results",
            "channel": { "alternatives": [{ "words": [
                {"word": "test", "confidence": 0.99}
            ] }] },
            "is_final": false,
            "speech_final": false
        });
        let r = parse_results_message(&p).unwrap();
        assert_eq!(r.text, "test");
    }

    #[test]
    fn typical_revising_interim_then_final_sequence() {
        // Models the canonical revision flow:
        //   "send the"      (interim)
        //   "send a report" (interim, revised)
        //   "send a report" (final, speech_final)
        let i1 = results(vec![("send", 0.99), ("the", 0.99)], false, false);
        let i2 = results(vec![("send", 0.99), ("a", 0.96), ("report", 0.99)], false, false);
        let f  = results(vec![("send", 0.99), ("a", 0.96), ("report", 0.99)], true, true);

        let r1 = parse_results_message(&i1).unwrap();
        let r2 = parse_results_message(&i2).unwrap();
        let r3 = parse_results_message(&f).unwrap();

        assert_eq!(r1.text, "send the");
        assert!(!r1.is_final);
        assert_eq!(r2.text, "send a report");
        assert!(!r2.is_final);
        assert_eq!(r3.text, "send a report");
        assert!(r3.is_final);
        assert!(r3.speech_final);
    }

    #[test]
    fn speech_final_can_arrive_on_interim_message() {
        // Defensive: speech_final without is_final is unusual but possible —
        // the caller uses speech_final to start the 500 ms drain timer, so we
        // must surface it independently of is_final.
        let p = results(vec![("hello", 0.99)], false, true);
        let r = parse_results_message(&p).unwrap();
        assert!(!r.is_final);
        assert!(r.speech_final);
    }

    #[test]
    fn missing_is_final_or_speech_final_defaults_to_false() {
        // Deepgram should always include both fields, but a defensive parser
        // must not panic on a malformed payload.
        let p = json!({
            "type": "Results",
            "channel": { "alternatives": [{ "words": [
                {"word": "ok", "confidence": 0.99}
            ] }] }
            // no is_final, no speech_final
        });
        let r = parse_results_message(&p).unwrap();
        assert!(!r.is_final);
        assert!(!r.speech_final);
    }

    // ── InterimSender wiring ─────────────────────────────────────────────────

    #[tokio::test]
    async fn interim_sender_receives_clones_in_order() {
        // Simulates the WS task forwarding a sequence of updates: the
        // consumer must see them in arrival order and identical to source.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<InterimUpdate>();
        let updates = vec![
            InterimUpdate { text: "send".into(),         is_final: false, speech_final: false },
            InterimUpdate { text: "send the".into(),     is_final: false, speech_final: false },
            InterimUpdate { text: "send a report".into(), is_final: false, speech_final: false },
            InterimUpdate { text: "send a report".into(), is_final: true,  speech_final: true  },
        ];
        for u in &updates { tx.send(u.clone()).expect("send"); }
        drop(tx);
        let mut got = Vec::new();
        while let Some(u) = rx.recv().await { got.push(u); }
        assert_eq!(got, updates);
    }

    #[tokio::test]
    async fn dropped_receiver_does_not_panic_sender() {
        // The WS task's send-failure-ignored guarantee: if the consumer
        // disappears mid-stream (e.g. recording cancelled), the next send
        // returns Err but must not panic.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<InterimUpdate>();
        drop(rx);
        let result = tx.send(InterimUpdate {
            text: "anything".into(), is_final: false, speech_final: false,
        });
        assert!(result.is_err());
    }
}

