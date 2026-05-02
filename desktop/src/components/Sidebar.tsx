import React, { useState, useRef, useEffect } from "react";
import {
  LayoutDashboard,
  History,
  BarChart2,
  BookOpen,
  Settings,
  HelpCircle,
  UserPlus,
  Gift,
  Mail,
  ChevronRight,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { AppSnapshot } from "@/types";

// ── Nav item type ──────────────────────────────────────────────────────────────

interface NavItem {
  id:       string;
  label:    string;
  icon:     React.ReactNode;
  badge?:   string;
  disabled?: boolean;
}

const GENERAL_NAV: NavItem[] = [
  { id: "dashboard",  label: "Dashboard",  icon: <LayoutDashboard size={15} /> },
  { id: "history",    label: "History",    icon: <History         size={15} /> },
  { id: "vocabulary", label: "Vocabulary", icon: <BookOpen        size={15} /> },
  { id: "insights",   label: "Insights",   icon: <BarChart2       size={15} />, badge: "New" },
];

// ── Nav button ─────────────────────────────────────────────────────────────────

function NavButton({
  item,
  isActive,
  onClick,
}: {
  item: NavItem;
  isActive: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      disabled={item.disabled}
      className={cn("nav-item", isActive && "active", item.disabled && "disabled")}
    >
      <span className="flex-shrink-0 opacity-70">{item.icon}</span>
      <span className="flex-1 truncate">{item.label}</span>
      {item.badge && (
        <span
          className="text-[9px] font-bold px-1.5 py-0.5 rounded flex-shrink-0"
          style={{
            color:      "hsl(var(--chip-cyan-fg))",
            background: "hsl(var(--chip-cyan-bg))",
          }}
        >
          {item.badge}
        </span>
      )}
    </button>
  );
}

// ── Help dropdown — opens upward, glass style ─────────────────────────────────

function HelpDropdown({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDocClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    document.addEventListener("mousedown", onDocClick);
    return () => document.removeEventListener("mousedown", onDocClick);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      ref={ref}
      className="absolute left-3 right-3 bottom-full mb-2 rounded-xl glass-strong overflow-hidden z-50"
      style={{ animation: "fadeIn 0.15s ease-out" }}
    >
      <button
        onClick={() => {
          // Open mailto link to sales — friendly, no telemetry
          window.open("mailto:sales@said.app?subject=Pricing inquiry", "_blank");
          onClose();
        }}
        className="w-full flex items-center gap-3 px-3.5 py-3 text-left transition-colors"
        onMouseEnter={(e) => {
          e.currentTarget.style.background = "hsl(var(--surface-hover))";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background = "transparent";
        }}
      >
        <span
          className="flex items-center justify-center w-8 h-8 rounded-lg flex-shrink-0"
          style={{
            background: "hsl(var(--primary) / 0.15)",
            color:      "hsl(var(--primary))",
          }}
        >
          <Mail size={14} />
        </span>
        <span className="flex-1 min-w-0">
          <span className="block text-[13px] font-semibold text-foreground leading-tight">
            Contact sales
          </span>
          <span className="block text-[11px] text-muted-foreground leading-tight mt-0.5">
            Pricing, teams &amp; enterprise
          </span>
        </span>
        <ChevronRight size={13} className="opacity-50 flex-shrink-0" />
      </button>
    </div>
  );
}

// ── Props ──────────────────────────────────────────────────────────────────────

interface SidebarProps {
  snapshot:     AppSnapshot | null;
  activeView:   string;
  onViewChange: (view: string) => void;
  busy:         boolean;
  onOpenInvite?: () => void;
}

// ── Component ──────────────────────────────────────────────────────────────────

export function Sidebar({ snapshot, activeView, onViewChange, busy, onOpenInvite }: SidebarProps) {
  const isRecording  = snapshot?.state === "recording";
  const isProcessing = snapshot?.state === "processing" || busy;
  const [helpOpen, setHelpOpen] = useState(false);

  return (
    <aside
      className="flex flex-col h-full overflow-hidden flex-shrink-0 relative"
      style={{
        width:      "var(--sidebar-width)",
        background: "hsl(var(--surface-1))",
      }}
    >
      {/* ── Brand header — drag region + traffic light space ── */}
      <div className="flex items-center h-[var(--topbar-height)] px-4 flex-shrink-0 drag-region">
        {/* 70px left pad for macOS native traffic lights */}
        <div className="w-[70px] flex-shrink-0" />

        {/* Brand mark — cyan tile with stylized quotation glyph */}
        <div className="no-drag" title="Said — Voice Polish Studio">
          <svg
            width="32"
            height="32"
            viewBox="0 0 32 32"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            {/* Rounded mint-green tile with subtle gradient */}
            <defs>
              <linearGradient id="brand-grad" x1="0" y1="0" x2="32" y2="32" gradientUnits="userSpaceOnUse">
                <stop offset="0%"  stopColor="hsl(105 80% 72%)" />
                <stop offset="100%" stopColor="hsl(160 70% 55%)" />
              </linearGradient>
            </defs>
            <rect width="32" height="32" rx="9" fill="url(#brand-grad)" />

            {/* Two opening curly-quote glyphs */}
            <path
              d="
                M 9.5 11
                C 9.5 9 11 8 12.5 8
                L 12.5 9.5
                C 11.7 9.5 11.2 10 11.2 10.7
                L 12.7 10.7
                C 13.4 10.7 13.7 11.2 13.7 12
                L 13.7 14.5
                C 13.7 15.3 13.2 15.8 12.4 15.8
                L 10.8 15.8
                C 10 15.8 9.5 15.3 9.5 14.5
                Z
                M 17.5 11
                C 17.5 9 19 8 20.5 8
                L 20.5 9.5
                C 19.7 9.5 19.2 10 19.2 10.7
                L 20.7 10.7
                C 21.4 10.7 21.7 11.2 21.7 12
                L 21.7 14.5
                C 21.7 15.3 21.2 15.8 20.4 15.8
                L 18.8 15.8
                C 18 15.8 17.5 15.3 17.5 14.5
                Z
              "
              fill="white"
            />

            {/* Voice motif baseline wave */}
            <path
              d="M 9 21 Q 12 19 14 21 T 19 21 T 23 21"
              stroke="white"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeOpacity="0.65"
              fill="none"
            />
          </svg>
        </div>
      </div>

      {/* ── Scrollable nav area ─────────────────────────────── */}
      <div className="flex-1 overflow-y-auto px-3 py-4 flex flex-col gap-6">

        {/* General section */}
        <section>
          <p className="section-label px-3 mb-2">General</p>
          <div className="space-y-0.5">
            {GENERAL_NAV.map((item) => (
              <NavButton
                key={item.id}
                item={item}
                isActive={activeView === item.id}
                onClick={() => !busy && onViewChange(item.id)}
              />
            ))}
          </div>
        </section>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Status card — glass */}
        <div className="rounded-xl glass p-3.5">
          <div className="flex items-center gap-2 mb-1.5">
            <div
              className={cn(
                "w-1.5 h-1.5 rounded-full flex-shrink-0",
                isRecording  ? "animate-pulse"   :
                isProcessing ? "animate-pulse" : ""
              )}
              style={{
                background: isRecording  ? "hsl(var(--recording))"
                          : isProcessing ? "hsl(38 90% 55%)"
                          :                "hsl(var(--primary))",
                boxShadow:  isRecording  ? "0 0 8px hsl(var(--recording) / 0.6)"
                          : isProcessing ? "0 0 8px hsl(38 90% 55% / 0.6)"
                          :                "0 0 8px hsl(var(--primary) / 0.5)",
              }}
            />
            <span className="text-[11px] font-semibold text-foreground tracking-wide">
              {isRecording  ? "RECORDING"  :
               isProcessing ? "PROCESSING" :
                              "READY"}
            </span>
          </div>
          <p className="text-[11px] text-muted-foreground leading-relaxed tabular-nums">
            {snapshot
              ? `${snapshot.total_words.toLocaleString()} words · ${snapshot.daily_streak}d streak`
              : "Loading…"}
          </p>
        </div>
      </div>

      {/* ── Footer nav — Invite / Free month / Settings / Help ─────── */}
      <div className="px-3 py-3 flex-shrink-0 space-y-0.5 relative">

        {/* Help dropdown — anchored above the buttons */}
        <HelpDropdown open={helpOpen} onClose={() => setHelpOpen(false)} />

        {/* Invite team — opens the in-app modal */}
        <button
          className="nav-item"
          onClick={() => onOpenInvite?.()}
        >
          <span className="flex-shrink-0 opacity-70">
            <UserPlus size={15} />
          </span>
          <span className="flex-1 truncate">Invite your team</span>
        </button>

        {/* Get a free month — promotional, cyan accent on hover */}
        <button
          className="nav-item group"
          onClick={() => {
            window.open(
              "mailto:?subject=Get a free month of Said&body=Refer a friend, both of you get a free month of Said Pro. https://said.app/refer",
              "_blank"
            );
          }}
        >
          <span
            className="flex items-center justify-center w-[18px] h-[18px] rounded flex-shrink-0"
            style={{
              background: "hsl(var(--primary) / 0.18)",
              color:      "hsl(var(--primary))",
            }}
          >
            <Gift size={11} />
          </span>
          <span className="flex-1 truncate">Get a free month</span>
        </button>

        {/* Settings — same as before, navigates to settings view */}
        <button
          className={cn("nav-item", activeView === "settings" && "active")}
          onClick={() => onViewChange("settings")}
        >
          <span className="flex-shrink-0 opacity-70">
            <Settings size={15} />
          </span>
          <span className="flex-1 truncate">Settings</span>
        </button>

        {/* Help — opens "Contact sales" dropdown */}
        <button
          className={cn("nav-item", helpOpen && "active")}
          onClick={() => setHelpOpen((o) => !o)}
        >
          <span className="flex-shrink-0 opacity-70">
            <HelpCircle size={15} />
          </span>
          <span className="flex-1 truncate">Help</span>
          <ChevronRight
            size={11}
            className="opacity-50 flex-shrink-0 transition-transform"
            style={{ transform: helpOpen ? "rotate(90deg)" : "rotate(0deg)" }}
          />
        </button>
      </div>
    </aside>
  );
}
