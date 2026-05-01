import React, { useMemo, useState } from "react";
import { Mic, Zap, Bot, Sparkles, Copy, Check, Timer, BookOpen, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { ScrollArea } from "@/components/ui/scroll-area";
import { HeroBanner } from "@/components/HeroBanner";
import type { AppSnapshot, HistoryItem, PendingEdit } from "@/types";

// ── Helpers ────────────────────────────────────────────────────────────────────

function modeIcon(key: string, size = 12) {
  if (key.includes("claude"))  return <Bot      size={size} />;
  if (key.includes("gemini"))  return <Sparkles size={size} />;
  return <Zap size={size} />;
}

function modelLabel(model: string): string {
  if (model.includes("mini"))   return "Fast";
  if (model.includes("claude")) return "Claude";
  if (model.includes("gemini")) return "Gemini";
  return "Smart";
}

function relativeTime(ms: number): string {
  const now = Date.now();
  const diff = now - ms;
  const min = Math.floor(diff / 60_000);
  if (min < 1)   return "just now";
  if (min < 60)  return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24)   return `${hr}h ago`;
  const d = Math.floor(hr / 24);
  if (d === 1)   return "yesterday";
  if (d < 7)     return `${d}d ago`;
  return new Date(ms).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

type FilterTab = "all" | "today" | "week";

// ── Speed breakdown ────────────────────────────────────────────────────────────

interface SpeedPhase {
  label:    string;
  ms:       number;
  color:    string;
  hint:     string;
}

function SpeedBreakdown({ item }: { item: HistoryItem }) {
  const total = item.transcribe_ms + item.embed_ms + item.polish_ms;
  if (total === 0) return null;

  const phases: SpeedPhase[] = [
    {
      label: "LLM Polish",
      ms:    item.polish_ms,
      color: "hsl(var(--primary))",
      hint:  "Gateway LLM generating the polished output",
    },
    {
      label: "Transcription",
      ms:    item.transcribe_ms,
      color: "hsl(38 90% 55%)",
      hint:  "Deepgram converting audio → text",
    },
    {
      label: "Embedding",
      ms:    item.embed_ms,
      color: "hsl(270 60% 65%)",
      hint:  "Gemini embedding + RAG retrieval (0 = cache hit)",
    },
  ]
    .filter((p) => p.ms > 0)
    .sort((a, b) => b.ms - a.ms);

  const maxMs = phases[0]?.ms ?? 1;

  return (
    <div
      className="rounded-2xl px-5 py-4 mb-6"
      style={{ background: "hsl(var(--surface-2))" }}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <Timer size={13} className="text-muted-foreground" />
          <span className="text-[11px] font-bold uppercase tracking-[0.12em] text-muted-foreground">
            Last recording · speed breakdown
          </span>
        </div>
        <span
          className="text-[11px] font-semibold tabular-nums"
          style={{ color: "hsl(var(--chip-lime-fg))" }}
        >
          {total.toLocaleString()} ms total
        </span>
      </div>

      {/* Phases — sorted by duration descending */}
      <div className="space-y-2.5">
        {phases.map((p) => {
          const pct = Math.round((p.ms / total) * 100);
          const barW = Math.round((p.ms / maxMs) * 100);
          return (
            <div key={p.label} title={p.hint}>
              {/* Label row */}
              <div className="flex items-center justify-between mb-1">
                <span className="text-[12px] font-medium text-foreground">{p.label}</span>
                <div className="flex items-center gap-2 tabular-nums">
                  <span className="text-[11px] text-muted-foreground">{pct}%</span>
                  <span
                    className="text-[12px] font-semibold"
                    style={{ color: p.color, minWidth: "4.5ch", textAlign: "right" }}
                  >
                    {p.ms.toLocaleString()} ms
                  </span>
                </div>
              </div>
              {/* Bar */}
              <div
                className="h-1.5 rounded-full overflow-hidden"
                style={{ background: "hsl(var(--surface-4))" }}
              >
                <div
                  className="h-full rounded-full transition-all duration-500"
                  style={{ width: `${barW}%`, background: p.color }}
                />
              </div>
            </div>
          );
        })}
      </div>

      {/* Footer hint */}
      <p className="text-[10px] text-muted-foreground mt-3 leading-relaxed opacity-60">
        Sorted slowest → fastest · hover a row for details · Embedding = 0 ms means SQLite cache hit
      </p>
    </div>
  );
}

// ── Stat tile ──────────────────────────────────────────────────────────────────

function StatTile({ label, value, accent = false }: {
  label: string; value: string | number; accent?: boolean;
}) {
  return (
    <div className="tile px-5 py-4">
      <div
        className={cn(
          "text-[26px] font-bold tracking-tight leading-none tabular-nums",
          accent ? "" : "text-foreground"
        )}
        style={accent ? { color: "hsl(var(--chip-lime-fg))" } : undefined}
      >
        {typeof value === "number" ? value.toLocaleString() : value}
      </div>
      <div className="section-label mt-2">{label}</div>
    </div>
  );
}

// ── Recording row (numbered list-item style) ──────────────────────────────────

function RecordingRow({ item, index }: { item: HistoryItem; index: number }) {
  const wpm = item.recording_seconds > 0
    ? Math.round(item.word_count / (item.recording_seconds / 60))
    : 0;

  const firstDot   = item.polished.search(/[.?!]/);
  const titleText  = firstDot > 0 ? item.polished.slice(0, firstDot + 1) : item.polished;

  // Local "just copied" feedback (resets after 1.5s)
  const [copied, setCopied] = useState(false);

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(item.polished);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard API may fail in non-secure contexts — silently ignore
    }
  };

  return (
    <div
      className="group flex items-center gap-4 px-5 py-3.5 transition-colors cursor-default"
      onMouseEnter={(e) => { e.currentTarget.style.background = "hsl(var(--surface-hover))"; }}
      onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
    >
      {/* Numbered badge */}
      <div className="num-badge">{index + 1}</div>

      {/* Body */}
      <div className="flex-1 min-w-0">
        <p className="text-[14px] font-medium text-foreground leading-snug truncate">
          {titleText}
        </p>
        <div className="flex items-center gap-3 mt-1.5 text-[11px] text-muted-foreground tabular-nums">
          <span className="flex items-center gap-1">
            <span className="opacity-70">{modeIcon(item.model, 10)}</span>
            {modelLabel(item.model)}
          </span>
          <span className="opacity-40">·</span>
          <span>{item.word_count} words</span>
          <span className="opacity-40">·</span>
          <span>{item.recording_seconds.toFixed(1)}s</span>
          {wpm > 0 && (
            <>
              <span className="opacity-40">·</span>
              <span>{wpm} WPM</span>
            </>
          )}
          <span className="opacity-40">·</span>
          <span>{relativeTime(item.timestamp_ms)}</span>
        </div>
      </div>

      {/* Copy button — appears on row hover, persists "Copied" state for 1.5s */}
      <button
        onClick={handleCopy}
        title={copied ? "Copied!" : "Copy polished text"}
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-lg flex-shrink-0 transition-all",
          copied ? "opacity-100" : "opacity-0 group-hover:opacity-100"
        )}
        style={{
          background: copied
            ? "hsl(var(--chip-lime-bg))"
            : "hsl(var(--surface-4))",
          color: copied
            ? "hsl(var(--chip-lime-fg))"
            : "hsl(var(--muted-foreground))",
        }}
        onMouseEnter={(e) => {
          if (!copied) e.currentTarget.style.color = "hsl(var(--foreground))";
        }}
        onMouseLeave={(e) => {
          if (!copied) e.currentTarget.style.color = "hsl(var(--muted-foreground))";
        }}
      >
        {copied ? <Check size={14} strokeWidth={2.5} /> : <Copy size={14} />}
      </button>
    </div>
  );
}

