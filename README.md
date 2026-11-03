# autorekt

**Market observation for AI agents.** Real-time quotes across asset classes, positioning
(funding, open interest, long/short), volatility and fear gauges, prediction-market odds,
dollar-liquidity data, news and the macro calendar — rendered as compact text an agent can
read, plus rules and monitors that wake the agent when something happens.

> *autorekt*: the automatic liquidation device. It shows you everything, so you feel sure of
> yourself — and then you get liquidated. Not investment advice.

It is one Rust binary: a CLI for agents and an auto-started background daemon. It is not tied
to any agent framework: anything that can run a shell command can use it.

```
$ autorekt board fear
board fear  2026-11-02 20:32 UTC  (...)
id         last      1h      day       5d      30d  trend                     flags
VIX       20.92  +0.97%   +0.71   +4.05   +5.62   ▄▂▂▁▁▃▄▅▅▅▅▄▃▂▂▂▁▁▁▄█▄▄█  above 20d high
VIX9D     22.11  ...
DVOL      39.97  ...
CNNFNG    23.04  ...                                                        extreme fear

$ autorekt candles BTC -i 4h -n 6
okx:BTC/USDT:USDT (BTC)  4h  UTC  11-01 20:00..11-02 16:00  scale 76667..78831 (1 char = 69.81)
time           open   high    low  close   chg%    vol  |76667                      78831|
11-01 20:00   77767  78388  77556  78309  +0.70   5.1k  |       -████-                   |
11-02 00:00   78309  78831  78001  78582  +0.35   9.6k  |         --███-                 |
...
summary  chg -0.91%  range 76667..78831 (2.82%)  ATR(5) 640 (0.83%)  high@11-02 00:00 low@11-02 16:00
```

## Install

```
cargo build --release            # binary at target/release/autorekt
# or a fully static Linux binary:
docker run --rm -v $PWD:/src -w /src rust:1-alpine sh -c 'apk add musl-dev && cargo build --release'
```

Read **[GUIDE.md](GUIDE.md)** (also printed by `autorekt guide`): it is written for agents.

## What is inside

| Area | Commands | Sources |
|---|---|---|
| Overview | `brief`, `board [default\|crypto\|macro\|global\|fear\|liquidity\|rates\|commodities\|positioning\|sectors\|megacaps\|asia]`, `diff --as AGENT`, `quote`, `why ID` | all |
| Price history | `candles ID -i TF -n N`, `metric ID`, `corr` | OKX, Hyperliquid (incl. HIP-3 24/7 equity perps), Binance, Deribit, Yahoo |
| Positioning | `metric BTC#funding_rate` `#open_interest` `#long_short_ratio` `#top_long_short_ratio` `#taker_buy_sell_ratio` `#basis` `#liquidations`; `board positioning`; `COT_ES#lev_funds_net` | OKX, Binance, Hyperliquid, CFTC COT |
| Dark pool | `finra:SPY` (daily off-exchange short-volume ratio) | FINRA Reg SHO |
| Volatility & fear | `VIX VIX9D VIX3M VVIX MOVE SKEW OVX GVZ`, `DVOL`, `deribit:BTC#skew_10pct`, `FNG`, `CNNFNG` (+components) | Yahoo, Deribit, alternative.me, CNN |
| Liquidity & macro | `board liquidity`, any `fred:SERIES`, `calendar` | FRED, ForexFactory |
| Prediction markets | `prob search\|show\|trending` | Polymarket, Kalshi |
| Pre-launch perps | `prelaunch` | OKX pre-market, Hyperliquid HIP-3 |
| News | `news [--about Q] [--since 2h] [--min-severity N]` | 12 RSS feeds + Google News |
| Watching | `rule add\|test\|ls\|check`, `wait`, `events`, hooks | daemon |
| Custom logic | `monitor new\|ls\|logs` (Python SDK) | daemon |
| Showing humans | `share ID` → public chart URL | daemon + cloudflared |

## Design

- **Unified types**: OHLCV bars, tickers, metrics, prediction-market outcomes and events. Every
  provider maps onto them, so every command works the same for every source.
- **Familiar conventions**: CCXT symbols and timeframes (`okx:BTC/USDT:USDT`, `1h`), provider-native
  ids elsewhere (`yahoo:^VIX`, `fred:DGS10`, `kalshi:<ticker>`), Pine Script–style expressions
  (`ta.crossover`, `ta.sma`), Prometheus-style rules (`for`, cooldown, templates), a
  Nautilus/LEAN-style actor SDK for monitors, pandas-ready CSV.
- **Text first**: one line per unit, exact numbers before pictures, headers that state scope,
  footers with the stats an agent would otherwise compute.
- **Observe only**: no orders, no keys, no portfolio.

`$AUTOREKT_HOME` (default `~/.autorekt`) holds `config.yaml`, `aliases.yaml`, `rules/`,
`monitors/`, `autorekt.db` (events, recorded series, cursors) and `logs/`.

### config.yaml

```yaml
default_exchange: okx            # where bare crypto aliases resolve (binance outside the US)
proxy: http://127.0.0.1:7890     # optional, for all requests
base_urls: { binance_futures: https://fapi.binance.com }
boards: { mine: [BTC, ETH, SPX, VIX, "fred:SOFR - fred:IORB"] }
record:                          # sample metrics that have no public history
  - { id: "hyperliquid:BTC#open_interest", every: 5m }
  - { id: "deribit:BTC#skew_10pct", every: 15m }
default_hook: "notify-send autorekt \"$AUTOREKT_MESSAGE\""
hook_chart_url: true             # pass AUTOREKT_CHART_URL (a shared chart) to hooks
share_base_url: https://charts.example.com   # if you run your own tunnel / reverse proxy
```

## License

MIT
