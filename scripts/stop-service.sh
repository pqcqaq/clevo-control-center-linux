#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

uid="$(id -u 2>/dev/null || echo unknown)"

stop_app() {
    local app_id="$1"
    local pid_name="$2"
    local lock_name="$3"
    local runtime_base="${XDG_RUNTIME_DIR:-/tmp/${app_id}-${uid}}"
    local runtime_dir="$runtime_base/$app_id"
    local pid_file="$runtime_dir/$pid_name"
    local lock_file="$runtime_dir/$lock_name"

    if [[ ! -f "$pid_file" ]]; then
        return 1
    fi

    local pid
    pid="$(tr -d '[:space:]' < "$pid_file")"
    if [[ -z "$pid" ]]; then
        rm -f "$pid_file" "$lock_file"
        return 0
    fi

    if [[ -d "/proc/$pid" ]]; then
        kill "$pid"
        echo "stopped $app_id service pid $pid"
    else
        echo "$app_id service pid $pid is not running"
    fi

    rm -f "$pid_file" "$lock_file"
    return 0
}

stopped=0
stop_app "clevo-control-center" "clevo-control-center.pid" "clevo-control-center.lock" && stopped=1
stop_app "clevo-keyboard-led" "clevo-keyboard-led.pid" "clevo-keyboard-led.lock" && stopped=1

if [[ "$stopped" -eq 0 ]]; then
    echo 'service pid file not found'
fi
