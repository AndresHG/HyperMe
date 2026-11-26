#!/bin/sh
# Phase 1 live acceptance pass (P1-A): walks the owner through the live checks of
# specs/25-dictation-core and writes the numbers to docs/results/. Numbers and yes/no answers
# only: never audio, transcripts or app content. macOS only.
#
#   scripts/live-checks.sh ac10 wired|wifi   # 50 dictations, then the timing summary
#   scripts/live-checks.sh matrix            # AC-12 paste matrix, AC-14 and AC-17, by hand
#   scripts/live-checks.sh ac19              # 20 dictations, then 10 idle minutes
#   scripts/live-checks.sh ac21              # record the fresh-user walkthrough result
#
# Each step writes its own file, docs/results/<date>-phase1-<step>.md, and can be re-run.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
day=$(date +%Y-%m-%d)
results="docs/results"
log="$HOME/Library/Logs/HyperMe/timing.tsv"
app_bin="$HOME/Applications/HyperMe.app/Contents/MacOS/hyperme"

machine() {
  echo "- Date: $day"
  echo "- Machine: $(sysctl -n hw.model), $(sysctl -n machdep.cpu.brand_string), macOS $(sw_vers -productVersion)"
  commit=$(defaults read "$HOME/Applications/HyperMe.app/Contents/Info" HyperMeGitCommit 2>/dev/null || echo unknown)
  echo "- Build: HyperMe.app at $commit"
}

ask() { # ask "question" -> prints y, n or s (skip)
  while :; do
    printf '%s [y/n/s=skip] ' "$1" >&2
    read -r a
    case "$a" in y|Y) echo y; return ;; n|N) echo n; return ;; s|S) echo s; return ;; esac
  done
}

word() { case "$1" in y) echo pass ;; n) echo "**fail**" ;; *) echo "not checked" ;; esac; }

note() { # note "prompt" -> one line, no app content
  printf '%s (short, no dictated text; Enter for none): ' "$1" >&2
  read -r n
  echo "$n"
}

ac10() {
  net="$1"
  case "$net" in wired|wifi) ;; *) echo "usage: $0 ac10 wired|wifi" >&2; exit 2 ;; esac
  label=$([ "$net" = wired ] && echo "wired (Ethernet), Wi-Fi off" || echo "Wi-Fi, reference only")
  echo "AC-10 on $label."
  echo "1. Settings > Privacy > Timing log is on."
  [ "$net" = wired ] && echo "2. Ethernet plugged in and Wi-Fi turned off (menu bar > Wi-Fi > off)."
  [ "$net" = wifi ] && echo "2. Ethernet unplugged, Wi-Fi on."
  if [ -s "$log" ]; then
    mv "$log" "$log.before-$net-$(date +%H%M%S)"
    echo "   (The old timing log was moved aside, so this run starts empty.)"
  fi
  echo "3. Dictate 50 times in any text field, normal sentences of 5 to 30 words, in Spanish and"
  echo "   English as you usually would. Press Enter here when done."
  read -r _
  out="$results/$day-phase1-ac10-$net.md"
  body=$(cargo run -q --release -p hyperme-platform --example timing_summary -- "$log" --network "$label")
  { printf '%s\n' "$body" | sed -n '1,2p'; machine; printf '%s\n' "$body" | sed '1,2d'; } > "$out"
  cp "$log" "$log.$net-$day"
  echo; cat "$out"; echo; echo "Written to $out"
}

