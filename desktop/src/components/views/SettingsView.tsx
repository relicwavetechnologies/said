import React, { useEffect, useRef, useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import {
  Shield, Cpu, Key, Info, Wifi, Check, Zap, Brain, Bot, Sparkles,
  Languages, MessageSquareText, Loader2, Cloud, LogIn, LogOut, RefreshCw, UserPlus,
  TestTube, Eye, EyeOff,
} from "lucide-react";
import type { AppSnapshot, CloudStatus, OpenAIStatus, Preferences } from "@/types";
import {
  cloudLogin, cloudLogout, cloudSignup, getCloudStatus,
  getPreferences, patchPreferences, diagnoseAx,
  getOpenAIStatus, initiateOpenAIOAuth, disconnectOpenAI,
  type AxDiagnostics,
} from "@/lib/invoke";

// ── Tone presets ──────────────────────────────────────────────────────────────

const TONE_PRESETS = [
  { key: "neutral",      label: "Neutral",      desc: "Clear and balanced — no strong stylistic lean" },
  { key: "professional", label: "Professional",  desc: "Formal and polished — great for work emails" },
  { key: "casual",       label: "Casual",        desc: "Friendly and conversational — light and easy" },
  { key: "assertive",    label: "Assertive",     desc: "Direct and confident — strong calls-to-action" },
  { key: "concise",      label: "Concise",       desc: "Minimal words — every word earns its place" },
  { key: "custom",       label: "Custom",        desc: "Write your own persona instructions below" },
] as const;

type ToneKey = (typeof TONE_PRESETS)[number]["key"];

// ── Language options ──────────────────────────────────────────────────────────

const LANGUAGES = [
  { key: "auto", label: "Auto-detect" },
  { key: "hi",   label: "Hindi / Hinglish" },
  { key: "en",   label: "English" },
  { key: "en-IN",label: "English (India)" },
];

// ── Helpers ───────────────────────────────────────────────────────────────────

function modeIcon(key: string) {
  if (key.includes("fast") || key.includes("mini")) return <Zap      size={16} />;
  if (key.includes("claude"))                        return <Bot      size={16} />;
  if (key.includes("gemini"))                        return <Sparkles size={16} />;
  return <Brain size={16} />;
}

// ── Sub-components ────────────────────────────────────────────────────────────

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-7">
      <p className="section-label px-1 mb-2.5">{title}</p>
      <div className="tile overflow-hidden">
        {children}
      </div>
    </div>
  );
}

function Row({
  icon, label, description, action,
}: {
  icon:         React.ReactNode;
  label:        string;
  description?: string;
  action?:      React.ReactNode;
  last?:        boolean;
}) {
  return (
    <div className="flex items-center gap-4 px-5 py-4">
      <div
        className="w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0 text-muted-foreground"
        style={{ background: "hsl(var(--surface-4))" }}
      >
        {icon}
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-[13px] font-medium text-foreground">{label}</p>
        {description && (
          <p className="text-[12px] text-muted-foreground mt-0.5 leading-relaxed">{description}</p>
        )}
      </div>
      {action && <div className="flex-shrink-0 ml-4">{action}</div>}
    </div>
  );
}

// ── Props ──────────────────────────────────────────────────────────────────────

interface SettingsViewProps {
  snapshot:          AppSnapshot | null;
  onAccessibility:   () => void;
  onInputMonitoring: () => void;
  openAIModel?:      "smart" | "mini";
  onOpenAIModel?:    (m: "smart" | "mini") => void;
}

// ── View ───────────────────────────────────────────────────────────────────────

