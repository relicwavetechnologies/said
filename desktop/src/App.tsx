import React, { useEffect, useState, useCallback } from "react";
import { X, Mic } from "lucide-react";
import { Sidebar } from "@/components/Sidebar";
import { InviteTeamModal } from "@/components/InviteTeamModal";
import { SettingsModal } from "@/components/SettingsModal";
import { Topbar } from "@/components/Topbar";
import { DashboardView } from "@/components/views/DashboardView";
import { HistoryView } from "@/components/views/HistoryView";
import { InsightsView } from "@/components/views/InsightsView";
import { VocabularyView } from "@/components/views/VocabularyView";
import {
  invoke,
  listHistory,
  onAppState,
  onNavSettings,
  onOpenAIReconnectInitiated,
  onVoiceDone,
  onVoiceStatus,
  onVoiceToken,
  onVoiceError,
  onEditDetected,
  onPendingEditsChanged,
  getPendingEdits,
  resolvePendingEdit,
  sendNotification,
  cloudLogin,
  cloudSignup,
  getCloudStatus,
  getOpenAIStatus,
  initiateOpenAIOAuth,
  requestInputMonitoring,
  submitEditFeedback,
  onVocabToast,
  deleteVocabularyTerm,
  type VocabToastPayload,
} from "@/lib/invoke";
import { useTheme } from "@/lib/useTheme";
import type { AppSnapshot, HistoryItem, PendingEdit, Recording } from "@/types";
import { RetryToast, EditConfirmToast, VocabularyToast } from "@/components/NotificationToast";

export type ActiveView = "dashboard" | "history" | "vocabulary" | "insights" | "settings";
const VALID_VIEWS: ActiveView[] = ["dashboard", "history", "vocabulary", "insights", "settings"];

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Compute current consecutive-day streak from a newest-first history array. */
function computeStreak(items: HistoryItem[]): number {
  if (items.length === 0) return 0;
  const todayDay = Math.floor(Date.now() / 86_400_000);
  const activeDays = new Set(items.map((h) => Math.floor(h.timestamp_ms / 86_400_000)));
  let streak = 0;
  let day = todayDay;
  // Allow today OR yesterday as the streak start (don't break if user hasn't recorded today yet)
  if (!activeDays.has(day) && !activeDays.has(day - 1)) return 0;
  if (!activeDays.has(day)) day = day - 1;
  while (activeDays.has(day)) {
    streak++;
    day--;
  }
  return streak;
}

/** Map a backend Recording to the simpler HistoryItem for display. */
function recordingToHistoryItem(r: Recording): HistoryItem {
  return {
    timestamp_ms:      r.timestamp_ms,
    polished:          r.polished,
    word_count:        r.word_count,
    recording_seconds: r.recording_seconds,
    model:             r.model_used,
    transcribe_ms:     r.transcribe_ms ?? 0,
    embed_ms:          r.embed_ms ?? 0,
    polish_ms:         r.polish_ms ?? 0,
    audio_id:          r.audio_id,
  };
}

// ── App ───────────────────────────────────────────────────────────────────────

