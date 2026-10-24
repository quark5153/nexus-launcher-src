#!/usr/bin/env bash
# Install Nexus Launcher for the current user only (no root needed).
# The app then shows up in the KDE/Plasma application menu as "Nexus Launcher".
set -euo pipefail
cd "$(dirname "$0")"

BIN="target/release/nexus-launcher"
PREFIX="${PREFIX:-$HOME/.local}"
APP_ID="com.nexus.sakuralauncher"

[[ -x "$BIN" ]] || { echo "ยังไม่มีไบนารี — รัน ./build-package.sh ก่อน" >&2; exit 1; }

install -Dm755 "$BIN" "$PREFIX/bin/nexus-launcher"
install -Dm644 packaging/nexus-launcher.desktop "$PREFIX/share/applications/nexus-launcher.desktop"

# KDE/Plasma on Wayland matches a window to a desktop entry by the GTK app id
# (Tauri uses the bundle identifier), so ship a hidden alias with that name.
sed -e "s/^StartupWMClass=.*/StartupWMClass=${APP_ID}/" \
    -e "s/^Name=.*/Name=Nexus Launcher/" \
    packaging/nexus-launcher.desktop > /tmp/nexus-alias.desktop
printf 'NoDisplay=true\n' >> /tmp/nexus-alias.desktop
install -Dm644 /tmp/nexus-alias.desktop "$PREFIX/share/applications/${APP_ID}.desktop"
rm -f /tmp/nexus-alias.desktop

# install every hicolor size so panels/menus pick a crisp icon (no upscaling)
for size in 32 48 64 128 256 512; do
  case $size in
    32) src=apps/app/icons/32x32.png ;;
    48) src=apps/app/icons/48x48.png ;;
    64) src=apps/app/icons/64x64.png ;;
    128) src=apps/app/icons/128x128.png ;;
    256) src=apps/app/icons/128x128@2x.png ;;
    512) src=apps/app/icons/icon.png ;;
  esac
  [[ -f "$src" ]] || src=apps/app/icons/icon.png
  install -Dm644 "$src" "$PREFIX/share/icons/hicolor/${size}x${size}/apps/nexus-launcher.png"
done

command -v update-desktop-database >/dev/null && update-desktop-database "$PREFIX/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" || true
# KDE keeps its own icon/menu cache — rebuild it so the new artwork appears
for tool in kbuildsycoca6 kbuildsycoca5; do
  command -v "$tool" >/dev/null && "$tool" --noincremental >/dev/null 2>&1 || true
done

echo "ติดตั้งแล้ว: $PREFIX/bin/nexus-launcher"
echo "เปิดได้จากเมนูแอป (Nexus Launcher) หรือพิมพ์: nexus-launcher"
