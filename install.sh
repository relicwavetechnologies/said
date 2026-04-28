#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
#  Voice Polish — SINGLE-FILE installer
#
#  Install (first time or update):
#    curl -fsSL https://raw.githubusercontent.com/relicwavetechnologies/said/main/install.sh | bash
#
#  After install, everything is managed with one command:
#    vp              → start
#    vp stop         → stop
#    vp update       → get latest version
#    vp status       → check if running
#    vp logs         → live logs
#    vp delete       → remove everything
# ══════════════════════════════════════════════════════════════════════════════

GATEWAY_KEY="cnsc_gw_23450226f2fdcaa1f661284ae8d54c12acae140c51c24fc7"
INSTALL_URL="https://raw.githubusercontent.com/relicwavetechnologies/said/main/install.sh"

INSTALL_DIR="$HOME/VoicePolish"
PLIST_NAME="com.voicepolish.app"
PLIST_PATH="$HOME/Library/LaunchAgents/$PLIST_NAME.plist"
LOG_OUT="/tmp/voice-polish.log"
LOG_ERR="/tmp/voice-polish.err"

# ─────────────────────────────────────────────────────────────────────────────
BOLD='\033[1m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'

ok()   { echo -e "  ${GREEN}✓ $1${NC}"; }
skip() { echo -e "  ${GREEN}✓ $1 — already done, skipping${NC}"; }
info() { echo -e "  ${YELLOW}→ $1${NC}"; }
fail() { echo -e "  ${RED}✗ $1${NC}"; exit 1; }
step() { echo -e "\n${BOLD}[$1]${NC} $2"; }

echo ""
echo -e "${BOLD}🎤  Voice Polish — Setup${NC}"
echo "══════════════════════════════════════════════"

# ── 1. Homebrew ───────────────────────────────────────────────────────────────
step "1/9" "Homebrew"
if command -v brew &>/dev/null; then
    skip "Homebrew $(brew --version | head -1)"
else
    info "Not found — installing Homebrew (takes ~2 min, needs your password) …"
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)" \
        || fail "Homebrew install failed"
    [ -f /opt/homebrew/bin/brew ] && eval "$(/opt/homebrew/bin/brew shellenv)"
    ok "Homebrew installed"
fi
[ -f /opt/homebrew/bin/brew ] && eval "$(/opt/homebrew/bin/brew shellenv)"

# ── 2. Python 3.11+ ───────────────────────────────────────────────────────────
step "2/9" "Python 3.11+"
PYTHON=""
for c in /opt/homebrew/bin/python3.11 /opt/homebrew/bin/python3.12 /opt/homebrew/bin/python3.13 \
         /usr/local/bin/python3.11 python3.11 python3.12; do
    if command -v "$c" &>/dev/null; then PYTHON="$c"; break; fi
done
if [ -n "$PYTHON" ]; then
    skip "$($PYTHON --version)"
else
    info "Not found — installing Python 3.11 via Homebrew …"
    brew install python@3.11 || fail "Python install failed"
    PYTHON=/opt/homebrew/bin/python3.11
    ok "$($PYTHON --version) installed"
fi

# ── 3. Project folder ─────────────────────────────────────────────────────────
step "3/9" "Project folder"
if [ -d "$INSTALL_DIR" ]; then skip "$INSTALL_DIR"
else mkdir -p "$INSTALL_DIR" || fail "Could not create $INSTALL_DIR"; ok "Created $INSTALL_DIR"
fi

# ── 4. Source files (always written — ensures latest version) ─────────────────
step "4/9" "Writing source files"

cat > "$INSTALL_DIR/requirements.txt" << 'REQEOF'
python-dotenv>=1.0
sounddevice>=0.4.6
pynput>=1.7
rumps>=0.4
pyobjc-framework-Cocoa>=10.0
REQEOF

# ── config.py ─────────────────────────────────────────────────────────────────
cat > "$INSTALL_DIR/config.py" << 'CFGEOF'
import os, platform
from dotenv import load_dotenv
load_dotenv(dotenv_path=os.path.join(os.path.dirname(__file__), ".env"))

IS_MAC = platform.system() == "Darwin"
IS_WIN = platform.system() == "Windows"

