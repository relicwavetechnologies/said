import React from "react";
import { Sun, Moon } from "lucide-react";
import type { Theme } from "@/lib/useTheme";

interface Props {
  theme:  Theme;
  toggle: () => void;
}

/**
 * Cute segmented toggle: a pill track with sun + moon icons and a sliding
 * thumb that snaps to the active side. Smooth 200ms transition, lime accent.
 */
export function ThemeToggle({ theme, toggle }: Props) {
  const isDark = theme === "dark";

  return (
    <button
      onClick={toggle}
      aria-label={`Switch to ${isDark ? "light" : "dark"} mode`}
      title={`Switch to ${isDark ? "light" : "dark"} mode`}
      className="relative no-drag rounded-full transition-colors duration-200 flex items-center"
      style={{
        width:      "52px",
        height:     "26px",
        background: isDark ? "hsl(240 5% 13%)" : "hsl(30 6% 90%)",
        padding:    "3px",
      }}
    >
      {/* Sun icon (left) */}
      <span
        className="absolute flex items-center justify-center transition-colors duration-200"
        style={{
          left:   "6px",
          width:  "14px",
          height: "14px",
          top:    "50%",
          transform: "translateY(-50%)",
          color:  isDark ? "hsl(240 5% 38%)" : "hsl(38 90% 48%)",
        }}
      >
        <Sun size={11} strokeWidth={2.5} />
      </span>

      {/* Moon icon (right) */}
      <span
        className="absolute flex items-center justify-center transition-colors duration-200"
        style={{
          right:  "6px",
          width:  "14px",
          height: "14px",
          top:    "50%",
          transform: "translateY(-50%)",
          color:  isDark ? "hsl(73 80% 67%)" : "hsl(240 5% 55%)",
        }}
      >
        <Moon size={10} strokeWidth={2.5} />
      </span>

      {/* Sliding thumb */}
      <span
        className="absolute rounded-full transition-all duration-200 ease-out"
        style={{
          width:      "20px",
          height:     "20px",
          top:        "3px",
          left:       isDark ? "29px" : "3px",
          background: isDark ? "hsl(0 0% 96%)" : "hsl(73 22% 24%)",
          boxShadow:  isDark
            ? "0 1px 3px rgba(0,0,0,0.4), 0 0 0 0.5px rgba(255,255,255,0.1) inset"
            : "0 1px 2px hsl(73 30% 15% / 0.2)",
        }}
      />
    </button>
  );
}
