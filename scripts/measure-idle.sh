#!/bin/sh
# AC-19: idle memory and CPU of every HyperMe process (the app and its WebKit helpers).
#
#   scripts/measure-idle.sh [--seconds N] [--after-dictations] [--out FILE] [path/to/hyperme]
#
# Starts the release app, notes which WebKit processes appear with it, optionally waits while
# you dictate (AC-19 asks for 20 dictations first), then samples CPU every 10 s for N seconds
# (default 600, AC-19's 10 minutes) and reports the physical footprint (`footprint`, what
# Activity Monitor shows as Memory) of each process. Numbers only. macOS only.
set -eu

seconds=600
wait_dictations=0
out=""
app="target/release/hyperme"
while [ $# -gt 0 ]; do
  case "$1" in
    --seconds) seconds="$2"; shift 2 ;;
    --after-dictations) wait_dictations=1; shift ;;
    --out) out="$2"; shift 2 ;;
    *) app="$1"; shift ;;
  esac
done
[ -x "$app" ] || { echo "No app at $app: run 'cargo build --release -p hyperme-app' first." >&2; exit 1; }
if pgrep -x hyperme >/dev/null; then
  echo "HyperMe is already running; quit it first so its WebKit processes can be told apart." >&2
  exit 1
fi

webkit() { pgrep -f 'com.apple.WebKit' | sort; }
before=$(mktemp); after=$(mktemp)
webkit > "$before"
HYPERME_NO_WALKTHROUGH=1 "$app" >/dev/null 2>&1 &
app_pid=$!
trap 'kill "$app_pid" 2>/dev/null; rm -f "$before" "$after"' EXIT
sleep 10
webkit > "$after"
helpers=$(comm -13 "$before" "$after" | tr '\n' ' ')
pids="$app_pid $helpers"

dictations="none"
if [ "$wait_dictations" = 1 ]; then
  echo "Dictate 20 times now, then press Enter to start the idle measurement."
  read -r _
  dictations="20, before the measurement"
fi

echo "Idle for $seconds s; sampling CPU every 10 s…" >&2
samples=0; cpu_total=0
end=$(( $(date +%s) + seconds ))
while [ "$(date +%s)" -lt "$end" ]; do
  sleep 10
  # ps reports a short decaying average per process, which suits an idle app.
  cpu=$(ps -o %cpu= -p "$(echo $pids | tr ' ' ',')" 2>/dev/null | awk '{s+=$1} END {printf "%.2f", s}')
  cpu_total=$(echo "$cpu_total + $cpu" | bc)
  samples=$((samples + 1))
done
cpu_avg=$(echo "scale=2; $cpu_total / $samples" | bc)

kib() { # footprint in KiB for one pid
  footprint -p "$1" 2>/dev/null | awk '/Footprint:/ {
    v=$0; sub(/.*Footprint: /, "", v); split(v, a, " ");
    n=a[1]; u=a[2]; if (u=="MB") n*=1024; else if (u=="GB") n*=1048576; else if (u=="B") n/=1024;
    printf "%d", n; exit }'
}
rows=""; total=0
for pid in $pids; do
  name=$(ps -o comm= -p "$pid" | sed 's#.*/##')
  k=$(kib "$pid"); k=${k:-0}
  total=$((total + k))
  rows="$rows| $name | $(echo "scale=1; $k / 1024" | bc) MB |
"
done

report="# Idle memory and CPU (AC-19), $(date +%Y-%m-%d)

- Machine: $(sysctl -n hw.model), $(sysctl -n machdep.cpu.brand_string), macOS $(sw_vers -productVersion)
- Build: $(git rev-parse --short HEAD 2>/dev/null || echo unknown), release
- Dictations before measuring: $dictations
- Idle period: $seconds s, CPU sampled every 10 s ($samples samples)
- Tools: \`footprint -p\` (physical footprint) and \`ps -o %cpu\`

| Process | Footprint |
| --- | --- |
$rows| **All** | **$(echo "scale=1; $total / 1024" | bc) MB** |

Average idle CPU of all processes: **$cpu_avg%**. AC-19: under 100 MB and under 0.5%.
"
printf '%s' "$report"
[ -n "$out" ] && printf '%s' "$report" > "$out"
exit 0
