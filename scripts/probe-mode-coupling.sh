#!/usr/bin/env bash
set -u

CONFIG_PATH="${CLEVO_DCHU_CONFIG_PROC:-/proc/clevo_dchu_config}"
DELAY_SECONDS="${CLEVO_MODE_PROBE_DELAY:-0.8}"

if [[ -n "${CLEVO_BIN:-}" ]]; then
  BIN="$CLEVO_BIN"
elif command -v clevo-control-center >/dev/null 2>&1; then
  BIN="$(command -v clevo-control-center)"
elif [[ -x "$HOME/.local/lib/clevo-control-center/clevo-control-center" ]]; then
  BIN="$HOME/.local/lib/clevo-control-center/clevo-control-center"
elif [[ -x "target/release/clevo-control-center" ]]; then
  BIN="target/release/clevo-control-center"
elif [[ -x "target/debug/clevo-control-center" ]]; then
  BIN="target/debug/clevo-control-center"
else
  echo "clevo-control-center binary not found; set CLEVO_BIN=/path/to/clevo-control-center" >&2
  exit 2
fi

read_mode_status() {
  python3 - "$CONFIG_PATH" <<'PY'
import re
import sys

path = sys.argv[1]
try:
    text = open(path, "r", encoding="utf-8", errors="replace").read()
except OSError as exc:
    print(f"ERR:{exc}")
    sys.exit(1)

data = []
for line in text.splitlines():
    line = line.strip()
    if not line or line.startswith("config_0d ") or line.startswith("psf"):
        continue
    for token in line.split():
        if re.fullmatch(r"[0-9a-fA-F]{2}", token):
            data.append(int(token, 16))

if len(data) <= 0x0E:
    print(f"ERR:config buffer too short ({len(data)} bytes)")
    sys.exit(1)

print(f"0x{data[0x0E]:02x}")
PY
}

power_raw_hint() {
  case "$1" in
    0x80) printf "power0" ;;
    0x08) printf "power2/old-fan2" ;;
    0x02) printf "power1-or-3/maxq" ;;
    *) printf "none" ;;
  esac
}

fan_raw_hint() {
  case "$1" in
    0x10) printf "fan-max" ;;
    0x08) printf "power2/old-fan2" ;;
    0x02) printf "power1-or-3/maxq" ;;
    *) printf "none" ;;
  esac
}

run_write() {
  local group="$1"
  local value="$2"
  "$BIN" dchu "${group}-mode" "$value" --i-understand
}

print_row() {
  local baseline="$1"
  local action="$2"
  local before_raw="$3"
  local before_power="$4"
  local before_fan="$5"
  local after_raw="$6"
  local after_power="$7"
  local after_fan="$8"
  local status="$9"
  local power_change="same"
  local fan_change="same"

  if [[ "$before_power" != "$after_power" ]]; then
    power_change="${before_power}->${after_power}"
  fi
  if [[ "$before_fan" != "$after_fan" ]]; then
    fan_change="${before_fan}->${after_fan}"
  fi

  printf "| %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s |\n" \
    "$baseline" "$action" "$before_raw" "$before_power" "$before_fan" \
    "$after_raw" "$after_power" "$after_fan" "$power_change" "$fan_change" "$status"
}

probe_action() {
  local baseline_group="$1"
  local baseline_value="$2"
  local group="$3"
  local value="$4"
  local baseline="${baseline_group}:${baseline_value}"
  local action="${group}:${value}"
  local status="ok"

  if ! run_write "$baseline_group" "$baseline_value" >/dev/null 2>&1; then
    status="baseline-failed"
  fi
  sleep "$DELAY_SECONDS"

  before_raw="$(read_mode_status)"
  before_power="$(power_raw_hint "$before_raw")"
  before_fan="$(fan_raw_hint "$before_raw")"

  if [[ "$status" == "ok" ]]; then
    if ! output="$(run_write "$group" "$value" 2>&1)"; then
      code="$?"
      status="failed(${code})"
    fi
  fi

  sleep "$DELAY_SECONDS"

  after_raw="$(read_mode_status)"
  after_power="$(power_raw_hint "$after_raw")"
  after_fan="$(fan_raw_hint "$after_raw")"
  print_row "$baseline" "$action" "$before_raw" "$before_power" "$before_fan" \
    "$after_raw" "$after_power" "$after_fan" "$status"
}

echo "Mode coupling probe"
echo "Binary: $BIN"
echo "Config: $CONFIG_PATH"
echo "Delay: ${DELAY_SECONDS}s"
echo
echo "Power actions use fan:max as the baseline. Fan actions use power:0 as the baseline."
echo "Raw hints are only labels for config_0d[0x0E]; OEM UI selected state comes from AppSettings, not this byte."
echo
echo "| baseline | action | raw before | power hint before | fan hint before | raw after | power hint after | fan hint after | power hint change | fan hint change | command |"
echo "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"

for value in 0 1 2 3; do
  probe_action fan max power "$value"
done

for value in max silent maxq auto; do
  probe_action power 0 fan "$value"
done

run_write fan auto >/dev/null 2>&1 || true
