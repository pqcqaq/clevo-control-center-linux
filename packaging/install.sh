#!/usr/bin/env bash
set -euo pipefail

APP_ID="clevo-control-center"
PREFIX="${PREFIX:-$HOME/.local}"
LIB_DIR="${LIB_DIR:-$PREFIX/lib/$APP_ID}"
BIN_DIR="${BIN_DIR:-$PREFIX/bin}"
DESKTOP_DIR="${DESKTOP_DIR:-$PREFIX/share/applications}"
MODE="${1:-install}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -d "$SCRIPT_DIR/bin" && -d "$SCRIPT_DIR/module" ]]; then
    ROOT_DIR="$SCRIPT_DIR"
else
    ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
fi

install_app() {
    mkdir -p "$LIB_DIR" "$BIN_DIR" "$DESKTOP_DIR"
    install -m 0755 "$ROOT_DIR/bin/$APP_ID" "$LIB_DIR/$APP_ID"
    rm -rf "$LIB_DIR/module"
    cp -R "$ROOT_DIR/module" "$LIB_DIR/module"

    cat > "$BIN_DIR/$APP_ID" <<EOF
#!/usr/bin/env bash
cd "$LIB_DIR"
exec "$LIB_DIR/$APP_ID" "\$@"
EOF
    chmod 0755 "$BIN_DIR/$APP_ID"

    rm -f "$BIN_DIR/clevo-keyboard-led" "$DESKTOP_DIR/clevo-keyboard-led.desktop"
    sed "s#^Exec=.*#Exec=$BIN_DIR/$APP_ID#" "$ROOT_DIR/app/$APP_ID.desktop" > "$DESKTOP_DIR/$APP_ID.desktop"
    chmod 0644 "$DESKTOP_DIR/$APP_ID.desktop"
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
    fi

    install_module || true
    echo "installed $APP_ID"
}

install_module() {
    if ! command -v make >/dev/null 2>&1; then
        echo "skip kernel module: make not found"
        return 0
    fi

    local kernel_build="/lib/modules/$(uname -r)/build"
    if [[ ! -d "$kernel_build" ]]; then
        echo "skip kernel module: missing headers at $kernel_build"
        return 0
    fi

    make -C "$LIB_DIR/module"
    if command -v pkexec >/dev/null 2>&1 && [[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]]; then
        pkexec install -m 0644 "$LIB_DIR/module/clevo_kbd_led.ko" "/lib/modules/$(uname -r)/extra/clevo_kbd_led.ko"
        pkexec depmod -a
        pkexec modprobe clevo_kbd_led
    elif [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
        install -m 0644 "$LIB_DIR/module/clevo_kbd_led.ko" "/lib/modules/$(uname -r)/extra/clevo_kbd_led.ko"
        depmod -a
        modprobe clevo_kbd_led
    else
        echo "kernel module built; run sudo install manually if pkexec is unavailable"
    fi
}

uninstall_app() {
    rm -f "$BIN_DIR/$APP_ID" "$BIN_DIR/clevo-keyboard-led" "$DESKTOP_DIR/$APP_ID.desktop" "$DESKTOP_DIR/clevo-keyboard-led.desktop"
    rm -rf "$LIB_DIR"
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
    fi
    echo "uninstalled $APP_ID"
}

case "$MODE" in
    install) install_app ;;
    module) install_module ;;
    uninstall) uninstall_app ;;
    *) echo "usage: $0 [install|module|uninstall]" >&2; exit 2 ;;
esac
