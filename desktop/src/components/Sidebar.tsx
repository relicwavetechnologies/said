import React from "react";
import {
  LayoutDashboard,
  History,
  BarChart2,
  BookOpen,
  Settings,
  HelpCircle,
  UserPlus,
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

  return (
    <aside
      className="flex flex-col h-full overflow-hidden flex-shrink-0 relative"
      style={{
        width:      "var(--sidebar-width)",
        background: "hsl(var(--surface-1))",
      }}
    >
      {/* ── Brand header — drag region + traffic light space ── */}
      <div
        data-tauri-drag-region
        className="flex items-center h-[var(--topbar-height)] px-4 flex-shrink-0 drag-region"
      >
        {/* 70px left pad for macOS native traffic lights */}
        <div data-tauri-drag-region className="w-[70px] flex-shrink-0" />

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

      {/* ── Footer nav — Invite / Settings / Help ─────────────────── */}
      <div className="px-3 py-3 flex-shrink-0 space-y-0.5">

        {/* Invite a friend — opens the in-app modal */}
        <button
          className="nav-item"
          onClick={() => onOpenInvite?.()}
        >
          <span className="flex-shrink-0 opacity-70">
            <UserPlus size={15} />
          </span>
          <span className="flex-1 truncate">Invite a friend</span>
        </button>

        {/* Settings — navigates to settings view */}
        <button
          className={cn("nav-item", activeView === "settings" && "active")}
          onClick={() => onViewChange("settings")}
        >
          <span className="flex-shrink-0 opacity-70">
            <Settings size={15} />
          </span>
          <span className="flex-1 truncate">Settings</span>
        </button>

        {/* Help — opens user's mail app to support */}
        <button
          className="nav-item"
          onClick={() => {
            window.open(
              "mailto:support@emiactech.com?subject=Said%20support",
              "_blank",
            );
          }}
        >
          <span className="flex-shrink-0 opacity-70">
            <HelpCircle size={15} />
          </span>
          <span className="flex-1 truncate">Help</span>
        </button>
      </div>
    </aside>
  );
}
