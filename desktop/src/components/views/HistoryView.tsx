import React, { useMemo } from "react";
import { Clock, Tag } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { groupHistory } from "@/types";
import type { AppSnapshot } from "@/types";

interface HistoryViewProps {
  snapshot: AppSnapshot | null;
}

export function HistoryView({ snapshot }: HistoryViewProps) {
  const history  = snapshot?.history ?? [];
  const timeline = useMemo(() => groupHistory(history), [history]);

  /* ── Empty state ──────────────────────────────────────────── */
  if (history.length === 0) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-center px-8">
          <div
            className="w-12 h-12 rounded-full flex items-center justify-center mx-auto mb-4"
            style={{ background: "hsl(var(--primary) / 0.15)" }}
          >
            <Clock size={20} style={{ color: "hsl(var(--chip-lime-fg))" }} />
          </div>
          <p className="text-[14px] font-semibold text-foreground mb-1">No history yet</p>
          <p className="text-[12px] text-muted-foreground max-w-xs leading-relaxed">
            Your recordings will appear here after your first session.
          </p>
        </div>
      </div>
    );
  }

  return (
    <ScrollArea className="h-full">
      <div className="p-7 pb-12 max-w-3xl mx-auto">

        {/* ── Header ──────────────────────────────────── */}
        <div className="mb-7">
          <h1 className="text-[28px] font-bold tracking-tight text-foreground leading-tight">
            History
          </h1>
          <p className="text-[13px] text-muted-foreground mt-1 tabular-nums">
            {history.length} total recording{history.length !== 1 ? "s" : ""}
          </p>
        </div>

        {/* ── Timeline groups ──────────────────────────── */}
        <div className="space-y-7">
          {timeline.map((group) => (
            <div key={group.label}>
              {/* Date label */}
              <div className="flex items-center justify-between mb-3 px-1">
                <span className="section-label">{group.label}</span>
                <span className="text-[10px] text-muted-foreground tabular-nums">
                  {group.items.length} {group.items.length === 1 ? "recording" : "recordings"}
                </span>
              </div>

              {/* Card list */}
              <div className="tile overflow-hidden">
                {group.items.map((item, idx) => (
                  <div
                    key={idx}
                    className="flex gap-4 px-5 py-4 transition-colors"
                    onMouseEnter={(e) => { e.currentTarget.style.background = "hsl(var(--surface-hover))"; }}
                    onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
                  >
                    {/* Timestamp */}
                    <div className="w-20 flex-shrink-0 pt-0.5">
                      <div className="flex items-center gap-1 text-[11px] text-muted-foreground tabular-nums">
                        <Clock size={10} className="opacity-70" />
                        <span>{item.time}</span>
                      </div>
                    </div>

                    {/* Content */}
                    <div className="flex-1 min-w-0">
                      <p className="text-[14px] text-foreground leading-relaxed">
                        {item.text || (
                          <span className="italic text-muted-foreground">—</span>
                        )}
                      </p>

                      {/* Meta row */}
                      <div className="flex items-center gap-3 mt-2 flex-wrap">
                        {item.word_count != null && (
                          <span className="text-[11px] text-muted-foreground tabular-nums">
                            {item.word_count} words
                          </span>
                        )}
                        {item.model && (
                          <span className="flex items-center gap-1 text-[11px] text-muted-foreground">
                            <Tag size={9} className="opacity-70" />
                            {item.model}
                          </span>
                        )}
                      </div>
                    </div>

                    {/* Right badge */}
                    <div className="flex-shrink-0 flex items-start pt-0.5">
                      <span className="badge-done">Polished</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </ScrollArea>
  );
}