GATEWAY_API_KEY = os.environ.get("GATEWAY_API_KEY", "")

GATEWAY_BASE   = "https://gateway-v21w.onrender.com"
VOICE_URL      = f"{GATEWAY_BASE}/v1/voice/polish"
SAMPLE_RATE    = 16_000
CHANNELS       = 1
MIN_DURATION_S = 0.5
FN_VK          = 63

MODES = ["fast", "smart", "claude", "gemini"]
MODE_LABELS = {
    "fast":   "⚡  Fast   (gpt-5.4-mini)",
    "smart":  "🧠  Smart  (gpt-5.4)",
    "claude": "🤖  Claude (claude-sonnet)",
    "gemini": "✨  Gemini (gemini-3.1-flash)",
}
_active_mode: str = "fast"

def get_mode() -> str:    return _active_mode
def cycle_mode() -> str:
    global _active_mode
    _active_mode = MODES[(MODES.index(_active_mode) + 1) % len(MODES)]
    return _active_mode
def mode_label() -> str:  return MODE_LABELS[_active_mode]
def validate():
    if not GATEWAY_API_KEY:
        raise EnvironmentError("GATEWAY_API_KEY missing from .env")
CFGEOF

# ── recorder.py ───────────────────────────────────────────────────────────────
cat > "$INSTALL_DIR/recorder.py" << 'RECEOF'
import wave, tempfile
import sounddevice as sd
from config import SAMPLE_RATE, CHANNELS, MIN_DURATION_S

class AudioRecorder:
    def __init__(self):
        self._frames = []; self._recording = False; self._stream = None

    def start(self):
        self._frames = []; self._recording = True
        self._stream = sd.InputStream(samplerate=SAMPLE_RATE, channels=CHANNELS,
                                      dtype="int16", callback=self._cb)
        self._stream.start()
        print("[rec] 🎤  recording … press hotkey again to stop")

    def _cb(self, indata, frames, t, status):
        if status: print(f"[rec] ⚠ {status}")
        if self._recording:
            self._frames.append(bytes(indata))

    def stop(self) -> str | None:
        self._recording = False
        if self._stream: self._stream.stop(); self._stream.close(); self._stream = None
        if not self._frames: print("[rec] no audio captured"); return None
        audio = b"".join(self._frames)
        dur = len(audio) / (SAMPLE_RATE * 2)
        print(f"[rec] ⏹  {dur:.1f}s recorded")
        if dur < MIN_DURATION_S: print("[rec] too short — ignored"); return None
        tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
        with wave.open(tmp.name, "wb") as wf:
            wf.setnchannels(CHANNELS); wf.setsampwidth(2)
            wf.setframerate(SAMPLE_RATE); wf.writeframes(audio)
        return tmp.name
RECEOF

# ── voice.py ──────────────────────────────────────────────────────────────────
cat > "$INSTALL_DIR/voice.py" << 'VOICEEOF'
import json, uuid, urllib.request, urllib.error
import config

def _multipart_body(fields: dict, files: dict) -> tuple:
    boundary = uuid.uuid4().hex
    ctype = f"multipart/form-data; boundary={boundary}"
    parts = []
    for name, value in fields.items():
        parts.append(f"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
    for name, (filename, data, mime) in files.items():
        parts.append(f"--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n")
        parts.append(data)
        parts.append("\r\n")
    parts.append(f"--{boundary}--\r\n")
    body = b"".join(p.encode() if isinstance(p, str) else p for p in parts)
    return body, ctype

def process(wav_path: str) -> str:
    mode = config.get_mode()
    print(f"[voice] mode={mode}  sending to gateway…")
    with open(wav_path, "rb") as f:
        audio_bytes = f.read()
    body, ctype = _multipart_body(
        fields={"mode": mode, "lang": "auto"},
        files={"audio": ("recording.wav", audio_bytes, "audio/wav")},
    )
    req = urllib.request.Request(
        config.VOICE_URL, data=body,
        headers={"X-API-Key": config.GATEWAY_API_KEY, "Content-Type": ctype},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read())
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"Gateway error {e.code}: {e.read()[:300].decode(errors='replace')}")
    transcript = data.get("transcript", "")
    polished   = data.get("polished", "")
    lat        = data.get("latency", {})
    print(f"[voice] transcript ({data.get('confidence',0):.2f}): {transcript}")
    print(f"[voice] polished   [{data.get('model','?')}]: {polished}")
    if lat: print(f"[voice] latency: stt={lat.get('transcribe_ms')}ms  llm={lat.get('polish_ms')}ms")
    if not polished: raise ValueError("Gateway returned empty polished text")
    return polished
