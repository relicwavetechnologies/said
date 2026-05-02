import React, { useEffect, useRef, useState } from "react";
import {
  X, RefreshCw, Gift, ArrowRight, Check, Loader2,
} from "lucide-react";

/* ════════════════════════════════════════════════════════════════════════════
   InviteTeamModal — single-pane modal focused on team invitation.
   Sidebar removed: this modal only does one thing (invite teammates),
   so the misleading nav (which didn't actually navigate anywhere) is gone.
   Other Settings live in SettingsModal.
   ════════════════════════════════════════════════════════════════════════════ */

interface Props {
  open:   boolean;
  onClose: () => void;
}

const VALID_EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function InviteTeamModal({ open, onClose }: Props) {
  const [emails,      setEmails]      = useState<string[]>(["", "", ""]);
  const [submitting,  setSubmitting]  = useState(false);
  const [submitted,   setSubmitted]   = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);

  // Close on ESC
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  // Reset state when reopened
  useEffect(() => {
    if (open) {
      setEmails(["", "", ""]);
      setSubmitting(false);
      setSubmitted(false);
    }
  }, [open]);

  if (!open) return null;

  const filledEmails = emails.map((e) => e.trim()).filter(Boolean);
  const validEmails  = filledEmails.filter((e) => VALID_EMAIL.test(e));
  const canContinue  = validEmails.length > 0 && validEmails.length === filledEmails.length;

  async function handleContinue() {
    if (!canContinue || submitting) return;
    setSubmitting(true);
    // Stub: open mailto with all addresses prefilled
    const to = validEmails.join(",");
    const subject = encodeURIComponent("Try Said — voice that sounds like you");
    const body    = encodeURIComponent(
      "Hey — I've been using Said to dictate and polish text. " +
      "Thought you'd like it: https://said.app"
    );
    window.open(`mailto:${to}?subject=${subject}&body=${body}`, "_blank");
    await new Promise((r) => setTimeout(r, 500));
    setSubmitting(false);
    setSubmitted(true);
    setTimeout(() => onClose(), 1200);
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{
        background: "hsl(220 50% 2% / 0.55)",
        backdropFilter: "blur(8px)",
        WebkitBackdropFilter: "blur(8px)",
        animation: "fadeIn 0.18s ease-out",
      }}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        className="rounded-[20px] overflow-hidden flex"
        style={{
          background: "hsl(var(--surface-2))",
          width:  "min(640px, 92vw)",
          height: "min(620px, 90vh)",
          boxShadow:
            "0 1px 0 hsl(0 0% 100% / 0.06) inset, 0 30px 80px hsl(220 60% 2% / 0.65)",
        }}
      >

        {/* Single content pane — sidebar removed; this modal does one thing. */}
        <main className="flex-1 flex flex-col min-w-0 relative overflow-hidden">

          {/* Subtle radial wash top-right (matches our app's mesh aesthetic) */}
          <div
            aria-hidden
            className="absolute pointer-events-none"
            style={{
              right: -120, top: -120, width: 360, height: 360, borderRadius: "50%",
              background: "radial-gradient(circle, hsl(var(--accent-violet) / 0.12) 0%, transparent 70%)",
            }}
          />

          {/* Header */}
          <header
            className="relative flex items-center justify-between px-7 py-5"
            style={{ borderBottom: "1px solid hsl(var(--surface-4))" }}
          >
            <h2
              className="text-[24px] font-extrabold tracking-tight"
              style={{
                color: "hsl(var(--foreground))",
                letterSpacing: "-0.02em",
              }}
            >
              Team
            </h2>
            <div className="flex items-center gap-2">
              <button
                title="Refresh"
                className="w-8 h-8 rounded-full flex items-center justify-center transition-colors"
                style={{ color: "hsl(var(--muted-foreground))" }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = "hsl(var(--surface-4))";
                  e.currentTarget.style.color      = "hsl(var(--foreground))";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "transparent";
                  e.currentTarget.style.color      = "hsl(var(--muted-foreground))";
                }}
              >
                <RefreshCw size={14} />
              </button>
              <button
                onClick={onClose}
                title="Close"
                className="w-8 h-8 rounded-full flex items-center justify-center transition-colors"
                style={{ color: "hsl(var(--muted-foreground))" }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = "hsl(var(--surface-4))";
                  e.currentTarget.style.color      = "hsl(var(--foreground))";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "transparent";
                  e.currentTarget.style.color      = "hsl(var(--muted-foreground))";
                }}
              >
                <X size={14} />
              </button>
            </div>
          </header>

          {/* Body */}
          <div className="relative flex-1 overflow-y-auto px-10 py-7 flex flex-col items-center">

            {/* Pitch line */}
            <p
              className="text-[14px] text-center max-w-[480px] leading-relaxed mb-4"
              style={{ color: "hsl(var(--muted-foreground))" }}
            >
              Unlock shared snippets, dictionary, unified billing, admin controls and more.
            </p>

            {/* Promo pill — violet glow chip */}
            <div
              className="inline-flex items-center gap-2 px-4 py-2 rounded-full mb-7"
              style={{
                background: "hsl(var(--accent-violet) / 0.16)",
                color:      "hsl(var(--accent-violet))",
                boxShadow:  "inset 0 0 0 1px hsl(var(--accent-violet) / 0.25)",
              }}
            >
              <Gift size={14} />
              <span className="text-[12.5px] font-semibold">
                You'll get a new Pro trial when you create a team!
              </span>
            </div>

            {/* Section heading */}
            <h3
              className="text-[18px] font-extrabold tracking-tight mb-5 text-center"
              style={{
                color: "hsl(var(--foreground))",
                letterSpacing: "-0.02em",
              }}
            >
              Invite your teammates
            </h3>

            {/* Email inputs */}
            <div className="w-full max-w-[420px] space-y-2.5 mb-4">
              {emails.map((email, i) => {
                const trimmed = email.trim();
                const looksValid = trimmed === "" || VALID_EMAIL.test(trimmed);
                return (
                  <input
                    key={i}
                    type="email"
                    value={email}
                    onChange={(e) => {
                      const next = [...emails];
                      next[i] = e.target.value;
                      setEmails(next);
                    }}
                    placeholder={`teammate${i + 1}@company.com`}
                    autoComplete="email"
                    className="w-full px-4 py-3 rounded-xl text-[13.5px] transition-all"
                    style={{
                      background: "hsl(var(--surface-3))",
                      color:      "hsl(var(--foreground))",
                      boxShadow:  looksValid
                        ? "inset 0 0 0 1px hsl(var(--surface-4))"
                        : "inset 0 0 0 1px hsl(354 78% 60% / 0.5)",
                      outline:    "none",
                    }}
                    onFocus={(e) => {
                      e.currentTarget.style.boxShadow = looksValid
                        ? "inset 0 0 0 1px hsl(var(--accent-violet) / 0.6), 0 0 0 3px hsl(var(--accent-violet) / 0.12)"
                        : "inset 0 0 0 1px hsl(354 78% 60% / 0.6), 0 0 0 3px hsl(354 78% 60% / 0.12)";
                    }}
                    onBlur={(e) => {
                      e.currentTarget.style.boxShadow = looksValid
                        ? "inset 0 0 0 1px hsl(var(--surface-4))"
                        : "inset 0 0 0 1px hsl(354 78% 60% / 0.5)";
                    }}
                  />
                );
              })}
            </div>

            {/* Helper line */}
            <p
              className="text-[12px] text-center mb-6"
              style={{ color: "hsl(var(--muted-foreground))" }}
            >
              You can also add team members after team creation.
            </p>

            {/* Continue CTA */}
            <div className="w-full max-w-[420px] flex justify-end">
              <button
                onClick={handleContinue}
                disabled={!canContinue || submitting || submitted}
                className="inline-flex items-center gap-2 px-5 py-2.5 rounded-full text-[13px] font-semibold transition-all"
                style={{
                  background: submitted
                    ? "hsl(var(--primary))"
                    : canContinue
                    ? "hsl(var(--pill-active-bg))"
                    : "hsl(var(--surface-4))",
                  color: submitted
                    ? "hsl(var(--primary-foreground))"
                    : canContinue
                    ? "hsl(var(--pill-active-fg))"
                    : "hsl(var(--muted-foreground))",
                  cursor: !canContinue || submitting || submitted ? "default" : "pointer",
                  boxShadow: canContinue && !submitted
                    ? "0 6px 18px hsl(var(--pill-active-bg) / 0.30)"
                    : "none",
                  opacity: !canContinue && !submitted ? 0.7 : 1,
                }}
              >
                {submitting ? (
                  <>
                    <Loader2 size={13} className="animate-spin" />
                    Sending…
                  </>
                ) : submitted ? (
                  <>
                    <Check size={13} strokeWidth={2.5} />
                    Sent!
                  </>
                ) : (
                  <>
                    Continue
                    <ArrowRight size={13} />
                  </>
                )}
              </button>
            </div>
          </div>

          {/* Footer */}
          <footer
            className="relative flex items-center justify-between px-7 py-4 flex-shrink-0"
            style={{ borderTop: "1px solid hsl(var(--surface-4))" }}
          >
            <a
              href="#"
              onClick={(e) => { e.preventDefault(); }}
              className="text-[12px] font-medium transition-colors"
              style={{ color: "hsl(var(--muted-foreground))" }}
              onMouseEnter={(e) => { e.currentTarget.style.color = "hsl(var(--foreground))"; }}
              onMouseLeave={(e) => { e.currentTarget.style.color = "hsl(var(--muted-foreground))"; }}
            >
              Teams FAQ
            </a>
            <a
              href="mailto:sales@said.app?subject=Teams pricing"
              className="text-[12px] font-medium"
              style={{ color: "hsl(var(--muted-foreground))" }}
              onMouseEnter={(e) => { e.currentTarget.style.color = "hsl(var(--foreground))"; }}
              onMouseLeave={(e) => { e.currentTarget.style.color = "hsl(var(--muted-foreground))"; }}
            >
              More questions about teams?{" "}
              <span style={{ color: "hsl(var(--accent-violet))", fontWeight: 600 }}>
                Contact us
              </span>
            </a>
          </footer>
        </main>
      </div>
    </div>
  );
}
