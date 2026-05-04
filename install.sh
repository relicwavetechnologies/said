#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
#  Voice Polish — Installer
#
#  Install (first time or update):
#    curl -fsSL https://raw.githubusercontent.com/relicwavetechnologies/said/main/install.sh | bash
#
#  After install, manage with:
#    vp              → start
#    vp stop         → stop
#    vp update       → get latest version
#    vp status       → check if running
#    vp logs         → live logs
#    vp errors       → show recent errors
#    vp delete       → remove everything
# ══════════════════════════════════════════════════════════════════════════════

INSTALL_URL="https://raw.githubusercontent.com/relicwavetechnologies/said/main/install.sh"
REPO="relicwavetechnologies/said"

INSTALL_DIR="$HOME/VoicePolish"
APP_BUNDLE="$INSTALL_DIR/VoicePolish.app"
APP_EXEC="$APP_BUNDLE/Contents/MacOS/VoicePolish"
PLIST_NAME="com.voicepolish.app"
PLIST_PATH="$HOME/Library/LaunchAgents/$PLIST_NAME.plist"
LOG_OUT="/tmp/voice-polish.log"
LOG_ERR="/tmp/voice-polish.err"

# ─────────────────────────────────────────────────────────────────────────────
BOLD='\033[1m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'

ok()   { echo -e "  ${GREEN}✓ $1${NC}"; }
skip() { echo -e "  ${GREEN}✓ $1 — already done, skipping${NC}"; }
info() { echo -e "  ${YELLOW}→ $1${NC}"; }
fail() { echo -e "\n  ${RED}✗ ERROR: $1${NC}\n"; exit 1; }
step() { echo -e "\n${BOLD}[$1]${NC} $2"; }
note() { echo -e "  ${CYAN}ℹ $1${NC}"; }

echo ""
echo -e "${BOLD}🎤  Voice Polish — Setup${NC}"
echo "══════════════════════════════════════════════"

# ── 1. Stop any running instance ─────────────────────────────────────────────
step "1/5" "Stopping any running instance"
pkill -f "VoicePolish/voice-polish"            2>/dev/null || true
pkill -f "VoicePolish.app/Contents/MacOS"      2>/dev/null || true
launchctl bootout "gui/$(id -u)/$PLIST_NAME"   2>/dev/null || true
sleep 1
ok "Ready"

# ── 2. Download binary ──────────────────────────────────────────────────────
step "2/5" "Downloading Voice Polish"
mkdir -p "$INSTALL_DIR"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

ARCH=$(uname -m)
case "$ARCH" in
    arm64|aarch64) ASSET_NAME="voice-polish-aarch64-apple-darwin" ;;
    x86_64)        ASSET_NAME="voice-polish-x86_64-apple-darwin"  ;;
    *)             fail "Unsupported architecture: $ARCH" ;;
esac

info "Downloading latest release for $ARCH …"

TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d'"' -f4)

[ -z "$TAG" ] && fail "Could not find latest release — check https://github.com/$REPO/releases"

DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME"
curl -fsSL -o "$APP_EXEC" "$DOWNLOAD_URL" \
    || fail "Download failed — check https://github.com/$REPO/releases"
chmod +x "$APP_EXEC"

# Remove the old standalone binary if it exists (no longer needed)
rm -f "$INSTALL_DIR/voice-polish"

ok "Binary downloaded $(du -h "$APP_EXEC" | cut -f1 | xargs) — tag $TAG"

# ── 3. Standalone config ────────────────────────────────────────────────────
step "3/5" "Standalone config"
note "This standalone build stores its own Deepgram key + OpenAI OAuth token locally"
note "No shared app DB and no gateway API key are used anymore"
ok "Config flow updated"

# ── 4. .app bundle ──────────────────────────────────────────────────────────
step "4/5" "Configuring .app bundle"

