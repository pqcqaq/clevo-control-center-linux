#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

pid_file="$ROOT_DIR/clevo-keyboard-led.pid"
if [[ ! -f "$pid_file" ]]; then
    echo 'service pid file not found'
    exit 0
fi

pid="$(tr -d '[:space:]' < "$pid_file")"
if [[ -z "$pid" ]]; then
    rm -f "$pid_file"
    exit 0
fi

if [[ -d "/proc/$pid" ]]; then
    kill "$pid"
    echo "stopped service pid $pid"
else
    echo "service pid $pid is not running"
fi

rm -f "$pid_file"
