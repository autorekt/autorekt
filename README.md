# autorekt

**Market observation for AI agents.** Prices across asset classes, positioning, volatility
and fear gauges, prediction-market odds, dollar liquidity, news and the macro calendar,
rendered as compact text an agent can read, plus rules that wake the agent when something
happens.

> *autorekt*: the automatic liquidation device. It shows you everything, so you feel sure of
> yourself, and then you get liquidated. Not investment advice.

```
$ autorekt show fear
board fear  2026-11-02 20:32 UTC
id         last      1h     day      5d     30d  trend                     flags
VIX       20.92   +0.20   +1.48   +4.05   +5.62  ▄▂▂▁▁▃▄▅▅▅▅▄▃▂▂▂▁▁▁▄█▄▄█  above 20d high
VIX9D     22.11   +0.02   +2.86   +6.65  +10.05  ▂▂▁▂▁▁▂▂▄▃▆▅▄▄▆▇▆▅▅▄▃▆▅█  above 20d high
DVOL      39.97   +0.08   -0.70   +2.64   +4.62  ▃▃▃▃▄▁▂▄▅▃▇█▇██▇▆▆▇█▆▄▆▇
CNNFNG    23.04       -   -3.28  -11.33  -15.13  ▅▅▆▄▄▄▆█▇▆▇█▄▅▁▄▄▂▁▅▅▄▃▂  extreme fear
```

Four verbs:

| | |
|---|---|
| `show` | the briefing, boards, charts, odds, news, calendar, alerts |
| `search` | ids, tickers, prediction markets, news |
| `why` | what explains a move; how assets relate |
| `watch` | rules, backtests, waiting for alerts, Python monitors |

Sources: OKX, Binance, Hyperliquid (incl. 24/7 equity perps), Deribit, Coinbase, Yahoo
Finance, FRED, CFTC Commitments of Traders, FINRA short volume, DefiLlama, Fear & Greed
indices, Polymarket, Kalshi, news feeds and the ForexFactory calendar. Free data only.

## Install

Give your agent this prompt:

> Install autorekt from https://github.com/autorekt/autorekt: build it with `cargo build --release`
> (or download the static Linux binary from the latest build artifact), put the binary on PATH,
> run `autorekt show` to check it works, and learn the commands from `autorekt --help`.

## License

MIT
