#!/bin/bash
# Type a line into a focused probe window through the live compositor and
# compare what the application received with the expectation.
#
# usage: tools/run_line.sh <name> <delay_ms> <gap_ms> <expected> <evdev keys...>
#
# Build first: cargo build --release -p vktap -p improbe.  Keys are evdev
# codes; "42+38" is a chord (Shift+l).  The tap client aborts the moment the
# probe window loses keyboard focus, so nothing lands in another window.
# Logs go to $RUN_LINE_DIR (default /tmp/cosmic-ext-ime-probe); the probe
# stays open $HOLD seconds (default 15) after the last key.
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
bin=$here/target/release
out=${RUN_LINE_DIR:-/tmp/cosmic-ext-ime-probe}
mkdir -p "$out"
name=$1 delay=$2 gap=$3 expected=$4
shift 4
focus=$out/$name.focus
log=$out/$name.log

echo 0 > "$focus"
PROBE_FOCUS_FILE=$focus "$bin/improbe" 150 > "$log" 2>&1 &
probe=$!
# Wait up to 20 s for the probe window to get keyboard focus.
for _ in $(seq 1 80); do
    [ "$(cat "$focus")" = "1" ] && break
    sleep 0.25
done
if [ "$(cat "$focus")" != "1" ]; then
    echo "[$name] probe never focused in 20s; skipped"
    kill "$probe"
    exit 0
fi
sleep 0.5
# RUN_LINE_START_CHINESE=1 with RUN_LINE_IM_LOG=<daemon log>: if the daemon's
# last layout decision was the English source, press Super+Space first so
# the line always starts on the Chinese source.
if [ "${RUN_LINE_START_CHINESE:-0}" = "1" ] && [ -n "${RUN_LINE_IM_LOG:-}" ]; then
    if grep -E "layout [0-9]+: " "$RUN_LINE_IM_LOG" | tail -1 | grep -q -- "-> english"; then
        VKTAP_FOCUS_FILE=$out/$name.focus "$bin/vktap" 125+57 >/dev/null; sleep 0.8
    fi
fi
VKTAP_FOCUS_FILE=$focus VKTAP_DELAY_MS=$delay VKTAP_GAP_MS=$gap \
    "$bin/vktap" "$@" 2>&1 | grep -v "^ok$"
sleep "${HOLD:-15}"
kill "$probe" 2>/dev/null
sleep 0.2

# What the application received: the text of forwarded key presses and of
# text-input commits, in order.
got=$(grep -E "kbd key .*Pressed|COMMIT" "$log" \
    | sed -n 's/.*utf8 "\(.*\)"$/\1/p; s/.*COMMIT Some("\(.*\)")$/\1/p' \
    | tr -d '\n')
echo "[$name] delay=${delay}ms"
echo "  expected: $expected"
echo "  got:      $got"
if [ "$got" = "$expected" ]; then
    echo "  PASS"
else
    echo "  MISMATCH (focus changes: $(grep -c 'kbd LEAVE' "$log"))"
fi
