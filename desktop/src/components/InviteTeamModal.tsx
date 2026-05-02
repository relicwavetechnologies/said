import React, { useEffect, useRef, useState } from "react";
import { X, Send, Check, Loader2, Heart } from "lucide-react";

/* ════════════════════════════════════════════════════════════════════════════
   InviteTeamModal — single-input "invite a friend" modal.
   Free product, no team/billing pitch — just one warm ask.
   ════════════════════════════════════════════════════════════════════════════ */

interface Props {
  open:   boolean;
  onClose: () => void;
}

const VALID_EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function InviteTeamModal({ open, onClose }: Props) {
  const [email,      setEmail]      = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [submitted,  setSubmitted]  = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  useEffect(() => {
    if (open) {
      setEmail("");
      setSubmitting(false);
      setSubmitted(false);
      setTimeout(() => inputRef.current?.focus(), 60);
    }
  }, [open]);

  if (!open) return null;

  const trimmed   = email.trim();
  const looksValid = trimmed === "" || VALID_EMAIL.test(trimmed);
  const canSend    = trimmed !== "" && VALID_EMAIL.test(trimmed);

  async function handleSend() {
    if (!canSend || submitting) return;
    setSubmitting(true);
    const subject = encodeURIComponent("You should try Said");
    const body    = encodeURIComponent(
      "Hey — I've been using Said to dictate and polish text. " +
      "It's quietly become my favourite way to write. " +
      "Thought you'd like it: https://said.app"
    );
    window.open(`mailto:${trimmed}?subject=${subject}&body=${body}`, "_blank");
    await new Promise((r) => setTimeout(r, 450));
    setSubmitting(false);
    setSubmitted(true);
    setTimeout(() => onClose(), 1100);
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
        className="rounded-[20px] overflow-hidden flex flex-col relative"
        style={{
          background: "hsl(var(--surface-2))",
          width:  "min(460px, 92vw)",
          boxShadow:
            "0 1px 0 hsl(0 0% 100% / 0.06) inset, 0 30px 80px hsl(220 60% 2% / 0.65)",
        }}
      >
        {/* Subtle violet wash top-right */}
        <div
          aria-hidden
          className="absolute pointer-events-none"
          style={{
            right: -100, top: -100, width: 280, height: 280, borderRadius: "50%",
            background: "radial-gradient(circle, hsl(var(--accent-violet) / 0.14) 0%, transparent 70%)",
          }}
        />

        {/* Close button — floats top-right */}
        <button
          onClick={onClose}
          title="Close"
          className="absolute top-4 right-4 z-10 w-8 h-8 rounded-full flex items-center justify-center transition-colors"
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

        {/* Body */}
        <div className="relative px-8 pt-10 pb-8 flex flex-col items-center">

          {/* Heart icon chip */}
          <div
            className="w-12 h-12 rounded-2xl flex items-center justify-center mb-5"
            style={{
              background: "hsl(var(--accent-violet) / 0.16)",
              color:      "hsl(var(--accent-violet))",
              boxShadow:  "inset 0 0 0 1px hsl(var(--accent-violet) / 0.25)",
            }}
          >
            <Heart size={20} strokeWidth={2.2} />
          </div>

          {/* Headline */}
          <h2
            className="text-[22px] font-extrabold tracking-tight text-center mb-2"
            style={{
              color: "hsl(var(--foreground))",
              letterSpacing: "-0.02em",
            }}
          >
            Invite a friend
          </h2>

          {/* Sub-line */}
          <p
            className="text-[13.5px] text-center max-w-[340px] leading-relaxed mb-7"
            style={{ color: "hsl(var(--muted-foreground))" }}
          >
            Said is free while we're early. If someone in your life would love it, send them a note.
          </p>

          {/* Email input */}
          <div className="w-full mb-3">
            <input
              ref={inputRef}
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleSend(); }}
              placeholder="friend@example.com"
              autoComplete="email"
              className="w-full px-4 py-3 rounded-xl text-[14px] transition-all"
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
          </div>

          {/* Send button — full width, matches our pill-active style */}
          <button
            onClick={handleSend}
            disabled={!canSend || submitting || submitted}
            className="w-full inline-flex items-center justify-center gap-2 px-5 py-3 rounded-xl text-[13.5px] font-semibold transition-all"
            style={{
              background: submitted
                ? "hsl(var(--primary))"
                : canSend
                ? "hsl(var(--pill-active-bg))"
                : "hsl(var(--surface-4))",
              color: submitted
                ? "hsl(var(--primary-foreground))"
                : canSend
                ? "hsl(var(--pill-active-fg))"
                : "hsl(var(--muted-foreground))",
              cursor: !canSend || submitting || submitted ? "default" : "pointer",
              boxShadow: canSend && !submitted
                ? "0 6px 18px hsl(var(--pill-active-bg) / 0.30)"
                : "none",
              opacity: !canSend && !submitted ? 0.7 : 1,
            }}
          >
            {submitting ? (
              <>
                <Loader2 size={14} className="animate-spin" />
                Opening mail…
              </>
            ) : submitted ? (
              <>
                <Check size={14} strokeWidth={2.5} />
                Sent
              </>
            ) : (
              <>
                <Send size={13} />
                Send invite
              </>
            )}
          </button>

          {/* Tiny footnote */}
          <p
            className="text-[11.5px] text-center mt-5"
            style={{ color: "hsl(var(--muted-foreground))", opacity: 0.75 }}
          >
            Opens your mail app with a short note already written.
          </p>
        </div>
      </div>
    </div>
  );
}
