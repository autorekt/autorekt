# autorekt — guide for agents

You are an agent asked to analyze or watch markets. autorekt gives you prices, positioning,
volatility, prediction-market odds, macro data and news as compact text, plus rules that wake
you when something happens. Everything also works with `--json` / `--csv`.

## 0. Recipes

**"What is happening in the markets?"**
1. `autorekt brief`: the cross-asset board, vol curve, liquidity, crypto positioning, Fed odds, calendar and headlines.
2. For each big mover, run `autorekt why <ID>`. It finds the move, shows what moved with it (z-scores) and dated news. If many assets moved together, it is macro; if one moved alone, it is idiosyncratic.
3. Check breadth before calling a crash: `autorekt board sectors`, `board megacaps`, `board asia`, `board "RSP / SPY"`. A cap-weighted index can fall while the average stock rises.
4. Find the regime: `autorekt corr -i 1d -n 30`. Do stocks and yields move together? Is gold hedging? Is crypto trading as tech beta?
5. Get forward-looking odds: `autorekt prob show kalshi:<next Fed event>`, and `prob search` for elections, recession or geopolitics.

**"Is there a liquidity crisis?"**
- `autorekt board liquidity` plus the spreads `"fred:SOFR - fred:IORB"` and `"fred:SOFR99 - fred:IORB"`.
- Squeeze: repo above IORB, reserves falling, TGA rising, RRP near zero.
- Crisis: standing repo usage (`fred:RPONTSYD`) in the tens of billions, HY OAS > 5–6%, NFCI > 0, the VIX curve (`VIX - VIX3M`) inverted for days, and correlated selling in everything including Treasuries.

**"Watch X and tell me"**
1. Backtest first, so you neither spam nor miss: `autorekt rule test '<expr>' --range 30d --tf 5m`.
2. `autorekt rule add NAME --expr '<expr>' --hook '<how to reach you>'`.
3. Without a hook route: `autorekt wait --timeout 1h` in a loop, or `autorekt diff --as <you>` whenever you wake.

## 1. Orient (start here)

```
autorekt brief                 # one-shot briefing: board, vol, liquidity, crypto positioning, odds, calendar, headlines
autorekt board                 # cross-asset overview: last, 1h/day/5d/30d, trend, flags
autorekt board fear            # VIX family, MOVE, OVX, DVOL, Fear & Greed, credit ETFs
autorekt board liquidity       # SOFR/IORB/reserves/TGA/RRP/credit spreads/NFCI (FRED, lagged)
autorekt board crypto | macro | global | rates | commodities | positioning | sectors | megacaps | asia
autorekt board 'BTC,ETH,RSP / SPY,CBP|(coinbase:BTC/USD / okx:BTC/USDT - 1) * 10000'   # ids, expressions, LABEL|expr
autorekt diff --as <you>       # only what changed since you last looked (events, moves, headlines)
autorekt news --since 6h --min-severity 2
autorekt calendar              # macro releases this week (ForexFactory)
```

## 2. Drill down

```
autorekt candles BTC -i 1h -n 48          # one line per candle, exact OHLCV + picture + stats
autorekt candles NQ -i 15m --end 2026-10-30T20:00:00Z -n 40   # look at a past window
autorekt metric BTC#funding_rate -n 60    # sparkline + stats + table
autorekt metric "fred:SOFR - fred:IORB" -i 1d -r 90d   # any expression is a series
autorekt quote BTC ETH SPX VIX GOLD US10Y
autorekt corr -i 1d -n 30                 # cross-asset correlation matrix
autorekt why GOLD                         # explain the biggest move in 7d: the move, what else moved (z-scores), dated news
autorekt why BTC --at 2026-10-30T00:00:00Z --window 4h
autorekt prob search "fed december"; autorekt prob show kalshi:KXFEDDECISION-26DEC
autorekt prob trending
autorekt prelaunch                        # pre-launch / pre-IPO perps
autorekt funding BTC ETH SOL              # funding across OKX/Hyperliquid/Deribit/Binance, annualized
autorekt news --about "binance hack"      # Google News search
autorekt markets hyperliquid xyz          # discover ids
```

## 3. Ids

`<provider>:<symbol>[#metric]`. Use aliases when unsure (`autorekt resolve X` shows the id).

