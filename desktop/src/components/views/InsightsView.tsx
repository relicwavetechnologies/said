import React, { useMemo } from "react";
import { Brain, Zap, Bot, Sparkles, Info } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import type { AppSnapshot, HistoryItem } from "@/types";

interface InsightsViewProps {
  snapshot: AppSnapshot | null;
}

// ── WPM Gauge (refined for dark theme) ───────────────────────────────────────

function WpmGauge({ wpm }: { wpm: number }) {
  const maxWpm        = 250;
  const pct           = Math.min(wpm / maxWpm, 1);
  const r             = 52;
  const cx            = 70;
  const cy            = 70;
  const circumference = Math.PI * r;
  const dashOffset    = circumference * (1 - pct);
  const topPct        =
    wpm >= 200 ? "5%" : wpm >= 175 ? "10%" : wpm >= 150 ? "20%" : wpm >= 120 ? "35%" : "50%";

  return (
    <div className="flex flex-col items-center mt-4">
      <svg width="140" height="80" viewBox="0 0 140 80">
        <defs>
          <linearGradient id="wpmGrad" x1="0" y1="0" x2="140" y2="0" gradientUnits="userSpaceOnUse">
            <stop offset="0%"   stopColor="hsl(var(--accent-violet))" />
            <stop offset="100%" stopColor="hsl(var(--primary))" />
          </linearGradient>
        </defs>
        {/* Track — uses muted-foreground so visible in both modes */}
        <path
          d={`M ${cx - r},${cy} A ${r},${r} 0 0 1 ${cx + r},${cy}`}
          strokeWidth="8" fill="none"
          stroke="hsl(var(--muted-foreground) / 0.18)"
          strokeLinecap="round"
        />
        {/* Fill — violet → mint gradient */}
        <path
          d={`M ${cx - r},${cy} A ${r},${r} 0 0 1 ${cx + r},${cy}`}
          strokeWidth="8" fill="none"
          stroke="url(#wpmGrad)"
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={dashOffset}
          style={{ transition: "stroke-dashoffset 0.6s ease" }}
        />
        <text x={cx} y={cy - 8} textAnchor="middle" fill="hsl(var(--muted-foreground))" fontSize="11">
          Top
        </text>
        <text x={cx} y={cy + 6} textAnchor="middle"
              fill="hsl(var(--foreground))" fontSize="15" fontWeight="bold"
              style={{ fontVariantNumeric: "tabular-nums" }}>
          {topPct}
        </text>
      </svg>
    </div>
  );
}

// ── Heatmap helpers ────────────────────────────────────────────────────────────

const DAYS      = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS    = ["Jan", "Feb", "Mar", "Apr"];
const COL_COUNT = 18;

// Local-calendar day index — UTC `floor(ms / DAY)` would split IST evenings
// onto the wrong cell (UTC+5:30 means 1am IST is actually 7:30pm UTC the
// previous day). Using the LOCAL midnight gives consistent same-day grouping.
function localDayIdx(ms: number): number {
  const d = new Date(ms);
  const localMidnight = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  return Math.floor(localMidnight / 86_400_000);
}

function buildDayMap(history: HistoryItem[]): Map<number, number> {
  const map = new Map<number, number>();
  for (const item of history) {
    const day = localDayIdx(item.timestamp_ms);
    map.set(day, (map.get(day) ?? 0) + item.word_count);
  }
  return map;
}

function wordCountToLevel(words: number): 0 | 1 | 2 | 3 | 4 {
  if (words === 0)  return 0;
  if (words < 20)   return 1;
  if (words < 60)   return 2;
  if (words < 150)  return 3;
  return 4;
}

// ── Usage rows ─────────────────────────────────────────────────────────────────

function buildUsageRows(history: HistoryItem[]) {
  const modelCounts: Record<string, number> = {};
  let totalWords = 0;
  for (const h of history) {
    modelCounts[h.model] = (modelCounts[h.model] ?? 0) + h.word_count;
    totalWords += h.word_count;
  }

  const MODELS = [
    { key: "gpt-5.4",                       Icon: Brain,    label: "Smart",  primary: true  },
    { key: "gpt-5.4-mini",                  Icon: Zap,      label: "Fast",   primary: false },
    { key: "claude-sonnet-4-6",             Icon: Bot,      label: "Claude", primary: false },
    { key: "gemini-3.1-flash-lite-preview", Icon: Sparkles, label: "Gemini", primary: false },
  ];

  return MODELS.map((m) => {
    const words = modelCounts[m.key] ?? 0;
    const pct   = totalWords > 0 ? Math.round((words / totalWords) * 100) : 0;
    const uses  = history.filter((h) => h.model === m.key).length;
    return { Icon: m.Icon, value: pct, uses, label: m.label, primary: m.primary };
  });
}

