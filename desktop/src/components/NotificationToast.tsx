import React, { useEffect } from "react";
import { X, RotateCcw, Check, Sparkles, BookOpen, Star, Undo2 } from "lucide-react";
import { cn } from "@/lib/utils";

// ── Retry Toast ───────────────────────────────────────────────────────────────

interface RetryToastProps {
  message:  string;
  onRetry:  () => void;
  onDismiss: () => void;
}

export function RetryToast({ message, onRetry, onDismiss }: RetryToastProps) {
  return (
    <div
      className="fixed bottom-5 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 px-4 py-3 rounded-2xl shadow-xl max-w-sm w-max"
      style={{
        background:  "hsl(var(--surface-3))",
        border:      "1px solid hsl(var(--border))",
        boxShadow:   "0 8px 32px hsl(0 0% 0% / 0.28)",
      }}
    >
      {/* Red dot */}
      <span
        className="w-2 h-2 rounded-full flex-shrink-0"
        style={{ background: "hsl(0 70% 60%)" }}
      />

      {/* Message */}
      <p className="text-[12px] text-foreground leading-snug max-w-[200px] truncate" title={message}>
        {message}
      </p>

      {/* Actions */}
      <div className="flex items-center gap-1.5 flex-shrink-0 ml-1">
        <button
          onClick={onRetry}
          className="flex items-center gap-1 px-2.5 py-1 rounded-lg text-[11px] font-semibold transition-colors"
          style={{
            background: "hsl(var(--primary))",
            color:      "hsl(var(--primary-foreground))",
          }}
        >
          <RotateCcw size={11} />
          Retry
        </button>
        <button
          onClick={onDismiss}
          className="w-6 h-6 rounded-lg flex items-center justify-center transition-colors opacity-50 hover:opacity-100"
          style={{ background: "hsl(var(--surface-4))" }}
        >
          <X size={11} />
        </button>
      </div>
    </div>
  );
}

// ── Edit Confirmation Toast ───────────────────────────────────────────────────

interface EditToastProps {
  aiOutput:    string;
  userKept:    string;
  onSave:      () => void;
  onDismiss:   () => void;
}

export function EditConfirmToast({ aiOutput, userKept, onSave, onDismiss }: EditToastProps) {
  const aiShort   = aiOutput.length  > 60 ? aiOutput.slice(0, 60)  + "…" : aiOutput;
  const keptShort = userKept.length  > 60 ? userKept.slice(0, 60)  + "…" : userKept;

  return (
    <div
      className="fixed bottom-5 left-1/2 -translate-x-1/2 z-50 flex flex-col gap-3 px-4 py-3.5 rounded-2xl shadow-xl"
      style={{
        background:  "hsl(var(--surface-3))",
        border:      "1px solid hsl(var(--border))",
        boxShadow:   "0 8px 32px hsl(0 0% 0% / 0.28)",
        minWidth:    "280px",
        maxWidth:    "360px",
      }}
    >
      {/* Header */}
      <div className="flex items-center justify-between gap-2">
        <p className="text-[12px] font-semibold text-foreground">Save this edit?</p>
        <button
          onClick={onDismiss}
          className="w-5 h-5 rounded flex items-center justify-center opacity-50 hover:opacity-100 transition-opacity"
        >
          <X size={10} />
        </button>
      </div>

      {/* Diff preview */}
      <div className="flex flex-col gap-1.5 text-[11px]">
        <div className="flex gap-2 items-start">
          <span className="flex-shrink-0 font-semibold opacity-50 w-5 text-right">AI</span>
          <span
            className="flex-1 leading-snug px-2 py-1 rounded-lg text-muted-foreground line-through"
            style={{ background: "hsl(0 50% 50% / 0.1)" }}
            title={aiOutput}
          >
            {aiShort}
          </span>
        </div>
        <div className="flex gap-2 items-start">
          <span className="flex-shrink-0 font-semibold opacity-50 w-5 text-right">You</span>
          <span
            className="flex-1 leading-snug px-2 py-1 rounded-lg text-foreground"
            style={{ background: "hsl(var(--chip-lime-fg) / 0.1)", color: "hsl(var(--chip-lime-fg))" }}
            title={userKept}
          >
            {keptShort}
          </span>
        </div>
      </div>

      <p className="text-[10px] text-muted-foreground leading-relaxed opacity-70">
        Said will remember this style for future recordings.
      </p>

      {/* Buttons */}
      <div className={cn("flex gap-2")}>
        <button
          onClick={onDismiss}
          className="flex-1 py-1.5 rounded-xl text-[11px] font-medium transition-colors text-muted-foreground"
          style={{ background: "hsl(var(--surface-4))" }}
        >
          Skip
        </button>
        <button
          onClick={onSave}
          className="flex-1 py-1.5 rounded-xl text-[11px] font-semibold transition-colors flex items-center justify-center gap-1"
          style={{
            background: "hsl(var(--primary))",
            color:      "hsl(var(--primary-foreground))",
          }}
        >
          <Check size={11} />
          Save preference
        </button>
      </div>
    </div>
  );
}