VOICEEOF

# ── paster.py ─────────────────────────────────────────────────────────────────
cat > "$INSTALL_DIR/paster.py" << 'PASEOF'
import time, platform
from pynput.keyboard import Key, Controller
IS_MAC = platform.system() == "Darwin"
_kbd = Controller()

def paste(text: str):
    orig = _get()
    _set(text); time.sleep(0.08)
    if IS_MAC:
        with _kbd.pressed(Key.cmd): _kbd.press("v"); _kbd.release("v")
    else:
        with _kbd.pressed(Key.ctrl): _kbd.press("v"); _kbd.release("v")
    time.sleep(0.4); _set(orig)

def _get() -> str:
    if IS_MAC:
        from AppKit import NSPasteboard, NSStringPboardType
        return NSPasteboard.generalPasteboard().stringForType_(NSStringPboardType) or ""
    import pyperclip; return pyperclip.paste() or ""

def _set(text: str):
    if IS_MAC:
        from AppKit import NSPasteboard, NSStringPboardType
        pb = NSPasteboard.generalPasteboard(); pb.clearContents()
        pb.setString_forType_(text, NSStringPboardType)
    else:
        import pyperclip; pyperclip.copy(text)
PASEOF

# ── hotkey.py ─────────────────────────────────────────────────────────────────
cat > "$INSTALL_DIR/hotkey.py" << 'HKEOF'
import platform
from pynput import keyboard as kb
from pynput.keyboard import Key
IS_MAC = platform.system() == "Darwin"
FN_VK  = 63

def _label(key):
    try:
        if key.vk == FN_VK: return f"fn/Globe vk={key.vk}"
    except: pass
    try:
        if key.char: return f"'{key.char}' vk={getattr(key,'vk','?')}"
    except: pass
    try: return f"{key.name} vk={getattr(key,'vk','?')}"
    except: pass
    try: return f"<vk={key.vk}>"
    except: return str(key)

class HotkeyListener:
    def __init__(self, on_toggle):
        self._on_toggle = on_toggle
        self._vk = set(); self._ch = set(); self._sp = set(); self._fired = False

    def start(self):
        l = kb.Listener(on_press=self._press, on_release=self._release)
        l.daemon = True; l.start()
        label = "fn + Shift" if IS_MAC else "Ctrl + Alt + Shift"
        print(f"[hotkey] listening for {label}")

    def _shift(self): return any(k in self._sp for k in (Key.shift, Key.shift_l, Key.shift_r))

    def _track(self, key, add):
        op = set.add if add else set.discard
        try:
            if key.vk: op(self._vk, key.vk)
        except: pass
        try:
            if key.char: op(self._ch, key.char.lower())
        except: pass
        if isinstance(key, Key): op(self._sp, key)

    def _press(self, key):
        self._track(key, True)
        fn = FN_VK in self._vk; sh = self._shift()
        print(f"[hotkey] ↓ {_label(key):<35} shift={'YES' if sh else 'no '}" +
              (f"  fn={'YES' if fn else 'no'}" if IS_MAC else ""))
        if not IS_MAC: self._check_win()

    def _release(self, key):
        if IS_MAC:
            try: is_fn = key.vk == FN_VK
            except: is_fn = False
            if is_fn:
                sh = self._shift()
                print(f"[hotkey] ↑ {_label(key):<35} shift={'YES ← FIRING!' if sh else 'no ← need Shift'}")
                if sh and not self._fired:
                    print("[hotkey] 🔥 fn+Shift → toggling"); self._fired = True; self._on_toggle()
                    self._track(key, False); return
            else:
                print(f"[hotkey] ↑ {_label(key)}")
        else:
            print(f"[hotkey] ↑ {_label(key)}")
        self._track(key, False)
        if IS_MAC:
            if FN_VK not in self._vk: self._fired = False
        else:
            c = Key.ctrl_l in self._sp or Key.ctrl_r in self._sp
            a = Key.alt_l  in self._sp or Key.alt_r  in self._sp
            if not (c and a and self._shift()): self._fired = False

    def _check_win(self):
        if self._fired: return
        c = Key.ctrl_l in self._sp or Key.ctrl_r in self._sp
        a = Key.alt_l  in self._sp or Key.alt_r  in self._sp
        if c and a and self._shift():
            print("[hotkey] 🔥 Ctrl+Alt+Shift → toggling"); self._fired = True; self._on_toggle()
        else:
            miss = [x for x,v in [("Ctrl",c),("Alt",a),("Shift",self._shift())] if not v]
            if miss: print(f"[hotkey]   need: {', '.join(miss)}")
