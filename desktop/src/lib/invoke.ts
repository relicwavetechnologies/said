import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppSnapshot,
  BackendEndpoint,
  CloudAuthResponse,
  CloudStatus,
  HistoryItem,
  OpenAIStatus,
  PendingEditsResponse,
  PolishDone,
  Preferences,
  PrefsUpdate,
  Recording,
} from "../types";

// ── Mock history ──────────────────────────────────────────────────────────────

const now = Date.now();
const DAY = 86_400_000;

const MOCK_HISTORY: HistoryItem[] = [
  {
    timestamp_ms: now - 2 * 60 * 1000,
    polished: "User will need to install VP.",
    word_count: 7,
    recording_seconds: 3.2,
    model: "gpt-5.4",
    transcribe_ms: 420,
    embed_ms: 210,
    polish_ms: 610,
  },
  {
    timestamp_ms: now - DAY - 2 * 60 * 60 * 1000,
    polished:
      "The analyze with AI button should only trigger on the existing DB metadata. The pull button had to do the detailed crawl.",
    word_count: 23,
    recording_seconds: 8.4,
    model: "gpt-5.4",
    transcribe_ms: 640,
    embed_ms: 290,
    polish_ms: 980,
  },
  {
    timestamp_ms: now - DAY - 2 * 60 * 60 * 1000 - 60 * 1000,
    polished:
      "Yes, but this time we will get the whole tree, no? The earlier data was limited to only 1 page, so this time the whole tree will come and it will analyze the difference, right?",
    word_count: 38,
    recording_seconds: 11.1,
    model: "gpt-5.4",
    transcribe_ms: 710,
    embed_ms: 0,
    polish_ms: 890,
  },
  {
    timestamp_ms: now - DAY - 9 * 60 * 60 * 1000,
    polished: "कि अभी तो मैं टाइप कर रहा हूं वैसे",
    word_count: 8,
    recording_seconds: 4.0,
    model: "gpt-5.4",
    transcribe_ms: 510,
    embed_ms: 180,
    polish_ms: 730,
  },
  {
    timestamp_ms: now - DAY - 9 * 60 * 60 * 1000 - 3 * 60 * 1000,
    polished: "Theek hai.",
    word_count: 2,
    recording_seconds: 1.5,
    model: "gpt-5.4-mini",
    transcribe_ms: 280,
    embed_ms: 0,
    polish_ms: 330,
  },
  {
    timestamp_ms: now - 2 * DAY - 3 * 60 * 60 * 1000,
    polished: "Can you check the latest deployment logs and see if there are any 5xx errors in the last hour?",
    word_count: 18,
    recording_seconds: 6.8,
    model: "claude-sonnet-4-6",
    transcribe_ms: 590,
    embed_ms: 240,
    polish_ms: 840,
  },
  {
    timestamp_ms: now - 3 * DAY - 11 * 60 * 60 * 1000,
    polished: "Schedule a team sync for Thursday at 3 PM and share the agenda by Wednesday evening.",
    word_count: 16,
    recording_seconds: 5.9,
    model: "gpt-5.4",
    transcribe_ms: 480,
    embed_ms: 195,
    polish_ms: 700,
  },
];

const MOCK_TOTAL_WORDS = MOCK_HISTORY.reduce((s, h) => s + h.word_count, 0) + 1132;
const MOCK_AVG_WPM = Math.round(
  MOCK_HISTORY.reduce((s, h) => s + h.word_count, 0) /
    MOCK_HISTORY.reduce((s, h) => s + h.recording_seconds / 60, 0)
);

// ── Mock snapshot ─────────────────────────────────────────────────────────────

const mockSnapshot: AppSnapshot = {
  state: "idle",
  platform: "browser-preview",
  current_mode: "mini",
  current_mode_label: "Fast (gpt-5.4-mini)",
  current_model: "gpt-5.4-mini",
  auto_paste_supported: false,
  accessibility_granted: false,
  input_monitoring_granted: false,
  modes: [
    { key: "mini", label: "Fast (gpt-5.4-mini)", model: "gpt-5.4-mini", icon: "fast" },
  ],
  last_result: {
    transcript: "kal sham meeting thodi delayed ho gayi thi",
    polished: "Kal sham meeting thodi delayed ho gayi thi.",
    model: "gpt-5.4",
    confidence: 0.94,
    transcribe_ms: 640,
    polish_ms: 980,
  },
  last_error: null,
  history: [...MOCK_HISTORY],
  total_words: MOCK_TOTAL_WORDS,
  daily_streak: 8,
  avg_wpm: MOCK_AVG_WPM || 186,
};

