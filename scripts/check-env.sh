#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

missing=0

check_cmd() {
    local name="$1"
    if command -v "$name" >/dev/null 2>&1; then
        printf '[ok] %s: %s\n' "$name" "$(command -v "$name")"
    else
        printf '[missing] %s\n' "$name"
        missing=1
    fi
}

check_cmd cargo
check_cmd rustc
check_cmd make
check_cmd pkexec

if command -v dpkg-deb >/dev/null 2>&1; then
    printf '[ok] dpkg-deb: %s\n' "$(command -v dpkg-deb)"
else
    printf '[warn] dpkg-deb not found; .deb packaging will be unavailable\n'
fi

if command -v zenity >/dev/null 2>&1 || command -v kdialog >/dev/null 2>&1; then
    printf '[ok] color picker: '
    command -v zenity 2>/dev/null || command -v kdialog
else
    printf '[missing] color picker: install zenity or kdialog\n'
    missing=1
fi

kernel_build="/lib/modules/$(uname -r)/build"
if [[ -d "$kernel_build" ]]; then
    printf '[ok] kernel headers: %s\n' "$kernel_build"
else
    printf '[missing] kernel headers: %s\n' "$kernel_build"
    missing=1
fi

if [[ -e /proc/clevo_kbd_led ]]; then
    if [[ -w /proc/clevo_kbd_led ]]; then
        printf '[ok] /proc/clevo_kbd_led is writable\n'
    else
        printf '[warn] /proc/clevo_kbd_led exists but is not writable by this user\n'
    fi
else
    printf '[warn] /proc/clevo_kbd_led not found; load module/clevo_kbd_led.ko first\n'
fi

if [[ -e /proc/clevo_dchu_status ]]; then
    if [[ -r /proc/clevo_dchu_status ]]; then
        printf '[ok] /proc/clevo_dchu_status is readable\n'
    else
        printf '[warn] /proc/clevo_dchu_status exists but is not readable by this user\n'
    fi
else
    printf '[warn] /proc/clevo_dchu_status not found; rebuild and reload module/clevo_kbd_led.ko\n'
fi

if [[ "$missing" -ne 0 ]]; then
    exit 1
fi
