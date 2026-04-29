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

DEFAULT_GATEWAY_KEY="cnsc_gw_23450226f2fdcaa1f661284ae8d54c12acae140c51c24fc7"
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

# ── 3. API key ──────────────────────────────────────────────────────────────
step "3/5" "API key"

EXISTING_KEY=""
[ -f "$INSTALL_DIR/.env" ] && EXISTING_KEY=$(grep "^GATEWAY_API_KEY=" "$INSTALL_DIR/.env" 2>/dev/null | cut -d= -f2-)

if [ -n "$EXISTING_KEY" ] && [ "$EXISTING_KEY" != "$DEFAULT_GATEWAY_KEY" ]; then
    skip "Custom API key already in .env"
    GATEWAY_KEY="$EXISTING_KEY"
else
    echo ""
    echo -e "  ${BOLD}Enter your Gateway API key${NC} (press Enter to use the default shared key):"
    echo -e "  ${CYAN}[leave blank for the shared key]${NC}"
    echo -n "  Key: "
    read -r USER_KEY
    if [ -n "$USER_KEY" ]; then
        GATEWAY_KEY="$USER_KEY"
        ok "Using your custom API key"
    else
        GATEWAY_KEY="$DEFAULT_GATEWAY_KEY"
        note "Using default shared key"
    fi
    printf 'GATEWAY_API_KEY=%s\n' "$GATEWAY_KEY" > "$INSTALL_DIR/.env"
    ok ".env written"
fi

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

case "${1:-}" in
  start|"")
    if pgrep -f "VoicePolish.app/Contents/MacOS" &>/dev/null; then
      echo "✅  Already running — look for ● in menu bar"
    else
      : > "$LOG_ERR"
      open -g -a "$APP_BUNDLE" 2>/dev/null || \
        "$APP_EXEC" >> "$LOG_OUT" 2>> "$LOG_ERR" &
      sleep 2
      if pgrep -f "VoicePolish.app/Contents/MacOS" &>/dev/null; then
        echo "✅  Voice Polish started — look for ● in menu bar"
      else
        echo "❌  Failed to start. Run: vp errors"
      fi
    fi
    ;;
  stop)
    launchctl bootout "gui/$(id -u)/$PLIST_NAME" 2>/dev/null || true
    pkill -f "VoicePolish.app/Contents/MacOS"    2>/dev/null || true
    echo "⏹   Voice Polish stopped"
    ;;
  update)
    echo "→  Fetching latest version…"
    curl -fsSL "$INSTALL_URL" | bash
    ;;
  status)
    if pgrep -f "VoicePolish.app/Contents/MacOS" &>/dev/null; then
      echo "● Running (pid $(pgrep -f 'VoicePolish.app/Contents/MacOS'))"
    else
      echo "○ Stopped"
    fi
    ;;
  logs)
    tail -f "$LOG_OUT"
    ;;
  errors)
    if [ -s "$LOG_ERR" ]; then tail -30 "$LOG_ERR"; else echo "No errors."; fi
    ;;
  permissions)
    echo ""
    echo "Opening System Settings for the two required permissions…"
    echo ""
    # Input Monitoring — for the fn+Shift hotkey
    open "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
    sleep 1
    # Accessibility — for paste
    open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
    echo "  1. Find 'VoicePolish' in each list and toggle it ON"
    echo "  2. Run 'vp' to start"
    ;;
  delete)
    echo "→  Removing Voice Polish completely…"
    pkill -f "VoicePolish.app/Contents/MacOS"    2>/dev/null || true
    launchctl bootout "gui/$(id -u)/$PLIST_NAME" 2>/dev/null || true
    rm -f "$PLIST_PATH"
    rm -rf "$INSTALL_DIR"
    rm -f "$HOME/bin/vp"
    echo "✓  Done. To reinstall: curl -fsSL $INSTALL_URL | bash"
    ;;
  *)
    echo ""
    echo "  Voice Polish"
    echo ""
    echo "  vp                start"
    echo "  vp stop           stop"
    echo "  vp status         check if running"
    echo "  vp logs           live output log"
    echo "  vp errors         show recent errors"
    echo "  vp permissions    open System Settings for mic + accessibility"
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

# ── Launch ───────────────────────────────────────────────────────────────────
echo ""
info "Starting Voice Polish …"
> "$LOG_ERR"
open -g -a "$APP_BUNDLE" 2>/dev/null || "$APP_EXEC" >> "$LOG_OUT" 2>> "$LOG_ERR" &
sleep 3

if pgrep -f "VoicePolish.app/Contents/MacOS" &>/dev/null; then
    ok "App running — look for ● in your menu bar"
else
    echo -e "  ${YELLOW}⚠  App may not have started. Run: vp errors${NC}"
fi

# ── Permission instructions ───────────────────────────────────────────────────
echo ""
echo "══════════════════════════════════════════════"
echo -e "${YELLOW}${BOLD}⚠️  2 permissions required — takes 1 minute${NC}"
echo "══════════════════════════════════════════════"
echo ""
echo -e "  Run this to open both settings pages automatically:"
echo -e "  ${CYAN}${BOLD}vp permissions${NC}"
echo ""
echo -e "  Or open manually:"
echo ""
echo -e "  ${BOLD}1. Input Monitoring${NC}  (for the fn+Shift hotkey)"
echo -e "     System Settings → Privacy & Security → Input Monitoring"
echo -e "     Find ${BOLD}VoicePolish${NC} → toggle ${BOLD}ON${NC}"
echo ""
echo -e "  ${BOLD}2. Accessibility${NC}  (to paste text at your cursor)"
echo -e "     System Settings → Privacy & Security → Accessibility"
echo -e "     Find ${BOLD}VoicePolish${NC} → toggle ${BOLD}ON${NC}"
echo ""
echo -e "  ${BOLD}3. Microphone${NC}  (auto-prompted on first recording)"
echo -e "     Just say Allow when the popup appears."
echo ""
echo "══════════════════════════════════════════════"
echo -e "${GREEN}${BOLD}✅  Done!${NC}"
echo ""
echo -e "  ${BOLD}Hotkey:${NC}  Hold Shift → tap fn  (start / stop recording)"
echo -e "  ${BOLD}Manage:${NC}  type ${CYAN}vp${NC} in Terminal for all commands"
echo ""
