#!/bin/bash
# Periodically sample memory / FD / thread counts for Moxin Voice processes.
# Output: scripts/diagnostics/memsample-<ts>.csv (one row per process per tick).
#
# Usage:
#   ./scripts/diagnostics/sample_mem.sh [duration_sec] [interval_sec]
# Defaults: 5400s (90min) at 30s interval.
#
# Run while a real workload is active (live translation actually transcribing).
# Idle processes leak nothing.

set -u

DURATION="${1:-5400}"
INTERVAL="${2:-30}"

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT_DIR="$REPO_ROOT/scripts/diagnostics"
mkdir -p "$OUT_DIR"
TS=$(date +%Y%m%d-%H%M%S)
CSV="$OUT_DIR/memsample-$TS.csv"
LOG="$OUT_DIR/memsample-$TS.log"

# Patterns to match against `pgrep -f` (full command line).
# Keep them specific enough to avoid matching unrelated processes.
PATTERNS=(
  "moxin-voice-shell"
  "dora-qwen3-asr"
  "dora-qwen35-translator"
  "qwen-tts-node"
  "dora-daemon"
  "dora-coordinator"
)

echo "timestamp,elapsed_sec,name,pid,rss_kb,footprint,fd_count,thread_count" > "$CSV"

{
  echo "=== sample_mem.sh starting ==="
  echo "duration=${DURATION}s interval=${INTERVAL}s"
  echo "csv=$CSV"
  echo "patterns: ${PATTERNS[*]}"
  echo "started at: $(date)"
  echo "pid of this script: $$"
} | tee -a "$LOG"

START_EPOCH=$(date +%s)
ITER=0

cleanup() {
  echo "=== sample_mem.sh stopping (signal) at $(date) ===" | tee -a "$LOG"
  summarize
  exit 0
}
trap cleanup INT TERM

summarize() {
  echo "" | tee -a "$LOG"
  echo "=== Summary (first vs last observation per pid) ===" | tee -a "$LOG"
  # awk: group by name+pid, capture first/last rss_kb and footprint
  awk -F',' '
    NR==1 { next }
    {
      key = $3 "|" $4
      if (!(key in firstSeen)) {
        firstSeen[key] = 1
        firstRss[key] = $5
        firstFp[key]  = $6
        firstFd[key]  = $7
        firstTh[key]  = $8
        firstTime[key]= $2
      }
      lastRss[key]   = $5
      lastFp[key]    = $6
      lastFd[key]    = $7
      lastTh[key]    = $8
      lastTime[key]  = $2
      name[key]      = $3
      pid[key]       = $4
    }
    END {
      printf("%-26s %-8s %12s %12s %14s %14s %10s %10s %10s %10s\n",
        "name","pid","rss_first","rss_last","fp_first","fp_last","fd_first","fd_last","thr_first","thr_last")
      for (k in firstSeen) {
        printf("%-26s %-8s %12s %12s %14s %14s %10s %10s %10s %10s\n",
          name[k], pid[k],
          firstRss[k], lastRss[k],
          firstFp[k], lastFp[k],
          firstFd[k], lastFd[k],
          firstTh[k], lastTh[k])
      }
    }
  ' "$CSV" | tee -a "$LOG"
  echo "" | tee -a "$LOG"
  echo "Full data: $CSV" | tee -a "$LOG"
}

# Return the "Physical footprint" line from vmmap --summary, in MiB.
# Output: bare number (MiB), or empty string on failure.
get_footprint_mib() {
  local pid="$1"
  # `vmmap --summary <pid>` prints a "Physical footprint:" line like:
  #   Physical footprint:         1234.5M
  #   Physical footprint:         1.2G
  # We normalize to MiB.
  local raw
  raw=$(vmmap --summary "$pid" 2>/dev/null \
        | awk -F':' '/^Physical footprint:/ { gsub(/ /,"",$2); print $2; exit }')
  if [[ -z "$raw" ]]; then
    echo ""
    return
  fi
  # Strip trailing letter, convert
  local num unit
  num=$(echo "$raw" | sed -E 's/[A-Za-z]+$//')
  unit=$(echo "$raw" | grep -oE '[A-Za-z]+$' || true)
  case "$unit" in
    G|GB) awk -v n="$num" 'BEGIN { printf("%.1f", n*1024) }' ;;
    M|MB|"") awk -v n="$num" 'BEGIN { printf("%.1f", n) }' ;;
    K|KB) awk -v n="$num" 'BEGIN { printf("%.3f", n/1024) }' ;;
    *) echo "$raw" ;;
  esac
}

sample_pid() {
  local name="$1"
  local pid="$2"
  local rss thr fd fp thr_lines fd_lines
  rss=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ')
  # macOS has no `thcount` ps keyword; enumerate via `ps -M`.
  # Header line + 1 per thread → subtract 1.
  thr_lines=$(ps -M -p "$pid" 2>/dev/null | wc -l | tr -d ' ')
  if [[ -n "$thr_lines" && "$thr_lines" -gt 0 ]]; then
    thr=$((thr_lines - 1))
  else
    thr=""
  fi
  # `timeout` is GNU coreutils; macOS may not have it. lsof itself is usually fast.
  # Header line + 1 per FD → subtract 1.
  fd_lines=$(lsof -p "$pid" 2>/dev/null | wc -l | tr -d ' ')
  if [[ -n "$fd_lines" && "$fd_lines" -gt 0 ]]; then
    fd=$((fd_lines - 1))
  else
    fd=""
  fi
  fp=$(get_footprint_mib "$pid")
  printf "%s,%d,%s,%s,%s,%s,%s,%s\n" \
    "$(date -u +%FT%TZ)" "$ELAPSED" "$name" "$pid" \
    "${rss:-NA}" "${fp:-NA}" "${fd:-NA}" "${thr:-NA}"
}

self_pid=$$
while :; do
  NOW=$(date +%s)
  ELAPSED=$((NOW - START_EPOCH))
  if (( ELAPSED >= DURATION )); then
    echo "=== sample_mem.sh duration reached at $(date) ===" | tee -a "$LOG"
    break
  fi

  ITER=$((ITER + 1))
  observed_any=0
  for pat in "${PATTERNS[@]}"; do
    # pgrep -f to match across full argv.
    pids=$(pgrep -f -- "$pat" 2>/dev/null || true)
    for pid in $pids; do
      # Skip if pid is this script, or a shell descendant running pgrep/awk for us.
      [[ "$pid" == "$self_pid" ]] && continue
      # Skip processes whose comm is a shell or our own helpers.
      comm=$(ps -p "$pid" -o comm= 2>/dev/null | awk -F'/' '{print $NF}')
      case "$comm" in
        bash|zsh|sh|pgrep|awk|ps|lsof|vmmap|grep|tee|sed|tr|wc|sleep|timeout|date) continue ;;
      esac
      sample_pid "$pat" "$pid" >> "$CSV"
      observed_any=1
    done
  done

  if (( ITER == 1 || ITER % 4 == 0 )); then
    # Heartbeat every ~2min (4 ticks @ 30s)
    echo "[t=${ELAPSED}s iter=$ITER] sampled; observed_any=$observed_any" | tee -a "$LOG"
  fi

  sleep "$INTERVAL"
done

summarize
echo "=== sample_mem.sh done at $(date) ===" | tee -a "$LOG"