cat > "$APP_BUNDLE/Contents/Info.plist" << 'INFOPLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>com.voicepolish.app</string>
  <key>CFBundleName</key>
  <string>Voice Polish</string>
  <key>CFBundleDisplayName</key>
  <string>Voice Polish</string>
  <key>CFBundleExecutable</key>
  <string>VoicePolish</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSMicrophoneUsageDescription</key>
  <string>Voice Polish needs microphone access to record and transcribe your voice.</string>
  <key>NSAccessibilityUsageDescription</key>
  <string>Voice Polish needs Accessibility access to paste transcribed text at your cursor.</string>
  <key>NSInputMonitoringUsageDescription</key>
  <string>Voice Polish needs Input Monitoring access to detect the fn+Shift hotkey.</string>
</dict>
</plist>
INFOPLIST

# Clear quarantine flag so macOS doesn't block the unsigned binary
xattr -cr "$APP_BUNDLE" 2>/dev/null || true

# Ad-hoc code-sign the bundle.
# Without a signature, TCC (Privacy permissions) tracks the binary by its hash.
# That means every "vp update" changes the hash and macOS silently revokes
# Input Monitoring + Accessibility — making the app appear broken after updates.
# An ad-hoc signature (-) makes TCC track by bundle ID (com.voicepolish.app)
# so permissions survive future updates.
if command -v codesign &>/dev/null; then
    codesign --force --deep --sign - "$APP_BUNDLE" 2>/dev/null && \
        ok "Bundle signed (ad-hoc) — permissions will survive future updates" || \
        note "codesign failed (non-fatal) — permissions may need re-granting after updates"
else
    note "codesign not found — install Xcode CLI tools to avoid re-granting permissions after updates"
fi

# Register the bundle with Launch Services so it gets a proper icon in System Settings
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
    -f "$APP_BUNDLE" 2>/dev/null || true

ok ".app bundle ready"

# ── 5. vp command + LaunchAgent ─────────────────────────────────────────────
step "5/5" "Installing vp command + auto-start"

mkdir -p "$HOME/Library/LaunchAgents"
cat > "$PLIST_PATH" << PLEOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>${PLIST_NAME}</string>
  <key>ProgramArguments</key>
  <array><string>${APP_EXEC}</string></array>
  <key>WorkingDirectory</key><string>${INSTALL_DIR}</string>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><false/>
  <key>StandardOutPath</key><string>${LOG_OUT}</string>
  <key>StandardErrorPath</key><string>${LOG_ERR}</string>
</dict></plist>
PLEOF

launchctl bootstrap "gui/$(id -u)" "$PLIST_PATH" 2>/dev/null || \
launchctl load      "$PLIST_PATH"                 2>/dev/null || true
ok "Auto-start at login registered"

mkdir -p "$HOME/bin"
cat > "$HOME/bin/vp" << 'VPEOF'
#!/bin/bash
INSTALL_DIR="$HOME/VoicePolish"
APP_BUNDLE="$INSTALL_DIR/VoicePolish.app"
APP_EXEC="$APP_BUNDLE/Contents/MacOS/VoicePolish"
PLIST_NAME="com.voicepolish.app"
PLIST_PATH="$HOME/Library/LaunchAgents/$PLIST_NAME.plist"
INSTALL_URL="https://raw.githubusercontent.com/relicwavetechnologies/said/main/install.sh"
LOG_OUT="/tmp/voice-polish.log"
LOG_ERR="/tmp/voice-polish.err"

_launch() {
  # Always start via LaunchAgent so stdout/stderr go to the log files.
  # open -a bypasses the LaunchAgent plist and logs nothing — never use it.
  : > "$LOG_OUT"
  : > "$LOG_ERR"
  launchctl bootout "gui/$(id -u)/$PLIST_NAME" 2>/dev/null || true
  pkill -f "VoicePolish.app/Contents/MacOS" 2>/dev/null || true
  sleep 0.5
  launchctl bootstrap "gui/$(id -u)" "$PLIST_PATH" 2>/dev/null || \
    launchctl load "$PLIST_PATH" 2>/dev/null || true
  sleep 2
}