HKEOF

# ── app.py ────────────────────────────────────────────────────────────────────
cat > "$INSTALL_DIR/app.py" << 'APPEOF'
import os, time, platform, threading
import config
from recorder import AudioRecorder
from voice    import process
from paster   import paste
from hotkey   import HotkeyListener
IS_MAC = platform.system() == "Darwin"

class Core:
    IDLE="idle"; REC="recording"; PROC="processing"
    def __init__(self, status_fn):
        self.state=self.IDLE; self._rec=AudioRecorder()
        self._status=status_fn; self._lock=threading.Lock()

    def toggle(self):
        with self._lock:
            if   self.state==self.IDLE: self._start()
            elif self.state==self.REC:  self._stop()
            else: print("[app] busy — waiting for gateway response")

    def _start(self):
        self.state=self.REC; self._status("🔴"); self._rec.start()

    def _stop(self):
        self.state=self.PROC; self._status("⏳")
        wav = self._rec.stop()
        threading.Thread(target=self._run, args=(wav,), daemon=True).start()

    def _run(self, wav):
        try:
            if not wav: return
            print("[app] ── sending to gateway ─────────────────────")
            polished = process(wav)
            paste(polished)
            print("[app] ✓ pasted\n[app] ─────────────────────────────────────────")
            self._status("✅"); time.sleep(1.5)
        except Exception as e:
            print(f"[app] ✗ {e}"); self._status("❌"); time.sleep(2)
        finally:
            if wav and os.path.exists(wav): os.unlink(wav)
            self.state=self.IDLE; self._status("●")

def run():
    if IS_MAC:
        import rumps
        class App(rumps.App):
            def __init__(self):
                super().__init__("●", quit_button="Quit Voice Polish")
                self.core = Core(self._set_status)
                self._mi  = rumps.MenuItem(config.mode_label(), callback=self._cycle_mode)
                self.menu = [
                    rumps.MenuItem("🎤  Toggle recording  (fn+Shift)", callback=lambda _: self.core.toggle()),
                    self._mi,
                    None,
                ]
                HotkeyListener(self.core.toggle).start()

            def _set_status(self, i): self.title = i

            def _cycle_mode(self, _):
                mode = config.cycle_mode()
                self._mi.title = config.mode_label()
                print(f"[app] mode → {mode}")
                icons = {"fast": "⚡", "smart": "🧠", "claude": "🤖", "gemini": "✨"}
                self.title = icons.get(mode, "●"); time.sleep(0.8); self.title = "●"

        App().run()
    else:
        def st(i): print(f"[app] {i}")
        core = Core(st); HotkeyListener(core.toggle).start()
        print("[app] type 'm' + Enter to cycle mode, Ctrl+C to quit")
        try:
            while True:
                if input().strip().lower() == "m":
                    mode = config.cycle_mode()
                    print(f"[app] mode → {mode}  ({config.mode_label()})")
        except KeyboardInterrupt: print("\n[app] bye!")
APPEOF

# ── main.py ───────────────────────────────────────────────────────────────────
cat > "$INSTALL_DIR/main.py" << 'MAINEOF'
import sys, os, fcntl, tempfile
sys.stdout.reconfigure(line_buffering=True)
sys.stderr.reconfigure(line_buffering=True)

import config, app

LOCK_FILE = os.path.join(tempfile.gettempdir(), "voice-polish.lock")

