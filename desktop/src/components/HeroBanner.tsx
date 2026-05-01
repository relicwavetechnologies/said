import React from "react";
import { ArrowRight } from "lucide-react";

interface Props {
  /** Friendly greeting prefix (defaults to time-of-day) */
  greeting?:   string;
  /** Action when the CTA is clicked (typically: navigate to Settings) */
  onCustomize: () => void;
}

function timeOfDayGreeting(): string {
  const h = new Date().getHours();
  if (h < 5)  return "Late night session";
  if (h < 12) return "Good morning";
  if (h < 17) return "Good afternoon";
  if (h < 21) return "Good evening";
  return "Working late";
}

/**
 * Hero banner — sits at the top of the Dashboard.
 *
 * - Personal greeting line ("Hey, get back into the flow with ⇪ Caps Lock")
 * - Dark gradient hero card with serif italic headline + CTA button
 * - Decorative gradient blobs on the right (lime + cool accent)
 *
 * The hero card always uses the dark gradient regardless of theme — it's the
 * visual focal point at the top of the page.
 */
export function HeroBanner({ greeting, onCustomize }: Props) {
  const hello = greeting ?? timeOfDayGreeting();

  return (
    <div className="mb-7">
      {/* ── Greeting line ───────────────────────────────────────── */}
      <p className="flex items-center gap-2 text-[14px] mb-3"
         style={{ color: "hsl(var(--muted-foreground))" }}>
        <span style={{ color: "hsl(var(--foreground))", fontWeight: 600 }}>{hello}</span>
        <span>— ready to talk? Hold</span>
        {/* Caps Lock key badge */}
        <span
          className="inline-flex items-center justify-center px-2 py-0.5 rounded-md text-[11px] font-bold tabular-nums"
          style={{
            background: "hsl(var(--primary))",
            color:      "hsl(var(--primary-foreground))",
            letterSpacing: "0.02em",
          }}
        >
          ⇪ Caps Lock
        </span>
      </p>

      {/* ── Hero card ────────────────────────────────────────────── */}
      <div
        className="relative overflow-hidden rounded-2xl"
        style={{
          /* Distinctly brighter than the mat (240 5% 8%) and warmly tinted
             so the banner reads as a focal element in dark mode.            */
          background:
            "linear-gradient(135deg, hsl(265 25% 16%) 0%, hsl(240 12% 14%) 50%, hsl(73 30% 12%) 100%)",
          minHeight:  "180px",
        }}
      >
        {/* ── Decorative gradient blobs (right side) ───────────── */}
        <div
          aria-hidden
          className="absolute pointer-events-none"
          style={{
            right:      "-100px",
            top:        "-80px",
            width:      "340px",
            height:     "340px",
            borderRadius: "50%",
            background: "radial-gradient(circle, hsl(73 80% 67% / 0.55) 0%, transparent 65%)",
            filter:     "blur(6px)",
          }}
        />
        <div
          aria-hidden
          className="absolute pointer-events-none"
          style={{
            right:      "-140px",
            bottom:     "-110px",
            width:      "320px",
            height:     "320px",
            borderRadius: "50%",
            background: "radial-gradient(circle, hsl(265 75% 60% / 0.45) 0%, transparent 65%)",
            filter:     "blur(8px)",
          }}
        />

        {/* ── Subtle waveform bars on the right (voice motif) ── */}
        <div
          aria-hidden
          className="absolute right-10 top-1/2 -translate-y-1/2 hidden md:flex items-center gap-1 pointer-events-none"
          style={{ opacity: 0.55 }}
        >
          {[28, 56, 38, 72, 44, 88, 50, 64, 32, 76, 40, 22].map((h, i) => (
            <span
              key={i}
              style={{
                width:        "3px",
                height:       `${h}px`,
                borderRadius: "2px",
                background:   "hsl(73 80% 75% / 0.6)",
              }}
            />
          ))}
        </div>

        {/* ── Content ───────────────────────────────────────────── */}
        <div className="relative px-7 py-8 max-w-[460px]">
          <h2
            className="text-[28px] font-bold tracking-tight leading-[1.15]"
            style={{ color: "white" }}
          >
            Make Said sound like{" "}
            <span
              className="italic"
              style={{
                color:      "hsl(var(--primary))",
                fontWeight: 700,
                fontFamily: '"Inter", serif',
              }}
            >
              you
            </span>
          </h2>
          <p
            className="text-[13px] mt-2.5 mb-5 leading-relaxed"
            style={{ color: "hsl(0 0% 70%)", maxWidth: "360px" }}
          >
            Pick a tone, write a custom persona, and choose your output
            language — Said learns from every edit you make.
          </p>

          <button
            onClick={onCustomize}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-full text-[13px] font-semibold transition-all"
            style={{
              background: "hsl(0 0% 96%)",
              color:      "hsl(240 8% 8%)",
            }}
            onMouseEnter={(e) => { e.currentTarget.style.filter = "brightness(0.96)"; }}
            onMouseLeave={(e) => { e.currentTarget.style.filter = "none"; }}
          >
            <span
              className="w-2 h-2 rounded-full flex-shrink-0"
              style={{ background: "hsl(var(--primary))" }}
            />
            Customize your voice
            <ArrowRight size={13} className="opacity-60" />
          </button>
        </div>
      </div>
    </div>
  );
}