// ── Mock invoke ───────────────────────────────────────────────────────────────

async function mockInvoke(
  command: string,
  args?: Record<string, unknown>
): Promise<AppSnapshot> {
  if (command === "bootstrap" || command === "request_accessibility") {
    return structuredClone(mockSnapshot);
  }

  if (command === "set_mode") {
    // Model switching removed — always mini
    return structuredClone(mockSnapshot);
  }

  if (command === "toggle_recording") {
    if (mockSnapshot.state === "idle") {
      mockSnapshot.state = "recording";
      return structuredClone(mockSnapshot);
    }

    // Simulate finish recording
    const newText = "Yeh draft thoda aur natural lagna chahiye.";
    const wordCount = newText.split(" ").length;
    const newItem: HistoryItem = {
      timestamp_ms: Date.now(),
      polished: newText,
      word_count: wordCount,
      recording_seconds: 4.5,
      model: mockSnapshot.current_model,
      transcribe_ms: 580,
      polish_ms: 910,
    };

    mockSnapshot.history = [newItem, ...mockSnapshot.history];
    mockSnapshot.total_words += wordCount;
    mockSnapshot.daily_streak = Math.max(mockSnapshot.daily_streak, 1);

    // Recalculate avg_wpm
    const recent = mockSnapshot.history.slice(0, 10);
    const totalW = recent.reduce((s, h) => s + h.word_count, 0);
    const totalM = recent.reduce((s, h) => s + h.recording_seconds / 60, 0);
    mockSnapshot.avg_wpm = totalM > 0 ? Math.round(totalW / totalM) : 186;

    mockSnapshot.state = "idle";
    mockSnapshot.last_result = {
      transcript: "yeh draft thoda aur natural lagna chahiye",
      polished: newText,
      model: mockSnapshot.current_model,
      confidence: 0.97,
      transcribe_ms: 580,
      polish_ms: 910,
    };
    return structuredClone(mockSnapshot);
  }

  throw new Error(`Unknown mock command: ${command}`);
}

// ── Tauri detection ───────────────────────────────────────────────────────────

export function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    (("__TAURI_INTERNALS__" in window && window.__TAURI_INTERNALS__ != null) ||
      ("__TAURI__" in window &&
        (window as Record<string, unknown>).__TAURI__ != null))
  );
}

// ── Public invoke ─────────────────────────────────────────────────────────────

export async function invoke<T = AppSnapshot>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  if (!isTauriRuntime()) {
    return mockInvoke(command, args) as Promise<T>;
  }
  return tauriInvoke<T>(command, args);
}

// ── Backend-aware commands (Phase E) ─────────────────────────────────────────

/** Get the local daemon URL + secret (for direct HTTP calls from the frontend). */
export async function getBackendEndpoint(): Promise<BackendEndpoint | null> {
  if (!isTauriRuntime()) return null;
  try {
    return await tauriInvoke<BackendEndpoint>("get_backend_endpoint");
  } catch {
    return null;
  }
}

/** Fetch current user preferences from the backend. */
export async function getPreferences(): Promise<Preferences | null> {
  if (!isTauriRuntime()) return null;
  try {
    return await tauriInvoke<Preferences>("get_preferences");
  } catch {
    return null;
  }
}

/** Partially update preferences. Returns the updated preferences. */
export async function patchPreferences(
  update: PrefsUpdate
): Promise<Preferences | null> {
  if (!isTauriRuntime()) return null;
  try {
    return await tauriInvoke<Preferences>("patch_preferences", { update });
  } catch {
    return null;
  }
}

/** Fetch recording history from the backend (newest first). */
export async function listHistory(limit = 50): Promise<Recording[]> {
  if (!isTauriRuntime()) return [];
  try {
    return await tauriInvoke<Recording[]>("get_history", { limit });
  } catch {
    return [];
  }
}

