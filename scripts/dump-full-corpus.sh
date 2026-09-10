#!/bin/sh
# Download the whole Judilibre corpus to data/full/, one file per jurisdiction-year.
#
# Judilibre publishes no bulk archive, and PISTE returns sporadic 500s and
# connection resets during a long export. So the work is sliced by year: a slice
# that fails is retried on its own, and a completed slice is never re-fetched.
# Safe to re-run at any time — it resumes where it stopped.
#
# Usage: scripts/dump-full-corpus.sh [jurisdiction ...]     (default: all four)
set -u

cd "$(dirname "$0")/../indexer" || exit 1
OUT=../data/full
BIN=./target/release/judilibre-indexer
mkdir -p "$OUT"
cargo build --release -q || exit 1

# First year with data, per jurisdiction (measured against /export).
first_year() {
  case $1 in
    cc) echo 1900 ;;
    ca) echo 1996 ;;
    tj) echo 2023 ;;
    tcom) echo 2024 ;;
    *) echo "unknown jurisdiction: $1" >&2; exit 1 ;;
  esac
}

slice() {
  j=$1
  y=$2
  f="$OUT/$j-$y.jsonl"
  if [ -s "$f" ]; then
    echo "skip $j $y ($(wc -l < "$f" | tr -d ' ') decisions)"
    return 0
  fi
  try=1
  while [ "$try" -le 3 ]; do
    if "$BIN" index --jurisdiction "$j" \
        --date-start "$y-01-01" --date-end "$y-12-31" \
        --batch-size 1000 --delay-ms 200 --out "$f" 2>>"$OUT/$j-$y.err"; then
      n=$(wc -l < "$f" | tr -d ' ')
      if [ "$n" -eq 0 ]; then
        rm -f "$f" "$OUT/$j-$y.err"   # empty year: leave it re-checkable
        echo "none $j $y"
      else
        rm -f "$OUT/$j-$y.err"
        echo "ok   $j $y -> $n decisions"
      fi
      return 0
    fi
    echo "retry $j $y (attempt $try failed)"
    rm -f "$f"
    sleep $((try * 30))
    try=$((try + 1))
  done
  echo "FAIL $j $y - see $OUT/$j-$y.err"
  return 1
}

this_year=$(date +%Y)
failures=0
for j in ${*:-tcom tj ca cc}; do
  start=$(first_year "$j")
  year=$this_year
  while [ "$year" -ge "$start" ]; do
    slice "$j" "$year" || failures=$((failures + 1))
    year=$((year - 1))
  done
  echo "--- $j: $(cat "$OUT/$j"-*.jsonl 2>/dev/null | wc -l | tr -d ' ') decisions so far"
done

echo "TOTAL $(cat "$OUT"/*.jsonl 2>/dev/null | wc -l | tr -d ' ') decisions in $OUT ($failures failed slices)"
[ "$failures" -eq 0 ]
