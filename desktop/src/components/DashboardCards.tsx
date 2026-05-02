import React, { useState } from "react";
import {
  Settings2,
  Filter, Play, Pause, Check, Copy,
  ChevronRight, ChevronDown as CaretDown,
} from "lucide-react";
import { useAudioPlayer } from "@/lib/useAudioPlayer";
import type { AppSnapshot, Recording } from "@/types";

/* ════════════════════════════════════════════════════════════════════════════
   1) HeroStat — REAL DB number: total recordings count, with streak + WPM
      context underneath. The colors use --foreground so they work in both
      dark and light mode without going invisible.
   ════════════════════════════════════════════════════════════════════════════ */

export function HeroStat({ snapshot }: { snapshot: AppSnapshot | null }) {
  const history    = snapshot?.history  ?? [];
  const recordings = history.length;
  const streak     = snapshot?.daily_streak ?? 0;
  const avgWpm     = snapshot?.avg_wpm  ?? 0;
  const totalWords = snapshot?.total_words ?? 0;

  // Format the big number — use compact notation if huge, else raw count
  const heroValue =
    recordings >= 10_000 ? `${(recordings / 1000).toFixed(1)}k` :
    recordings.toLocaleString();

  // Today's words for the "fresh today" pill
  const start = new Date();
  start.setHours(0, 0, 0, 0);
  const todayWords = history
    .filter((h) => h.timestamp_ms >= start.getTime())
    .reduce((s, h) => s + h.word_count, 0);

  return (
    <div className="panel relative overflow-hidden p-7 h-full flex flex-col justify-between">
      {/* Soft violet glow top-right (subtle in both modes) */}
      <div
        aria-hidden
        className="absolute pointer-events-none"
        style={{
          right: -120, top: -120, width: 320, height: 320, borderRadius: "50%",
          background: "radial-gradient(circle, hsl(var(--accent-violet) / 0.16) 0%, transparent 70%)",
        }}
      />

      {/* Big number — gradient that contrasts in BOTH modes */}
      <div className="relative">
        <p
          className="font-bold tabular-nums leading-none tracking-tight"
          style={{
            fontSize: 100,
            background:
              "linear-gradient(135deg, hsl(var(--foreground)) 0%, hsl(var(--accent-violet)) 100%)",
            WebkitBackgroundClip: "text",
            WebkitTextFillColor: "transparent",
            backgroundClip: "text",
            letterSpacing: "-0.03em",
          }}
        >
          {heroValue}
        </p>
        <p
          className="text-[14px] font-bold uppercase tracking-[0.16em] mt-1"
          style={{ color: "hsl(var(--muted-foreground))" }}
        >
          recordings
        </p>
      </div>

      {/* Subtitle: real metrics from DB */}
      <div className="relative mt-4">
        <p className="text-[13px] leading-snug"
           style={{ color: "hsl(var(--foreground) / 0.85)" }}>
          {streak > 0 ? (
            <>
              You're on a{" "}
              <span
                className="inline-flex items-center px-2 py-0.5 rounded-md font-bold tabular-nums"
                style={{
                  background: "hsl(var(--primary) / 0.20)",
                  color:      "hsl(var(--primary))",
                  fontSize: 13,
                }}
              >
                {streak} day streak
              </span>
              {avgWpm > 0 && (
                <>
                  {" "}with{" "}
                  <span style={{ color: "hsl(var(--foreground))", fontWeight: 600 }}>
                    {avgWpm} WPM
                  </span>
                  {" "}avg pace.
                </>
              )}
            </>
          ) : recordings > 0 ? (
            <>
              {totalWords.toLocaleString()} polished words ·{" "}
              <span style={{ color: "hsl(var(--foreground))", fontWeight: 600 }}>
                +{todayWords} today
              </span>
            </>
          ) : (
            <>
              Hold{" "}
              <span
                className="inline-flex items-center px-2 py-0.5 rounded-md font-bold"
                style={{
                  background: "hsl(var(--primary) / 0.20)",
                  color:      "hsl(var(--primary))",
                  fontSize: 12,
                }}
              >
                ⇪ Caps Lock
              </span>
              {" "}to record your first.
            </>
          )}
        </p>
      </div>
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   2) DonutCard — center number ALWAYS readable (uses --foreground), track
      darker in light mode (uses --muted-foreground at 0.18 alpha so it's
      clearly visible on white). Status pill in header (no overlap).
   ════════════════════════════════════════════════════════════════════════════ */

export function DonutCard({
  snapshot,
  isProcessing,
  isRecording,
}: {
  snapshot:     AppSnapshot | null;
  isProcessing?: boolean;
  isRecording?:  boolean;
}) {
  const history    = snapshot?.history ?? [];
  const totalWords = snapshot?.total_words ?? 0;
  const goal       = 50_000;
  const pct        = Math.min(100, (totalWords / goal) * 100);

  const start = new Date();
  start.setHours(0, 0, 0, 0);
  const todayWords = history
    .filter((h) => h.timestamp_ms >= start.getTime())
    .reduce((s, h) => s + h.word_count, 0);

  const SIZE   = 200;
  const STROKE = 14;
  const R      = (SIZE - STROKE) / 2;
  const C      = 2 * Math.PI * R;
  const dash   = (pct / 100) * C;

  const statusLabel =
    isRecording  ? "RECORDING" :
    isProcessing ? "POLISHING" :
                   "IDLE";
  const statusColor =
    isRecording  ? "hsl(var(--recording))" :
    isProcessing ? "hsl(var(--accent-violet))" :
                   "hsl(var(--muted-foreground))";

  return (
    <div className="panel relative p-6 h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <p className="text-[14px] font-semibold text-foreground">Total weight</p>
          <span
            className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[9.5px] font-bold tracking-wide tabular-nums"
            style={{
              background: "hsl(var(--surface-4))",
              color:      statusColor,
            }}
          >
            <span
              className={isRecording || isProcessing ? "animate-pulse" : ""}
              style={{
                display: "inline-block", width: 5, height: 5, borderRadius: "50%",
                background: statusColor,
              }}
            />
            {statusLabel}
          </span>
        </div>
        <button
          className="w-7 h-7 rounded-full flex items-center justify-center transition-colors"
          style={{ background: "hsl(var(--surface-4))", color: "hsl(var(--muted-foreground))" }}
        >
          <Settings2 size={12} />
        </button>
      </div>

      {/* Donut */}
      <div className="flex-1 flex items-center justify-center relative" style={{ minHeight: 220 }}>
        <svg width={SIZE} height={SIZE} className="-rotate-90 absolute inset-0 m-auto">
          <defs>
            <linearGradient id="donutGrad" x1="0" y1="0" x2={SIZE} y2={SIZE} gradientUnits="userSpaceOnUse">
              <stop offset="0%"   stopColor="hsl(var(--accent-violet))" stopOpacity="1" />
              <stop offset="100%" stopColor="hsl(var(--primary))"        stopOpacity="1" />
            </linearGradient>
          </defs>
          {/* Track — uses muted-foreground so it's visible on both bg colors */}
          <circle
            cx={SIZE / 2} cy={SIZE / 2} r={R}
            fill="none"
            stroke="hsl(var(--muted-foreground) / 0.18)"
            strokeWidth={STROKE}
          />
          {/* Progress */}
          <circle
            cx={SIZE / 2} cy={SIZE / 2} r={R}
            fill="none"
            stroke="url(#donutGrad)"
            strokeWidth={STROKE}
            strokeLinecap="round"
            strokeDasharray={`${dash} ${C}`}
            style={{ transition: "stroke-dasharray 0.7s ease" }}
          />
        </svg>

        {/* Centre stat — uses --foreground so always visible. z-10 to sit above SVG. */}
        <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none z-10">
          <p
            className="font-bold tabular-nums leading-none tracking-tight"
            style={{
              fontSize: 36,
              color: "hsl(var(--foreground))",
            }}
          >
            {totalWords.toLocaleString()}
          </p>
          <p className="text-[10.5px] mt-1.5 uppercase tracking-[0.12em] font-bold"
             style={{ color: "hsl(var(--muted-foreground))" }}>
            words polished
          </p>
        </div>
      </div>

      {/* Footer */}
      <div className="text-[11.5px] mt-3 flex items-center justify-between">
        <span style={{ color: "hsl(var(--muted-foreground))" }}>
          {Math.round(pct)}% of {(goal / 1000).toFixed(0)}k goal
        </span>
        <span className="tabular-nums font-semibold" style={{ color: "hsl(var(--primary))" }}>
          +{todayWords.toLocaleString()} today
        </span>
      </div>
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   3) TimeSavedCard — minutes saved by dictating instead of typing.
      Big mint number + breakdown stats. Uses real DB data:
        words polished × (1/typing_wpm − 1/dictating_wpm) = minutes saved.
   ════════════════════════════════════════════════════════════════════════════ */

const TYPING_WPM = 40;  // industry-average sustained typing speed

function formatMinutes(min: number): { value: string; unit: string } {
  if (min < 1)    return { value: "0",                         unit: "min" };
  if (min < 60)   return { value: `${min}`,                    unit: "min" };
  if (min < 600)  {
    const h = Math.floor(min / 60), m = min % 60;
    return { value: m === 0 ? `${h}` : `${h}h ${m}`,           unit: m === 0 ? "hours" : "m"   };
  }
  const h = Math.round(min / 60);
  return { value: `${h}`, unit: "hours" };
}

export function TimeSavedCard({ snapshot }: { snapshot: AppSnapshot | null }) {
  const history    = snapshot?.history ?? [];
  const dictWpm    = snapshot?.avg_wpm ?? 0;
  const totalWords = snapshot?.total_words ?? 0;

  // Words this week (last 7 days)
  const weekStart      = Date.now() - 7 * 86_400_000;
  const wordsThisWeek  = history
    .filter((h) => h.timestamp_ms >= weekStart)
    .reduce((s, h) => s + h.word_count, 0);

  // Use weekly if there's recent activity, else fall back to all-time totals
  const useWeek      = wordsThisWeek > 0;
  const wordsForCalc = useWeek ? wordsThisWeek : totalWords;

  // Effective dictation WPM (use rolling avg or sane fallback)
  const effectiveDictWpm = dictWpm > 0 ? dictWpm : 120;

  // Time saved = (typing-time) − (dictating-time) in minutes
  const minutesSaved = Math.max(
    0,
    Math.round(
      wordsForCalc / TYPING_WPM - wordsForCalc / effectiveDictWpm,
    ),
  );

  const multiplier = (effectiveDictWpm / TYPING_WPM).toFixed(1);
  const f = formatMinutes(minutesSaved);

  return (
    <div className="panel relative p-6 h-full flex flex-col overflow-hidden">
      {/* Subtle mint glow bottom-right */}
      <div
        aria-hidden
        className="absolute pointer-events-none"
        style={{
          right: -100, bottom: -100, width: 260, height: 260, borderRadius: "50%",
          background: "radial-gradient(circle, hsl(var(--primary) / 0.16) 0%, transparent 70%)",
        }}
      />

      {/* Header */}
      <div className="relative flex items-center justify-between mb-4">
        <p className="text-[14px] font-semibold text-foreground">Time saved</p>
        <span
          className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[9.5px] font-bold tracking-wide"
          style={{
            background: "hsl(var(--surface-4))",
            color:      "hsl(var(--muted-foreground))",
          }}
        >
          {useWeek ? "THIS WEEK" : "ALL TIME"}
        </span>
      </div>

      {/* Big number */}
      <div className="relative flex-1 flex flex-col justify-center">
        <p
          className="font-bold tabular-nums leading-none tracking-tight"
          style={{
            fontSize: 52,
            color:    "hsl(var(--primary))",
            letterSpacing: "-0.02em",
          }}
        >
          {f.value}
          <span className="text-[20px] ml-1.5"
                style={{ color: "hsl(var(--primary) / 0.7)", fontWeight: 600 }}>
            {f.unit}
          </span>
        </p>
        <p className="text-[10.5px] mt-2 uppercase tracking-[0.12em] font-bold"
           style={{ color: "hsl(var(--muted-foreground))" }}>
          vs typing at {TYPING_WPM} WPM
        </p>
      </div>

      {/* Stats list */}
      <div className="relative space-y-2 text-[12px] mt-3">
        <div className="flex items-center justify-between">
          <span style={{ color: "hsl(var(--muted-foreground))" }}>Words polished</span>
          <span className="font-semibold tabular-nums"
                style={{ color: "hsl(var(--foreground))" }}>
            {wordsForCalc.toLocaleString()}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span style={{ color: "hsl(var(--muted-foreground))" }}>Your pace</span>
          <span className="tabular-nums"
                style={{ color: "hsl(var(--foreground))" }}>
            <span className="font-semibold">{effectiveDictWpm}</span>
            <span className="opacity-50 ml-1">vs {TYPING_WPM} typing</span>
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span style={{ color: "hsl(var(--muted-foreground))" }}>Speed</span>
          <span className="font-bold tabular-nums"
                style={{ color: "hsl(var(--primary))" }}>
            ≈ {multiplier}× faster
          </span>
        </div>
      </div>
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   4) RecordingsTable — Mytasky tasks-list pattern.
      Takes full Recording[] (with .id) so the play button can fetch audio
      via getRecordingAudioUrl, identical to the History view.
   ════════════════════════════════════════════════════════════════════════════ */

function modelLabel(model: string): string {
  if (model.includes("mini"))   return "Fast";
  if (model.includes("claude")) return "Claude";
  if (model.includes("gemini")) return "Gemini";
  return "Smart";
}

function relTime(ms: number): string {
  const diff = Date.now() - ms;
  const min  = Math.floor(diff / 60_000);
  if (min < 1)  return "just now";
  if (min < 60) return `${min}m ago`;
  const hr   = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const d = Math.floor(hr / 24);
  if (d === 1) return "yesterday";
  if (d < 7)   return `${d}d ago`;
  return new Date(ms).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function RecordingsTable({
  recordings, onSeeAll,
}: {
  recordings: Recording[];
  onSeeAll:   () => void;
}) {
  const items = recordings.slice(0, 4);
  const { playingId, play } = useAudioPlayer();

  return (
    <div className="panel p-6">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2.5">
          <h3 className="text-[16px] font-bold tracking-tight"
              style={{ color: "hsl(var(--foreground))" }}>
            Recordings list
          </h3>
          <span style={{ color: "hsl(var(--muted-foreground) / 0.4)" }}>|</span>
          <button
            className="flex items-center gap-1 text-[12.5px] font-medium transition-colors"
            style={{ color: "hsl(var(--muted-foreground))" }}
            onMouseEnter={(e) => { e.currentTarget.style.color = "hsl(var(--foreground))"; }}
            onMouseLeave={(e) => { e.currentTarget.style.color = "hsl(var(--muted-foreground))"; }}
          >
            <Filter size={12} />
            Filter
          </button>
        </div>
        <button
          onClick={onSeeAll}
          className="flex items-center gap-1 text-[12.5px] font-medium transition-colors"
          style={{ color: "hsl(var(--muted-foreground))" }}
          onMouseEnter={(e) => { e.currentTarget.style.color = "hsl(var(--foreground))"; }}
          onMouseLeave={(e) => { e.currentTarget.style.color = "hsl(var(--muted-foreground))"; }}
        >
          See all
          <ChevronRight size={12} />
        </button>
      </div>

      {/* Column headers */}
      <div
        className="grid items-center gap-4 py-2 text-[11px] font-medium uppercase tracking-wider"
        style={{
          gridTemplateColumns: "1fr 110px 110px 100px 90px",
          color: "hsl(var(--muted-foreground) / 0.7)",
        }}
      >
        <span>Polished text</span>
        <span>Status</span>
        <span>When</span>
        <span className="text-right">Words</span>
        <span className="text-right">Play</span>
      </div>

      {items.length === 0 ? (
        <div className="py-10 text-center">
          <p className="text-[12.5px]" style={{ color: "hsl(var(--muted-foreground))" }}>
            Press <span className="font-semibold" style={{ color: "hsl(var(--foreground))" }}>⇪ Caps Lock</span>
            {" "}to record. Recent recordings appear here.
          </p>
        </div>
      ) : (
        <div className="flex flex-col">
          {items.map((rec, i) => (
            <Row
              key={rec.id}
              rec={rec}
              live={i === 0}
              last={i === items.length - 1}
              isPlaying={playingId === rec.id}
              onPlay={() => play(rec.id, rec.audio_id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function Row({
  rec, live, last, isPlaying, onPlay,
}: {
  rec:        Recording;
  live:       boolean;
  last:       boolean;
  isPlaying:  boolean;
  onPlay:     () => void;
}) {
  const firstDot = rec.polished.search(/[.?!]/);
  const title    = firstDot > 0 ? rec.polished.slice(0, firstDot + 1) : rec.polished;
  const model    = modelLabel(rec.model_used);

  // Status chip
  const isRecent = Date.now() - rec.timestamp_ms < 5 * 60_000;
  const chipBg   = isPlaying
                   ? "hsl(var(--primary) / 0.18)"
                   : (live || isRecent ? "hsl(var(--chip-violet-bg))" : "hsl(var(--chip-mint-bg))");
  const chipFg   = isPlaying
                   ? "hsl(var(--primary))"
                   : (live || isRecent ? "hsl(var(--chip-violet-fg))" : "hsl(var(--chip-mint-fg))");
  const chipText = isPlaying ? "Playing…" : live ? "Latest" : isRecent ? "Recent" : "Polished";

  // Copy state
  const [copied, setCopied] = useState(false);
  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(rec.polished);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch { /* ignore */ }
  };

  const canPlay = Boolean(rec.audio_id);

  // Play button visual state
  const playBg = isPlaying
    ? "hsl(var(--primary) / 0.18)"
    : !canPlay
    ? "hsl(var(--surface-4))"
    : live
    ? "hsl(var(--accent-violet))"
    : "hsl(var(--surface-4))";
  const playFg = isPlaying
    ? "hsl(var(--primary))"
    : !canPlay
    ? "hsl(var(--muted-foreground) / 0.5)"
    : live
    ? "white"
    : "hsl(var(--muted-foreground))";
  const playShadow = !isPlaying && live && canPlay
    ? "0 4px 12px hsl(var(--accent-violet) / 0.40)"
    : "none";

  return (
    <div
      className="grid items-center gap-4 py-3.5 group"
      style={{
        gridTemplateColumns: "1fr 110px 110px 100px 90px",
        borderBottom: last ? "none" : "1px dashed hsl(var(--surface-4))",
      }}
    >
      <div className="flex items-center gap-2 min-w-0">
        <span
          className="text-[13.5px] font-medium leading-snug truncate"
          style={{ color: "hsl(var(--foreground))" }}
        >
          {title}
        </span>
        {/* Copy button — appears on row hover */}
        <button
          onClick={handleCopy}
          title={copied ? "Copied!" : "Copy polished text"}
          className="w-6 h-6 rounded-md flex items-center justify-center flex-shrink-0 transition-all opacity-0 group-hover:opacity-100"
          style={{
            background: copied ? "hsl(var(--primary) / 0.18)" : "transparent",
            color:      copied ? "hsl(var(--primary))" : "hsl(var(--muted-foreground))",
          }}
        >
          {copied ? <Check size={11} strokeWidth={2.5} /> : <Copy size={11} />}
        </button>
        {live && !isPlaying && (
          <span
            className="text-[11px] flex-shrink-0"
            title="Most recent"
            style={{ color: "hsl(var(--accent-violet))" }}
          >
            ●
          </span>
        )}
      </div>

      <div>
        <span
          className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-semibold tabular-nums"
          style={{ background: chipBg, color: chipFg }}
        >
          {isPlaying && (
            <span
              className="inline-block w-1.5 h-1.5 rounded-full animate-pulse"
              style={{ background: "currentColor" }}
            />
          )}
          {chipText}
        </span>
      </div>

      <span className="text-[12.5px] tabular-nums"
            style={{ color: "hsl(var(--foreground))" }}>
        {relTime(rec.timestamp_ms)}
      </span>

      <span className="text-[12.5px] tabular-nums text-right font-semibold"
            style={{ color: "hsl(var(--foreground))" }}>
        {rec.word_count}
        <span className="text-[10px] ml-0.5"
              style={{ color: "hsl(var(--muted-foreground))" }}>
          · {model}
        </span>
      </span>

      <div className="flex justify-end">
        <button
          onClick={onPlay}
          disabled={!canPlay}
          title={
            !canPlay   ? "Audio not available"
            : isPlaying ? "Pause"
            :             "Play recording"
          }
          className="w-8 h-8 rounded-full flex items-center justify-center transition-all"
          style={{
            background: playBg,
            color:      playFg,
            boxShadow:  playShadow,
            cursor:     canPlay ? "pointer" : "not-allowed",
          }}
        >
          {isPlaying ? (
            <Pause size={11} fill="currentColor" strokeWidth={0} />
          ) : (
            <Play size={11} fill="currentColor" strokeWidth={0} style={{ marginLeft: 1 }} />
          )}
        </button>
      </div>
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   5) ActivityHeatmap — GitHub-style contribution graph with circular dots.
      Reference: "Test Generation Activity" with mint gradient + month labels
      across the top + "Daily avg" + More/Less legend at the bottom.
   ════════════════════════════════════════════════════════════════════════════ */

const MONTH_LABELS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                      "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

function wordsToLevel(words: number, max: number): 0 | 1 | 2 | 3 | 4 {
  if (words === 0)        return 0;
  if (words < max * 0.25) return 1;
  if (words < max * 0.50) return 2;
  if (words < max * 0.75) return 3;
  return 4;
}

export function ActivityHeatmap({
  snapshot,
  isRecording,
  isProcessing,
  onToggle,
  onView,
}: {
  snapshot:     AppSnapshot | null;
  isRecording:  boolean;
  isProcessing: boolean;
  onToggle:     () => void;
  onView:       () => void;
}) {
  const history = snapshot?.history ?? [];

  // Bucket history by calendar day (UTC-day index)
  const dayMap = new Map<number, number>();
  for (const h of history) {
    const d = Math.floor(h.timestamp_ms / 86_400_000);
    dayMap.set(d, (dayMap.get(d) ?? 0) + h.word_count);
  }

  // Build a 26-week window ending today (≈6 months)
  const COLS    = 26;
  const ROWS    = 7;
  const today   = new Date();
  today.setHours(0, 0, 0, 0);
  const todayIdx = Math.floor(today.getTime() / 86_400_000);
  const todayDow = today.getDay(); // 0=Sun … 6=Sat

  // Anchor on the most recent Sunday so columns align to weeks
  const lastSundayIdx = todayIdx - todayDow;
  const startIdx      = lastSundayIdx - (COLS - 1) * 7;

  // Find peak in window for normalization
  let max = 1;
  for (let c = 0; c < COLS; c++) {
    for (let r = 0; r < ROWS; r++) {
      const idx = startIdx + c * 7 + r;
      const w   = dayMap.get(idx) ?? 0;
      if (w > max) max = w;
    }
  }
  // Apply a soft floor so a single big day doesn't flatten the rest
  max = Math.max(max, 30);

  // Per-day breakdown for stats
  const inWindow: number[] = [];
  for (let c = 0; c < COLS; c++) {
    for (let r = 0; r < ROWS; r++) {
      const idx = startIdx + c * 7 + r;
      if (idx > todayIdx) continue;
      inWindow.push(dayMap.get(idx) ?? 0);
    }
  }
  const totalWords = inWindow.reduce((s, w) => s + w, 0);
  const activeDays = inWindow.filter((w) => w > 0).length || 1;
  const dailyAvg   = Math.round(totalWords / activeDays);

  // Build column → month label map
  const colMonthLabel: (string | null)[] = Array(COLS).fill(null);
  let lastMonth = -1;
  for (let c = 0; c < COLS; c++) {
    const colStartIdx = startIdx + c * 7;
    const d           = new Date(colStartIdx * 86_400_000);
    const m           = d.getMonth();
    if (m !== lastMonth) {
      colMonthLabel[c] = MONTH_LABELS[m];
      lastMonth        = m;
    }
  }

  return (
    <div className="panel relative p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-5">
        <div>
          <h3 className="text-[16px] font-bold tracking-tight"
              style={{ color: "hsl(var(--foreground))" }}>
            Activity
          </h3>
          <p className="text-[11.5px] mt-0.5"
             style={{ color: "hsl(var(--muted-foreground))" }}>
            Words polished per day
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            className="flex items-center gap-1.5 px-3 h-8 rounded-full text-[12px] font-medium transition-colors"
            style={{
              background: "hsl(var(--surface-4))",
              color:      "hsl(var(--muted-foreground))",
            }}
          >
            Last 6 months
            <CaretDown size={11} />
          </button>
          <button
            onClick={onToggle}
            disabled={isProcessing}
            className="px-4 h-8 rounded-full text-[12px] font-semibold transition-all flex items-center gap-1.5"
            style={{
              background: isRecording
                ? "hsl(var(--recording))"
                : "hsl(var(--accent-violet))",
              color: "white",
              boxShadow: isRecording
                ? "0 4px 14px hsl(var(--recording) / 0.50)"
                : "0 4px 14px hsl(var(--accent-violet) / 0.40)",
              opacity: isProcessing ? 0.6 : 1,
              cursor: isProcessing ? "not-allowed" : "pointer",
            }}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${isRecording ? "orb-recording" : ""}`}
              style={{ background: "white" }}
            />
            {isRecording ? "Stop" : isProcessing ? "Working…" : "Record"}
          </button>
          <button
            onClick={onView}
            className="px-3 h-8 rounded-full text-[12px] font-semibold transition-colors"
            style={{
              background: "transparent",
              color:      "hsl(var(--muted-foreground))",
              boxShadow:  "inset 0 0 0 1px hsl(var(--surface-4))",
            }}
            onMouseEnter={(e) => { e.currentTarget.style.color = "hsl(var(--foreground))"; }}
            onMouseLeave={(e) => { e.currentTarget.style.color = "hsl(var(--muted-foreground))"; }}
          >
            View all
          </button>
        </div>
      </div>

      {/* Month labels strip */}
      <div
        className="grid mb-2"
        style={{
          gridTemplateColumns: `repeat(${COLS}, minmax(0, 1fr))`,
          columnGap: 4,
        }}
      >
        {colMonthLabel.map((label, i) => (
          <span
            key={i}
            className="text-[10.5px] font-medium tabular-nums"
            style={{
              color: "hsl(var(--muted-foreground))",
              opacity: label ? 1 : 0,
              minHeight: 14,
            }}
          >
            {label ?? ""}
          </span>
        ))}
      </div>

      {/* Heatmap grid — 7 rows × 26 cols of circles */}
      <div
        className="grid"
        style={{
          gridTemplateColumns: `repeat(${COLS}, minmax(0, 1fr))`,
          gridTemplateRows:    `repeat(${ROWS}, minmax(0, 1fr))`,
          gridAutoFlow:        "column",
          columnGap: 4,
          rowGap:    4,
        }}
      >
        {Array.from({ length: COLS * ROWS }).map((_, i) => {
          const c   = Math.floor(i / ROWS);
          const r   = i % ROWS;
          const idx = startIdx + c * 7 + r;
          const future = idx > todayIdx;
          const words  = future ? 0 : (dayMap.get(idx) ?? 0);
          const level  = future ? 0 : wordsToLevel(words, max);
          const isToday = idx === todayIdx;
          const date    = new Date(idx * 86_400_000);
          const tip = future
            ? ""
            : `${words} word${words === 1 ? "" : "s"} on ${date.toLocaleDateString("en-US", { month: "short", day: "numeric" })}`;
          return (
            <span
              key={i}
              title={tip}
              className={`rounded-full transition-transform ${isToday ? "heat-current" : `heat-${level}`}`}
              style={{
                aspectRatio: "1 / 1",
                opacity: future ? 0.18 : 1,
                cursor:  future ? "default" : "default",
              }}
            />
          );
        })}
      </div>

      {/* Footer — daily avg + legend */}
      <div className="flex items-center justify-between mt-5">
        <p className="text-[12px] font-medium"
           style={{ color: "hsl(var(--muted-foreground))" }}>
          Daily avg:{" "}
          <span style={{ color: "hsl(var(--foreground))", fontWeight: 600 }}>
            {dailyAvg.toLocaleString()} words / day
          </span>
        </p>

        <div className="flex items-center gap-1.5 text-[10.5px]"
             style={{ color: "hsl(var(--muted-foreground))" }}>
          <span>Less</span>
          {([0, 1, 2, 3, 4] as const).map((l) => (
            <span
              key={l}
              className={`heat-${l} rounded-full`}
              style={{ width: 10, height: 10 }}
            />
          ))}
          <span>More</span>
        </div>
      </div>
    </div>
  );
}
