#!/bin/sh
# AC-19: idle memory and CPU of every HyperMe process (the app and its WebKit helpers).
#
#   scripts/measure-idle.sh [--seconds N] [--after-dictations] [--build SHA] [--out FILE]
#                           [path/to/hyperme]
#
# Starts the release app, optionally waits while you dictate (AC-19 asks for 20 dictations
# first), then samples CPU every 10 s for N seconds (default 600, AC-19's 10 minutes) and reports
# the physical footprint (`footprint`, what Activity Monitor shows as Memory) of each process.
# Numbers only. macOS only.
#
# WebKit runs the web view in XPC helpers that launchd starts, so they are not the app's
# children. The helpers are found again at every sample (#194): the app's children, plus every
# WebKit process that was not running before the launch. A helper that is replaced or starts
# late is still counted; keep Safari and Mail closed so their new helpers are not.
#
# --build names the commit of the binary measured. Without it the commit is read when the app
# starts, from the git tree the binary sits in, and marked when it may not match (#194).
set -eu

seconds=600
wait_dictations=0
out=""
build=""
app="target/release/hyperme"
while [ $# -gt 0 ]; do
  case "$1" in
    --seconds) seconds="$2"; shift 2 ;;
    --after-dictations) wait_dictations=1; shift ;;
    --build) build="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    *) app="$1"; shift ;;
  esac
done
[ -x "$app" ] || { echo "No app at $app: run 'cargo build --release -p hyperme-app' first." >&2; exit 1; }
if pgrep -x hyperme >/dev/null; then
  echo "HyperMe is already running; quit it first so its WebKit processes can be told apart." >&2
  exit 1
fi

# The commit of the binary, read now rather than at report time, when the tree may have moved.
if [ -z "$build" ]; then
  dir=$(dirname "$app")
  if sha=$(git -C "$dir" rev-parse --short HEAD 2>/dev/null); then
    build="$sha"
    [ -n "$(git -C "$dir" status --porcelain --untracked-files=no 2>/dev/null)" ] && build="$build, uncommitted changes"
    head_time=$(git -C "$dir" log -1 --format=%ct)
    bin_time=$(stat -c %Y "$app" 2>/dev/null || stat -f %m "$app")
    [ "$bin_time" -lt "$head_time" ] && build="$build (binary older than this commit: pass --build)"
  else
    build="unknown (pass --build)"
  fi
fi

webkit() { pgrep -f 'com.apple.WebKit' | sort; }
before=$(mktemp); seen=$(mktemp)
webkit > "$before"
HYPERME_NO_WALKTHROUGH=1 "$app" >/dev/null 2>&1 &
app_pid=$!
trap 'kill "$app_pid" 2>/dev/null; rm -f "$before" "$seen"' EXIT

# The app and its helpers as they are now: its children, and WebKit processes new since launch.
current_pids() {
  { pgrep -P "$app_pid"; webkit | comm -13 "$before" -; } | sort -u | tr '\n' ' '
  echo "$app_pid"
}
sample_pids() { pids=$(current_pids); echo "$pids" | tr ' ' '\n' >> "$seen"; }

sleep 10
sample_pids

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
  sample_pids
  # ps reports a short decaying average per process, which suits an idle app.
  cpu=$(ps -o %cpu= -p "$(echo $pids | tr ' ' ',')" 2>/dev/null | awk '{s+=$1} END {printf "%.2f", s}')
  cpu_total=$(echo "$cpu_total + $cpu" | bc)
  samples=$((samples + 1))
done
cpu_avg=$(echo "scale=2; $cpu_total / $samples" | bc)
sample_pids
gone=$(sort -u "$seen" | while read -r p; do kill -0 "$p" 2>/dev/null || echo "$p"; done | wc -l | tr -d ' ')

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
- Build: $build, release
- Dictations before measuring: $dictations
- Idle period: $seconds s, CPU sampled every 10 s ($samples samples)
- Processes that exited during the run (not in the table): $gone
- Tools: \`footprint -p\` (physical footprint) and \`ps -o %cpu\`

| Process | Footprint |
| --- | --- |
$rows| **All** | **$(echo "scale=1; $total / 1024" | bc) MB** |

Average idle CPU of all processes: **$cpu_avg%**. AC-19: under 100 MB and under 0.5%.
"
printf '%s' "$report"
[ -n "$out" ] && printf '%s' "$report" > "$out"
exit 0