def acquire_lock():
    lock = open(LOCK_FILE, "w")
    try:
        import fcntl as _fcntl
        _fcntl.flock(lock, _fcntl.LOCK_EX | _fcntl.LOCK_NB)
    except OSError:
        print("[voice] already running — only one instance allowed. Exiting.")
        sys.exit(0)
    lock.write(str(os.getpid())); lock.flush()
    return lock

def main():
    _lock = acquire_lock()
    config.validate()
    mode_map = {"fast": "gpt-5.4-mini", "smart": "gpt-5.4",
                "claude": "claude-sonnet-4-6", "gemini": "gemini-3.1-flash-lite-preview"}
    mode = config.get_mode()
    print("🎤  Voice Polish")
    print("─────────────────────────────────────────────")
    print(f"  gateway  : {config.GATEWAY_BASE}")
    print(f"  mode     : {mode}  →  {mode_map.get(mode, mode)}")
    print(f"  hotkey   : fn + Shift  (start / stop recording)")
    print(f"  menu bar : click ● to cycle mode")
    print("─────────────────────────────────────────────")
    app.run()

if __name__ == "__main__":
    main()
MAINEOF

ok "All source files written"

# ── 5. API key ────────────────────────────────────────────────────────────────
step "5/9" "API key"
cat > "$INSTALL_DIR/.env" << ENVEOF
GATEWAY_API_KEY=${GATEWAY_KEY}
ENVEOF
ok ".env written"

# ── 6. Virtual environment ────────────────────────────────────────────────────
step "6/9" "Python virtual environment"
if [ -d "$INSTALL_DIR/venv" ] && [ -f "$INSTALL_DIR/venv/bin/python" ]; then
    skip "venv already exists"
else
    info "Creating venv …"
    $PYTHON -m venv "$INSTALL_DIR/venv" || fail "venv creation failed"
    ok "venv created"
fi
source "$INSTALL_DIR/venv/bin/activate"

# ── 7. Python packages ────────────────────────────────────────────────────────
step "7/9" "Python packages"
info "Installing / updating packages …"
pip install -q --upgrade pip
pip install -q -r "$INSTALL_DIR/requirements.txt" \
    && ok "All packages installed" \
    || fail "pip install failed — check your internet connection"

# ── 8. Auto-start on login ────────────────────────────────────────────────────
step "8/9" "Auto-start on login"
PYTHON_BIN="$INSTALL_DIR/venv/bin/python"
mkdir -p "$HOME/Library/LaunchAgents"

cat > "$PLIST_PATH" << PLEOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>${PLIST_NAME}</string>
  <key>ProgramArguments</key>
  <array><string>${PYTHON_BIN}</string><string>${INSTALL_DIR}/main.py</string></array>
  <key>WorkingDirectory</key><string>${INSTALL_DIR}</string>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>${LOG_OUT}</string>
  <key>StandardErrorPath</key><string>${LOG_ERR}</string>
</dict></plist>
PLEOF

launchctl bootout "gui/$(id -u)/$PLIST_NAME" 2>/dev/null || \
launchctl unload  "$PLIST_PATH"              2>/dev/null || true

launchctl bootstrap "gui/$(id -u)" "$PLIST_PATH" 2>/dev/null || \
launchctl load      "$PLIST_PATH"                 2>/dev/null || \
fail "Could not register auto-start"

ok "Auto-start registered"

# ── 9. vp command ─────────────────────────────────────────────────────────────
step "9/9" "vp command"
mkdir -p "$HOME/bin"

cat > "$HOME/bin/vp" << VPEOF
#!/bin/bash
INSTALL_DIR="\$HOME/VoicePolish"
PLIST_NAME="com.voicepolish.app"
PLIST_PATH="\$HOME/Library/LaunchAgents/\$PLIST_NAME.plist"
INSTALL_URL="${INSTALL_URL}"
LOG_OUT="/tmp/voice-polish.log"
LOG_ERR="/tmp/voice-polish.err"

