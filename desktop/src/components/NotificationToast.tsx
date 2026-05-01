import React from "react";
import { X, RotateCcw, Check } from "lucide-react";
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