| What | Example ids |
|---|---|
| Crypto perps (CCXT symbols) | `BTC` (= okx:BTC/USDT:USDT), `okx:ETH/USDT:USDT`, `hyperliquid:BTC/USDC:USDC` |
| Crypto spot | `okx:BTC/USDT`, `binance:BTC/USDT` |
| 24/7 equity/commodity perps | `hyperliquid:xyz:XYZ100` (Nasdaq-100), `hyperliquid:xyz:TSLA`, `hyperliquid:xyz:GOLD` |
| Crypto metrics | `BTC#funding_rate` `#open_interest` `#long_short_ratio` `#top_long_short_ratio` `#taker_buy_sell_ratio` `#basis` |
| Crypto options | `DVOL` (deribit:BTC-DVOL), `ETHDVOL`, `deribit:BTC#put_call_oi`, `#put_call_volume`, `#skew_10pct`, `#atm_iv` |
| Indices, futures, FX, yields | `SPX NDX ES NQ VIX VIX9D VIX3M MOVE DXY USDJPY US10Y GOLD WTI` or `yahoo:<ticker>` |
| Macro / liquidity (FRED) | `fred:SOFR fred:IORB fred:WRESBAL fred:WTREGEN fred:RRPONTSYD fred:BAMLH0A0HYM2 fred:NFCI` |
| Sentiment | `FNG` (crypto), `CNNFNG` (stocks), `alt:cnn_fng#put_call_options` (components) |
| Futures positioning (CFTC COT, weekly) | `COT_ES COT_NQ COT_ZN COT_GOLD COT_WTI COT_JPY COT_EUR COT_BTC COT_VIX` (= `cftc:<code>`), metrics `#noncommercial_net` `#lev_funds_net` `#asset_mgr_net` `#dealer_net` `#open_interest`; find codes: `markets cftc UST` |
| Dark-pool short volume (FINRA, daily) | `finra:SPY` (= short volume / off-exchange volume), `#short_volume`, `#total_volume` |
| Prediction markets | `polymarket:<market-slug>` (`#No` for another outcome), `kalshi:<TICKER>` |
| Crypto liquidity | `STABLES` (llama:stablecoins, total supply), `llama:USDT`, Coinbase premium `(coinbase:BTC/USD / okx:BTC/USDT - 1) * 10000` |
| Your own recorded series | `local:<name>` |

Timeframes: `1m 5m 15m 30m 1h 4h 1d 1w`. Durations: `30m 6h 7d 90d`.

## 4. Watch: rules that wake you

A rule is a Pine-style expression in a Prometheus-style wrapper. The daemon (auto-started)
evaluates it; when it fires it logs an event, releases `autorekt wait`, and runs your hook.

```
autorekt rule add btc_dump --expr 'pct(BTC, "1h") < -3' --tf 1m --cooldown 2h \
    --message 'BTC {{value}}% in 1h' --hook '<command that notifies you>'
autorekt rule add repo --expr 'fred:SOFR - fred:IORB > 0.10' --tf 1d
autorekt rule add reclaim --expr 'ta.crossover(close("BTC","1h"), ta.sma(close("BTC","1h"), 50))' --tf 1h --eval bar_close
autorekt rule add vix_inv --expr 'VIX - VIX3M > 1.5' --tf 15m --for 30m
autorekt rule add hourly --every 1h --hook '...'          # timer
autorekt rule add open --schedule '30 13 * * 1-5' --hook '...'   # cron, UTC
autorekt rule add btc_80k --id BTC --above 80000          # shorthand
autorekt rule add macro --calendar-before 15m --calendar-countries USD --hook '...'  # before high-impact releases
autorekt rule test btc_dump --range 30d                   # backtest first: how often, and what followed?
autorekt rule ls | show | check | rm | enable | disable
autorekt wait [--rule a,b] [--timeout 30m]                # block until something fires (exit 2 on timeout)
autorekt events --since 24h
```

Expression language: ids are series of closes at the rule's `--tf`; `close/open/high/low/volume(id, tf)`,
`funding_rate(id)`, `open_interest(id)`, `prob(id[, outcome])`, `pct(id, "1h")`, `stale(id, "2h")`,
`atr(id, tf, n)`; `ta.sma ema rma wma rsi stdev highest lowest sum zscore change roc crossover crossunder
cross rising falling`; `abs min max sqrt log nz hour dayofweek`; `+ - * / > < >= <= == != and or not`.
Quote ids that contain operators: `"kalshi:KXFED-26DEC-H25" > 0.5`.

Hook environment: `AUTOREKT_RULE AUTOREKT_MESSAGE AUTOREKT_VALUE AUTOREKT_RHS AUTOREKT_TIME
AUTOREKT_SEVERITY AUTOREKT_EXPR AUTOREKT_CONTEXT` (recent candles as text) `AUTOREKT_CHART_URL`
(interactive chart, when `hook_chart_url: true` in config.yaml) `AUTOREKT_LABEL_<KEY>`. Examples: `microteams api say --thread-id N --text "$AUTOREKT_MESSAGE"`,
`curl -d "$AUTOREKT_MESSAGE" ntfy.sh/topic`, or a command that starts your agent with the message as its prompt.

## 5. Custom logic: Python monitors

```
autorekt monitor new basis_watch     # writes ~/.autorekt/monitors/basis_watch.py from a template
autorekt monitor ls | logs NAME | rm NAME | sdk
```

A monitor subclasses `autorekt.Monitor` (on_start, subscribe_bars, on_bar, schedule, history →
list of OHLCV dicts, eval, record → `local:<series>`, alert). The daemon runs and restarts it.

## 6. Show a human

```
autorekt share BTC -i 15m --overlay BTC#funding_rate      # prints a public link (Cloudflare quick tunnel)
autorekt share "fred:SOFR - fred:IORB" -i 1d --kind line
```

## 7. Caveats you should state when reporting

- Yahoo index/futures quotes may be ~15 min delayed. FRED series lag 1–5 days (weekly ones more).
- Binance futures and Bybit are geo-restricted in some regions (e.g. the US); OKX is the default.
- Hyperliquid open interest and Deribit put/call/skew have no public history; add them to
  `record:` in config.yaml so the daemon samples them.
- Not investment advice. The tool is named autorekt for a reason.
