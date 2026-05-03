import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalPosition, LogicalSize, primaryMonitor } from "@tauri-apps/api/window";
import type { AppSnapshot } from "./types";

// ── State machine ─────────────────────────────────────────────────────────────

type BarState =
  | { kind: "idle" }
  | { kind: "recording"; startMs: number }
  | { kind: "processing"; phase: string }
  | { kind: "done" }
  | { kind: "pasted" }
  | { kind: "manual_paste" }
  | { kind: "error"; message: string; audioId?: string };

type PillKind = BarState["kind"];
const BOTTOM_OFFSET = 64;

// ── Helpers ───────────────────────────────────────────────────────────────────

function pillSize(kind: PillKind, hovered = false): { width: number; height: number } {
  if (kind === "idle") return hovered ? { width: 100, height: 30 } : { width: 72, height: 20 };
  if (kind === "recording") return { width: 76, height: 26 };
  if (kind === "processing") return { width: 70, height: 26 };
  if (kind === "manual_paste") return { width: 82, height: 26 };
  if (kind === "error") return { width: 90, height: 26 };
  return { width: 70, height: 26 };
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function StatusBar() {
  const [bar, setBar] = useState<BarState>({ kind: "idle" });
  const [idleHovered, setIdleHovered] = useState(false);
  const doneTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const win = getCurrentWindow();

  useEffect(() => {
    console.info("[status-bar] mounted", {
      label: win.label,
      href: window.location.href,
      hash: window.location.hash,
      search: window.location.search,
    });
  }, []);

  // Keep the overlay visible at idle as a tiny reference-style hover pill.
  // Rust owns the native always-on-top behavior; React only changes content.
  useEffect(() => {
    console.info("[status-bar] state", bar);
    const { width, height } = pillSize(bar.kind, bar.kind === "idle" && idleHovered);
    primaryMonitor()
      .then((monitor) => {
        const scale = monitor?.scaleFactor ?? 1;
        const sw = monitor ? monitor.size.width / scale : 1440;
        const sh = monitor ? monitor.size.height / scale : 900;
        const sx = monitor ? monitor.position.x / scale : 0;
        const sy = monitor ? monitor.position.y / scale : 0;
        const x = sx + sw / 2 - width / 2;
        const y = sy + sh - height - BOTTOM_OFFSET;
        return win
          .setSize(new LogicalSize(width, height))
          .then(() => win.setPosition(new LogicalPosition(x, y)));
      })
      .then(() => console.info("[status-bar] chrome sized", { kind: bar.kind, idleHovered, width, height }))
      .catch((err) => console.warn("[status-bar] chrome size failed", err));
  }, [bar.kind, idleHovered]);

  // Seed from current snapshot on mount so we reflect any in-progress state
  useEffect(() => {
    invoke<AppSnapshot>("get_snapshot")
      .then((snap) => {
        console.info("[status-bar] initial snapshot", snap.state);
        if (snap.state === "recording") {
          setBar({ kind: "recording", startMs: Date.now() });
        } else if (snap.state === "processing") {
          setBar({ kind: "processing", phase: "stt" });
        }
      })
      .catch((err) => {
        console.warn("[status-bar] initial snapshot failed", err);
      });
  }, []);

  useEffect(() => {
    const subs: Array<() => void> = [];

    // ── Source of truth for recording / processing / idle ──────────────────
    listen<AppSnapshot>("app-state", (e) => {
      const { state } = e.payload;
      console.info("[status-bar] app-state event", state);
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
          if (prev.kind === "done" || prev.kind === "pasted" || prev.kind === "manual_paste") {
            return prev; // timer handles it
          }
          return { kind: "idle" };
        });
      }
    }).then((fn) => {
      console.info("[status-bar] subscribed app-state");
      subs.push(fn);
    }).catch((err) => console.warn("[status-bar] app-state subscribe failed", err));

    // ── Sub-phase label updates ────────────────────────────────────────────
    listen<{ phase: string; transcript?: string }>("voice-status", (e) => {
      const { phase } = e.payload;
      console.info("[status-bar] voice-status event", phase);
      setBar((prev) =>
        prev.kind === "processing" ? { kind: "processing", phase } : prev
      );
    }).then((fn) => {
      console.info("[status-bar] subscribed voice-status");
      subs.push(fn);
    }).catch((err) => console.warn("[status-bar] voice-status subscribe failed", err));

    // ── Success: flash "Done" for 1.8 s then hide ──────────────────────────
    listen("voice-done", () => {
      console.info("[status-bar] voice-done event");
      if (doneTimer.current) clearTimeout(doneTimer.current);
      setBar({ kind: "done" });
      doneTimer.current = setTimeout(() => setBar({ kind: "idle" }), 2400);
    }).then((fn) => {
      console.info("[status-bar] subscribed voice-done");
      subs.push(fn);
    }).catch((err) => console.warn("[status-bar] voice-done subscribe failed", err));

    listen<{ status: "pasted" | "manual_paste"; message?: string }>("voice-output", (e) => {
      console.info("[status-bar] voice-output event", e.payload);
      if (doneTimer.current) clearTimeout(doneTimer.current);
      setBar({ kind: e.payload.status });
      doneTimer.current = setTimeout(
        () => setBar({ kind: "idle" }),
        e.payload.status === "pasted" ? 1800 : 5200,
      );
    }).then((fn) => {
      console.info("[status-bar] subscribed voice-output");
      subs.push(fn);
    }).catch((err) => console.warn("[status-bar] voice-output subscribe failed", err));

    // ── Error: show message + optional retry ──────────────────────────────
    listen<{ message: string; audio_id?: string }>("voice-error", (e) => {
      const { message, audio_id } = e.payload;
      console.info("[status-bar] voice-error event", { message, hasAudioId: Boolean(audio_id) });
      if (doneTimer.current) clearTimeout(doneTimer.current);
      setBar({ kind: "error", message, audioId: audio_id });
    }).then((fn) => {
      console.info("[status-bar] subscribed voice-error");
      subs.push(fn);
    }).catch((err) => console.warn("[status-bar] voice-error subscribe failed", err));

    return () => {
      console.info("[status-bar] unmount subscriptions", subs.length);
      subs.forEach((fn) => fn());
    };
  }, []);

  useEffect(() => () => { if (doneTimer.current) clearTimeout(doneTimer.current); }, []);

  return (
    <div
      className={`sb-pill sb-pill--${bar.kind}${bar.kind === "idle" && idleHovered ? " sb-pill--hovered" : ""}`}
      data-tauri-drag-region
      aria-label={`Said ${bar.kind}`}
      title={`Said ${bar.kind}`}
      onMouseEnter={() => {
        if (bar.kind === "idle") setIdleHovered(true);
      }}
      onMouseLeave={() => setIdleHovered(false)}
    >

      {bar.kind === "idle" && (
        <span className="sb-idle-line" />
      )}

      {bar.kind === "recording" && (
        <>
          <span className="sb-rec-wave sb-rec-wave--a" />
          <span className="sb-rec-wave sb-rec-wave--b" />
          <span className="sb-rec-wave sb-rec-wave--c" />
        </>
      )}

      {bar.kind === "processing" && (
        <span className="sb-spinner" />
      )}

      {bar.kind === "done" && (
        <span className="sb-burst sb-burst--ok" />
      )}

      {bar.kind === "pasted" && (
        <span className="sb-burst sb-burst--ok" />
      )}

      {bar.kind === "manual_paste" && (
        <span className="sb-manual-paste" />
      )}

      {bar.kind === "error" && (
        <>
          <span className="sb-error-pulse" />
          {bar.audioId && (
            <button
              className="sb-btn sb-btn--retry"
              title="Retry"
              aria-label="Retry"
              onClick={async () => {
                try {
                  await invoke("retry_recording", { audioId: bar.audioId });
                  setBar({ kind: "processing", phase: "stt" });
                } catch (e) {
                  setBar({ kind: "error", message: String(e) });
                }
              }}
            >
              ↻
            </button>
          )}
          <button
            className="sb-btn sb-btn--dismiss"
            title="Dismiss"
            aria-label="Dismiss"
            onClick={() => setBar({ kind: "idle" })}
          >
            ✕
          </button>
        </>
      )}

    </div>
  );
}
