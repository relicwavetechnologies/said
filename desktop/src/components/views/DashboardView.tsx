import React, { useEffect, useState } from "react";
import { Mic, BookOpen } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  HeroStat,
  DonutCard,
  TimeSavedCard,
  PaceCard,
  RecordingsTable,
  ActivityHeatmap,
  WorkspaceTopBar,
  FilterBar,
} from "@/components/DashboardCards";
import { listHistory } from "@/lib/invoke";
import type { AppSnapshot, PendingEdit, Recording } from "@/types";

// ── Props ──────────────────────────────────────────────────────────────────────

interface DashboardViewProps {
  snapshot:        AppSnapshot | null;
  busy:            boolean;
  onToggle:        () => void;
  onAccessibility: () => void;
  onNavigate?:     (view: string) => void;
  onOpenInvite?:   () => void;
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
  onOpenInvite,
  statusPhase    = "",
  liveText       = "",
  pendingEdits   = [],
  onResolvePending,
}: DashboardViewProps) {
  const [reviewOpen, setReviewOpen] = useState(false);
  const [recordings, setRecordings] = useState<Recording[]>([]);

  const isRecording  = snapshot?.state === "recording";
  const isProcessing = snapshot?.state === "processing" || busy;

  const history = snapshot?.history ?? [];

  // Fetch full Recording[] (with .id and .audio_id) for the table.
  // Re-fetch whenever a new recording lands (snapshot.history.length changes).
  useEffect(() => {
    let alive = true;
    listHistory(10).then((r) => { if (alive) setRecordings(r); });
    return () => { alive = false; };
  }, [history.length]);

  return (
    <ScrollArea className="h-full">
      <div className="px-7 pt-4 pb-10 max-w-[1280px] mx-auto">

        {/* ── Workspace top bar ──────────────────────── */}
        <WorkspaceTopBar onInvite={() => onOpenInvite?.()} />

        {/* ── Filter / search bar ─────────────────────── */}
        <FilterBar onNewRecording={onToggle} />


        {/* ── Pending learning approvals banner ──────── */}
        {pendingEdits.length > 0 && (
          <div
            className="mb-5 rounded-2xl px-4 py-3 flex items-center justify-between gap-3"
            style={{
              background: "hsl(var(--chip-mint-bg))",
              boxShadow:  "inset 0 0 0 1px hsl(var(--chip-mint-fg) / 0.20)",
            }}
          >
            <div className="flex items-center gap-2.5">
              <BookOpen size={15} style={{ color: "hsl(var(--chip-mint-fg))" }} />
              <p className="text-[12px] font-medium" style={{ color: "hsl(var(--chip-mint-fg))" }}>
                {pendingEdits.length} learning approval{pendingEdits.length > 1 ? "s" : ""} pending
              </p>
            </div>
            <button
              onClick={() => setReviewOpen((o) => !o)}
              className="text-[11px] font-semibold px-3 py-1 rounded-lg transition-colors"
              style={{
                background: "hsl(var(--chip-mint-fg) / 0.18)",
                color:      "hsl(var(--chip-mint-fg))",
              }}
            >
              {reviewOpen ? "Close" : "Review"}
            </button>
          </div>
        )}

        {/* ── Pending edits review panel ──────────────── */}
        {reviewOpen && pendingEdits.length > 0 && (
          <div className="mb-6 panel overflow-hidden">
            {pendingEdits.map((pe, i) => (
              <div
                key={pe.id}
                className="px-4 py-3 flex flex-col gap-2"
                style={{
                  borderBottom: i < pendingEdits.length - 1
                    ? "1px solid hsl(var(--surface-4))"
                    : undefined,
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
                        background: "hsl(var(--chip-mint-fg) / 0.10)",
                        color:      "hsl(var(--chip-mint-fg))",
                      }}
                    >
                      {pe.user_kept.length > 80 ? pe.user_kept.slice(0, 80) + "…" : pe.user_kept}
                    </span>
                  </div>
                </div>
                <div className="flex gap-2 justify-end">
                  <button
                    onClick={() => onResolvePending?.(pe.id, "skip")}
                    className="px-3 py-1 rounded-lg text-[11px] font-medium text-muted-foreground transition-colors"
                    style={{ background: "hsl(var(--surface-4))" }}
                  >
                    Skip
                  </button>
                  <button
                    onClick={() => onResolvePending?.(pe.id, "approve")}
                    className="btn-primary px-3 py-1 text-[11px]"
                  >
                    Save to learning
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* ── Live streaming preview ──────────────────── */}
        {(statusPhase || liveText) && (
          <div className="rounded-2xl panel px-5 py-4 mb-5 relative overflow-hidden">
            <div className="relative flex items-center gap-2 mb-2">
              <span
                className="w-1.5 h-1.5 rounded-full flex-shrink-0 animate-pulse"
                style={{
                  background: statusPhase === "transcribing"
                    ? "hsl(38 90% 55%)"
                    : "hsl(var(--primary))",
                  boxShadow: "0 0 8px currentColor",
                }}
              />
              <span
                className="text-[10px] font-bold uppercase tracking-[0.14em]"
                style={{ color: "hsl(var(--chip-mint-fg))" }}
              >
                {statusPhase === "transcribing" ? "Transcribing audio…" : "Polishing with LLM…"}
              </span>
            </div>
            {liveText && (
              <p className="relative text-[14px] text-foreground leading-relaxed">
                {liveText}
                <span className="caret-blink" />
              </p>
            )}
          </div>
        )}

        {/* ─────────────────────────────────────────────────────────────────
           Single-column flow — full-width main content
           ─────────────────────────────────────────────────────────────── */}
        <div className="space-y-4">

          {/* Stat tiles — auto-fit so cards re-flow naturally as the
              window shrinks (4 → 3 → 2 → 1 across) */}
          <div
            className="grid gap-4"
            style={{
              gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))",
            }}
          >
            <HeroStat snapshot={snapshot} />
            <PaceCard snapshot={snapshot} />
            <DonutCard
              snapshot={snapshot}
              isRecording={isRecording}
              isProcessing={isProcessing}
            />
            <TimeSavedCard snapshot={snapshot} />
          </div>

          {/* Activity heatmap */}
          <ActivityHeatmap
            snapshot={snapshot}
            isRecording={isRecording}
            isProcessing={isProcessing}
            onToggle={onToggle}
            onView={() => onNavigate?.("history")}
          />

          {/* Recordings list */}
          <RecordingsTable
            recordings={recordings}
            onSeeAll={() => onNavigate?.("history")}
          />
        </div>

        {/* ── Accessibility nudge ────────────────────── */}
        {snapshot && !snapshot.accessibility_granted && snapshot.auto_paste_supported && (
          <div
            className="mt-5 rounded-2xl px-5 py-4 flex items-start justify-between gap-4"
            style={{
              background: "hsl(var(--chip-amber-bg))",
              boxShadow:  "inset 0 0 0 1px hsl(var(--chip-amber-fg) / 0.20)",
            }}
          >
            <div className="flex items-start gap-3">
              <span
                className="flex items-center justify-center w-8 h-8 rounded-lg flex-shrink-0 mt-0.5"
                style={{
                  background: "hsl(var(--chip-amber-fg) / 0.20)",
                  color:      "hsl(var(--chip-amber-fg))",
                }}
              >
                <Mic size={14} />
              </span>
              <div>
                <p className="text-[13px] font-semibold" style={{ color: "hsl(var(--chip-amber-fg))" }}>
                  Enable auto-paste
                </p>
                <p
                  className="text-[11.5px] mt-1 leading-relaxed"
                  style={{ color: "hsl(var(--chip-amber-fg) / 0.85)" }}
                >
                  Grant Accessibility access so Said can paste polished text directly into any app.
                </p>
              </div>
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