/** Diagnostic — try all 5 AX field-reading methods on whatever is focused. */
export interface AxMethodResult {
  method: string;
  label:  string;
  ok:     boolean;
  text:   string | null;
  err:    string | null;
}
export interface AxDiagnostics {
  ax_trusted:   boolean;
  app_name:     string | null;
  app_pid:      number | null;
  element_role: string | null;
  attributes:   string[];
  methods:      AxMethodResult[];
  clipboard:    string;
}
export async function diagnoseAx(delaySecs: number): Promise<AxDiagnostics | null> {
  if (!isTauriRuntime()) return null;
  try {
    return await tauriInvoke<AxDiagnostics>("diagnose_ax", { delaySecs });
  } catch (e) {
    console.error("diagnose_ax failed", e);
    return null;
  }
}

/** Open System Settings → Privacy & Security → Input Monitoring. */
export async function requestInputMonitoring(): Promise<void> {
  if (!isTauriRuntime()) return;
  try {
    await tauriInvoke("request_input_monitoring");
  } catch {
    // silently ignore
  }
}

/** Retry a recording by re-submitting its saved WAV. Result is auto-pasted. */
export async function retryRecording(audioId: string): Promise<void> {
  if (!isTauriRuntime()) return;
  await tauriInvoke("retry_recording", { audioId });
}

/** Delete a recording (SQLite row + WAV file). */
export async function deleteRecording(id: string): Promise<void> {
  if (!isTauriRuntime()) return;
  await tauriInvoke("delete_recording", { id });
}

/** Return { url, secret } to fetch a recording's WAV audio with Authorization header. */
export async function getRecordingAudioUrl(
  id: string
): Promise<{ url: string; secret: string } | null> {
  if (!isTauriRuntime()) return null;
  try {
    return await tauriInvoke<{ url: string; secret: string }>(
      "get_recording_audio_url", { id }
    );
  } catch {
    return null;
  }
}

/** Submit edit feedback so the backend can learn from user corrections. */
export async function submitEditFeedback(
  recordingId: string,
  userKept: string,
  targetApp?: string
): Promise<void> {
  if (!isTauriRuntime()) return;
  try {
    await tauriInvoke("submit_edit_feedback", {
      recording_id: recordingId,
      user_kept: userKept,
      target_app: targetApp ?? null,
    });
  } catch {
    // Non-critical — swallow silently
  }
}

// ── SSE event listeners (Phase E streaming) ───────────────────────────────────

type Unsubscribe = () => void;

/** Listen for individual LLM tokens as they stream in. */
export function onVoiceToken(
  handler: (token: string) => void
): Unsubscribe {
  if (!isTauriRuntime()) return () => {};
  let unsub: Unsubscribe = () => {};
  listen<{ token: string }>("voice-token", (e) => handler(e.payload.token)).then(
    (fn) => { unsub = fn; }
  );
  return () => unsub();
}

/** Listen for status updates (transcribing / polishing). */
export function onVoiceStatus(
  handler: (phase: string, transcript?: string) => void
): Unsubscribe {
  if (!isTauriRuntime()) return () => {};
  let unsub: Unsubscribe = () => {};
  listen<{ phase: string; transcript?: string }>("voice-status", (e) =>
    handler(e.payload.phase, e.payload.transcript)
  ).then((fn) => { unsub = fn; });
  return () => unsub();
}

/** Listen for the final done event. */
export function onVoiceDone(
  handler: (done: PolishDone) => void
): Unsubscribe {
  if (!isTauriRuntime()) return () => {};
  let unsub: Unsubscribe = () => {};
  listen<PolishDone>("voice-done", (e) => handler(e.payload)).then(
    (fn) => { unsub = fn; }
  );
  return () => unsub();
}

/** Listen for error events. `audioId` is the saved WAV id for retrying. */
export function onVoiceError(
  handler: (message: string, audioId?: string) => void
): Unsubscribe {
  if (!isTauriRuntime()) return () => {};
  let unsub: Unsubscribe = () => {};
  listen<{ message: string; audio_id?: string }>("voice-error", (e) =>
    handler(e.payload.message, e.payload.audio_id)
  ).then((fn) => { unsub = fn; });
  return () => unsub();
}

