#!/bin/sh
# Install WaveGlance for this user (no root needed):
#   program  -> ~/.local/bin/waveglance
#   icons    -> ~/.local/share/icons/hicolor/
#   menu     -> ~/.local/share/applications/waveglance.desktop
# Remove: ./install.sh --remove
set -e
here=$(cd "$(dirname "$0")" && pwd)
bin="$HOME/.local/bin"
share="${XDG_DATA_HOME:-$HOME/.local/share}"
if [ "$1" = "--remove" ]; then
    rm -f "$bin/waveglance" "$share/applications/waveglance.desktop"
    find "$share/icons/hicolor" -path '*/apps/waveglance.*' -delete 2>/dev/null || true
    echo "WaveGlance removed."
    exit 0
fi
mkdir -p "$bin" "$share/applications" "$share/icons"
install -m 755 "$here/waveglance" "$bin/waveglance"
cp -R "$here/icons/hicolor" "$share/icons/"
install -m 644 "$here/waveglance.desktop" "$share/applications/waveglance.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$share/icons/hicolor" || true
echo "WaveGlance installed: $bin/waveglance (and in the applications menu)."
case ":$PATH:" in *":$bin:"*) ;; *) echo "Note: $bin is not in your PATH." ;; esac
