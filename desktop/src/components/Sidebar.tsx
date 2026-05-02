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
import { openExternal } from "@/lib/invoke";
import { BrandMark } from "@/components/BrandMark";
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

        {/* Brand mark — single source of truth in BrandMark.tsx */}
        <div className="no-drag" title="Said — Voice Polish Studio">
          <BrandMark size={32} idSuffix="sidebar" />
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
            openExternal("mailto:support@emiactech.com?subject=Said%20support");
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
