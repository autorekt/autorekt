---
name: autorekt
description: Read live markets with the autorekt CLI before answering any question about prices, why something moved, rates and dollar liquidity, positioning, options, crypto, China and Hong Kong markets, prediction-market odds or market news, and set alerts that wake the agent when a market condition holds. Use it instead of guessing from training data or headlines.
---

# autorekt

autorekt prints market data as compact text: numbers first, each with how old it is. It fetches
from public sources on this machine and never trades. If `autorekt` is not on PATH, install it with
`curl -fsSL https://autorekt.com/install.sh | sh` (Windows: `irm https://autorekt.com/install.ps1 | iex`).

## Look before you explain

1. Start every market question with `autorekt show`: the briefing covers cross-asset moves,
   volatility, dollar liquidity, rates, crypto positioning, odds, the calendar and headlines.
2. Drill into what stands out, then check what else moved at the same time:

```sh
autorekt show liquidity                  # repo vs IORB, swap spreads, TGA, reserves, credit
autorekt show positioning                # CFTC, dealer gamma, put/call, ETF flows, short volume
autorekt show sectors                    # also: fear rates commodities megacaps asia crypto
autorekt show sectors --with 'chg(5d),chg(ytd),from_high(52w)'   # a table with the columns you pick
autorekt show SPY QQQ IWM --from 2026-11-06 --to 2026-11-12       # change over a span
autorekt show NQ -i 15m -r 1d            # one series: candles, swings, levels, events, data checks
autorekt show 'RSP / SPY' -i 1d -r 120d  # any expression works where an id does
autorekt show BTC --view explain -r 2d                     # the biggest move, what moved with it, dated news
autorekt show SPX BTC GOLD US10Y --view corr -r 30d         # correlations and which pairs broke
autorekt show sectors --where 'chg(ytd) > 15' --sort 'chg(5d)'   # filter and sort a table
autorekt show @watchlist.txt                               # the ids in a file, one per line
autorekt show cn.industry --sort 'flow(5d)' --top 10 --bottom 10   # A-share boards by fund flow (also cn.concept, hk.southbound)
autorekt show 0788.HK 2601.HK AAPL --with 'pe,pb,dy,payout,roe'      # company figures (US needs sec_contact in config.yaml)
autorekt show news 0788.HK -r 14d                         # a stock's exchange filings and headlines
autorekt show calendar NVDA AAPL                          # coming earnings and dividend dates
autorekt show 0788.HK 9988.HK --with lists                # US restriction lists: OFAC NS-CMIC, DoD 1260H
autorekt show 0788.HK 2601.HK --vs 3033.HK --with 'beta(),upcap(),downcap()' -i 1M -r 5y --adjust
autorekt search fed december             # find ids, prediction markets and news
autorekt show SPY TLT -i 1d -r 1y --csv  # every bar of each series, for your own analysis
```

3. Answer from the numbers. Quote the values and their times, say which ones are stale or
   delayed, and keep the caveats the output prints (Yahoo and Cboe lag about 15 minutes, FRED and
   CFTC lag days, gamma assumes dealers are long calls and short puts).

## Ids

`<provider>:<symbol>[#metric]`, or an alias: `BTC`, `SPX`, `NQ`, `VIX`, `US10Y`, `DXY`, `GOLD`, `WTI`,
`SOFR`, `TGA`, `SWAP10Y`, `CGB10Y`, `SPX_GEX`. Metrics: `BTC#funding_rate`, `#open_interest`,
`#long_short_ratio`, `#liquidations`. When unsure of an id, `autorekt search <words>`; `autorekt --help`
lists the providers.

## Get woken up

Rules are expressions the daemon evaluates; a hook runs when one holds.

```sh
autorekt watch vix_inv --expr 'VIX - VIX3M > 1.5' --tf 15m --for 30m --hook 'notify.sh'
autorekt watch 'pct(BTC, "1h") < -3' --test -r 30d   # backtest before relying on a rule
autorekt watch --wait --timeout 1h                   # block until a rule fires
autorekt watch                                       # list rules and their state
```

The hook gets `AUTOREKT_MESSAGE`, `AUTOREKT_VALUE`, `AUTOREKT_CONTEXT` (recent candles) and more in
its environment. `autorekt watch --help` lists the expression functions.

## Share a report

Write markdown with chart blocks, one chart per line in `show` syntax, then publish it as a page
with live charts:

````markdown
```chart
BTC -i 1h -r 7d --overlay BTC#open_interest
VIX -i 1h -r 5d
```
````

```sh
autorekt show report.md --share            # again later: updates the same link
autorekt show report.md --share --ttl 0    # remove it
autorekt show shares                       # every live link, views, expiry
```