case "\${1:-}" in
  start|"")
    if pgrep -f "VoicePolish/main.py" &>/dev/null; then
      echo "✅  Already running — look for ● in menu bar"
    else
      launchctl bootstrap "gui/\$(id -u)" "\$PLIST_PATH" 2>/dev/null || \
        "\$INSTALL_DIR/venv/bin/python" "\$INSTALL_DIR/main.py" >> "\$LOG_OUT" 2>> "\$LOG_ERR" &
      echo "✅  Voice Polish started — look for ● in menu bar"
    fi
    ;;
  stop)
    launchctl bootout "gui/\$(id -u)/\$PLIST_NAME" 2>/dev/null || true
    pkill -f "VoicePolish/main.py" 2>/dev/null || true
    echo "⏹   Voice Polish stopped"
    ;;
  update)
    echo "→  Fetching latest version…"
    curl -fsSL "\$INSTALL_URL" | bash
    ;;
  status)
    if pgrep -f "VoicePolish/main.py" &>/dev/null; then
      echo "● Running"
    else
      echo "○ Stopped"
    fi
    ;;
  logs)
    tail -f "\$LOG_OUT"
    ;;
  delete)
    echo "→  Removing Voice Polish completely…"
    pkill -f "VoicePolish/main.py" 2>/dev/null || true
    launchctl bootout "gui/\$(id -u)/\$PLIST_NAME" 2>/dev/null || true
    rm -f "\$PLIST_PATH"
    rm -rf "\$INSTALL_DIR"
    rm -f "\$HOME/bin/vp"
    echo "✓  Done."
    echo "   To reinstall: curl -fsSL \$INSTALL_URL | bash"
    ;;
  *)
    echo ""
    echo "  Voice Polish"
    echo ""
    echo "  vp              start"
    echo "  vp stop         stop"
    echo "  vp update       get latest version"
    echo "  vp status       check if running"
    echo "  vp logs         live logs"
    echo "  vp delete       remove everything"
    echo ""
    ;;
esac
VPEOF

chmod +x "$HOME/bin/vp"

# Add ~/bin to PATH in shell profiles (idempotent)
for PROFILE in "$HOME/.zshrc" "$HOME/.bash_profile"; do
    if [ -f "$PROFILE" ] && ! grep -q 'PATH="$HOME/bin' "$PROFILE" 2>/dev/null; then
        echo 'export PATH="$HOME/bin:$PATH"' >> "$PROFILE"
    fi
done
# Also ensure it's available right now
export PATH="$HOME/bin:$PATH"

ok "vp command installed to ~/bin/vp"

# ── Accessibility permission ──────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}${BOLD}⚠️  One manual step — takes 10 seconds:${NC}"
echo ""
echo -e "  System Settings is opening now."
echo -e "  Find ${BOLD}Terminal${NC} in the list → toggle it ${BOLD}ON${NC}"
echo -e "  (This lets the app paste text at your cursor)"
echo ""
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"

# ── Launch app now ────────────────────────────────────────────────────────────
sleep 2
if pgrep -f "VoicePolish/main.py" &>/dev/null; then
    info "App is already running"
else
    info "Launching app …"
    "$PYTHON_BIN" "$INSTALL_DIR/main.py" >> "$LOG_OUT" 2>> "$LOG_ERR" &
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "══════════════════════════════════════════════"
echo -e "${GREEN}${BOLD}✅  Voice Polish installed and running!${NC}"
echo ""
echo -e "  ${BOLD}HOTKEY${NC}"
echo -e "    Hold Shift → tap fn   start recording"
echo -e "    Hold Shift → tap fn   stop & paste"
echo ""
echo -e "  ${BOLD}MENU BAR${NC}"
echo -e "    Look for ${BOLD}●${NC} — click to cycle:  ⚡ fast → 🧠 smart → 🤖 claude → ✨ gemini"
echo ""
echo -e "  ${BOLD}COMMANDS  (open a new Terminal tab first)${NC}"
echo -e "    vp              start"
echo -e "    vp stop         stop"
echo -e "    vp update       get latest version"
echo -e "    vp status       check if running"
echo -e "    vp logs         live logs"
echo -e "    vp delete       remove everything"
echo ""
echo -e "  ${BOLD}INSTALL URL  (share this to set up on a new Mac)${NC}"
echo -e "    curl -fsSL ${INSTALL_URL} | bash"
echo ""
echo "══════════════════════════════════════════════"