export default function App() {
  const [snapshot,    setSnapshot]    = useState<AppSnapshot | null>(null);
  const [history,     setHistory]     = useState<HistoryItem[]>([]);
  const [statusPhase, setStatusPhase] = useState<string>("");
  const [tokenBuf,    setTokenBuf]    = useState<string>("");
  const [busy,        setBusy]        = useState(false);
  const [errorBanner, setErrorBanner] = useState<string>("");
  const [activeView,  setActiveView]  = useState<ActiveView>("dashboard");
  const [inviteOpen,  setInviteOpen]  = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  // ── Retry toast ───────────────────────────────────────────────────────────
  const [retryToast, setRetryToast] = useState<{ message: string; audioId: string } | null>(null);

  // ── Edit confirmation toast ────────────────────────────────────────────────
  const [editToast, setEditToast] = useState<{
    recordingId: string; aiOutput: string; userKept: string;
  } | null>(null);

  // ── Vocabulary toast (manual add, auto-promote, star) ─────────────────────
  const [vocabToast, setVocabToast] = useState<VocabToastPayload | null>(null);

  // ── Pending edits ─────────────────────────────────────────────────────────
  const [pendingEdits, setPendingEdits] = useState<PendingEdit[]>([]);

  // ── Cloud auth gate ────────────────────────────────────────────────────────
  // null = still checking, false = signed in, true = needs sign-in
  const [needsAuth,   setNeedsAuth]   = useState<boolean | null>(null);
  const [authMode,    setAuthMode]    = useState<"login" | "signup">("login");
  const [authEmail,   setAuthEmail]   = useState("");
  const [authPass,    setAuthPass]    = useState("");
  const [authBusy,    setAuthBusy]    = useState(false);
  const [authError,   setAuthError]   = useState("");

  // ── OpenAI connection gate ─────────────────────────────────────────────────
  // null = still checking, true = connected, false = must connect
  const [openAIConnected, setOpenAIConnected] = useState<boolean | null>(null);
  const [connectBusy,     setConnectBusy]     = useState(false);
  const [connectError,    setConnectError]    = useState("");

  // Theme (light/dark) — persisted in localStorage, applied to <html>
  const { theme, toggle: toggleTheme } = useTheme();

  // ── Fetch history from backend ─────────────────────────────────────────────
  const refreshHistory = useCallback(async () => {
    const recs = await listHistory(100);
    setHistory(recs.map(recordingToHistoryItem));
  }, []);

  // ── Bootstrap + auth check ─────────────────────────────────────────────────
  useEffect(() => {
    invoke("bootstrap")
      .then(async (snap) => {
        setSnapshot(snap as AppSnapshot);
        // Cloud auth — skippable
        const cloudStatus = await getCloudStatus();
        setNeedsAuth(cloudStatus ? !cloudStatus.connected : false);
        // OpenAI connection — REQUIRED
        const oaStatus = await getOpenAIStatus();
        setOpenAIConnected(oaStatus?.connected ?? false);
      })
      .catch((err: unknown) => {
        setErrorBanner(err instanceof Error ? err.message : String(err));
        setNeedsAuth(false);
        setOpenAIConnected(false); // still show connect gate on error
      });
    refreshHistory();
  }, [refreshHistory]);

  // ── OpenAI OAuth connect ───────────────────────────────────────────────────
  const handleOpenAIConnect = useCallback(async () => {
    setConnectBusy(true);
    setConnectError("");
    try {
      await initiateOpenAIOAuth(); // opens system browser
      // Poll every 2 s until the callback server saves the token (max 5 min)
      const deadline = Date.now() + 5 * 60 * 1000;
      const poll = setInterval(async () => {
        if (Date.now() > deadline) {
          clearInterval(poll);
          setConnectBusy(false);
          setConnectError("Timed out waiting for sign-in. Please try again.");
          return;
        }
        try {
          const status = await getOpenAIStatus();
          if (status?.connected) {
            clearInterval(poll);
            setOpenAIConnected(true);
            setConnectBusy(false);
          }
        } catch { /* ignore poll errors */ }
      }, 2000);
    } catch (err: unknown) {
      setConnectError(err instanceof Error ? err.message : String(err));
      setConnectBusy(false);
    }
  }, []);

  // ── Auth submit ────────────────────────────────────────────────────────────
  const handleAuthSubmit = useCallback(async (e: React.FormEvent) => {
    e.preventDefault();
    setAuthBusy(true);
    setAuthError("");
    try {
      if (authMode === "login") {
        await cloudLogin(authEmail, authPass);
      } else {
        await cloudSignup(authEmail, authPass);
      }
      setNeedsAuth(false);
      refreshHistory();
    } catch (err: unknown) {
      setAuthError(err instanceof Error ? err.message : String(err));
    } finally {
      setAuthBusy(false);
    }
  }, [authMode, authEmail, authPass, refreshHistory]);

  // ── Real-time Tauri event subscriptions ────────────────────────────────────
  useEffect(() => {
    // State changes pushed from Rust (hotkey recording, processing, done)
    const unsubState  = onAppState((snap) => {
      setSnapshot(snap);
      setBusy(snap.state === "processing");
      if (snap.state === "idle") {
        setStatusPhase("");
        setTokenBuf("");
      }
    });

    // Voice pipeline status (transcribing / polishing)
    const unsubStatus = onVoiceStatus((phase) => {
      setStatusPhase(phase);
    });

    // Individual LLM tokens for live preview
    const unsubToken  = onVoiceToken((token) => {
      setTokenBuf((prev) => prev + token);
    });

    // Final done event — refresh history with the new recording
    const unsubDone   = onVoiceDone((_done) => {
      refreshHistory();
      setTokenBuf("");
      setStatusPhase("");
    });

    // Voice error → show retry toast
    const unsubError = onVoiceError((msg, audioId) => {
      setRetryToast({ message: msg, audioId: audioId ?? "" });
      setBusy(false);
      setSnapshot((p) => (p ? { ...p, state: "idle" } : p));
      setStatusPhase("");
      setTokenBuf("");
    });

    // Edit detected (legacy in-app toast — still fires as fallback)
    const unsubEdit = onEditDetected((payload) => {
      setEditToast({
        recordingId: payload.recording_id,
        aiOutput:    payload.ai_output,
        userKept:    payload.user_kept,
      });
    });

    // Pending edits changed → refresh list + send native notification
    const refreshPending = async () => {
      const r = await getPendingEdits();
      setPendingEdits(r.edits);
      if (r.edits.length > 0) {
        const edit = r.edits[0];
        const ai   = edit.ai_output.length > 50 ? edit.ai_output.slice(0, 50) + "…" : edit.ai_output;
        const kept = edit.user_kept.length  > 50 ? edit.user_kept.slice(0, 50)  + "…" : edit.user_kept;
        sendNotification(
          "Said noticed an edit — tap to review",
          `"${ai}"  →  "${kept}"`
        );
      }
    };
    refreshPending();
    const unsubPending = onPendingEditsChanged(refreshPending);

    // Vocabulary toast — fires on auto-promote during dictation,
    // manual add via the Vocabulary panel, and star toggles.
    const unsubVocabToast = onVocabToast(setVocabToast);

    // Tray menu → navigate to Settings
    const unsubNav = onNavSettings(() => setSettingsOpen(true));

    // Tray "Reconnect OpenAI…" — browser already opened by Rust; start polling
    const unsubReconnect = onOpenAIReconnectInitiated(() => {
      setConnectBusy(true);
      setConnectError("");
      const deadline = Date.now() + 5 * 60 * 1000;
      const poll = setInterval(async () => {
        if (Date.now() > deadline) {
          clearInterval(poll);
          setConnectBusy(false);
          setConnectError("Timed out waiting for sign-in. Please try again.");
          return;
        }
        try {
          const status = await getOpenAIStatus();
          if (status?.connected) {
            clearInterval(poll);
            setOpenAIConnected(true);
            setConnectBusy(false);
          }
        } catch { /* ignore */ }
      }, 2000);
    });

    return () => {
      unsubNav();
      unsubReconnect();
      unsubState();
      unsubStatus();
      unsubToken();
      unsubDone();
      unsubError();
      unsubEdit();
      unsubPending();
      unsubVocabToast();
    };
  }, [refreshHistory]);

  // ── Periodic snapshot poll — picks up Accessibility/Input Monitoring grants ──
  // 5 s is fast enough — permission changes require a user trip to System Settings.
  useEffect(() => {
    const interval = setInterval(async () => {
      if (busy) return;
      try {
        const next = await invoke("get_snapshot");
        setSnapshot(next);
      } catch {
        // silently ignore
      }
    }, 5000);
    return () => clearInterval(interval);
  }, [busy]);

  // ── Record toggle (button click) ───────────────────────────────────────────
  const handleToggle = useCallback(async () => {
    if (!snapshot) return;
    setErrorBanner("");
    if (snapshot.state === "recording") {
      setBusy(true);
      setSnapshot((p) => (p ? { ...p, state: "processing" } : p));
    }
    try {
      const next = await invoke("toggle_recording");
      setSnapshot(next);
      if (next.state === "idle") {
        await refreshHistory();
        setBusy(false);
      }
    } catch (err: unknown) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
      setSnapshot((p) => (p ? { ...p, state: "idle" } : p));
      setBusy(false);
    }
  }, [snapshot, refreshHistory]);

  // ── Accessibility ──────────────────────────────────────────────────────────
  const handleAccessibility = useCallback(async () => {
    setErrorBanner("");
    try {
      const next = await invoke("request_accessibility");
      setSnapshot(next);
    } catch (err: unknown) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    }
  }, []);

  // ── Input Monitoring ───────────────────────────────────────────────────────
  const handleInputMonitoring = useCallback(async () => {
    setErrorBanner("");
    try {
      await requestInputMonitoring();
      // Re-read snapshot after a short delay to pick up new permission state
      setTimeout(async () => {
        try {
          const next = await invoke("get_snapshot");
          setSnapshot(next);
        } catch { /* ignore */ }
      }, 1000);
    } catch (err: unknown) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    }
  }, []);

  // ── Navigation ─────────────────────────────────────────────────────────────
  const handleViewChange = useCallback((view: string) => {
    // Settings is now a modal — intercept the route and open the modal instead
    if (view === "settings") {
      setSettingsOpen(true);
      return;
    }
    if (VALID_VIEWS.includes(view as ActiveView)) {
      setActiveView(view as ActiveView);
      // Refresh history when user opens the history tab
      if (view === "history") refreshHistory();
    }
  }, [refreshHistory]);

  // ── Merge history into snapshot for child components ──────────────────────
  const snapshotWithHistory: AppSnapshot | null = snapshot
    ? {
        ...snapshot,
        history,
        total_words:  history.reduce((s, h) => s + h.word_count, 0),
        daily_streak: computeStreak(history),
        avg_wpm:      (() => {
          const recent = history.slice(0, 10);
          if (!recent.length) return 0;
          const tw = recent.reduce((s, h) => s + h.word_count, 0);
          const tm = recent.reduce((s, h) => s + h.recording_seconds / 60, 0);
          return tm > 0 ? Math.round(tw / tm) : 0;
        })(),
      }
    : null;

  // ── Live status / token overlay for DashboardView ─────────────────────────
  // We pass these as extra props; DashboardView can render a streaming preview.
  const liveText = statusPhase === "polishing" ? tokenBuf : "";

  /* ── Auth gate ──────────────────────────────────────────────────────────── */
  if (needsAuth === null || openAIConnected === null) {
    // Still checking — show a bare loading screen
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-3 opacity-60">
          <Mic size={28} />
          <span className="text-sm">Starting…</span>
        </div>
      </div>
    );
  }

  if (needsAuth) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-background">
        <div className="w-full max-w-sm flex flex-col gap-6 p-8 rounded-2xl" style={{ background: "hsl(var(--surface-2))" }}>

          {/* Brand */}
          <div className="flex flex-col items-center gap-2 mb-2">
            <Mic size={32} className="opacity-80" />
            <h1 className="text-xl font-semibold tracking-tight">Said</h1>
            <p className="text-xs text-muted-foreground text-center">Voice Polish Studio</p>
          </div>

          {/* Mode toggle */}
          <div className="flex gap-1 p-1 rounded-xl" style={{ background: "hsl(var(--surface-1))" }}>
            {(["login", "signup"] as const).map((m) => (
              <button
                key={m}
                onClick={() => { setAuthMode(m); setAuthError(""); }}
                className={`flex-1 py-1.5 text-xs font-medium rounded-lg transition-all ${authMode === m ? "bg-foreground text-background" : "text-muted-foreground hover:text-foreground"}`}
              >
                {m === "login" ? "Sign in" : "Create account"}
              </button>
            ))}
          </div>

          {/* Form */}
          <form onSubmit={handleAuthSubmit} className="flex flex-col gap-3">
            <input
              type="email"
              placeholder="Email"
              value={authEmail}
              onChange={(e) => setAuthEmail(e.target.value)}
              required
              className="w-full px-3 py-2 text-sm rounded-lg bg-background border border-border outline-none focus:ring-1 focus:ring-foreground/30"
            />
            <input
              type="password"
              placeholder="Password"
              value={authPass}
              onChange={(e) => setAuthPass(e.target.value)}
              required
              className="w-full px-3 py-2 text-sm rounded-lg bg-background border border-border outline-none focus:ring-1 focus:ring-foreground/30"
            />
            {authError && (
              <p className="text-xs text-red-400 px-1">{authError}</p>
            )}
            <button
              type="submit"
              disabled={authBusy || !authEmail || !authPass}
              className="w-full py-2 text-sm font-medium rounded-lg bg-foreground text-background disabled:opacity-40 transition-opacity"
            >
              {authBusy ? "…" : authMode === "login" ? "Sign in" : "Create account"}
            </button>
          </form>

          {/* Skip option — for offline / dev use */}
          <button
            onClick={() => setNeedsAuth(false)}
            className="text-xs text-muted-foreground hover:text-foreground text-center transition-colors"
          >
            Continue without account (offline mode)
          </button>
        </div>
      </div>
    );
  }

  /* ── OpenAI connection gate (required — no skip) ───────────────────────── */
  if (!openAIConnected) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-background">
        <div className="w-full max-w-sm flex flex-col gap-6 p-8 rounded-2xl" style={{ background: "hsl(var(--surface-2))" }}>

          {/* Brand */}
          <div className="flex flex-col items-center gap-2 mb-2">
            <Mic size={32} className="opacity-80" />
            <h1 className="text-xl font-semibold tracking-tight">Said</h1>
            <p className="text-xs text-muted-foreground text-center">Voice Polish Studio</p>
          </div>

          {/* Copy */}
          <div className="flex flex-col items-center gap-2 text-center">
            <p className="text-sm font-medium">Connect your ChatGPT account</p>
            <p className="text-xs text-muted-foreground leading-relaxed">
              Said uses your OpenAI account to polish your voice recordings. Connect once — it works silently from there.
            </p>
          </div>

          {connectError && (
            <p className="text-xs text-red-400 text-center px-1">{connectError}</p>
          )}

          {/* CTA */}
          <button
            onClick={handleOpenAIConnect}
            disabled={connectBusy}
            className="w-full py-2.5 text-sm font-medium rounded-lg bg-foreground text-background disabled:opacity-40 transition-opacity flex items-center justify-center gap-2"
          >
            {connectBusy ? (
              <>
                <span className="inline-block w-3 h-3 rounded-full border border-background/30 border-t-background animate-spin" />
                Waiting for browser…
              </>
            ) : (
              "Connect OpenAI account →"
            )}
          </button>

          {connectBusy && (
            <p className="text-xs text-muted-foreground text-center leading-relaxed">
              Complete the sign-in in your browser. This window will update automatically.
            </p>
          )}
        </div>
      </div>
    );
  }

  /* ── Render ─────────────────────────────────────────────────────────────── */
  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background">

      {/* ── Sidebar — full height left column ────────── */}
      <Sidebar
        snapshot={snapshotWithHistory}
        activeView={activeView}
        onViewChange={handleViewChange}
        busy={busy}
        onOpenInvite={() => setInviteOpen(true)}
      />

      {/* ── Invite team modal (overlays everything) ────── */}
      <InviteTeamModal open={inviteOpen} onClose={() => setInviteOpen(false)} />

      {/* ── Settings modal (replaces the old Settings route) ── */}
      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        snapshot={snapshotWithHistory}
        onAccessibility={handleAccessibility}
        onInputMonitoring={handleInputMonitoring}
      />

      {/* ── Right column: topbar + content ───────────── */}
      <div className="flex flex-col flex-1 overflow-hidden min-w-0">

        <Topbar
          snapshot={snapshotWithHistory}
          theme={theme}
          toggleTheme={toggleTheme}
          onLoginClick={() => setNeedsAuth(true)}
        />

        {/* ── The "mat" — elevated content surface ─────── */}
        <main className="flex-1 overflow-hidden p-3 pt-2">
          <div className="h-full rounded-2xl overflow-hidden" style={{ background: "hsl(var(--surface-2))" }}>
            {activeView === "dashboard" && (
              <DashboardView
                snapshot={snapshotWithHistory}
                busy={busy}
                onToggle={handleToggle}
                onAccessibility={handleAccessibility}
                onNavigate={handleViewChange}
                statusPhase={statusPhase}
                liveText={liveText}
                pendingEdits={pendingEdits}
                onResolvePending={async (id, action) => {
                  await resolvePendingEdit(id, action);
                  setPendingEdits((prev) => prev.filter((e) => e.id !== id));
                }}
              />
            )}
            {activeView === "history"    && <HistoryView />}
            {activeView === "vocabulary" && <VocabularyView />}
            {activeView === "insights"   && <InsightsView snapshot={snapshotWithHistory} />}
            {/* Settings is now a modal — opened via setSettingsOpen */}
          </div>
        </main>
      </div>

      {/* ── Retry toast (bottom-center) ──────────────── */}
      {retryToast && (
        <RetryToast
          message={retryToast.message}
          canRetry={retryToast.audioId.length > 0}
          onRetry={async () => {
            setRetryToast(null);
            if (retryToast.audioId) {
              try {
                await invoke("retry_recording", { audioId: retryToast.audioId });
              } catch (e) {
                setErrorBanner(e instanceof Error ? e.message : String(e));
              }
            }
          }}
          onOpenHistory={() => {
            setRetryToast(null);
            handleViewChange("history");
          }}
          onDismiss={() => setRetryToast(null)}
        />
      )}

      {/* ── Edit confirmation toast (bottom-center) ── */}
      {editToast && !retryToast && (
        <EditConfirmToast
          aiOutput={editToast.aiOutput}
          userKept={editToast.userKept}
          onSave={async () => {
            setEditToast(null);
            try {
              await submitEditFeedback(editToast.recordingId, editToast.userKept);
            } catch { /* non-critical */ }
          }}
          onDismiss={() => setEditToast(null)}
        />
      )}

      {/* ── Vocabulary toast (bottom-center) ─────────── */}
      {vocabToast && !retryToast && !editToast && (
        <VocabularyToast
          kind={vocabToast.kind}
          term={vocabToast.term}
          source={vocabToast.source}
          onUndo={vocabToast.kind === "added" ? async () => {
            const t = vocabToast.term;
            setVocabToast(null);
            try {
              await deleteVocabularyTerm(t);
            } catch { /* non-critical */ }
          } : undefined}
          onDismiss={() => setVocabToast(null)}
        />
      )}

      {/* ── Floating error toast ──────────────────────── */}
      {errorBanner && (
        <div
          className="fixed bottom-4 right-4 max-w-sm rounded-xl px-4 py-3 flex items-start gap-3 z-50"
          style={{
            background: "hsl(0 75% 60% / 0.12)",
            color:      "hsl(0 75% 80%)",
          }}
        >
          <p className="text-[13px] flex-1 leading-snug">{errorBanner}</p>
          <button
            onClick={() => setErrorBanner("")}
            className="flex-shrink-0 transition-colors mt-0.5 no-drag opacity-60 hover:opacity-100"
          >
            <X size={14} />
          </button>
        </div>
      )}
    </div>
  );
}