// ── View ───────────────────────────────────────────────────────────────────────

export function InsightsView({ snapshot }: InsightsViewProps) {
  const history       = snapshot?.history ?? [];
  const wpm           = snapshot?.avg_wpm ?? 0;
  const words         = snapshot?.total_words ?? 0;
  const streak        = snapshot?.daily_streak ?? 0;

  const dayMap        = useMemo(() => buildDayMap(history), [history]);
  const usageRows     = useMemo(() => buildUsageRows(history), [history]);
  // Local day index — matches dayMap keys built from `localDayIdx`.
  const todayUnixDay  = localDayIdx(Date.now());

  return (
    <ScrollArea className="h-full">
      <div className="p-7 pb-12 max-w-4xl mx-auto">

        {/* ── Header ──────────────────────────────── */}
        <div className="mb-7">
          <h1 className="text-[24px] font-bold tracking-tight text-foreground leading-tight">
            Insights
          </h1>
          <p className="text-[12.5px] text-muted-foreground mt-1 flex items-center gap-2">
            <span
              className="inline-block w-1.5 h-1.5 rounded-full"
              style={{
                background: "hsl(var(--accent-violet))",
                boxShadow:  "0 0 8px hsl(var(--accent-violet) / 0.5)",
              }}
            />
            Your recording analytics · {history.length} session{history.length === 1 ? "" : "s"}
          </p>
        </div>

        {/* ── Top grid: 3 cols ─────────────────────── */}
        <div className="grid grid-cols-3 gap-4 mb-4">

          {/* WPM gauge */}
          <div className="panel p-5">
            <div className="text-[32px] font-bold tracking-tight text-foreground leading-none tabular-nums">
              {wpm || "—"}
            </div>
            <div className="section-label mt-2">Words per minute</div>
            {wpm > 0 ? (
              <WpmGauge wpm={wpm} />
            ) : (
              <p className="text-[12px] text-muted-foreground mt-4 leading-relaxed">
                Record something to see your speed.
              </p>
            )}
          </div>

          {/* Sessions card */}
          <div className="panel p-5">
            <div className="text-[32px] font-bold tracking-tight text-foreground leading-none tabular-nums">
              {history.length}
            </div>
            <div className="section-label mt-2 mb-4">Sessions polished</div>
            <div className="mt-2" />
            <div className="space-y-3">
              <div className="flex items-center justify-between text-[13px]">
                <span className="text-foreground tabular-nums">{words.toLocaleString()} words total</span>
                <Info size={11} className="text-muted-foreground/50" />
              </div>
              <div className="flex items-center justify-between text-[13px]">
                <span className="text-foreground tabular-nums">{streak} day streak</span>
                <Info size={11} className="text-muted-foreground/50" />
              </div>
            </div>
          </div>

          {/* Total words */}
          <div
            className="panel p-5 relative overflow-hidden"
            style={{
              background:
                "linear-gradient(135deg, hsl(var(--surface-3)) 0%, hsl(var(--surface-3)) 60%, hsl(var(--accent-violet) / 0.10) 100%)",
            }}
          >
            <div
              aria-hidden
              className="absolute pointer-events-none"
              style={{
                right: -60, bottom: -60, width: 180, height: 180, borderRadius: "50%",
                background: "radial-gradient(circle, hsl(var(--accent-violet) / 0.20) 0%, transparent 70%)",
              }}
            />
            <div
              className="relative font-bold tracking-tight leading-none tabular-nums"
              style={{
                fontSize: 32,
                background: "linear-gradient(135deg, hsl(var(--foreground)) 0%, hsl(var(--accent-violet)) 100%)",
                WebkitBackgroundClip: "text",
                WebkitTextFillColor: "transparent",
                backgroundClip: "text",
              }}
            >
              {words.toLocaleString()}
            </div>
            <div className="section-label mt-2 mb-4 relative">Total words dictated</div>
            <div className="mt-2" />
            <div className="text-[13px] text-foreground relative">Desktop · macOS</div>
            <div className="text-[11px] text-muted-foreground mt-0.5 tabular-nums relative">
              {words.toLocaleString()} polished words across {history.length} session{history.length === 1 ? "" : "s"}
            </div>
          </div>
        </div>

        {/* ── Bottom grid: 2 cols ──────────────────── */}
        <div className="grid grid-cols-2 gap-4">

          {/* Usage chart */}
          <div className="panel p-5">
            <div className="flex items-baseline justify-between mb-5">
              <h2 className="text-[14px] font-semibold text-foreground">Model usage</h2>
              <span className="section-label">{history.length} sessions</span>
            </div>
            <div className="space-y-3">
              {usageRows.map((row) => (
                <div key={row.label} className="flex items-center gap-3">
                  <span className="w-6 flex items-center justify-center flex-shrink-0 text-muted-foreground">
                    <row.Icon size={14} />
                  </span>
                  <div
                    className="flex-1 relative h-8 rounded-lg overflow-hidden"
                    style={{ background: "hsl(var(--surface-4))" }}
                  >
                    {row.value > 0 && (
                      <div
                        className="h-full rounded-lg transition-all"
                        style={{
                          width:      `${row.value}%`,
                          background: row.primary
                            ? "hsl(var(--primary))"
                            : "hsl(var(--primary) / 0.35)",
                        }}
                      />
                    )}
                    {/*
                      Two-layer text trick: render the percentage twice with
                      different colors, then clip each layer to the half of
                      the bar where its color contrasts. Lime fill → dark
                      text; empty fill → light text. Always legible.
                    */}
                    <span
                      className="absolute inset-0 flex items-center justify-center text-[12px] font-semibold tabular-nums"
                      style={{
                        color:     "hsl(var(--foreground))",
                        clipPath:  `inset(0 0 0 ${row.value}%)`,
                      }}
                    >
                      {row.value}%
                    </span>
                    <span
                      className="absolute inset-0 flex items-center justify-center text-[12px] font-semibold tabular-nums"
                      style={{
                        color:     "hsl(var(--primary-foreground))",
                        clipPath:  `inset(0 ${100 - row.value}% 0 0)`,
                      }}
                    >
                      {row.value}%
                    </span>
                  </div>
                  <span className="text-[11px] font-medium text-muted-foreground w-24 flex-shrink-0 truncate tabular-nums">
                    {row.uses} {row.label} use{row.uses !== 1 ? "s" : ""}
                  </span>
                </div>
              ))}
            </div>
          </div>

          {/* Heatmap */}
          <div className="tile p-5">
            <div className="flex items-baseline justify-between mb-4">
              <h2 className="text-[14px] font-semibold text-foreground">
                {streak > 0 ? `${streak} day streak` : "No streak yet"}
              </h2>
              <span className="section-label">Best · {streak}d</span>
            </div>

            {/* Month labels */}
            <div className="flex items-center justify-between text-[11px] text-muted-foreground mb-3 px-1">
              <button className="hover:text-foreground transition-colors">‹</button>
              {MONTHS.map((m) => (
                <span key={m} className={cn(m === "Apr" && "text-foreground font-medium")}>{m}</span>
              ))}
              <button className="hover:text-foreground transition-colors">›</button>
            </div>

            {/* Grid — proper week × weekday calendar:
                · each column = one week (oldest → newest, left → right)
                · each row    = one weekday (Sun…Sat)
                · cells anchored to the most-recent Sunday so columns align */}
            <div
              className="grid gap-1"
              style={{ gridTemplateColumns: `36px repeat(${COL_COUNT}, 1fr)` }}
            >
              {(() => {
                const todayDow     = new Date().getDay();   // local DOW
                const lastSundayIx = todayUnixDay - todayDow;

                return DAYS.map((day, dayOfWeek) => (
                  <React.Fragment key={day}>
                    <span className="text-[10px] text-muted-foreground flex items-center">{day}</span>
                    {Array.from({ length: COL_COUNT }, (_, col) => {
                      const weeksAgo  = COL_COUNT - 1 - col;
                      const cellDay   = lastSundayIx - weeksAgo * 7 + dayOfWeek;
                      const isFuture  = cellDay > todayUnixDay;
                      const isCurrent = cellDay === todayUnixDay;
                      const cellWords = isFuture ? 0 : (dayMap.get(cellDay) ?? 0);
                      const level     = isFuture ? 0 : wordCountToLevel(cellWords);
                      return (
                        <span
                          key={col}
                          className={cn(
                            "aspect-square rounded-[3px]",
                            isCurrent ? "heat-current" : `heat-${level}`
                          )}
                          style={{ opacity: isFuture ? 0.3 : 1 }}
                          title={!isFuture && cellWords > 0
                            ? `${cellWords} words on ${(() => {
                                const daysAgo = todayUnixDay - cellDay;
                                const d = new Date();
                                d.setHours(0, 0, 0, 0);
                                d.setDate(d.getDate() - daysAgo);
                                return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
                              })()}`
                            : undefined}
                        />
                      );
                    })}
                  </React.Fragment>
                ));
              })()}
            </div>

            {/* Legend */}
            <div className="flex items-center justify-between mt-4 text-[10px] text-muted-foreground">
              <div className="flex items-center gap-1.5">
                <span>More</span>
                {([4, 3, 2, 1, 0] as const).map((l) => (
                  <span key={l} className={cn("w-3 h-3 rounded-[2px]", `heat-${l}`)} />
                ))}
                <span>Less</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="w-3 h-3 rounded-[2px] heat-current" />
                <span>Today</span>
              </div>
            </div>
          </div>
        </div>

      </div>
    </ScrollArea>
  );
}