// ── Props ──────────────────────────────────────────────────────────────────────

interface DashboardViewProps {
  snapshot:        AppSnapshot | null;
  busy:            boolean;
  onToggle:        () => void;
  onAccessibility: () => void;
  onNavigate?:     (view: string) => void;
  statusPhase?:    string;
  liveText?:       string;
  pendingEdits?:   PendingEdit[];
  onResolvePending?: (id: string, action: "approve" | "skip") => void;
}

// ── View ───────────────────────────────────────────────────────────────────────

export function DashboardView({
  snapshot,
  busy,
  onToggle,
  onAccessibility,
  onNavigate,
  statusPhase    = "",
  liveText       = "",
  pendingEdits   = [],
  onResolvePending,
}: DashboardViewProps) {
  const [filterTab, setFilterTab]       = useState<FilterTab>("all");
  const [reviewOpen, setReviewOpen]     = useState(false);

  const isRecording  = snapshot?.state === "recording";
  const isProcessing = snapshot?.state === "processing" || busy;

  const history    = snapshot?.history ?? [];
  const totalWords = snapshot?.total_words ?? 0;
  const avgWpm     = snapshot?.avg_wpm ?? 0;
  const streak     = snapshot?.daily_streak ?? 0;

  /* Filter recordings by tab */
  const filtered = useMemo(() => {
    if (filterTab === "all") return history;
    const now  = Date.now();
    const sod  = new Date(now); sod.setHours(0, 0, 0, 0);
    const week = new Date(sod.getTime() - 6 * 86_400_000);
    if (filterTab === "today") return history.filter((h) => h.timestamp_ms >= sod.getTime());
    return history.filter((h) => h.timestamp_ms >= week.getTime());
  }, [history, filterTab]);

  const FILTER_TABS: { id: FilterTab; label: string }[] = [
    { id: "all",   label: "All recordings" },
    { id: "today", label: "Today"          },
    { id: "week",  label: "This week"      },
  ];

  return (
    <ScrollArea className="h-full">
      <div className="p-7 pb-12 max-w-5xl mx-auto">

        {/* ── Pending learning approvals banner ──────── */}
        {pendingEdits.length > 0 && (
          <div
            className="mb-5 rounded-2xl px-4 py-3 flex items-center justify-between gap-3"
            style={{
              background: "hsl(var(--chip-lime-fg) / 0.08)",
              border:     "1px solid hsl(var(--chip-lime-fg) / 0.25)",
            }}
          >
            <div className="flex items-center gap-2.5">
              <BookOpen size={15} style={{ color: "hsl(var(--chip-lime-fg))" }} />
              <p className="text-[12px] font-medium" style={{ color: "hsl(var(--chip-lime-fg))" }}>
                {pendingEdits.length} learning approval{pendingEdits.length > 1 ? "s" : ""} pending
              </p>
            </div>
            <button
              onClick={() => setReviewOpen((o) => !o)}
              className="text-[11px] font-semibold px-3 py-1 rounded-lg transition-colors"
              style={{
                background: "hsl(var(--chip-lime-fg) / 0.15)",
                color:      "hsl(var(--chip-lime-fg))",
              }}
            >
              {reviewOpen ? "Close" : "Review"}
            </button>
          </div>
        )}

        {/* ── Pending edits review panel ──────────────── */}
        {reviewOpen && pendingEdits.length > 0 && (
          <div
            className="mb-6 rounded-2xl overflow-hidden"
            style={{ border: "1px solid hsl(var(--border))" }}
          >
            {pendingEdits.map((pe, i) => (
              <div
                key={pe.id}
                className="px-4 py-3 flex flex-col gap-2"
                style={{
                  borderBottom: i < pendingEdits.length - 1 ? "1px solid hsl(var(--border))" : undefined,
                  background: "hsl(var(--surface-2))",
                }}
              >
                <div className="flex flex-col gap-1 text-[11px]">
                  <div className="flex gap-2 items-start">
                    <span className="opacity-40 font-semibold w-5 text-right flex-shrink-0">AI</span>
                    <span
                      className="leading-snug px-2 py-1 rounded-lg text-muted-foreground line-through flex-1"
                      style={{ background: "hsl(0 50% 50% / 0.08)" }}
                    >
                      {pe.ai_output.length > 80 ? pe.ai_output.slice(0, 80) + "…" : pe.ai_output}
                    </span>
                  </div>
                  <div className="flex gap-2 items-start">
                    <span className="opacity-40 font-semibold w-5 text-right flex-shrink-0">You</span>
                    <span
                      className="leading-snug px-2 py-1 rounded-lg flex-1"
                      style={{
                        background: "hsl(var(--chip-lime-fg) / 0.08)",
                        color:      "hsl(var(--chip-lime-fg))",
                      }}
                    >
                      {pe.user_kept.length > 80 ? pe.user_kept.slice(0, 80) + "…" : pe.user_kept}
                    </span>
                  </div>
                </div>
                <div className="flex gap-2 justify-end">
                  <button
                    onClick={() => { onResolvePending?.(pe.id, "skip"); }}
                    className="px-3 py-1 rounded-lg text-[11px] font-medium text-muted-foreground transition-colors"
                    style={{ background: "hsl(var(--surface-4))" }}
                  >
                    Skip
                  </button>
                  <button
                    onClick={() => { onResolvePending?.(pe.id, "approve"); }}
                    className="px-3 py-1 rounded-lg text-[11px] font-semibold transition-colors"
                    style={{
                      background: "hsl(var(--primary))",
                      color:      "hsl(var(--primary-foreground))",
                    }}
                  >
                    Save to learning
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* ── Hero banner ─────────────────────────────── */}
        <HeroBanner onCustomize={() => onNavigate?.("settings")} />

        {/* ── Page header ───────────────────────────────── */}
        <div className="flex items-start justify-between gap-4 mb-7">
          <div>
            <h1 className="text-[28px] font-bold tracking-tight text-foreground leading-tight">
              My Recordings
            </h1>
            <p className="text-[13px] text-muted-foreground mt-1">
              {history.length > 0
                ? `${history.length} recordings · press Caps Lock to capture more`
                : "Press Caps Lock or click Record to capture your voice"}
            </p>
          </div>

          {/* ── Record CTA button ── */}
          <button
            onClick={onToggle}
            disabled={isProcessing}
            className={cn(
              "flex items-center gap-2 px-5 py-2.5 rounded-full text-[13px] font-semibold",
              "transition-all duration-100 flex-shrink-0"
            )}
            style={{
              background: isRecording
                ? "hsl(var(--recording))"
                : isProcessing
                ? "hsl(var(--surface-4))"
                : "hsl(var(--primary))",
              color: isProcessing
                ? "hsl(var(--muted-foreground))"
                : isRecording
                ? "white"
                : "hsl(var(--primary-foreground))",
              cursor: isProcessing ? "not-allowed" : "pointer",
            }}
          >
            <span
              className={cn(
                "w-2 h-2 rounded-full flex-shrink-0",
                isRecording ? "orb-recording" : ""
              )}
              style={{
                background: isRecording
                  ? "white"
                  : isProcessing
                  ? "hsl(var(--muted-foreground))"
                  : "hsl(var(--primary-foreground) / 0.7)",
              }}
            />
            {isRecording  ? "Stop recording" :
             isProcessing ? "Processing…"    :
                            "Start recording"}
          </button>
        </div>

        {/* ── Stats row ─────────────────────────────────── */}
        <div className="grid grid-cols-4 gap-3 mb-7">
          <StatTile label="Total words"   value={totalWords}     />
          <StatTile label="Avg WPM"       value={avgWpm || "—"}  />
          <StatTile label="Day streak"    value={streak} accent  />
          <StatTile label="Sessions"      value={history.length} />
        </div>

        {/* ── Live streaming preview ────────────────────── */}
        {(statusPhase || liveText) && (
          <div
            className="rounded-2xl px-5 py-4 mb-6"
            style={{ background: "hsl(var(--primary) / 0.08)" }}
          >
            <div className="flex items-center gap-2 mb-2">
              <span
                className="w-1.5 h-1.5 rounded-full flex-shrink-0 animate-pulse"
                style={{
                  background: statusPhase === "transcribing"
                    ? "hsl(38 90% 55%)"
                    : "hsl(var(--primary))",
                }}
              />
              <span
                className="text-[10px] font-bold uppercase tracking-[0.14em]"
                style={{ color: "hsl(var(--chip-lime-fg))" }}
              >
                {statusPhase === "transcribing" ? "Transcribing audio…" : "Polishing with LLM…"}
              </span>
            </div>
            {liveText && (
              <p className="text-[14px] text-foreground leading-relaxed">
                {liveText}
                <span className="caret-blink" />
              </p>
            )}
          </div>
        )}

        {/* ── Speed breakdown (last recording, shown only when idle) ──────── */}
        {!isRecording && !isProcessing && !statusPhase && history.length > 0 && (
          <SpeedBreakdown item={history[0]} />
        )}

        {/* ── Recordings section header + filters ───────── */}
        <div className="flex items-center justify-between gap-3 mb-3">
          <div className="flex items-center gap-2">
            <h2 className="text-[14px] font-semibold text-foreground">Recordings</h2>
            <span
              className="text-[11px] tabular-nums px-1.5 py-0.5 rounded"
              style={{ background: "hsl(var(--surface-4))", color: "hsl(var(--muted-foreground))" }}
            >
              {filtered.length}
            </span>
          </div>
          <div className="flex items-center gap-1.5">
            {FILTER_TABS.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setFilterTab(tab.id)}
                className={cn("pill", filterTab === tab.id && "active")}
              >
                {tab.label}
              </button>
            ))}
          </div>
        </div>

        {/* ── Recordings list ───────────────────────────── */}
        <div className="tile overflow-hidden">
          {filtered.length === 0 ? (
            <div className="px-8 py-16 text-center">
              <div
                className="w-12 h-12 rounded-full flex items-center justify-center mx-auto mb-4"
                style={{ background: "hsl(var(--primary) / 0.15)" }}
              >
                <Mic size={20} style={{ color: "hsl(var(--chip-lime-fg))" }} />
              </div>
              <h3 className="text-[14px] font-semibold text-foreground mb-1">
                {filterTab === "all" ? "No recordings yet" : "Nothing here"}
              </h3>
              <p className="text-[12px] text-muted-foreground max-w-xs mx-auto leading-relaxed">
                {filterTab === "all"
                  ? "Hold Caps Lock to capture, release to polish. Each recording is auto-pasted into your active app."
                  : "Try \"All recordings\" to see your full history."}
              </p>
            </div>
          ) : (
            filtered.map((item, i) => (
              <RecordingRow key={item.timestamp_ms} item={item} index={i} />
            ))
          )}
        </div>

        {/* ── Accessibility nudge ────────────────────────── */}
        {snapshot && !snapshot.accessibility_granted && snapshot.auto_paste_supported && (
          <div
            className="mt-6 rounded-2xl px-5 py-4 flex items-start justify-between gap-4"
            style={{ background: "hsl(var(--chip-amber-bg))" }}
          >
            <div>
              <p className="text-[13px] font-semibold" style={{ color: "hsl(var(--chip-amber-fg))" }}>
                Enable auto-paste
              </p>
              <p
                className="text-[11px] mt-1 leading-relaxed"
                style={{ color: "hsl(var(--chip-amber-fg) / 0.8)" }}
              >
                Grant Accessibility access so Said can paste polished text directly into any app.
              </p>
            </div>
            <button onClick={onAccessibility} className="btn-primary flex-shrink-0">
              Enable
            </button>
          </div>
        )}

      </div>
    </ScrollArea>
  );
}
