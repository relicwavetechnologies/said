import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { AppSnapshot } from "./types";

// ── State machine ─────────────────────────────────────────────────────────────

type BarState =
  | { kind: "idle" }
  | { kind: "recording"; startMs: number }
  | { kind: "processing"; phase: string }
  | { kind: "done" }
  | { kind: "error"; message: string; audioId?: string };

// ── Helpers ───────────────────────────────────────────────────────────────────

function phaseLabel(phase: string): string {
  if (phase === "stt" || phase === "transcribe") return "Transcribing...";
  if (phase === "polish" || phase === "llm" || phase === "generate") return "Polishing...";
  if (phase === "classify" || phase === "learn") return "Learning...";
  if (phase === "embed") return "Indexing...";
  return "Processing...";
}

function shortError(msg: string): string {
  // strip leading "voice polish error:" prefix noise
  const clean = msg.replace(/^voice(?: polish)? error:?\s*/i, "").trim();
  if (clean.length <= 44) return clean;
  return clean.slice(0, 41) + "…";
}

function fmt(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

// ── Timer hook ────────────────────────────────────────────────────────────────

function useElapsed(startMs: number | null): number {
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    if (startMs === null) { setElapsed(0); return; }
    const id = setInterval(
      () => setElapsed(Math.floor((Date.now() - startMs) / 1000)),
      250,
    );
    return () => clearInterval(id);
  }, [startMs]);
  return elapsed;
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function StatusBar() {
  const [bar, setBar] = useState<BarState>({ kind: "idle" });
  const doneTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const win = getCurrentWebviewWindow();

  // Show / hide the native window based on state — no focus steal
  useEffect(() => {
    if (bar.kind === "idle") {
      win.hide();
    } else {
      win.show();
    }
  }, [bar.kind]);

  // Seed from current snapshot on mount so we reflect any in-progress state
  useEffect(() => {
    invoke<AppSnapshot>("get_snapshot")
      .then((snap) => {
        if (snap.state === "recording") {
          setBar({ kind: "recording", startMs: Date.now() });
        } else if (snap.state === "processing") {
          setBar({ kind: "processing", phase: "stt" });
        }
      })
      .catch(() => {/* backend not ready yet — events will catch up */});
  }, []);

  useEffect(() => {
    const subs: Array<() => void> = [];

    // ── Source of truth for recording / processing / idle ──────────────────
    listen<AppSnapshot>("app-state", (e) => {
      const { state } = e.payload;
      if (state === "recording") {
        if (doneTimer.current) clearTimeout(doneTimer.current);
        setBar({ kind: "recording", startMs: Date.now() });
      } else if (state === "processing") {
        setBar((prev) =>
          prev.kind === "recording"
            ? { kind: "processing", phase: "stt" }
            : prev.kind === "processing" ? prev
            : { kind: "processing", phase: "stt" }
        );
      } else if (state === "idle") {
        // Only auto-hide if we're not waiting on a user-action (error/done)
        setBar((prev) => {
          if (prev.kind === "error") return prev; // user must dismiss
          if (prev.kind === "done")  return prev; // timer handles it
          return { kind: "idle" };
        });
      }
    }).then((fn) => subs.push(fn));

    // ── Sub-phase label updates ────────────────────────────────────────────
    listen<{ phase: string; transcript?: string }>("voice-status", (e) => {
      const { phase } = e.payload;
      setBar((prev) =>
        prev.kind === "processing" ? { kind: "processing", phase } : prev
      );
    }).then((fn) => subs.push(fn));

    // ── Success: flash "Done" for 1.8 s then hide ──────────────────────────
    listen("voice-done", () => {
      if (doneTimer.current) clearTimeout(doneTimer.current);
      setBar({ kind: "done" });
      doneTimer.current = setTimeout(() => setBar({ kind: "idle" }), 1800);
    }).then((fn) => subs.push(fn));

    // ── Error: show message + optional retry ──────────────────────────────
    listen<{ message: string; audio_id?: string }>("voice-error", (e) => {
      const { message, audio_id } = e.payload;
      if (doneTimer.current) clearTimeout(doneTimer.current);
      setBar({ kind: "error", message, audioId: audio_id });
    }).then((fn) => subs.push(fn));

    return () => subs.forEach((fn) => fn());
  }, []);

  useEffect(() => () => { if (doneTimer.current) clearTimeout(doneTimer.current); }, []);

  const elapsed = useElapsed(bar.kind === "recording" ? bar.startMs : null);

  // Render nothing when idle (window is hidden anyway)
  if (bar.kind === "idle") return null;

  return (
    <div className="sb-pill" data-tauri-drag-region>

      {bar.kind === "recording" && (
        <>
          <span className="sb-dot sb-dot--rec" />
          <span className="sb-label">Recording</span>
          <span className="sb-timer">{fmt(elapsed)}</span>
        </>
      )}

      {bar.kind === "processing" && (
        <>
          <span className="sb-spinner" />
          <span className="sb-label">{phaseLabel(bar.phase)}</span>
        </>
      )}

      {bar.kind === "done" && (
        <>
          <span className="sb-dot sb-dot--ok" />
          <span className="sb-label">Done</span>
        </>
      )}

      {bar.kind === "error" && (
        <>
          <span className="sb-dot sb-dot--err" />
          <span className="sb-label sb-label--err">{shortError(bar.message)}</span>
          {bar.audioId && (
            <button
              className="sb-btn sb-btn--retry"
              onClick={async () => {
                try {
                  await invoke("retry_recording", { audioId: bar.audioId });
                  setBar({ kind: "processing", phase: "stt" });
                } catch (e) {
                  setBar({ kind: "error", message: String(e) });
                }
              }}
            >
              Retry
            </button>
          )}
          <button
            className="sb-btn sb-btn--dismiss"
            onClick={() => setBar({ kind: "idle" })}
          >
            ✕
          </button>
        </>
      )}

    </div>
  );
}