case "${1:-}" in
  start|"")
    if pgrep -f "VoicePolish.app/Contents/MacOS" &>/dev/null; then
      echo "✅  Already running — look for ● in menu bar"
      echo "   (run 'vp stop && vp' to restart)"
    else
      echo "→  Starting…"
      _launch
      if pgrep -f "VoicePolish.app/Contents/MacOS" &>/dev/null; then
        echo "✅  Voice Polish started — look for ● in menu bar"
      else
        echo "❌  Failed to start. Errors:"
        echo "──────────────────────────────"
        cat "$LOG_ERR" 2>/dev/null || echo "(no error log)"
        echo "──────────────────────────────"
      fi
    fi
    ;;
  stop)
    launchctl bootout "gui/$(id -u)/$PLIST_NAME" 2>/dev/null || true
    pkill -f "VoicePolish.app/Contents/MacOS" 2>/dev/null || true
    rm -f /tmp/voice-polish.lock
    echo "⏹   Stopped"
    ;;
  restart)
    echo "→  Restarting…"
    _launch
    if pgrep -f "VoicePolish.app/Contents/MacOS" &>/dev/null; then
      echo "✅  Restarted — look for ● in menu bar"
    else
      echo "❌  Failed. Run: vp doctor"
    fi
    ;;
  update)
    echo "→  Fetching latest version…"
    curl -fsSL "$INSTALL_URL" | bash
    ;;
  status)
    if pgrep -f "VoicePolish.app/Contents/MacOS" &>/dev/null; then
      echo "● Running  (pid $(pgrep -f 'VoicePolish.app/Contents/MacOS' | head -1))"
    else
      echo "○ Stopped"
    fi
    if [ -x "$APP_EXEC" ]; then
      echo ""
      "$APP_EXEC" status
    fi
    ;;
  auth)
    "$APP_EXEC" auth
    ;;
  deepgram-key)
    if [ -n "${2:-}" ]; then
      "$APP_EXEC" deepgram-key "$2"
    else
      "$APP_EXEC" deepgram-key
    fi
    ;;
  disconnect-openai)
    "$APP_EXEC" disconnect-openai
    ;;
  logs)
    echo "── stdout (/tmp/voice-polish.log) ──"
    tail -40 "$LOG_OUT" 2>/dev/null || echo "(empty)"
    ;;
  errors)
    echo "── stderr (/tmp/voice-polish.err) ──"
    if [ -s "$LOG_ERR" ]; then
      cat "$LOG_ERR"
    else
      echo "(no errors — good!)"
    fi
    ;;
  doctor)
    echo ""
    echo "🩺  Voice Polish — diagnostics"
    echo "──────────────────────────────────────────"
    # Running?
    if pgrep -f "VoicePolish.app/Contents/MacOS" &>/dev/null; then
      echo "  Process   : ✅ running (pid $(pgrep -f 'VoicePolish.app/Contents/MacOS' | head -1))"
    else
      echo "  Process   : ❌ NOT running  →  run: vp"
    fi
    # Binary present?
    if [ -x "$APP_EXEC" ]; then
      echo "  Binary    : ✅ $APP_EXEC"
    else
      echo "  Binary    : ❌ not found  →  run: vp update"
    fi
    # LaunchAgent?
    if [ -f "$PLIST_PATH" ]; then
      echo "  LaunchAgent: ✅ registered"
    else
      echo "  LaunchAgent: ❌ missing  →  run: vp update"
    fi
    echo ""
    echo "  Recent errors:"
    echo "  ──────────────"
    if [ -s "$LOG_ERR" ]; then
      sed 's/^/  /' "$LOG_ERR" | tail -20
    else
      echo "  (none)"
    fi
    echo ""
    echo "  Recent output:"
    echo "  ──────────────"
    grep -E "hotkey|paste|startup|preflight" "$LOG_OUT" 2>/dev/null | tail -10 | sed 's/^/  /' \
      || echo "  (none)"
    echo ""
    echo "  If hotkey or paste says NOT granted, run:"
    echo "    vp stop && vp"
    echo "  Then grant permissions in System Settings and run:"
    echo "    vp restart"
    echo ""
    ;;
  permissions)
    echo ""
    echo "  Binary to add in BOTH permission pages:"
    echo "  $APP_EXEC"
    echo ""
    echo "  In each page: click + → press Cmd+Shift+G → paste the path above"
    echo "  → select VoicePolish → Open → toggle ON"
    echo ""
    open "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
    sleep 1
    open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
    echo "  Both System Settings pages are now open."
    echo "  After granting both, run:  vp restart"
    echo ""
    ;;
  delete)
    echo "→  Removing Voice Polish completely…"
    pkill -f "VoicePolish.app/Contents/MacOS" 2>/dev/null || true
    launchctl bootout "gui/$(id -u)/$PLIST_NAME" 2>/dev/null || true
    rm -f "$PLIST_PATH"
    rm -rf "$INSTALL_DIR"
    rm -f "$HOME/bin/vp"
    rm -f /tmp/voice-polish.lock /tmp/voice-polish.log /tmp/voice-polish.err
    echo "✓  Done. To reinstall: curl -fsSL $INSTALL_URL | bash"
    ;;
  *)
    echo ""
    echo "  vp                start"
    echo "  vp stop           stop"
    echo "  vp restart        stop + start (use after granting permissions)"
    echo "  vp status         is it running?"
    echo "  vp auth           connect ChatGPT OAuth for this standalone app"
    echo "  vp deepgram-key   save your Deepgram API key"
    echo "  vp disconnect-openai  clear the saved OpenAI token"
    echo "  vp logs           recent output"
    echo "  vp errors         recent errors"
    echo "  vp doctor         full diagnostics"
    echo "  vp permissions    open System Settings + show exact binary path"
    echo "  vp update         download latest version"
    echo "  vp delete         remove everything"
    echo ""
    ;;