matrix() {
  out="$results/$day-phase1-paste-matrix.md"
  echo "AC-12 paste matrix. For each app: copy something (any word) first, focus a text field,"
  echo "dictate a short sentence, then check (a) the text arrived and (b) Cmd+V still pastes the"
  echo "word you copied (clipboard restored). Answer s to skip an app you don't have."
  rows=""; fails=""
  for target in "TextEdit" "Notes" "Safari text field" "Chrome text field" "Slack desktop" \
    "VS Code editor" "Terminal" "Ghostty" "Claude desktop" "Claude Code in a terminal" \
    "Mail compose" "Google Docs in a browser" "Spanish keyboard layout (in TextEdit)"; do
    echo; echo "== $target"
    p=$(ask "Text pasted?")
    c=s; [ "$p" = s ] || c=$(ask "Clipboard restored?")
    n=""; { [ "$p" = n ] || [ "$c" = n ]; } && n=$(note "What went wrong")
    rows="$rows| $target | $(word "$p") | $(word "$c") | $n |
"
    { [ "$p" = n ] || [ "$c" = n ]; } && fails="$fails- AC-12: $target
"
  done
  echo; echo "== AC-14: open a password field (e.g. Safari > any login page), focus it, hold the hotkey."
  a14=$(ask "Nothing was recorded and the pill showed the password notice?")
  n14=""; [ "$a14" = n ] && n14=$(note "What happened")
  echo; echo "== AC-17: with the pill visible (hold the hotkey), keep typing in TextEdit; then repeat"
  echo "   over a full-screen app (green button on Safari or TextEdit)."
  a17a=$(ask "Typing kept going in the app, windowed?")
  a17b=$(ask "Typing kept going over a full-screen app, and the pill showed there?")
  n17=""; { [ "$a17a" = n ] || [ "$a17b" = n ]; } && n17=$(note "What happened")
  [ "$a14" = n ] && fails="${fails}- AC-14: password field
"
  { [ "$a17a" = n ] || [ "$a17b" = n ]; } && fails="${fails}- AC-17: pill focus
"
  {
    echo "# Paste matrix, password field and focus (AC-12, AC-14, AC-17)"
    echo; machine; echo "- Checked by hand by the owner"; echo
    echo "## AC-12: paste and clipboard restore"; echo
    echo "| App | Text pasted | Clipboard restored | Note |"; echo "| --- | --- | --- | --- |"
    printf '%s' "$rows"; echo
    echo "## AC-14: password field"; echo
    echo "Nothing recorded and the notice shown: $(word "$a14"). $n14"; echo
    echo "## AC-17: the pill never takes focus"; echo
    echo "- Windowed app: $(word "$a17a")"
    echo "- Over a full-screen app: $(word "$a17b")"
    [ -n "$n17" ] && echo "- Note: $n17"
    echo; echo "## Failures"; echo
    if [ -n "$fails" ]; then printf '%s' "$fails"; else echo "None."; fi
  } > "$out"
  echo; cat "$out"; echo "Written to $out"
}

ac19() {
  [ -x "$app_bin" ] || { echo "No app at $app_bin: run scripts/bundle-macos.sh first." >&2; exit 1; }
  echo "AC-19, with the glass bead on. Quit HyperMe first (menu bar icon > Quit). The script starts it, you dictate 20"
  echo "times, then leave the Mac idle for 10 minutes (no typing; the screen may stay on)."
  printf 'Press Enter when HyperMe is quit. ' >&2; read -r _
  cfg="$HOME/Library/Application Support/HyperMe/config.toml"
  bead=$(awk -F'= *' '/^show_while_idle/ {print $2; exit}' "$cfg" 2>/dev/null)
  [ "$bead" = false ] && echo "Note: the glass bead is off (Settings > General > Show while idle). Turn it on: AC-19 is measured with it showing."
  out="$results/$day-phase1-ac19-idle.md"
  sh scripts/measure-idle.sh --after-dictations --out "$out" "$app_bin"
  # Read again after the run, in case it was switched during the dictations.
  bead=$(awk -F'= *' '/^show_while_idle/ {print $2; exit}' "$cfg" 2>/dev/null)
  case "$bead" in false) state="off" ;; *) state="on (shown at rest)" ;; esac
  printf '\nGlass bead while idle (Settings > General > Show while idle): %s.\n' "$state" >> "$out"
  echo "Written to $out"
}

ac21() {
  out="$results/$day-phase1-ac21-fresh-user.md"
  cat <<'EOF'
AC-21, done by hand:
1. System Settings > Users & Groups > Add User (Standard), log in to it (Fast User Switching).
2. Copy HyperMe.app into that user's ~/Applications (or /Applications), open it from Finder.
3. Follow the walkthrough only: microphone, Accessibility, Input Monitoring, both keys.
   No terminal at any point.
4. Dictate once into TextEdit.
Then come back to this account and answer.
EOF
  a=$(ask "First dictation pasted, through the walkthrough, without a terminal?")
  mins=$(note "Minutes from first launch to the first paste")
  n=$(note "Anything confusing or blocked")
  {
    echo "# Fresh macOS user to first dictation (AC-21)"; echo; machine; echo
    echo "- Result: $(word "$a")"
    echo "- Minutes from first launch to first paste: ${mins:-not noted}"
    if [ -n "$n" ]; then echo "- Notes: $n"; fi
  } > "$out"
  cat "$out"; echo "Written to $out"
}

case "${1:-}" in
  ac10) ac10 "${2:-}" ;;
  matrix) matrix ;;
  ac19) ac19 ;;
  ac21) ac21 ;;
  *) sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
