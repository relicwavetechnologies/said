import React from "react";
import { Search, Bell } from "lucide-react";
import type { AppSnapshot } from "@/types";
import { ThemeToggle } from "@/components/ThemeToggle";
import type { Theme } from "@/lib/useTheme";

interface TopbarProps {
  snapshot:    AppSnapshot | null;
  theme:       Theme;
  toggleTheme: () => void;
}

export function Topbar({ snapshot, theme, toggleTheme }: TopbarProps) {
  const modeInitial = snapshot?.current_mode?.[0]?.toUpperCase() ?? "?";

  return (
    <header
      data-tauri-drag-region
      className="flex items-center gap-3 h-[var(--topbar-height)] px-5 flex-shrink-0 drag-region"
      style={{ background: "transparent" }}
    >
      {/* Search — glass treatment */}
      <div
        className="flex items-center gap-2.5 rounded-full px-3.5 py-1.5 w-72 no-drag cursor-text transition-colors"
        style={{
          background: "hsl(var(--glass-bg))",
          color:      "hsl(var(--muted-foreground))",
          backdropFilter: "blur(20px) saturate(140%)",
          WebkitBackdropFilter: "blur(20px) saturate(140%)",
          boxShadow: "inset 0 0 0 1px hsl(var(--glass-stroke))",
        }}
      >
        <Search size={13} className="flex-shrink-0 opacity-70" />
        <span className="text-[13px] select-none">Search recordings…</span>
        <span
          className="ml-auto text-[10px] font-mono px-1.5 py-0.5 rounded tabular-nums"
          style={{
            background: "hsl(var(--surface-hover))",
            color:      "hsl(var(--muted-foreground))",
          }}
        >
          ⌘K
        </span>
      </div>

      <div data-tauri-drag-region className="flex-1 self-stretch" />

      {/* Right actions */}
      <div className="flex items-center gap-2.5 no-drag">
        {/* Theme toggle */}
        <ThemeToggle theme={theme} toggle={toggleTheme} />

        {/* Notification bell */}
        <button
          className="w-8 h-8 flex items-center justify-center rounded-full transition-colors"
          style={{ color: "hsl(var(--muted-foreground))" }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = "hsl(var(--glass-bg))";
            e.currentTarget.style.color      = "hsl(var(--foreground))";
            e.currentTarget.style.backdropFilter = "blur(20px)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = "transparent";
            e.currentTarget.style.color      = "hsl(var(--muted-foreground))";
            e.currentTarget.style.backdropFilter = "none";
          }}
        >
          <Bell size={14} />
        </button>

        {/* Mode avatar */}
        <div
          className="w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold flex-shrink-0"
          style={{
            background: "hsl(var(--primary) / 0.18)",
            color:      "hsl(var(--primary))",
            boxShadow:  "inset 0 0 0 1px hsl(var(--primary) / 0.30)",
          }}
          title={snapshot?.current_mode_label ?? "Mode"}
        >
          {modeInitial}
        </div>
      </div>
    </header>
  );
}