// ── Vocabulary Added Toast ────────────────────────────────────────────────────
//
// Replaces the previous OS-level "looks like a log" notification with an
// in-app toast that matches RetryToast's design exactly: bottom-center,
// surface-3 panel, drop shadow, dismiss + primary action affordances.
//
// Variants:
//   • "added"   — auto-promoted from STT_ERROR or manually added
//   • "starred" — pinned by the user
//   • "removed" — deleted (used by Undo confirmation)
//
// Auto-dismisses after 6 seconds; the host (App.tsx) controls visibility.

export type VocabToastKind = "added" | "starred" | "removed";

interface VocabToastProps {
  kind:     VocabToastKind;
  term:     string;
  source?:  "auto" | "manual" | "starred";   // present for "added"
  onUndo?:  () => void;
  onDismiss: () => void;
}

export function VocabularyToast({ kind, term, source, onUndo, onDismiss }: VocabToastProps) {
  // Auto-dismiss after 6s. User can interact with Undo before then.
  useEffect(() => {
    const t = setTimeout(onDismiss, 6000);
    return () => clearTimeout(t);
  }, [onDismiss]);

  // Icon + accent color per kind ───────────────────────────────────────────
  const accent = (() => {
    if (kind === "starred") {
      return {
        icon:  <Star size={11} fill="currentColor" />,
        color: "hsl(var(--chip-amber-fg))",
        bg:    "hsl(var(--chip-amber-bg))",
      };
    }
    if (kind === "removed") {
      return {
        icon:  <BookOpen size={11} />,
        color: "hsl(var(--muted-foreground))",
        bg:    "hsl(var(--surface-4))",
      };
    }
    // added — sparkle if auto, plus if manual
    return {
      icon:  source === "manual"
        ? <BookOpen size={11} />
        : <Sparkles size={11} />,
      color: "hsl(var(--chip-mint-fg))",
      bg:    "hsl(var(--chip-mint-bg))",
    };
  })();

  // Headline text per kind ─────────────────────────────────────────────────
  const headline = (() => {
    if (kind === "starred") return "Pinned to vocabulary";
    if (kind === "removed") return "Removed from vocabulary";
    return source === "manual"
      ? "Added to vocabulary"
      : "Said learned a new word";
  })();

  const subtle = (() => {
    if (kind === "starred") return "Protected from automatic demotion";
    if (kind === "removed") return "Won't bias future recordings";
    return source === "manual"
      ? "Will bias your next recording"
      : "Auto-learned from your edit";
  })();

  return (
    <div
      className="fixed bottom-5 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 px-4 py-3 rounded-2xl shadow-xl max-w-sm"
      style={{
        background:  "hsl(var(--surface-3))",
        border:      "1px solid hsl(var(--border))",
        boxShadow:   "0 8px 32px hsl(0 0% 0% / 0.28)",
        animation:   "fadeIn 0.18s ease-out",
      }}
    >
      {/* Accent circle with kind icon */}
      <span
        className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0"
        style={{ background: accent.bg, color: accent.color }}
      >
        {accent.icon}
      </span>

      {/* Two-line message */}
      <div className="flex-1 min-w-0">
        <p className="text-[12px] font-semibold text-foreground leading-tight">
          {headline}
        </p>
        <p className="text-[11px] text-muted-foreground leading-tight mt-0.5 truncate">
          <span
            className="font-mono px-1.5 py-0.5 rounded"
            style={{ background: "hsl(var(--surface-4))" }}
          >
            {term}
          </span>
          <span className="ml-1.5">· {subtle}</span>
        </p>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1.5 flex-shrink-0">
        {onUndo && (
          <button
            onClick={onUndo}
            className="flex items-center gap-1 px-2.5 py-1 rounded-lg text-[11px] font-semibold transition-colors"
            style={{
              background: "hsl(var(--surface-4))",
              color:      "hsl(var(--foreground))",
            }}
            onMouseEnter={(e) => { e.currentTarget.style.background = "hsl(var(--surface-hover))"; }}
            onMouseLeave={(e) => { e.currentTarget.style.background = "hsl(var(--surface-4))"; }}
          >
            <Undo2 size={11} />
            Undo
          </button>
        )}
        <button
          onClick={onDismiss}
          className="w-6 h-6 rounded-lg flex items-center justify-center transition-colors opacity-50 hover:opacity-100"
          style={{ background: "hsl(var(--surface-4))" }}
        >
          <X size={11} />
        </button>
      </div>
    </div>
  );
}
