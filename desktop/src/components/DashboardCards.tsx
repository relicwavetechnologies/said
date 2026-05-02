import React, { useState, useEffect, useRef } from "react";
import {
  ChevronUp, ChevronDown, ChevronRight,
  Filter, Play, Pause, Check, Copy, Download,
  ChevronDown as CaretDown,
  Search,
  Mic, Zap, Sparkles, Database, FileText, Send, Activity,
  CircleCheck, AlertCircle,
} from "lucide-react";
import { useAudioPlayer } from "@/lib/useAudioPlayer";
import { downloadRecordingAudio } from "@/lib/invoke";
import type { AppSnapshot, Recording } from "@/types";

/* ════════════════════════════════════════════════════════════════════════════
   Sentinel-inspired stat tile primitives.
   All hero cards share: bold title, ··· menu, tiny grey context label,
   GIANT tabular number, and a small green ▲ delta chip.
   ════════════════════════════════════════════════════════════════════════════ */

function DeltaChip({
  value, suffix = "%", neutral, color = "mint",
}: {
  value: number;
  suffix?: string;
  neutral?: boolean;
  color?: "mint" | "blue" | "amber";
}) {
  const isPositive = value > 0;
  const isZero     = value === 0 || neutral;
  const sign       = isZero ? "" : isPositive ? "+" : "";
  const colorMap = {
    mint:  { bg: "hsl(var(--chip-mint-bg))",  fg: "hsl(var(--chip-mint-fg))"  },
    blue:  { bg: "hsl(var(--chip-blue-bg))",  fg: "hsl(var(--chip-blue-fg))"  },
    amber: { bg: "hsl(var(--chip-amber-bg))", fg: "hsl(var(--chip-amber-fg))" },
  };
  const c = isZero
    ? { bg: "hsl(var(--surface-4))", fg: "hsl(var(--muted-foreground))" }
    : colorMap[color];
  return (
    <span
      className="inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-semibold tabular-nums"
      style={{ background: c.bg, color: c.fg, lineHeight: 1.4 }}
    >
      {sign}{value.toLocaleString()}{suffix}
    </span>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   StatTile — uniform compact card body.
   Layout: title (+optional status) + ··· menu / subtitle / NUMBER + chip
   No `mt-auto`, no `flex-1` — natural top-down flow so all cards in a
   row size to the same content height (no awkward empty space).
   ════════════════════════════════════════════════════════════════════════════ */

interface StatTileProps {
  title:     string;
  subtitle:  string;
  value:     React.ReactNode;       // big number, tabular-nums leaf
  delta?:    React.ReactNode;       // <DeltaChip /> or null
  status?:   { label: string; pulse?: boolean } | null;
}

function StatTile({ title, subtitle, value, delta, status }: StatTileProps) {
  return (
    <div className="panel px-5 pt-4 pb-5">
      {/* Title row */}
      <div className="flex items-center gap-2 min-w-0">
        <p className="text-[13px] font-bold tracking-tight truncate"
           style={{ color: "hsl(var(--foreground))" }}>
          {title}
        </p>
        {status && (
          <span
            className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-[9px] font-bold tabular-nums flex-shrink-0"
            style={{
              background: "hsl(var(--primary) / 0.14)",
              color:      "hsl(var(--primary))",
            }}
          >
            <span
              className={`inline-block w-1 h-1 rounded-full ${status.pulse ? "animate-pulse" : ""}`}
              style={{ background: "currentColor" }}
            />
            {status.label}
          </span>
        )}
      </div>

      {/* Subtitle — tight under title */}
      <p className="text-[11.5px] mt-0.5" style={{ color: "hsl(var(--muted-foreground))" }}>
        {subtitle}
      </p>

      {/* Number + delta — generous top space, no extra padding below */}
      <div className="flex items-baseline gap-2 mt-4 flex-wrap">
        <span
          className="font-bold tabular-nums leading-none tracking-tight"
          style={{
            fontSize: 28,
            color:    "hsl(var(--foreground))",
            letterSpacing: "-0.02em",
          }}
        >
          {value}
        </span>
        {delta}
      </div>
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   1) HeroStat — "Recordings" total + week-over-week delta.
   ════════════════════════════════════════════════════════════════════════════ */

export function HeroStat({ snapshot }: { snapshot: AppSnapshot | null }) {
  const history    = snapshot?.history ?? [];
  const recordings = history.length;

  const now      = Date.now();
  const D7       = 7 * 86_400_000;
  const last7    = history.filter((h) => h.timestamp_ms >= now - D7).length;
  const prev7    = history.filter((h) => h.timestamp_ms >= now - 2 * D7 && h.timestamp_ms < now - D7).length;
  const deltaPct = prev7 > 0
    ? Math.round(((last7 - prev7) / prev7) * 100)
    : last7 > 0 ? 100 : 0;

  return (
    <StatTile
      title="Recordings"
      subtitle="Last 30 days, all sessions"
      value={recordings.toLocaleString()}
      delta={<DeltaChip value={deltaPct} />}
    />
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   2) DonutCard — total words polished. Donut visualization dropped so the
      card matches the others' compact natural height.
   ════════════════════════════════════════════════════════════════════════════ */

export function DonutCard({
  snapshot,
  isProcessing,
  isRecording,
}: {
  snapshot:      AppSnapshot | null;
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

  const status = isRecording  ? { label: "REC",       pulse: true }
               : isProcessing ? { label: "POLISHING", pulse: true }
               : null;

  return (
    <StatTile
      title="Words polished"
      subtitle={`${Math.round(pct)}% of ${(goal / 1000).toFixed(0)}k goal`}
      value={totalWords.toLocaleString()}
      delta={<DeltaChip value={todayWords} suffix="" neutral={todayWords === 0} />}
      status={status}
    />
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   3) TimeSavedCard — minutes saved by dictating instead of typing.
   ════════════════════════════════════════════════════════════════════════════ */

const TYPING_WPM = 40;

function formatMinutes(min: number): { value: string; unit: string } {
  if (min < 1)    return { value: "0", unit: "min" };
  if (min < 60)   return { value: `${min}`, unit: "min" };
  const h = Math.floor(min / 60);
  const m = min % 60;
  return { value: m === 0 ? `${h}` : `${h}h ${m}`, unit: m === 0 ? "h" : "m" };
}

export function TimeSavedCard({ snapshot }: { snapshot: AppSnapshot | null }) {
  const history    = snapshot?.history ?? [];
  const dictWpm    = snapshot?.avg_wpm ?? 0;
  const totalWords = snapshot?.total_words ?? 0;

  const weekStart      = Date.now() - 7 * 86_400_000;
  const wordsThisWeek  = history
    .filter((h) => h.timestamp_ms >= weekStart)
    .reduce((s, h) => s + h.word_count, 0);

  const useWeek          = wordsThisWeek > 0;
  const wordsForCalc     = useWeek ? wordsThisWeek : totalWords;
  const effectiveDictWpm = dictWpm > 0 ? dictWpm : 120;
  const minutesSaved     = Math.max(
    0,
    Math.round(wordsForCalc / TYPING_WPM - wordsForCalc / effectiveDictWpm),
  );
  const multiplier = effectiveDictWpm / TYPING_WPM;
  const f          = formatMinutes(minutesSaved);

  return (
    <StatTile
      title="Time saved"
      subtitle={useWeek ? "Last 7 days, vs typing at 40 WPM" : "All time, vs typing at 40 WPM"}
      value={
        <>
          {f.value}
          <span className="text-[13px] ml-1"
                style={{ color: "hsl(var(--muted-foreground))", fontWeight: 600 }}>
            {f.unit}
          </span>
        </>
      }
      delta={<DeltaChip value={Number(multiplier.toFixed(1))} suffix="×" neutral={multiplier <= 1} />}
    />
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   4) PaceCard — average WPM, showing dictation speed at a glance.
   ════════════════════════════════════════════════════════════════════════════ */

export function PaceCard({ snapshot }: { snapshot: AppSnapshot | null }) {
  const wpm = snapshot?.avg_wpm ?? 0;
  // Delta vs typical typing speed (40 WPM) — % faster
  const deltaPct = wpm > 0 ? Math.round(((wpm - TYPING_WPM) / TYPING_WPM) * 100) : 0;

  return (
    <StatTile
      title="Avg pace"
      subtitle="Rolling 10-recording WPM"
      value={
        <>
          {wpm || "—"}
          {wpm > 0 && (
            <span className="text-[13px] ml-1"
                  style={{ color: "hsl(var(--muted-foreground))", fontWeight: 600 }}>
              WPM
            </span>
          )}
        </>
      }
      delta={wpm > 0 ? <DeltaChip value={deltaPct} /> : null}
    />
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   4) RecordingsTable — clean white table, mint accents, dotted dividers.
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

type RecordingsFilter = "all" | "today" | "week" | "month";
const FILTER_LABEL: Record<RecordingsFilter, string> = {
  all:   "All time",
  today: "Today",
  week:  "This week",
  month: "This month",
};

function audioFilename(recording: Recording): string {
  const d     = new Date(recording.timestamp_ms);
  const pad   = (n: number) => String(n).padStart(2, "0");
  const stamp = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}`;
  return `said-${stamp}-${recording.word_count}-words.wav`;
}

export function RecordingsTable({
  recordings, onSeeAll, onDownloadSuccess,
}: {
  recordings: Recording[];
  onSeeAll:   () => void;
  onDownloadSuccess?: (path: string) => void;
}) {
  const { playingId, play } = useAudioPlayer();

  const [filter,    setFilter]    = useState<RecordingsFilter>("all");
  const [filterOpen, setFilterOpen] = useState(false);
  const filterRef = useRef<HTMLDivElement>(null);

  // Outside-click + escape close
  useEffect(() => {
    if (!filterOpen) return;
    const onDown = (e: MouseEvent) => {
      if (filterRef.current && !filterRef.current.contains(e.target as Node)) setFilterOpen(false);
    };
    const onEsc = (e: KeyboardEvent) => { if (e.key === "Escape") setFilterOpen(false); };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onEsc);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onEsc);
    };
  }, [filterOpen]);

  // Apply the active filter — narrow the recordings list before slicing
  const filtered = (() => {
    if (filter === "all") return recordings;
    const now = new Date();
    let start: number;
    if (filter === "today") {
      const t = new Date(now); t.setHours(0, 0, 0, 0);
      start = t.getTime();
    } else if (filter === "week") {
      start = now.getTime() - 7 * 86_400_000;
    } else {
      start = now.getTime() - 30 * 86_400_000;
    }
    return recordings.filter((r) => r.timestamp_ms >= start);
  })();

  const items = filtered.slice(0, 4);

  return (
    <div className="panel p-5">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2.5">
          <h3 className="text-[15px] font-bold tracking-tight"
              style={{ color: "hsl(var(--foreground))" }}>
            Recordings list
          </h3>
          <span style={{ color: "hsl(var(--muted-foreground) / 0.4)" }}>|</span>

          {/* Filter dropdown */}
          <div ref={filterRef} className="relative">
            <button
              onClick={() => setFilterOpen((o) => !o)}
              className="flex items-center gap-1 text-[12.5px] font-medium transition-colors"
              style={{
                color: filter === "all"
                  ? "hsl(var(--muted-foreground))"
                  : "hsl(var(--primary))",
              }}
              onMouseEnter={(e) => {
                if (filter === "all") e.currentTarget.style.color = "hsl(var(--foreground))";
              }}
              onMouseLeave={(e) => {
                if (filter === "all") e.currentTarget.style.color = "hsl(var(--muted-foreground))";
              }}
            >
              <Filter size={12} />
              {filter === "all" ? "Filter" : FILTER_LABEL[filter]}
              <CaretDown
                size={10}
                style={{ transition: "transform 0.15s", transform: filterOpen ? "rotate(180deg)" : "none" }}
              />
            </button>
            {filterOpen && (
              <div
                className="absolute left-0 top-full mt-1 z-30 rounded-md py-1 min-w-[140px]"
                style={{
                  background: "hsl(var(--surface-3))",
                  boxShadow:
                    "inset 0 0 0 1px hsl(var(--border)), 0 8px 24px hsl(0 0% 0% / 0.12)",
                }}
              >
                {(Object.keys(FILTER_LABEL) as RecordingsFilter[]).map((k) => {
                  const active = filter === k;
                  return (
                    <button
                      key={k}
                      onClick={() => { setFilter(k); setFilterOpen(false); }}
                      className="w-full flex items-center justify-between gap-2 px-3 py-1.5 text-[12px] font-medium text-left transition-colors"
                      style={{
                        color:      active ? "hsl(var(--primary))" : "hsl(var(--foreground))",
                        background: active ? "hsl(var(--primary) / 0.08)" : "transparent",
                      }}
                      onMouseEnter={(e) => {
                        if (!active) e.currentTarget.style.background = "hsl(var(--surface-hover))";
                      }}
                      onMouseLeave={(e) => {
                        if (!active) e.currentTarget.style.background = "transparent";
                      }}
                    >
                      {FILTER_LABEL[k]}
                      {active && <Check size={11} strokeWidth={2.5} />}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
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
        className="grid items-center gap-4 py-2 text-[10.5px] font-semibold uppercase tracking-wider"
        style={{
          gridTemplateColumns: "1fr 110px 110px 100px 104px",
          color: "hsl(var(--muted-foreground))",
        }}
      >
        <span>Polished text</span>
        <span>Status</span>
        <span>When</span>
        <span className="text-right">Words</span>
        <span className="text-right">Audio</span>
      </div>

      {items.length === 0 ? (
        <div className="py-10 text-center">
          <p className="text-[12.5px]" style={{ color: "hsl(var(--muted-foreground))" }}>
            {filter === "all" ? (
              <>Press <span className="font-semibold" style={{ color: "hsl(var(--foreground))" }}>⇪ Caps Lock</span>
              {" "}to record. Recent recordings appear here.</>
            ) : (
              <>No recordings <span style={{ color: "hsl(var(--foreground))", fontWeight: 600 }}>
                {FILTER_LABEL[filter].toLowerCase()}</span>.{" "}
              <button
                onClick={() => setFilter("all")}
                className="underline"
                style={{ color: "hsl(var(--primary))" }}
              >
                Show all
              </button>
              </>
            )}
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
              onDownloadSuccess={onDownloadSuccess}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function Row({
  rec, live, last, isPlaying, onPlay, onDownloadSuccess,
}: {
  rec:        Recording;
  live:       boolean;
  last:       boolean;
  isPlaying:  boolean;
  onPlay:     () => void;
  onDownloadSuccess?: (path: string) => void;
}) {
  const firstDot = rec.polished.search(/[.?!]/);
  const title    = firstDot > 0 ? rec.polished.slice(0, firstDot + 1) : rec.polished;
  const model    = modelLabel(rec.model_used);

  const isRecent = Date.now() - rec.timestamp_ms < 5 * 60_000;
  const chipBg   = isPlaying
                   ? "hsl(var(--chip-mint-bg))"
                   : "hsl(var(--chip-mint-bg))";
  const chipFg   = "hsl(var(--chip-mint-fg))";
  const chipText = isPlaying ? "Playing…" : live ? "Latest" : isRecent ? "Recent" : "Polished";

  const [copied, setCopied] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(rec.polished);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch { /* ignore */ }
  };

  const canPlay = Boolean(rec.audio_id);
  const handleDownload = async () => {
    if (!rec.audio_id || downloading) return;
    setDownloading(true);
    try {
      const savedPath = await downloadRecordingAudio(rec.id, audioFilename(rec));
      if (savedPath) onDownloadSuccess?.(savedPath);
    } finally {
      setDownloading(false);
    }
  };

  // Sentinel-style: minimal — no fill, just hover/active states
  const playBg = isPlaying
    ? "hsl(var(--primary) / 0.14)"
    : "hsl(var(--surface-4))";
  const playFg = isPlaying
    ? "hsl(var(--primary))"
    : !canPlay
    ? "hsl(var(--muted-foreground) / 0.5)"
    : "hsl(var(--foreground))";

  return (
    <div
      className="grid items-center gap-4 py-3 group"
      style={{
        gridTemplateColumns: "1fr 110px 110px 100px 104px",
        borderBottom: last ? "none" : "1px dashed hsl(var(--border))",
      }}
    >
      <div className="flex items-center gap-2 min-w-0">
        <span
          className="text-[13.5px] font-medium leading-snug truncate"
          style={{ color: "hsl(var(--foreground))" }}
        >
          {title}
        </span>
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
            style={{ color: "hsl(var(--primary))" }}
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

      <div className="flex justify-end gap-1.5">
        <button
          onClick={handleDownload}
          disabled={!canPlay || downloading}
          title={!canPlay ? "Audio not available" : downloading ? "Saving..." : "Download audio"}
          className="w-8 h-8 rounded-full flex items-center justify-center transition-all"
          style={{
            background: downloading ? "hsl(var(--primary) / 0.14)" : "hsl(var(--surface-4))",
            color:      !canPlay ? "hsl(var(--muted-foreground) / 0.5)" : "hsl(var(--foreground))",
            cursor:     canPlay && !downloading ? "pointer" : "not-allowed",
          }}
        >
          {downloading ? (
            <span className="inline-block w-2 h-2 rounded-full bg-current animate-pulse" />
          ) : (
            <Download size={11} />
          )}
        </button>
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
   5) ActivityHeatmap — Sentinel-style: month labels across top, circular
      mint dots, "Daily avg" + More/Less legend in the footer.
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

/* Local-calendar day index — Math.floor(ms / DAY) gives a UTC day index, which
 * splits days mid-evening for IST users (UTC+5:30) and other east-of-UTC zones.
 * This helper rebuilds a date at the LOCAL midnight then floors that, so all
 * recordings made on the same local calendar date share the same index. */
function localDayIdx(ms: number): number {
  const d = new Date(ms);
  const localMidnight = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  return Math.floor(localMidnight / 86_400_000);
}

function SideStat({
  label, value, unit, highlight,
}: {
  label:      string;
  value:      number | string;
  unit?:      string;
  highlight?: boolean;
}) {
  return (
    <div>
      <p className="text-[10.5px] font-bold uppercase tracking-[0.14em]"
         style={{ color: "hsl(var(--muted-foreground))" }}>
        {label}
      </p>
      <p
        className="font-bold tabular-nums leading-none tracking-tight mt-1"
        style={{
          fontSize: 24,
          color: highlight ? "hsl(var(--primary))" : "hsl(var(--foreground))",
          letterSpacing: "-0.02em",
        }}
      >
        {typeof value === "number" ? value.toLocaleString() : value}
        {unit && (
          <span className="text-[11px] ml-1.5"
                style={{ color: "hsl(var(--muted-foreground))", fontWeight: 600 }}>
            {unit}
          </span>
        )}
      </p>
    </div>
  );
}

type HeatmapRange = "1m" | "3m" | "6m" | "12m";
const RANGE_COLS:  Record<HeatmapRange, number> = { "1m": 5,  "3m": 13, "6m": 26, "12m": 52 };
const RANGE_LABEL: Record<HeatmapRange, string> = {
  "1m":  "Last 30 days",
  "3m":  "Last 3 months",
  "6m":  "Last 6 months",
  "12m": "Last 12 months",
};

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

  const dayMap = new Map<number, number>();
  for (const h of history) {
    const d = localDayIdx(h.timestamp_ms);
    dayMap.set(d, (dayMap.get(d) ?? 0) + h.word_count);
  }

  // Range selector — drives column count and grid width
  const [range,    setRange]    = useState<HeatmapRange>("6m");
  const [rangeOpen, setRangeOpen] = useState(false);
  const rangeRef = useRef<HTMLDivElement>(null);

  // Hover tooltip state — { x/y in viewport, info text }
  const [hover, setHover] = useState<{ x: number; y: number; words: number; date: string } | null>(null);

  // Close range dropdown on outside click / esc
  useEffect(() => {
    if (!rangeOpen) return;
    const onDown = (e: MouseEvent) => {
      if (rangeRef.current && !rangeRef.current.contains(e.target as Node)) setRangeOpen(false);
    };
    const onEsc = (e: KeyboardEvent) => { if (e.key === "Escape") setRangeOpen(false); };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onEsc);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onEsc);
    };
  }, [rangeOpen]);

  const COLS    = RANGE_COLS[range];
  const ROWS    = 7;
  // Use the LOCAL day index (matches dayMap keys) — Math.floor of UTC ms
  // would give a UTC day, splitting IST evenings onto the wrong cell.
  const todayIdx = localDayIdx(Date.now());
  const todayDow = new Date().getDay();   // local day-of-week (0 = Sun)
  const lastSundayIdx = todayIdx - todayDow;
  const startIdx      = lastSundayIdx - (COLS - 1) * 7;

  // Fixed cell size keeps the section's HEIGHT constant when the user
  // toggles between 1m / 3m / 6m / 12m — the grid just gets narrower /
  // wider, the panel never grows or shrinks vertically.
  const CELL_PX = 18;

  let max = 1;
  for (let c = 0; c < COLS; c++) {
    for (let r = 0; r < ROWS; r++) {
      const idx = startIdx + c * 7 + r;
      const w   = dayMap.get(idx) ?? 0;
      if (w > max) max = w;
    }
  }
  max = Math.max(max, 30);

  // Build a real Date for any day-index, in LOCAL time (no UTC drift).
  function localDateFromIdx(idx: number): Date {
    const daysAgo = todayIdx - idx;
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    d.setDate(d.getDate() - daysAgo);
    return d;
  }

  const inWindow: number[] = [];
  for (let c = 0; c < COLS; c++) {
    for (let r = 0; r < ROWS; r++) {
      const idx = startIdx + c * 7 + r;
      if (idx > todayIdx) continue;
      inWindow.push(dayMap.get(idx) ?? 0);
    }
  }
  const totalWords     = inWindow.reduce((s, w) => s + w, 0);
  const activeDayCount = inWindow.filter((w) => w > 0).length;
  const totalDays      = inWindow.length;
  const dailyAvg       = activeDayCount > 0 ? Math.round(totalWords / activeDayCount) : 0;

  // Current streak — count back from today over consecutive non-zero days
  let streak = 0;
  for (let i = 0; i < totalDays; i++) {
    const idx = todayIdx - i;
    if ((dayMap.get(idx) ?? 0) > 0) streak += 1;
    else break;
  }

  const colMonthLabel: (string | null)[] = Array(COLS).fill(null);
  let lastMonth = -1;
  for (let c = 0; c < COLS; c++) {
    const colStartIdx = startIdx + c * 7;
    const d           = localDateFromIdx(colStartIdx);
    const m           = d.getMonth();
    if (m !== lastMonth) {
      colMonthLabel[c] = MONTH_LABELS[m];
      lastMonth        = m;
    }
  }

  return (
    <div className="panel p-5">
      {/* Header — Sentinel format */}
      <div className="flex items-start justify-between mb-5">
        <div>
          <h3 className="text-[15px] font-bold tracking-tight"
              style={{ color: "hsl(var(--foreground))" }}>
            Recording Activity
          </h3>
          <p className="text-[12px] mt-0.5"
             style={{ color: "hsl(var(--muted-foreground))" }}>
            Words polished per day across all sessions
          </p>
        </div>
        <div className="flex items-center gap-2">
          {/* Range dropdown */}
          <div ref={rangeRef} className="relative">
            <button
              onClick={() => setRangeOpen((o) => !o)}
              className="flex items-center gap-1.5 px-3 h-8 rounded-md text-[12px] font-medium transition-colors"
              style={{
                background: rangeOpen ? "hsl(var(--surface-hover))" : "hsl(var(--surface-3))",
                color:      "hsl(var(--foreground))",
                boxShadow:  "inset 0 0 0 1px hsl(var(--border))",
              }}
            >
              {RANGE_LABEL[range]}
              <CaretDown
                size={11}
                style={{ transition: "transform 0.15s", transform: rangeOpen ? "rotate(180deg)" : "none" }}
              />
            </button>
            {rangeOpen && (
              <div
                className="absolute right-0 top-full mt-1 z-30 rounded-md py-1 min-w-[160px]"
                style={{
                  background: "hsl(var(--surface-3))",
                  boxShadow:
                    "inset 0 0 0 1px hsl(var(--border)), 0 8px 24px hsl(0 0% 0% / 0.12)",
                }}
              >
                {(Object.keys(RANGE_LABEL) as HeatmapRange[]).map((k) => {
                  const active = range === k;
                  return (
                    <button
                      key={k}
                      onClick={() => { setRange(k); setRangeOpen(false); }}
                      className="w-full flex items-center justify-between gap-2 px-3 py-1.5 text-[12px] font-medium text-left transition-colors"
                      style={{
                        color:      active ? "hsl(var(--primary))" : "hsl(var(--foreground))",
                        background: active ? "hsl(var(--primary) / 0.08)" : "transparent",
                      }}
                      onMouseEnter={(e) => {
                        if (!active) e.currentTarget.style.background = "hsl(var(--surface-hover))";
                      }}
                      onMouseLeave={(e) => {
                        if (!active) e.currentTarget.style.background = "transparent";
                      }}
                    >
                      {RANGE_LABEL[k]}
                      {active && <Check size={11} strokeWidth={2.5} />}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
          <button
            onClick={onToggle}
            disabled={isProcessing}
            className="px-4 h-8 rounded-md text-[12px] font-semibold transition-all flex items-center gap-1.5"
            style={{
              background: isRecording
                ? "hsl(var(--recording))"
                : "hsl(var(--pill-active-bg))",
              color: isRecording ? "white" : "hsl(var(--pill-active-fg))",
              opacity: isProcessing ? 0.6 : 1,
              cursor:  isProcessing ? "not-allowed" : "pointer",
            }}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${isRecording ? "orb-recording" : ""}`}
              style={{ background: isRecording ? "white" : "hsl(var(--primary))" }}
            />
            {isRecording ? "Stop" : isProcessing ? "Working…" : "Record"}
          </button>
          <button
            onClick={onView}
            className="px-3 h-8 rounded-md text-[12px] font-semibold transition-colors"
            style={{
              background: "hsl(var(--surface-3))",
              color:      "hsl(var(--foreground))",
              boxShadow:  "inset 0 0 0 1px hsl(var(--border))",
            }}
          >
            View all
          </button>
        </div>
      </div>

      {/* Two-column layout: heatmap on the left, derived stats on the right.
          Heatmap container uses fixed-px tracks so the section's HEIGHT stays
          identical regardless of which range (1m / 3m / 6m / 12m) is selected
          — only the WIDTH of the dot cluster changes. Horizontal scroll kicks
          in for very wide ranges so we never overflow the panel. */}
      <div className="flex gap-8 items-start">

        {/* Heatmap — fixed-px cells, scrolls horizontally if too wide */}
        <div className="min-w-0 flex-shrink overflow-x-auto" style={{ paddingBottom: 2 }}>
          {/* Month labels strip */}
          <div
            className="grid mb-2"
            style={{
              gridTemplateColumns: `repeat(${COLS}, ${CELL_PX}px)`,
              columnGap: 4,
            }}
          >
            {colMonthLabel.map((label, i) => (
              <span
                key={i}
                className="text-[10.5px] font-medium tabular-nums whitespace-nowrap"
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

          {/* Heatmap grid — fixed pixel cell size keeps section height stable */}
          <div
            className="grid"
            style={{
              gridTemplateColumns: `repeat(${COLS}, ${CELL_PX}px)`,
              gridTemplateRows:    `repeat(${ROWS}, ${CELL_PX}px)`,
              gridAutoFlow:        "column",
              columnGap: 4,
              rowGap:    4,
            }}
            onMouseLeave={() => setHover(null)}
          >
            {Array.from({ length: COLS * ROWS }).map((_, i) => {
              const c   = Math.floor(i / ROWS);
              const r   = i % ROWS;
              const idx = startIdx + c * 7 + r;
              const future = idx > todayIdx;
              const words  = future ? 0 : (dayMap.get(idx) ?? 0);
              const level  = future ? 0 : wordsToLevel(words, max);
              const isToday = idx === todayIdx;
              return (
                <span
                  key={i}
                  className={`block rounded-full transition-transform ${isToday ? "heat-current" : `heat-${level}`}`}
                  style={{
                    width:    CELL_PX,
                    height:   CELL_PX,
                    opacity:  future ? 0.3 : 1,
                    cursor:   future ? "default" : "pointer",
                  }}
                  onMouseEnter={(e) => {
                    if (future) return;
                    const rect = e.currentTarget.getBoundingClientRect();
                    const localDate = localDateFromIdx(idx);
                    setHover({
                      x:     rect.left + rect.width / 2,
                      y:     rect.top,
                      words,
                      date:  localDate.toLocaleDateString("en-US", {
                        weekday: "short",
                        month:   "short",
                        day:     "numeric",
                        year:    "numeric",
                      }),
                    });
                    e.currentTarget.style.transform = "scale(1.4)";
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.transform = "scale(1)";
                  }}
                />
              );
            })}
          </div>
        </div>

        {/* Side stats — different metrics from the top stat tiles to avoid
            duplicating "Words polished" data when there's only one active day. */}
        <div
          className="flex-1 flex flex-col gap-4 pl-6"
          style={{
            minWidth: 140,
            borderLeft: "1px solid hsl(var(--border))",
          }}
        >
          <SideStat
            label="Streak"
            value={streak}
            unit={streak === 1 ? "day" : "days"}
            highlight
          />
          <SideStat
            label="Avg / day"
            value={dailyAvg > 0 ? dailyAvg : "—"}
            unit={dailyAvg > 0 ? "words on active days" : ""}
          />
          <SideStat
            label="Active"
            value={activeDayCount}
            unit={`of ${totalDays} days`}
          />
        </div>
      </div>

      {/* Footer */}
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
              className={`heat-${l} rounded-sm`}
              style={{ width: 11, height: 11 }}
            />
          ))}
          <span>More</span>
        </div>
      </div>

      {/* Floating hover tooltip — fixed-positioned so it lives above everything */}
      {hover && (
        <div
          className="fixed pointer-events-none z-50 px-2.5 py-1.5 rounded-md text-[11.5px] tabular-nums whitespace-nowrap"
          style={{
            left: hover.x,
            top:  hover.y - 10,
            transform: "translate(-50%, -100%)",
            background: "hsl(var(--foreground))",
            color:      "hsl(var(--background))",
            boxShadow:  "0 6px 20px hsl(0 0% 0% / 0.20)",
            fontWeight: 500,
          }}
        >
          <span style={{ fontWeight: 700 }}>
            {hover.words.toLocaleString()} word{hover.words === 1 ? "" : "s"}
          </span>
          <span style={{ opacity: 0.6 }}> · </span>
          {hover.date}
        </div>
      )}
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   6) WorkspaceTopBar — Sentinel "Vektora · Enterprise · Invite people" pattern.
      Adapted: "Said · Personal" + invite button.
   ════════════════════════════════════════════════════════════════════════════ */

export function WorkspaceTopBar({
  onInvite,
}: { onInvite: () => void }) {
  // Single-user app — show one avatar (the user) plus a "+" indicator
  return (
    <div className="flex items-center justify-between gap-4 mb-4">
      <div className="flex items-center gap-3 min-w-0">
        {/* Workspace logo — black tile with "S" */}
        <span
          className="flex items-center justify-center w-9 h-9 rounded-lg flex-shrink-0 text-[14px] font-bold"
          style={{
            background: "hsl(var(--pill-active-bg))",
            color:      "hsl(var(--pill-active-fg))",
          }}
        >
          S
        </span>
        <h1 className="text-[20px] font-bold tracking-tight truncate"
            style={{ color: "hsl(var(--foreground))" }}>
          Said
        </h1>
        <span
          className="inline-flex items-center px-2.5 py-1 rounded-md text-[11px] font-semibold flex-shrink-0"
          style={{
            background: "hsl(var(--surface-4))",
            color:      "hsl(var(--muted-foreground))",
          }}
        >
          Personal
        </span>
      </div>

      {/* Right cluster: avatar stack + invite */}
      <div className="flex items-center gap-3 flex-shrink-0">
        {/* Avatar group — hide entirely below md */}
        <div className="hidden md:flex items-center -space-x-2">
          {[
            { letter: "M", bg: "hsl(220 80% 60%)" },
            { letter: "D", bg: "hsl(15  80% 60%)" },
            { letter: "B", bg: "hsl(38  85% 55%)" },
            { letter: "J", bg: "hsl(150 65% 45%)" },
          ].map((a) => (
            <span
              key={a.letter}
              className="flex items-center justify-center w-7 h-7 rounded-full text-[10.5px] font-bold"
              style={{
                background: a.bg,
                color:      "white",
                boxShadow:  "0 0 0 2px hsl(var(--surface-2))",
              }}
            >
              {a.letter}
            </span>
          ))}
        </div>
        <button
          onClick={onInvite}
          className="px-4 h-9 rounded-md text-[12.5px] font-semibold transition-all flex items-center gap-2 whitespace-nowrap"
          style={{
            background: "hsl(var(--pill-active-bg))",
            color:      "hsl(var(--pill-active-fg))",
          }}
        >
          {/* Short label on small screens, full on larger */}
          <span className="sm:hidden">Invite</span>
          <span className="hidden sm:inline">Invite people</span>
        </button>
      </div>
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   7) FilterBar — Sentinel "Recent PRs · April 2026 · + · branch · repo · Search"
      Adapted: "Recent · Today · + new · search" with ⌘K hint.
   ════════════════════════════════════════════════════════════════════════════ */

interface FilterBarProps {
  onNewRecording?: () => void;
}

// Currently only the search field is wired up; chips/branch/workspace icons were
// dead decorations and have been removed. New-recording is reachable via the
// "Record" button inside the heatmap header, so the "+" is gone too.
export function FilterBar({ onNewRecording: _ }: FilterBarProps) {
  return (
    <div className="flex items-center mb-4">
      {/* Search — flexes to fill the row */}
      <div className="relative w-full" style={{ maxWidth: 480 }}>
        <Search
          size={13}
          className="absolute pointer-events-none"
          style={{
            left: 12, top: "50%", transform: "translateY(-50%)",
            color: "hsl(var(--muted-foreground))",
          }}
        />
        <input
          type="search"
          placeholder="Search recordings…"
          className="w-full pl-8 pr-12 py-2 rounded-md text-[12.5px] transition-all"
          style={{
            background: "hsl(var(--surface-3))",
            color:      "hsl(var(--foreground))",
            boxShadow:  "inset 0 0 0 1px hsl(var(--border))",
            outline:    "none",
            height:     34,
          }}
        />
        <span
          className="absolute text-[10.5px] font-semibold tabular-nums px-1.5 py-0.5 rounded hidden sm:block"
          style={{
            right: 8, top: "50%", transform: "translateY(-50%)",
            background: "hsl(var(--surface-4))",
            color:      "hsl(var(--muted-foreground))",
          }}
        >
          ⌘K
        </span>
      </div>
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   8) PolishPipeline — Sentinel "Six Checks Pipeline" pattern.
      6 stage cards showing the polish-engine pipeline for the latest recording.
   ════════════════════════════════════════════════════════════════════════════ */

interface PipelineStage {
  id:       number;
  label:    string;            // "Record"
  icon:     React.ReactNode;
  ms:       number;            // latency
  delta:    number;            // delta vs avg
  ok:       boolean;
}

export function PolishPipeline({ recordings }: { recordings: Recording[] }) {
  const latest = recordings[0];

  // Compute averages across last 10 recordings for the delta column
  const sample = recordings.slice(0, 10);
  const avg = (key: "transcribe_ms" | "embed_ms" | "polish_ms") => {
    const xs = sample.map((r) => r[key] ?? 0).filter((n) => n > 0);
    return xs.length ? Math.round(xs.reduce((s, n) => s + n, 0) / xs.length) : 0;
  };
  const avgT = avg("transcribe_ms");
  const avgE = avg("embed_ms");
  const avgP = avg("polish_ms");

  const transcribeMs = latest?.transcribe_ms ?? 0;
  const embedMs      = latest?.embed_ms      ?? 0;
  const polishMs     = latest?.polish_ms     ?? 0;
  // Estimated stage timings for stages we don't measure separately
  const recordMs     = latest ? Math.round((latest.recording_seconds ?? 0) * 1000) : 0;
  const retrieveMs   = latest?.embed_ms ? Math.max(5, Math.round(embedMs * 0.05)) : 0;
  const pasteMs      = latest ? 30 : 0;

  const stages: PipelineStage[] = [
    { id: 1, label: "Record",     icon: <Mic       size={11} />, ms: recordMs,     delta: 0,                                    ok: true                  },
    { id: 2, label: "Transcribe", icon: <Zap       size={11} />, ms: transcribeMs, delta: avgT > 0 ? transcribeMs - avgT : 0,   ok: transcribeMs > 0      },
    { id: 3, label: "Embed",      icon: <Sparkles  size={11} />, ms: embedMs,      delta: avgE > 0 ? embedMs - avgE : 0,        ok: embedMs >= 0          },
    { id: 4, label: "Retrieve",   icon: <Database  size={11} />, ms: retrieveMs,   delta: 0,                                    ok: retrieveMs >= 0       },
    { id: 5, label: "Polish",     icon: <FileText  size={11} />, ms: polishMs,     delta: avgP > 0 ? polishMs - avgP : 0,       ok: polishMs > 0          },
    { id: 6, label: "Paste",      icon: <Send      size={11} />, ms: pasteMs,      delta: 0,                                    ok: pasteMs > 0           },
  ];

  const totalMs = transcribeMs + embedMs + polishMs;
  const ago     = latest ? relTime(latest.timestamp_ms) : "—";

  return (
    <div className="panel p-5">
      {/* Header */}
      <div className="flex items-start justify-between gap-4 mb-4">
        <div>
          <h3 className="text-[15px] font-bold tracking-tight"
              style={{ color: "hsl(var(--foreground))" }}>
            Polish Pipeline
          </h3>
          <p className="text-[12px] mt-0.5"
             style={{ color: "hsl(var(--muted-foreground))" }}>
            {latest
              ? <>Last run · <span style={{ color: "hsl(var(--foreground))", fontWeight: 600 }}>{latest.word_count} words</span> in {totalMs.toLocaleString()} ms · {modelLabel(latest.model_used)} model</>
              : "No recordings yet — press ⇪ Caps Lock to start"}
          </p>
        </div>
        <div className="flex items-center gap-1.5 text-[11.5px] flex-shrink-0"
             style={{ color: "hsl(var(--muted-foreground))" }}>
          <span
            className="inline-block w-1.5 h-1.5 rounded-full"
            style={{ background: "hsl(var(--primary))" }}
          />
          {ago}
        </div>
      </div>

      {/* 6-stage grid — wraps cleanly across breakpoints */}
      <div
        className="grid gap-2.5"
        style={{
          gridTemplateColumns: "repeat(auto-fit, minmax(120px, 1fr))",
        }}
      >
        {stages.map((s, i) => (
          <PipelineCard
            key={s.id}
            stage={s}
            highlighted={i === stages.length - 1 && Boolean(latest)}
          />
        ))}
      </div>
    </div>
  );
}

function PipelineCard({
  stage, highlighted,
}: { stage: PipelineStage; highlighted: boolean }) {
  const deltaColor = stage.delta === 0
    ? "hsl(var(--muted-foreground))"
    : stage.delta < 0
    ? "hsl(var(--primary))"           // faster than avg = mint
    : "hsl(38 85% 50%)";              // slower = amber

  return (
    <div
      className="relative rounded-xl px-3 py-3 flex flex-col"
      style={{
        background: highlighted ? "hsl(var(--primary) / 0.06)" : "hsl(var(--surface-3))",
        boxShadow:  highlighted
          ? "inset 0 0 0 1.5px hsl(var(--primary) / 0.45)"
          : "inset 0 0 0 1px hsl(var(--border))",
      }}
    >
      {/* Step number with icon */}
      <p
        className="text-[9.5px] font-bold uppercase tracking-[0.12em] flex items-center gap-1.5"
        style={{ color: "hsl(var(--muted-foreground))" }}
      >
        <span
          className="inline-flex items-center justify-center w-4 h-4 rounded"
          style={{
            background: highlighted ? "hsl(var(--primary) / 0.20)" : "hsl(var(--surface-4))",
            color:      highlighted ? "hsl(var(--primary))" : "hsl(var(--muted-foreground))",
          }}
        >
          {stage.icon}
        </span>
        Step {String(stage.id).padStart(2, "0")}
      </p>

      {/* Label */}
      <p className="text-[14px] font-bold tracking-tight mt-2"
         style={{ color: "hsl(var(--foreground))" }}>
        {stage.label}
      </p>

      {/* Latency + delta — bottom of card */}
      <div className="flex items-baseline gap-1.5 mt-3 tabular-nums">
        <span className="text-[14px] font-bold leading-none"
              style={{ color: "hsl(var(--foreground))" }}>
          {stage.ms > 0 ? stage.ms.toLocaleString() : "—"}
        </span>
        <span className="text-[10.5px]"
              style={{ color: "hsl(var(--muted-foreground))" }}>
          ms
        </span>
        {stage.delta !== 0 && (
          <span
            className="text-[10.5px] font-semibold ml-auto"
            style={{ color: deltaColor }}
          >
            {stage.delta > 0 ? "+" : ""}{stage.delta}
          </span>
        )}
      </div>
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   9) LatestRunCard — right-rail "Latest Test Run" equivalent.
      Shows the most recent recording with id, breakdown, and code-style excerpt.
   ════════════════════════════════════════════════════════════════════════════ */

export function LatestRunCard({ recordings }: { recordings: Recording[] }) {
  const r = recordings[0];

  return (
    <div className="panel p-5">
      <div className="flex items-center gap-2 mb-3">
        <Activity size={13} style={{ color: "hsl(var(--primary))" }} />
        <h3 className="text-[14px] font-bold tracking-tight"
            style={{ color: "hsl(var(--foreground))" }}>
          Latest run
        </h3>
      </div>

      {!r ? (
        <p className="text-[12px] py-4" style={{ color: "hsl(var(--muted-foreground))" }}>
          No recordings yet.
        </p>
      ) : (
        <>
          {/* Identity row */}
          <div className="flex items-center gap-2.5 mb-3">
            <span
              className="flex items-center justify-center w-9 h-9 rounded-md text-[12px] font-bold flex-shrink-0"
              style={{
                background: "hsl(var(--pill-active-bg))",
                color:      "hsl(var(--pill-active-fg))",
              }}
            >
              S
            </span>
            <div className="min-w-0">
              <p className="text-[13px] font-semibold tracking-tight flex items-center gap-1.5"
                 style={{ color: "hsl(var(--foreground))" }}>
                Said
                <span className="inline-flex items-center px-1.5 py-px rounded text-[9px] font-bold uppercase"
                      style={{
                        background: "hsl(var(--chip-mint-bg))",
                        color:      "hsl(var(--chip-mint-fg))",
                      }}>
                  app
                </span>
              </p>
              <p className="text-[10.5px] tabular-nums"
                 style={{ color: "hsl(var(--muted-foreground))" }}>
                {modelLabel(r.model_used)} · {relTime(r.timestamp_ms)}
              </p>
            </div>
          </div>

          {/* Recording reference */}
          <p className="text-[12.5px] font-medium leading-snug mb-3"
             style={{ color: "hsl(var(--foreground))" }}>
            R-{r.id.slice(0, 4).toUpperCase()} · {r.word_count} words polished
            <br/>
            <span className="text-[11px]"
                  style={{ color: "hsl(var(--muted-foreground))" }}>
              {r.recording_seconds.toFixed(1)}s recording
            </span>
          </p>

          {/* Status banner */}
          <div
            className="rounded-md px-3 py-2 mb-3 flex items-center gap-2"
            style={{
              background: r.edit_count > 0 ? "hsl(354 78% 55% / 0.08)" : "hsl(var(--primary) / 0.08)",
              boxShadow:  r.edit_count > 0
                ? "inset 0 0 0 1px hsl(354 78% 55% / 0.18)"
                : "inset 0 0 0 1px hsl(var(--primary) / 0.18)",
            }}
          >
            {r.edit_count > 0 ? (
              <>
                <AlertCircle size={12} style={{ color: "hsl(354 78% 55%)" }} />
                <span className="text-[11.5px] font-medium"
                      style={{ color: "hsl(354 78% 45%)" }}>
                  {r.edit_count} edit{r.edit_count === 1 ? "" : "s"} after paste
                </span>
              </>
            ) : (
              <>
                <CircleCheck size={12} style={{ color: "hsl(var(--primary))" }} />
                <span className="text-[11.5px] font-medium"
                      style={{ color: "hsl(var(--primary))" }}>
                  Polished cleanly · no edits
                </span>
              </>
            )}
          </div>

          {/* Latency breakdown table */}
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-[12px] mb-3">
            <dt style={{ color: "hsl(var(--muted-foreground))" }}>Transcribe</dt>
            <dd className="text-right tabular-nums font-semibold"
                style={{ color: "hsl(var(--foreground))" }}>
              {(r.transcribe_ms ?? 0).toLocaleString()} ms
            </dd>
            <dt style={{ color: "hsl(var(--muted-foreground))" }}>Embed</dt>
            <dd className="text-right tabular-nums font-semibold"
                style={{ color: r.embed_ms === 0 ? "hsl(var(--primary))" : "hsl(var(--foreground))" }}>
              {r.embed_ms === 0 ? "0 (cached)" : `${r.embed_ms?.toLocaleString()} ms`}
            </dd>
            <dt style={{ color: "hsl(var(--muted-foreground))" }}>Polish</dt>
            <dd className="text-right tabular-nums font-semibold"
                style={{ color: "hsl(var(--foreground))" }}>
              {(r.polish_ms ?? 0).toLocaleString()} ms
            </dd>
          </dl>

          {/* Code-style polished excerpt */}
          <div
            className="rounded-md px-3 py-2.5"
            style={{
              background: "hsl(var(--surface-4) / 0.4)",
              boxShadow:  "inset 0 0 0 1px hsl(var(--border))",
              fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
              fontSize:   11,
              lineHeight: 1.55,
              color:      "hsl(var(--foreground))",
              maxHeight:  140,
              overflow:   "auto",
            }}
          >
            <p className="text-[9.5px] font-bold mb-1 tabular-nums tracking-wider uppercase"
               style={{ color: "hsl(var(--muted-foreground))", fontFamily: "Inter, sans-serif" }}>
              POLISHED OUTPUT
            </p>
            <p>{r.polished.length > 220 ? r.polished.slice(0, 220) + "…" : r.polished}</p>
          </div>
        </>
      )}
    </div>
  );
}

/* ════════════════════════════════════════════════════════════════════════════
   10) ActivityFeed — right-rail timeline of recent recordings.
   ════════════════════════════════════════════════════════════════════════════ */

export function ActivityFeed({ recordings }: { recordings: Recording[] }) {
  const items = recordings.slice(0, 6);
  const today = new Date();
  const todayLabel = today.toLocaleDateString("en-US", { day: "numeric", month: "long", year: "numeric" });

  return (
    <div className="panel p-5">
      <div className="flex items-center gap-2 mb-3">
        <Activity size={13} style={{ color: "hsl(var(--muted-foreground))" }} />
        <h3 className="text-[14px] font-bold tracking-tight"
            style={{ color: "hsl(var(--foreground))" }}>
          Activity
        </h3>
      </div>

      <p className="text-[10.5px] font-semibold uppercase tracking-wider mb-3 flex items-center gap-1.5"
         style={{ color: "hsl(var(--muted-foreground))" }}>
        <span
          className="inline-block w-1.5 h-1.5 rounded-full"
          style={{ background: "hsl(var(--primary))" }}
        />
        {todayLabel}
      </p>

      {items.length === 0 ? (
        <p className="text-[12px] py-2" style={{ color: "hsl(var(--muted-foreground))" }}>
          Recent recordings will appear here.
        </p>
      ) : (
        <div className="space-y-3">
          {items.map((r) => {
            const initial = modelLabel(r.model_used).charAt(0);
            const initialBg =
              initial === "S" ? "hsl(220 80% 60%)" :
              initial === "F" ? "hsl(38 85% 55%)"  :
              initial === "C" ? "hsl(15 80% 60%)"  :
                                "hsl(258 70% 60%)";
            const snippet = r.polished.length > 56
              ? r.polished.slice(0, 56) + "…"
              : r.polished;
            return (
              <div key={r.id} className="flex items-start gap-2.5">
                <span
                  className="flex items-center justify-center w-7 h-7 rounded-full text-[10.5px] font-bold flex-shrink-0 mt-0.5"
                  style={{
                    background: initialBg,
                    color:      "white",
                  }}
                >
                  {initial}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="text-[12px] leading-snug"
                     style={{ color: "hsl(var(--foreground))" }}>
                    Said polished{" "}
                    <span className="font-bold">{r.word_count} word{r.word_count === 1 ? "" : "s"}</span>
                    {" "}via{" "}
                    <span
                      className="inline-flex items-center px-1.5 py-px rounded font-mono text-[10px]"
                      style={{
                        background: "hsl(var(--surface-4))",
                        color:      "hsl(var(--foreground))",
                      }}
                    >
                      {modelLabel(r.model_used)}
                    </span>
                  </p>
                  <p className="text-[10.5px] mt-0.5 leading-snug truncate"
                     style={{ color: "hsl(var(--muted-foreground))" }}>
                    "{snippet}"
                  </p>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/* (Unused export shims — keep tree-shake happy) */
void ChevronUp; void ChevronDown;
