#!/usr/bin/env bash
# Live smoke test: drives the real binary against real providers. Run before a release.
# Usage: scripts/smoke.sh [path/to/autorekt]
set -u
BIN=${1:-target/release/autorekt}
export AUTOREKT_HOME=$(mktemp -d)
pass=0; fail=0
check() {  # check "<description>" <expected substring> <command...>
  local desc=$1 want=$2; shift 2
  local out
  if [ "$1" = cat-file ]; then out=$(cat "$2"); else out=$("$BIN" "$@" 2>&1); fi
  if [[ "$out" == *"$want"* ]]; then pass=$((pass+1)); printf 'ok    %s\n' "$desc"
  else fail=$((fail+1)); printf 'FAIL  %s\n      wanted "%s" in: %s\n' "$desc" "$want" "$(echo "$out" | head -3)"; fi
}
check "search resolves alias"   "okx:BTC/USDT:USDT"   search BTC --kind instruments -n 2
check "search normalizes id"    "okx:BTC/USDT:USDT"   search okx:BTC-USDT-SWAP --kind instruments -n 1
check "search odds"             "polymarket:"         search fed --kind odds -n 2
check "search news"             "## news"             search bitcoin --kind news -n 3
check "search sources"          "okx"                 search
check "show chart okx"          "summary"             show BTC -i 1h -n 10
check "show chart yahoo daily"  "summary"             show SPX -i 1d -n 10
check "show fx cross"           "AUDJPY=X"            show AUDJPY -i 1d -n 5
check "show context"            "swings ("            show GOLD -i 4h
check "show with column"        "ta.rsi(\$,14)"        show AUDUSD -i 1d -n 5 --with 'ta.rsi($,14)'
check "show hyperliquid HIP-3"  "summary"             show hyperliquid:xyz:XYZ100 -n 5
check "show metric line"        "stats"               show BTC#funding_rate -n 10
check "show expression"         "expression"          show "fred:SOFR - fred:IORB" -i 1d -r 30d
check "show board"              "DVOL"                show fear
check "show rows"               "ETH"                 show BTC ETH
check "show market"             "kalshi:"             show kalshi:KXFEDDECISION-26DEC
check "show news"               " "                   show news -n 3
check "show calendar"           "calendar"            show calendar
check "show funding"            "okx"                 show funding BTC
check "show unknown view"       "not a view"          show foo
check "watch backtest"          "backtest"            watch 'pct(BTC, "1h") < -1' --test --tf 5m -r 3d
"$BIN" daemon start >/dev/null
"$BIN" watch --wait --timeout 120s > "$AUTOREKT_HOME/wait.out" 2>&1 &
waiter=$!
sleep 2
check "watch add"               "smoke_rule"          watch smoke_rule --expr 'BTC > 1' --no-check
check "watch test now"          "condition=true"      watch smoke_rule --test
wait $waiter
check "watch wait fires"        "smoke_rule"          cat-file "$AUTOREKT_HOME/wait.out"
check "show alerts"             "smoke_rule"          show alerts -r 1h
check "show changes"            "diff for"            show crypto --changes --as smoke
printf '# Smoke report\n\nText.\n\n```chart\nBTC -i 1h -r 2d\nRSP / SPY -i 1d -r 30d\n```\n' > "$AUTOREKT_HOME/r.md"
check "report preview"          "[chart] RSP / SPY"   show "$AUTOREKT_HOME/r.md"
check "report publish"          "/r/"                 show "$AUTOREKT_HOME/r.md" --share --local
check "why move"                "what else moved"     why BTC -r 2d
check "why correlations"        "correlation"         why BTC ETH -r 30d
check "why breaks"              "breaks:"             why BTC ETH GOLD -r 60d
"$BIN" daemon stop >/dev/null 2>&1
rm -rf "$AUTOREKT_HOME"
echo "passed $pass, failed $fail"
[ "$fail" -eq 0 ]