export function SettingsView({ snapshot, onAccessibility, onInputMonitoring, openAIModel: activeModelProp, onOpenAIModel }: SettingsViewProps) {
  const axGranted  = snapshot?.accessibility_granted    ?? false;
  const imGranted  = snapshot?.input_monitoring_granted ?? false;
  const axSupported = snapshot?.auto_paste_supported    ?? false;

  // ── Prefs state ─────────────────────────────────────────────────────────────
  const [prefs,        setPrefs]        = useState<Preferences | null>(null);
  const [saving,       setSaving]       = useState(false);
  const [saveError,    setSaveError]    = useState("");
  const [customPrompt, setCustomPrompt] = useState("");
  const [promptDirty,  setPromptDirty]  = useState(false);

  // ── API key state ────────────────────────────────────────────────────────────
  const [gatewayKey,    setGatewayKey]    = useState("");
  const [deepgramKey,   setDeepgramKey]   = useState("");
  const [geminiKey,     setGeminiKey]     = useState("");
  const [showGateway,   setShowGateway]   = useState(false);
  const [showDeepgram,  setShowDeepgram]  = useState(false);
  const [showGemini,    setShowGemini]    = useState(false);
  const [keySaving,     setKeySaving]     = useState(false);
  const [keySaved,      setKeySaved]      = useState(false);
  const keySaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ── Cloud auth state ─────────────────────────────────────────────────────────
  // ── OpenAI OAuth state ──────────────────────────────────────────────────────
  const [openAIStatus,  setOpenAIStatus]  = useState<OpenAIStatus | null>(null);
  const [openAIBusy,    setOpenAIBusy]    = useState(false);
  const [openAIError,   setOpenAIError]   = useState("");

  const [cloudStatus,  setCloudStatus]  = useState<CloudStatus | null>(null);
  const [cloudEmail,   setCloudEmail]   = useState("");
  const [cloudPass,    setCloudPass]    = useState("");
  const [cloudMode,    setCloudMode]    = useState<"login" | "signup">("login");
  const [cloudBusy,    setCloudBusy]    = useState(false);
  const [cloudError,   setCloudError]   = useState("");

  // ── AX diagnostic state ────────────────────────────────────────────────────
  const [axCountdown, setAxCountdown] = useState<number>(0);
  const [axBusy,      setAxBusy]      = useState(false);
  const [axReport,    setAxReport]    = useState<AxDiagnostics | null>(null);

  async function runAxDiagnostic() {
    setAxReport(null);
    setAxBusy(true);
    // Visible 5-second countdown so the user can switch apps
    for (let s = 5; s > 0; s--) {
      setAxCountdown(s);
      await new Promise((r) => setTimeout(r, 1000));
    }
    setAxCountdown(0);
    // The Rust side waits 0 seconds (we already counted in the UI)
    const report = await diagnoseAx(0);
    setAxReport(report);
    setAxBusy(false);
  }

  useEffect(() => {
    getPreferences().then((p) => {
      if (p) {
        setPrefs(p);
        setCustomPrompt(p.custom_prompt ?? "");
        // Prefill key fields with masked placeholders only if keys already stored
        // (don't reveal the actual key — user can re-enter to change)
        setGatewayKey(p.gateway_api_key ? "••••••••••••••••" : "");
        setDeepgramKey(p.deepgram_api_key ? "••••••••••••••••" : "");
        setGeminiKey(p.gemini_api_key ? "••••••••••••••••" : "");
      }
    });
    getCloudStatus().then((s) => { if (s) setCloudStatus(s); });
    getOpenAIStatus().then((s) => { if (s) setOpenAIStatus(s); });
  }, []);

  async function handleCloudAuth() {
    setCloudBusy(true);
    setCloudError("");
    try {
      const resp = cloudMode === "login"
        ? await cloudLogin(cloudEmail, cloudPass)
        : await cloudSignup(cloudEmail, cloudPass);
      setCloudStatus({
        connected:    true,
        license_tier: resp.account.license_tier,
        email:        resp.account.email,
      });
      setCloudEmail("");
      setCloudPass("");
    } catch (err) {
      setCloudError(err instanceof Error ? err.message : String(err));
    } finally {
      setCloudBusy(false);
    }
  }

  async function handleCloudLogout() {
    setCloudBusy(true);
    try {
      await cloudLogout();
      setCloudStatus({ connected: false, license_tier: "free", email: null });
    } finally {
      setCloudBusy(false);
    }
  }

  async function handleOpenAIConnect() {
    setOpenAIBusy(true);
    setOpenAIError("");
    try {
      await initiateOpenAIOAuth();
      // Poll every 2s for up to 5 min until the backend confirms connection
      let attempts = 0;
      const poll = setInterval(async () => {
        attempts++;
        const s = await getOpenAIStatus();
        if (s?.connected) {
          setOpenAIStatus(s);
          setOpenAIBusy(false);
          clearInterval(poll);
        } else if (attempts > 150) {
          setOpenAIError("Timed out. Please try again.");
          setOpenAIBusy(false);
          clearInterval(poll);
        }
      }, 2000);
    } catch (err) {
      setOpenAIError(err instanceof Error ? err.message : String(err));
      setOpenAIBusy(false);
    }
  }

  async function handleOpenAIDisconnect() {
    setOpenAIBusy(true);
    try {
      await disconnectOpenAI();
      setOpenAIStatus((prev) => prev ? { ...prev, connected: false } : null);
    } catch (err) {
      setOpenAIError(err instanceof Error ? err.message : String(err));
    } finally {
      setOpenAIBusy(false);
    }
  }

  async function saveApiKeys() {
    setKeySaving(true);
    setSaveError("");
    try {
      const update: Partial<Preferences> = {};
      // Only send if the user has entered a real key (not the placeholder bullets)
      if (gatewayKey && !gatewayKey.startsWith("••"))   update.gateway_api_key  = gatewayKey;
      if (deepgramKey && !deepgramKey.startsWith("••"))  update.deepgram_api_key = deepgramKey;
      if (geminiKey   && !geminiKey.startsWith("••"))    update.gemini_api_key   = geminiKey;
      const updated = await patchPreferences(update);
      if (updated) setPrefs(updated);
      // Show brief "Saved ✓" feedback
      setKeySaved(true);
      if (keySaveTimer.current) clearTimeout(keySaveTimer.current);
      keySaveTimer.current = setTimeout(() => setKeySaved(false), 2500);
    } catch {
      setSaveError("Failed to save — is the backend running?");
    } finally {
      setKeySaving(false);
    }
  }

  async function patch(update: Partial<Preferences>) {
    if (!prefs) return;
    console.log("[patch] calling patchPreferences with:", JSON.stringify(update));
    setSaving(true);
    setSaveError("");
    try {
      const updated = await patchPreferences(update);
      console.log("[patch] got back:", updated ? JSON.stringify({ llm_provider: updated.llm_provider }) : "null");
      if (updated) setPrefs(updated);
    } catch (err) {
      console.error("[patch] error:", err);
      setSaveError("Failed to save — is the backend running?");
    } finally {
      setSaving(false);
    }
  }

  async function saveCustomPrompt() {
    await patch({ custom_prompt: customPrompt || null });
    setPromptDirty(false);
  }

  const tone = (prefs?.tone_preset ?? "neutral") as ToneKey;

  return (
    <ScrollArea className="h-full">
      <div className="p-6 pb-10 max-w-2xl mx-auto">

        {/* ── Header ───────────────────────────────────── */}
        <div className="mb-6 flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">Settings</h1>
            <p className="text-sm text-muted-foreground mt-0.5">
              Preferences are saved automatically
            </p>
          </div>
          {saving && (
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground mt-1">
              <Loader2 size={13} className="animate-spin" />
              Saving…
            </div>
          )}
          {saveError && (
            <p className="text-xs text-red-500 mt-1">{saveError}</p>
          )}
        </div>

        {/* ── Tone & Persona ───────────────────────────── */}
        <div className="mb-7">
          <p className="section-label px-1 mb-2.5">Writing Style</p>

          {/* Tone pill grid */}
          <div className="tile p-4 mb-3">
            <p className="text-[12px] font-semibold text-foreground mb-3">Tone Preset</p>
            <div className="grid grid-cols-3 gap-2">
              {TONE_PRESETS.map((t) => {
                const isActive = tone === t.key;
                return (
                  <button
                    key={t.key}
                    onClick={() => patch({ tone_preset: t.key })}
                    className="text-left px-3 py-2.5 rounded-xl transition-all"
                    style={{
                      background: isActive
                        ? "hsl(var(--chip-lime-bg))"
                        : "hsl(var(--surface-4))",
                      color: isActive
                        ? "hsl(var(--chip-lime-fg))"
                        : "hsl(var(--muted-foreground))",
                    }}
                  >
                    <p className="text-[12px] font-semibold leading-tight">{t.label}</p>
                    <p className="text-[10px] leading-snug mt-0.5 opacity-70">{t.desc}</p>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Custom persona textarea */}
          <div className={cn("tile p-4 transition-all", tone !== "custom" && "opacity-60")}>
            <div className="flex items-center gap-2 mb-2">
              <MessageSquareText size={14} className="text-muted-foreground" />
              <p className="text-[12px] font-semibold text-foreground">Custom Persona Instructions</p>
              {tone !== "custom" && (
                <span className="text-[10px] text-muted-foreground ml-auto">
                  Select "Custom" above to activate
                </span>
              )}
            </div>
            <textarea
              value={customPrompt}
              onChange={(e) => { setCustomPrompt(e.target.value); setPromptDirty(true); }}
              onBlur={() => { if (promptDirty) saveCustomPrompt(); }}
              placeholder={
                'e.g. "You are a direct, no-nonsense communicator. Use bullet points where possible."'
              }
              rows={4}
              disabled={tone !== "custom"}
              className={cn(
                "input resize-none leading-relaxed transition-opacity",
                tone !== "custom" && "cursor-not-allowed"
              )}
            />
            {promptDirty && tone === "custom" && (
              <div className="flex items-center justify-end mt-2 gap-2">
                <button
                  onClick={() => { setCustomPrompt(prefs?.custom_prompt ?? ""); setPromptDirty(false); }}
                  className="text-[12px] text-muted-foreground hover:text-foreground transition-colors"
                >
                  Cancel
                </button>
                <button onClick={saveCustomPrompt} className="btn-primary !py-1.5 !px-3 !text-[12px]">
                  Save
                </button>
              </div>
            )}
          </div>
        </div>

        {/* ── Language ─────────────────────────────────── */}
        <Section title="Language">
          {/* Output language toggle */}
          <div className="px-5 pt-4 pb-3">
            <div className="flex items-center gap-4">
              <div
                className="w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0 text-muted-foreground"
                style={{ background: "hsl(var(--surface-4))" }}
              >
                <Languages size={16} />
              </div>
              <div className="flex-1">
                <p className="text-[13px] font-medium text-foreground mb-0.5">Output Language</p>
                <p className="text-[12px] text-muted-foreground">
                  What language the polished text is written in
                </p>
              </div>
            </div>
            {/* Three-way pill toggle */}
            <div
              className="flex mt-3 rounded-xl p-0.5 gap-0.5"
              style={{ background: "hsl(var(--surface-4))" }}
            >
              {(["hinglish", "hindi", "english"] as const).map((opt) => {
                const label = opt === "hinglish" ? "Hinglish" : opt === "hindi" ? "हिंदी" : "English";
                const isActive = (prefs?.output_language ?? "hinglish") === opt;
                return (
                  <button
                    key={opt}
                    onClick={() => patch({ output_language: opt })}
                    className="flex-1 text-[13px] font-medium rounded-[10px] py-1.5 transition-all"
                    style={{
                      background: isActive ? "hsl(var(--surface-1))" : "transparent",
                      color: isActive ? "hsl(var(--foreground))" : "hsl(var(--muted-foreground))",
                      boxShadow: isActive ? "0 1px 3px rgba(0,0,0,0.25)" : "none",
                    }}
                  >
                    {label}
                  </button>
                );
              })}
            </div>
          </div>

          <div className="mx-5 border-t" style={{ borderColor: "hsl(var(--surface-3))" }} />

          {/* Transcription language */}
          <div className="px-5 py-4">
            <div className="flex items-center gap-4">
              <div
                className="w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0 text-muted-foreground"
                style={{ background: "hsl(var(--surface-4))" }}
              >
                <Languages size={16} />
              </div>
              <div className="flex-1">
                <p className="text-[13px] font-medium text-foreground mb-1">Transcription Language</p>
                <p className="text-[12px] text-muted-foreground">
                  Sent to Deepgram for speech recognition
                </p>
              </div>
              <select
                value={prefs?.language ?? "auto"}
                onChange={(e) => patch({ language: e.target.value })}
                className="text-[13px] rounded-lg px-3 py-1.5 cursor-pointer focus:outline-none"
                style={{
                  background: "hsl(var(--surface-4))",
                  color: "hsl(var(--foreground))",
                  border: "none",
                }}
              >
                {LANGUAGES.map((l) => (
                  <option key={l.key} value={l.key}>{l.label}</option>
                ))}
              </select>
            </div>
          </div>
        </Section>

        {/* ── Permissions ──────────────────────────────── */}
        <div className="mb-7">
          <p className="section-label px-1 mb-2.5">Permissions</p>

          {/* Combined info banner when any permission is missing */}
          {axSupported && (!axGranted || !imGranted) && (
            <div
              className="rounded-xl px-4 py-3 mb-3 text-[12px] leading-relaxed"
              style={{ background: "hsl(38 80% 12%)", color: "hsl(38 90% 70%)" }}
            >
              <p className="font-semibold mb-1">Permissions needed</p>
              {!axGranted && (
                <p>• <strong>Accessibility</strong> — lets Said paste text directly into any app.</p>
              )}
              {!imGranted && (
                <p>• <strong>Input Monitoring</strong> — lets Said listen for the Caps Lock hotkey.</p>
              )}
              <p className="mt-1.5 opacity-70">
                After granting each permission in System Settings, restart Said so macOS picks up the change.
              </p>
            </div>
          )}

          <div className="tile overflow-hidden">
            {/* Row 1: Accessibility */}
            <div className="flex items-center gap-4 px-5 py-4">
              <div
                className="w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0"
                style={{
                  background: axGranted
                    ? "hsl(var(--chip-lime-bg))"
                    : "hsl(var(--surface-4))",
                  color: axGranted
                    ? "hsl(var(--chip-lime-fg))"
                    : "hsl(var(--muted-foreground))",
                }}
              >
                <Shield size={16} />
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-[13px] font-medium text-foreground">Accessibility</p>
                <p className="text-[12px] text-muted-foreground mt-0.5 leading-relaxed">
                  {axGranted
                    ? "Granted — Said can paste text into any app."
                    : "Required for auto-paste. Opens System Settings → Privacy & Security → Accessibility."}
                </p>
              </div>
              <div className="flex-shrink-0 ml-4">
                {axSupported ? (
                  axGranted ? (
                    <span
                      className="text-[12px] font-semibold px-3 py-1.5 rounded-lg flex items-center gap-1"
                      style={{ background: "hsl(var(--chip-lime-bg))", color: "hsl(var(--chip-lime-fg))" }}
                    >
                      <Check size={11} /> Granted
                    </span>
                  ) : (
                    <button
                      onClick={onAccessibility}
                      className="text-[12px] font-semibold px-3 py-1.5 rounded-lg transition-colors"
                      style={{ background: "hsl(var(--primary))", color: "hsl(var(--primary-foreground))" }}
                    >
                      Open Settings
                    </button>
                  )
                ) : (
                  <span className="text-[12px] text-muted-foreground">macOS only</span>
                )}
              </div>
            </div>

            {/* Divider */}
            <div className="mx-5 border-t" style={{ borderColor: "hsl(var(--surface-3))" }} />

            {/* Row 2: Input Monitoring */}
            <div className="flex items-center gap-4 px-5 py-4">
              <div
                className="w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0"
                style={{
                  background: imGranted
                    ? "hsl(var(--chip-lime-bg))"
                    : "hsl(var(--surface-4))",
                  color: imGranted
                    ? "hsl(var(--chip-lime-fg))"
                    : "hsl(var(--muted-foreground))",
                }}
              >
                <Key size={16} />
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-[13px] font-medium text-foreground">Input Monitoring</p>
                <p className="text-[12px] text-muted-foreground mt-0.5 leading-relaxed">
                  {imGranted
                    ? "Granted — Caps Lock hotkey is active."
                    : "Required for the Caps Lock hotkey to work. Opens System Settings → Privacy & Security → Input Monitoring."}
                </p>
              </div>
              <div className="flex-shrink-0 ml-4">
                {axSupported ? (
                  imGranted ? (
                    <span
                      className="text-[12px] font-semibold px-3 py-1.5 rounded-lg flex items-center gap-1"
                      style={{ background: "hsl(var(--chip-lime-bg))", color: "hsl(var(--chip-lime-fg))" }}
                    >
                      <Check size={11} /> Granted
                    </span>
                  ) : (
                    <button
                      onClick={onInputMonitoring}
                      className="text-[12px] font-semibold px-3 py-1.5 rounded-lg transition-colors"
                      style={{ background: "hsl(var(--primary))", color: "hsl(var(--primary-foreground))" }}
                    >
                      Open Settings
                    </button>
                  )
                ) : (
                  <span className="text-[12px] text-muted-foreground">macOS only</span>
                )}
              </div>
            </div>
          </div>
        </div>

        {/* ── AX field-reading diagnostic ───────────────── */}
        <Section title="Field-Reading Diagnostic">
          <Row
            icon={<TestTube size={16} />}
            label="Test which AX method works on a focused app"
            description={
              axBusy && axCountdown > 0
                ? `Switch focus to your target app NOW — sampling in ${axCountdown}s…`
                : axBusy
                ? "Reading focused field via 5 methods…"
                : "Click Run, then switch focus to any text field within 5 seconds. Tells you which of 5 reading techniques works in that app."
            }
            action={
              <button
                onClick={runAxDiagnostic}
                disabled={axBusy}
                className="text-[12px] font-semibold px-3 py-1.5 rounded-lg transition-colors flex items-center gap-1"
                style={{
                  background: axBusy ? "hsl(var(--surface-4))" : "hsl(var(--primary))",
                  color: axBusy ? "hsl(var(--muted-foreground))" : "hsl(var(--primary-foreground))",
                  cursor: axBusy ? "not-allowed" : "pointer",
                }}
              >
                {axBusy
                  ? (axCountdown > 0 ? `${axCountdown}…` : "Reading…")
                  : "Run"}
              </button>
            }
            last
          />
          {axReport && (
            <div
              className="px-5 py-4 border-t"
              style={{ borderColor: "hsl(var(--surface-4))" }}
            >
              <div className="text-[12px] mb-3 leading-relaxed">
                <span className="text-muted-foreground">App:</span>{" "}
                <span className="font-medium text-foreground">
                  {axReport.app_name ?? "?"} (pid={axReport.app_pid ?? "?"})
                </span>{" "}
                <span className="text-muted-foreground">· role:</span>{" "}
                <span className="font-medium text-foreground">
                  {axReport.element_role ?? "?"}
                </span>
              </div>
              <div className="space-y-2">
                {axReport.methods.map((m) => {
                  // Method 6 (clipboard) is the universal fallback — highlight it amber
                  const isClipboard = m.method === "6_clipboard";
                  return (
                    <div
                      key={m.method}
                      className="text-[11px] leading-snug rounded-md p-2"
                      style={{
                        background: m.ok
                          ? isClipboard
                            ? "hsl(38 80% 14%)"          // amber tint for clipboard
                            : "hsl(var(--chip-lime-bg))"
                          : "hsl(var(--surface-4))",
                        color: m.ok
                          ? isClipboard
                            ? "hsl(38 90% 70%)"
                            : "hsl(var(--chip-lime-fg))"
                          : "hsl(var(--muted-foreground))",
                      }}
                    >
                      <div className="font-semibold mb-0.5">
                        {m.ok ? "✓" : "✗"} {m.label}
                        {isClipboard && m.ok && (
                          <span className="ml-2 font-normal opacity-70">
                            (fallback — briefly selects all)
                          </span>
                        )}
                      </div>
                      {m.ok && m.text != null && (
                        <div
                          className="font-mono text-[10px] mt-1 max-h-20 overflow-auto"
                          style={{ color: "hsl(var(--foreground))" }}
                        >
                          {m.text.length === 0 ? "(empty string)" : m.text}
                        </div>
                      )}
                      {m.err && (
                        <div className="font-mono text-[10px] mt-1 opacity-70">
                          {m.err}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
              <details className="mt-3">
                <summary className="text-[11px] text-muted-foreground cursor-pointer select-none">
                  AX attributes available on focused element ({axReport.attributes.length})
                </summary>
                <div className="font-mono text-[10px] mt-2 text-muted-foreground leading-relaxed">
                  {axReport.attributes.join(", ")}
                </div>
              </details>
            </div>
          )}
        </Section>

        {/* ── API Keys ──────────────────────────────────── */}
        <div className="mb-7">
          <p className="section-label px-1 mb-2.5">API Keys</p>
          <div className="tile p-5 space-y-4">
            <p className="text-[12px] text-muted-foreground leading-relaxed">
              Keys are stored locally in SQLite — they never leave your Mac.
              Enter each key once; you only need to re-enter to change it.
            </p>

            {/* Gateway API Key */}
            <div>
              <p className="text-[12px] font-semibold text-foreground mb-1.5 flex items-center gap-1.5">
                <Wifi size={12} className="text-muted-foreground" />
                Gateway API Key
              </p>
              <div className="relative">
                <input
                  type={showGateway ? "text" : "password"}
                  placeholder="sk-…"
                  value={gatewayKey}
                  onChange={(e) => setGatewayKey(e.target.value)}
                  onFocus={() => {
                    if (gatewayKey.startsWith("••")) setGatewayKey("");
                  }}
                  className="input pr-9 font-mono text-[12px]"
                />
                <button
                  type="button"
                  onClick={() => setShowGateway((v) => !v)}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                  tabIndex={-1}
                >
                  {showGateway ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
              </div>
            </div>

            {/* Deepgram API Key */}
            <div>
              <p className="text-[12px] font-semibold text-foreground mb-1.5 flex items-center gap-1.5">
                <Cpu size={12} className="text-muted-foreground" />
                Deepgram API Key
              </p>
              <div className="relative">
                <input
                  type={showDeepgram ? "text" : "password"}
                  placeholder="Token …"
                  value={deepgramKey}
                  onChange={(e) => setDeepgramKey(e.target.value)}
                  onFocus={() => {
                    if (deepgramKey.startsWith("••")) setDeepgramKey("");
                  }}
                  className="input pr-9 font-mono text-[12px]"
                />
                <button
                  type="button"
                  onClick={() => setShowDeepgram((v) => !v)}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                  tabIndex={-1}
                >
                  {showDeepgram ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
              </div>
            </div>

            {/* Gemini API Key */}
            <div>
              <p className="text-[12px] font-semibold text-foreground mb-1.5 flex items-center gap-1.5">
                <Sparkles size={12} className="text-muted-foreground" />
                Gemini API Key
              </p>
              <div className="relative">
                <input
                  type={showGemini ? "text" : "password"}
                  placeholder="AIza…"
                  value={geminiKey}
                  onChange={(e) => setGeminiKey(e.target.value)}
                  onFocus={() => {
                    if (geminiKey.startsWith("••")) setGeminiKey("");
                  }}
                  className="input pr-9 font-mono text-[12px]"
                />
                <button
                  type="button"
                  onClick={() => setShowGemini((v) => !v)}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                  tabIndex={-1}
                >
                  {showGemini ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
              </div>
            </div>

            {/* Save button */}
            <div className="flex items-center justify-between pt-1">
              {saveError && (
                <p className="text-[12px]" style={{ color: "hsl(0 75% 75%)" }}>{saveError}</p>
              )}
              <div className="ml-auto flex items-center gap-3">
                {keySaved && (
                  <span className="text-[12px] flex items-center gap-1" style={{ color: "hsl(var(--chip-lime-fg))" }}>
                    <Check size={12} /> Saved
                  </span>
                )}
                <button
                  onClick={saveApiKeys}
                  disabled={keySaving}
                  className="btn-primary !py-1.5 !px-4 !text-[12px] flex items-center gap-1.5"
                >
                  {keySaving ? <Loader2 size={12} className="animate-spin" /> : null}
                  Save Keys
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* ── Active config ─────────────────────────────── */}
        <Section title="Active Configuration">
          {openAIStatus?.connected ? (() => {
            // Use parent-controlled value so Active Configuration reflects the same selection as Dashboard
            const isMini = (activeModelProp ?? (prefs?.selected_model === "mini" || prefs?.selected_model === "fast" ? "mini" : "smart")) === "mini";
            const modelLabel = isMini ? "gpt-5.4-mini" : "gpt-5.4";
            return (
            /* ── Using OpenAI Codex ─────────────────────── */
            <>
              <Row
                icon={<Sparkles size={16} />}
                label="LLM Provider"
                description="ChatGPT · OpenAI Codex OAuth"
                action={<span className="badge-done">Connected</span>}
              />
              <Row
                icon={<Zap size={16} />}
                label="Active Model"
                description={isMini ? "Fast · lightweight" : "Full intelligence"}
                action={<span className="badge-model">{modelLabel}</span>}
                last
              />
            </>
            );
          })() : (
            /* ── Using Gateway (default) ─────────────────── */
            <>
              <Row
                icon={<Cpu size={16} />}
                label="Current Mode"
                description={snapshot?.current_mode_label ?? "Loading…"}
                action={<span className="badge-model">{snapshot?.current_mode ?? "—"}</span>}
              />
              <Row
                icon={<Wifi size={16} />}
                label="Gateway"
                description="Connected to gateway.voicepolish.app"
                action={<span className="badge-done">Connected</span>}
                last
              />
            </>
          )}
        </Section>

        {/* ── OpenAI Account ────────────────────────────── */}
        <div className="mb-7">
          <p className="section-label px-1 mb-2.5">OpenAI Account</p>

          {openAIStatus?.connected ? (
            /* ── Connected state ──────────────────────── */
            <div className="tile p-5 space-y-4">
              {/* Status row */}
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2.5">
                  <div
                    className="w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0"
                    style={{ background: "hsl(var(--chip-lime-fg) / 0.12)" }}
                  >
                    <Check size={16} style={{ color: "hsl(var(--chip-lime-fg))" }} />
                  </div>
                  <div>
                    <p className="text-[13px] font-semibold text-foreground leading-tight">ChatGPT Connected</p>
                    <p className="text-[11px] text-muted-foreground">OAuth token stored locally · models ready</p>
                  </div>
                </div>
                <div className="flex items-center gap-3">
                  <button
                    onClick={handleOpenAIConnect}
                    disabled={openAIBusy}
                    className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                  >
                    <RefreshCw size={12} />
                    {openAIBusy ? "…" : "Reconnect"}
                  </button>
                  <button
                    onClick={handleOpenAIDisconnect}
                    disabled={openAIBusy}
                    className="flex items-center gap-1 text-xs text-muted-foreground hover:text-red-500 transition-colors"
                  >
                    <LogOut size={12} />
                    Disconnect
                  </button>
                </div>
              </div>

              {openAIBusy && (
                <p className="text-[11px] text-muted-foreground">Waiting for browser sign-in… this window updates automatically.</p>
              )}
              {openAIError && (
                <p className="text-[11px]" style={{ color: "hsl(0 75% 75%)" }}>{openAIError}</p>
              )}
            </div>
          ) : (
            /* ── Not connected state ──────────────────── */
            <div className="tile p-5 space-y-4">
              <div className="flex items-start gap-3">
                <div
                  className="w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0 mt-0.5"
                  style={{ background: "hsl(var(--surface-4))" }}
                >
                  <Sparkles size={16} className="text-muted-foreground" />
                </div>
                <div>
                  <p className="text-[13px] font-semibold text-foreground leading-tight">Connect your ChatGPT account</p>
                  <p className="text-[12px] text-muted-foreground mt-1 leading-relaxed">
                    Use your ChatGPT Pro subscription to access <strong className="text-foreground">gpt-5.4</strong> and{" "}
                    <strong className="text-foreground">gpt-5.4-mini</strong> directly — no API key needed.
                    Your voice data never leaves your Mac.
                  </p>
                </div>
              </div>

              <button
                onClick={handleOpenAIConnect}
                disabled={openAIBusy}
                className="w-full btn-primary flex items-center justify-center gap-2 !py-2.5"
              >
                {openAIBusy ? (
                  <>
                    <Loader2 size={13} className="animate-spin" />
                    Waiting for browser…
                  </>
                ) : (
                  <>
                    <LogIn size={13} />
                    Connect OpenAI account
                  </>
                )}
              </button>

              {openAIBusy && (
                <p className="text-[11px] text-muted-foreground text-center leading-relaxed">
                  A browser window will open. Sign in with your ChatGPT account, then return here.
                </p>
              )}

              {openAIError && (
                <p className="text-[11px]" style={{ color: "hsl(0 75% 75%)" }}>{openAIError}</p>
              )}
            </div>
          )}
        </div>

        {/* ── Available models ──────────────────────────── */}
        {openAIStatus?.connected ? (() => {
          // Use parent-controlled value when available (keeps Dashboard in sync)
          const activeKey: "smart" | "mini" = activeModelProp
            ?? (prefs?.selected_model === "mini" || prefs?.selected_model === "fast" ? "mini" : "smart");

          function selectModel(key: "smart" | "mini") {
            // Optimistic local update
            setPrefs((p) => p ? { ...p, selected_model: key } : p);
            patch({ selected_model: key });
            // Lift to App.tsx so Dashboard pill also updates
            onOpenAIModel?.(key);
          }

          const models = [
            { key: "smart" as const, label: "GPT-5.4",      sub: "Full intelligence · ChatGPT Pro" },
            { key: "mini"  as const, label: "GPT-5.4 Mini", sub: "Faster · lightweight · ChatGPT Pro" },
          ];

          return (
            <div className="mb-7">
              <p className="section-label px-1 mb-2.5">Model</p>
              <div className="flex gap-3">
                {models.map((m) => {
                  const active = activeKey === m.key;
                  return (
                    <button
                      key={m.key}
                      onClick={() => selectModel(m.key)}
                      className="flex-1 flex flex-col items-start gap-1 rounded-xl border px-4 py-3 text-left transition-all"
                      style={{
                        borderColor: active ? "hsl(var(--chip-lime-fg) / 0.6)" : "hsl(var(--border))",
                        background:  active ? "hsl(var(--chip-lime-fg) / 0.07)" : "hsl(var(--surface-2))",
                      }}
                    >
                      <span className="flex items-center gap-1.5 w-full">
                        <Zap size={12} style={{ color: active ? "hsl(var(--chip-lime-fg))" : undefined }} className={active ? "" : "text-muted-foreground"} />
                        <span className="text-[12px] font-semibold text-foreground">{m.label}</span>
                        {active && (
                          <span className="ml-auto text-[10px] font-semibold px-1.5 py-0.5 rounded-full"
                            style={{ background: "hsl(var(--chip-lime-fg) / 0.15)", color: "hsl(var(--chip-lime-fg))" }}>
                            Active
                          </span>
                        )}
                      </span>
                      <span className="text-[11px] text-muted-foreground">{m.sub}</span>
                    </button>
                  );
                })}
              </div>
              {saving && (
                <p className="text-[11px] text-muted-foreground mt-2 flex items-center gap-1">
                  <Loader2 size={10} className="animate-spin" /> Saving…
                </p>
              )}
            </div>
          );
        })() : (
          (snapshot?.modes ?? []).length > 0 && (
            <Section title="Available Models">
              {(snapshot?.modes ?? []).map((mode, i, arr) => (
                <Row
                  key={mode.key}
                  icon={modeIcon(mode.key)}
                  label={mode.label}
                  description={mode.model}
                  action={
                    snapshot?.current_mode === mode.key ? (
                      <span className="badge-model">Active</span>
                    ) : undefined
                  }
                  last={i === arr.length - 1}
                />
              ))}
            </Section>
          )
        )}

        {/* ── Cloud Account ─────────────────────────────── */}
        <Section title="Cloud Account">
          {cloudStatus?.connected ? (
            /* ── Connected state ───────────────────────── */
            <Row
              icon={<Cloud size={16} />}
              label={cloudStatus.email ?? "Connected"}
              description={`License: ${cloudStatus.license_tier} tier`}
              action={
                <button
                  onClick={handleCloudLogout}
                  disabled={cloudBusy}
                  className="flex items-center gap-1 text-xs text-muted-foreground hover:text-red-600 transition-colors"
                >
                  <LogOut size={12} />
                  {cloudBusy ? "…" : "Sign out"}
                </button>
              }
              last
            />
          ) : (
            /* ── Auth form ─────────────────────────────── */
            <div className="px-5 py-4">
              <div className="flex items-center gap-3 mb-3">
                <div
                  className="w-9 h-9 rounded-xl flex items-center justify-center flex-shrink-0 text-muted-foreground"
                  style={{ background: "hsl(var(--surface-4))" }}
                >
                  <Cloud size={16} />
                </div>
                <div>
                  <p className="text-[13px] font-medium text-foreground">Sign in to Said Cloud</p>
                  <p className="text-[12px] text-muted-foreground mt-0.5">
                    Enables usage metering and license validation
                  </p>
                </div>
              </div>

              {/* Mode toggle */}
              <div className="flex gap-2 mb-3">
                {(["login", "signup"] as const).map((m) => (
                  <button
                    key={m}
                    onClick={() => { setCloudMode(m); setCloudError(""); }}
                    className={cn("pill", cloudMode === m && "active")}
                  >
                    {m === "login" ? <LogIn size={11} /> : <UserPlus size={11} />}
                    {m === "login" ? "Sign in" : "Create account"}
                  </button>
                ))}
              </div>

              {/* Inputs */}
              <div className="space-y-2 mb-3">
                <input
                  type="email"
                  placeholder="Email"
                  value={cloudEmail}
                  onChange={(e) => setCloudEmail(e.target.value)}
                  className="input"
                />
                <input
                  type="password"
                  placeholder="Password"
                  value={cloudPass}
                  onChange={(e) => setCloudPass(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") handleCloudAuth(); }}
                  className="input"
                />
              </div>

              {cloudError && (
                <p className="text-[12px] mb-2" style={{ color: "hsl(0 75% 75%)" }}>
                  {cloudError}
                </p>
              )}

              <button
                onClick={handleCloudAuth}
                disabled={cloudBusy || !cloudEmail || !cloudPass}
                className="btn-primary w-full justify-center"
              >
                {cloudBusy ? (
                  <Loader2 size={13} className="inline animate-spin mr-1" />
                ) : null}
                {cloudMode === "login" ? "Sign in" : "Create account"}
              </button>
            </div>
          )}
        </Section>

        {/* ── About ────────────────────────────────────── */}
        <Section title="About">
          <Row
            icon={<Info size={16} />}
            label="Said — Voice Polish Studio"
            description="Version 0.1.0 · Local-first · Built with Tauri + Rust + React"
            last
          />
        </Section>

      </div>
    </ScrollArea>
  );
}
