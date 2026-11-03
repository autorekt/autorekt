#!/usr/bin/env bash
# Live smoke test: drives the real binary against real providers. Run before a release.
# Usage: scripts/smoke.sh [path/to/autorekt]
set -u
BIN=${1:-target/release/autorekt}
export AUTOREKT_HOME=$(mktemp -d)
pass=0; fail=0
check() {  # check "<description>" <expected substring> <command...>
  local desc=$1 want=$2; shift 2
  local out; out=$("$BIN" "$@" 2>&1)
  if [[ "$out" == *"$want"* ]]; then pass=$((pass+1)); printf 'ok    %s\n' "$desc"
  else fail=$((fail+1)); printf 'FAIL  %s\n      wanted "%s" in: %s\n' "$desc" "$want" "$(echo "$out" | head -3)"; fi
}
check "resolve aliases"         "okx:BTC/USDT:USDT"   resolve BTC
check "normalize OKX native id" "okx:BTC/USDT:USDT"   resolve okx:BTC-USDT-SWAP
check "quote crypto"            "BTC"                 quote BTC
check "candles okx"             "summary"             candles BTC -i 1h -n 10
check "candles yahoo daily"     "summary"             candles SPX -i 1d -n 10
check "hyperliquid HIP-3"       "summary"             candles hyperliquid:xyz:XYZ100 -i 1h -n 5
check "funding metric"          "stats"               metric BTC#funding_rate -n 10
check "fred expression"         "expression"          metric "fred:SOFR - fred:IORB" -i 1d -r 30d
check "deribit dvol"            "DVOL"                board fear
check "polymarket search"       "polymarket:"         prob search fed --provider polymarket -n 2
check "kalshi event"            "kalshi:"             prob show kalshi:KXFEDDECISION-26DEC
check "news feeds"              " "                   news -n 3
check "calendar"                "calendar"            calendar --days 7 --min-impact 1
check "rule backtest"           "backtest"            rule test 'pct(BTC, "1h") < -1' --tf 5m --range 3d
check "rule add + ls"           "smoke_rule"          rule add smoke_rule --id BTC --above 1 --no-check
check "rule check"              "condition=true"      rule check smoke_rule
check "wait fires"              "smoke_rule"          wait --rule smoke_rule --timeout 90s
check "events logged"           "smoke_rule"          events --since 1h
check "diff"                    "diff for"            diff --as smoke --set crypto
check "why"                     "what else moved"     why BTC --lookback 2d
"$BIN" daemon stop >/dev/null 2>&1
rm -rf "$AUTOREKT_HOME"
echo "passed $pass, failed $fail"
[ "$fail" -eq 0 ]