esac
VPEOF
chmod +x "$HOME/bin/vp"
export PATH="$HOME/bin:$PATH"

for PROFILE in "$HOME/.zshrc" "$HOME/.bash_profile"; do
    if [ -f "$PROFILE" ] && ! grep -q 'HOME/bin' "$PROFILE" 2>/dev/null; then
        echo 'export PATH="$HOME/bin:$PATH"' >> "$PROFILE"
    fi
done
ok "vp command installed"

# ── Permission instructions ───────────────────────────────────────────────────
echo ""
echo "══════════════════════════════════════════════"
echo -e "${YELLOW}${BOLD}⚠️  Setup required before first run${NC}"
echo "══════════════════════════════════════════════"
echo ""
echo -e "  ${BOLD}1.${NC} Save your Deepgram key:"
echo -e "     ${CYAN}${BOLD}vp deepgram-key${NC}"
echo ""
echo -e "  ${BOLD}2.${NC} Connect ChatGPT OAuth for this standalone app:"
echo -e "     ${CYAN}${BOLD}vp auth${NC}"
echo ""
echo -e "  ${BOLD}3.${NC} Open the required macOS permission panes:"
echo -e "     ${CYAN}${BOLD}vp permissions${NC}"
echo ""
echo -e "  ${BOLD}4.${NC} Start Voice Polish:"
echo -e "     ${CYAN}${BOLD}vp${NC}"
echo ""
echo -e "  Permissions needed:"
echo ""
echo -e "  ${BOLD}• Input Monitoring${NC}  (for Caps Lock hold-to-record)"
echo -e "     System Settings → Privacy & Security → Input Monitoring"
echo -e "     Find ${BOLD}VoicePolish${NC} → toggle ${BOLD}ON${NC}"
echo ""
echo -e "  ${BOLD}• Accessibility${NC}  (to paste text at your cursor)"
echo -e "     System Settings → Privacy & Security → Accessibility"
echo -e "     Find ${BOLD}VoicePolish${NC} → toggle ${BOLD}ON${NC}"
echo ""
echo -e "  ${BOLD}• Microphone${NC}  (auto-prompted on first recording)"
echo -e "     Just say Allow when the popup appears."
echo ""
echo "══════════════════════════════════════════════"
echo -e "${GREEN}${BOLD}✅  Done!${NC}"
echo ""
echo -e "  ${BOLD}Hotkey:${NC}  Hold Caps Lock to record, release to polish"
echo -e "  ${BOLD}Manage:${NC}  type ${CYAN}vp${NC} in Terminal for all commands"
echo ""
