#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

app_id="clevo-keyboard-led"
uid="$(id -u 2>/dev/null || echo unknown)"
runtime_base="${XDG_RUNTIME_DIR:-/tmp/${app_id}-${uid}}"
runtime_dir="$runtime_base/$app_id"
pid_file="$runtime_dir/clevo-keyboard-led.pid"
lock_file="$runtime_dir/clevo-keyboard-led.lock"

if [[ ! -f "$pid_file" ]]; then
    echo 'service pid file not found'
    exit 0
fi

pid="$(tr -d '[:space:]' < "$pid_file")"
if [[ -z "$pid" ]]; then
    rm -f "$pid_file" "$lock_file"
    exit 0
fi

if [[ -d "/proc/$pid" ]]; then
    kill "$pid"
    echo "stopped service pid $pid"
else
    echo "service pid $pid is not running"
fi

rm -f "$pid_file" "$lock_file"