/** Listen for detected edits that need user confirmation before being saved. */
export interface EditDetectedPayload {
  recording_id: string;
  ai_output:    string;
  user_kept:    string;
}
export function onEditDetected(
  handler: (payload: EditDetectedPayload) => void
): Unsubscribe {
  if (!isTauriRuntime()) return () => {};
  let unsub: Unsubscribe = () => {};
  listen<EditDetectedPayload>("edit-detected", (e) => handler(e.payload)).then(
    (fn) => { unsub = fn; }
  );
  return () => unsub();
}

/** Listen for app-state updates (e.g. state changed to processing/idle). */
export function onAppState(
  handler: (snap: AppSnapshot) => void
): Unsubscribe {
  if (!isTauriRuntime()) return () => {};
  let unsub: Unsubscribe = () => {};
  listen<AppSnapshot>("app-state", (e) => handler(e.payload)).then(
    (fn) => { unsub = fn; }
  );
  return () => unsub();
}

/** Listen for "nav-settings" — fired when the tray menu's Settings entry is clicked. */
export function onNavSettings(handler: () => void): Unsubscribe {
  if (!isTauriRuntime()) return () => {};
  let unsub: Unsubscribe = () => {};
  listen("nav-settings", () => handler()).then((fn) => { unsub = fn; });
  return () => unsub();
}

export function onOpenAIReconnectInitiated(handler: () => void): Unsubscribe {
  if (!isTauriRuntime()) return () => {};
  let unsub: Unsubscribe = () => {};
  listen("openai-reconnect-initiated", () => handler()).then((fn) => { unsub = fn; });
  return () => unsub();
}

// ── Cloud auth commands ───────────────────────────────────────────────────────

/** Sign up for a new cloud account. Returns token + account info. */
export async function cloudSignup(
  email: string,
  password: string
): Promise<CloudAuthResponse> {
  if (!isTauriRuntime()) throw new Error("Tauri not available");
  return tauriInvoke<CloudAuthResponse>("cloud_signup", { email, password });
}

/** Log in to an existing cloud account. */
export async function cloudLogin(
  email: string,
  password: string
): Promise<CloudAuthResponse> {
  if (!isTauriRuntime()) throw new Error("Tauri not available");
  return tauriInvoke<CloudAuthResponse>("cloud_login", { email, password });
}

/** Log out (clears stored cloud token). */
export async function cloudLogout(): Promise<void> {
  if (!isTauriRuntime()) return;
  return tauriInvoke("cloud_logout");
}

/** Get current cloud connection status. */
export async function getCloudStatus(): Promise<CloudStatus | null> {
  if (!isTauriRuntime()) return null;
  try {
    return await tauriInvoke<CloudStatus>("get_cloud_status");
  } catch {
    return null;
  }
}

// ── OpenAI OAuth commands ─────────────────────────────────────────────────────

/** Get current OpenAI OAuth connection status. */
export async function getOpenAIStatus(): Promise<OpenAIStatus | null> {
  if (!isTauriRuntime()) return null;
  try {
    return await tauriInvoke<OpenAIStatus>("get_openai_status");
  } catch {
    return null;
  }
}

/** Initiate OpenAI OAuth flow — returns the URL to open in the browser.
 *  The backend spawns a one-shot callback server on localhost:1455. */
export async function initiateOpenAIOAuth(): Promise<string> {
  if (!isTauriRuntime()) throw new Error("Tauri not available");
  const res = await tauriInvoke<{ auth_url: string }>("initiate_openai_oauth");
  return res.auth_url;
}

/** Disconnect the linked OpenAI account (deletes local token, reverts to gateway). */
export async function disconnectOpenAI(): Promise<void> {
  if (!isTauriRuntime()) return;
  await tauriInvoke("disconnect_openai");
}

// ── Notification permission ───────────────────────────────────────────────────

// isPermissionGranted() returns a PermissionState string: "granted" | "denied" | "prompt"
// IMPORTANT — Tauri plugin-notification surface:
//   isPermissionGranted()  returns Promise<boolean>      (NOT a string!)
//   requestPermission()    returns Promise<"granted"|"denied"|"default">
// The previous version cast the boolean as a string, so `true` never matched
// "granted" and the UI was permanently stuck on "Allow".

export type NotifPermission = "granted" | "denied" | "prompt" | "unknown";

/** Check the current macOS notification permission state without prompting. */
export async function checkNotificationPermission(): Promise<NotifPermission> {
  if (!isTauriRuntime()) return "unknown";
  try {
    const { isPermissionGranted } = await import("@tauri-apps/plugin-notification");
    const granted = await isPermissionGranted();
    if (granted === true)  return "granted";
    if (granted === false) return "prompt";
    return "unknown";
  } catch {
    return "unknown";
  }
}

/** Request macOS notification permission.
 *  Returns the resulting PermissionState string.
 *  NOTE: if already "denied", macOS will NOT re-prompt — user must enable in System Settings. */
export async function requestNotifications(): Promise<NotifPermission> {
  if (!isTauriRuntime()) return "unknown";
  try {
    const { isPermissionGranted, requestPermission } = await import(
      "@tauri-apps/plugin-notification"
    );
    if (await isPermissionGranted() === true) return "granted";
    // requestPermission returns "granted" | "denied" | "default"
    const result = await requestPermission();
    if (result === "granted") return "granted";
    if (result === "denied")  return "denied";
    return "prompt";   // "default" → still un-decided; treat as prompt-able
  } catch {
    return "unknown";
  }
}

/** Send a native notification. Silently no-ops if permission is not granted. */
export async function sendNotification(title: string, body: string): Promise<void> {
  if (!isTauriRuntime()) return;
  try {
    const { isPermissionGranted, sendNotification: pluginSend } = await import(
      "@tauri-apps/plugin-notification"
    );
    const state = await isPermissionGranted();
    if (state === "granted") {
      await pluginSend({ title, body });
    }
  } catch {
    // silently ignore
  }
}

// ── Pending-edit review ───────────────────────────────────────────────────────

export async function getPendingEdits(): Promise<PendingEditsResponse> {
  if (!isTauriRuntime()) return { edits: [], total: 0 };
  try {
    return await tauriInvoke<PendingEditsResponse>("get_pending_edits");
  } catch {
    return { edits: [], total: 0 };
  }
}

export async function resolvePendingEdit(
  id: string,
  action: "approve" | "skip"
): Promise<void> {
  if (!isTauriRuntime()) return;
  try {
    await tauriInvoke("resolve_pending_edit", { id, action });
  } catch {
    // non-critical
  }
}

/** Listen for the backend's signal that pending edits list changed. */
export function onPendingEditsChanged(handler: () => void): () => void {
  if (!isTauriRuntime()) return () => {};
  let unsub: () => void = () => {};
  listen("pending-edits-changed", () => handler()).then((fn) => { unsub = fn; });
  return () => unsub();
}

// ── Vocabulary management ────────────────────────────────────────────────────

export interface VocabRow {
  term:      string;
  weight:    number;
  use_count: number;
  last_used: number;
  source:    "auto" | "manual" | "starred";
}

export interface VocabListResponse {
  terms: VocabRow[];
  total: number;
}

export async function listVocabulary(): Promise<VocabListResponse> {
  if (!isTauriRuntime()) return { terms: [], total: 0 };
  try {
    return await tauriInvoke<VocabListResponse>("list_vocabulary");
  } catch {
    return { terms: [], total: 0 };
  }
}

export async function addVocabularyTerm(term: string): Promise<void> {
  if (!isTauriRuntime()) return;
  await tauriInvoke("add_vocabulary_term", { term });
}

export async function deleteVocabularyTerm(term: string): Promise<void> {
  if (!isTauriRuntime()) return;
  await tauriInvoke("delete_vocabulary_term", { term });
}

export async function starVocabularyTerm(term: string): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  try {
    return await tauriInvoke<boolean>("star_vocabulary_term", { term });
  } catch {
    return false;
  }
}

/** Listen for vocabulary mutations (manual add / delete / star toggle / auto-promote). */
export function onVocabularyChanged(handler: () => void): () => void {
  if (!isTauriRuntime()) return () => {};
  let unsub: () => void = () => {};
  listen("vocabulary-changed", () => handler()).then((fn) => { unsub = fn; });
  return () => unsub();
}

// Suppress unused-import warnings for types only used in exported signatures
export type {
  CloudAuthResponse,
  CloudStatus,
  HistoryItem,
  OpenAIStatus,
  PendingEditsResponse,
  PolishDone,
  Preferences,
  PrefsUpdate,
  Recording,
  BackendEndpoint,
};
