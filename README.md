# autorekt

**Market observation for AI agents.** Prices across asset classes, positioning, volatility
and fear gauges, prediction-market odds, dollar liquidity, news and the macro calendar,
rendered as compact text an agent can read, plus rules that wake the agent when something
happens.

> *autorekt* (n.): an automatic liquidation device. It hands your agent every chart, funding
> rate, credit spread and prediction market there is, so it can be wrong with unprecedented
> confidence. People used to get rekt by hand. Not financial advice; if it were, it would
> have a more reassuring name.

Four verbs: `show` (read anything), `search` (find anything), `why` (explain a move, or how
assets relate), `watch` (rules, backtests, alerts). An agent can also publish its analysis as
a web page: `autorekt show report.md --share` turns markdown with ```chart blocks into a report
with live, interactive charts. Sources: OKX, Binance, Hyperliquid (incl.
24/7 equity perps), Deribit, Coinbase, Yahoo Finance, FRED, CFTC Commitments of Traders, FINRA
short volume, DefiLlama, Fear & Greed indices, Polymarket, Kalshi, news feeds and the
ForexFactory calendar. Free data only.

## What your agent sees

Real output from 2026-11-02 and 11-03 (AI-earnings selloff, a $6.3B exchange hack, a Fed
priced to hike). Lines marked `…` are trimmed.

**A chart** is one line per candle: exact numbers first, the picture only for shape, and the
stats an agent would otherwise compute. `--with` adds indicator columns (`$` is the series).
Below the candles comes the longer view: coarser bars further back, swing points and the price
zones they keep returning to, trend state on several timeframes, positioning, upcoming
releases, the most correlated instruments, and data-quality checks.

```
$ autorekt show BTC -i 4h -n 8 --with 'ta.rsi($,14)'
quotes  UTC  chg vs 24h ago (crypto) or previous close (TradFi)
id    last   chg    chg%    bid    ask    low   high  vol(quote)  time         notes
BTC  77022  -950  -1.22%  77022  77022  76360  78237        5.4B  11-03 10:51
okx:BTC/USDT:USDT (BTC)  4h  UTC  11-02 04:00..11-03 08:00  scale 76360..78676 (1 char = 74.72)
time           open   high    low  close   chg%    vol  ta.rsi($,14)  |76360                      78676|
11-02 04:00   78582  78676  77881  78072  -0.65   7.9k         47.90  |                    ---░░░░░░░░-|
11-02 08:00   78072  78354  77750  77855  -0.28   7.8k         45.75  |                   -░░░░----    |
11-02 12:00   77855  78237  77222  77486  -0.47  17.4k         42.28  |            ---░░░░░░-----      |
11-02 16:00   77486  77657  76667  77063  -0.55  15.1k         38.66  |    -----░░░░░░░--              |
11-02 20:00   77063  77197  76689  76796  -0.35   7.3k         36.53  |    --░░░░--                    |
11-03 00:00   76796  77298  76520  76909  +0.15  11.5k         38.09  |  ----██------                  |
11-03 04:00   76909  77127  76360  76796  -0.15  10.3k         37.12  |------░░---                     |
11-03 08:00*  76796  77088  76488  77022  +0.29   6.5k         40.40  |  ----████-                     |
summary  chg -1.99%  range 76360..78676 (3.03%)  ATR(7) 752 (0.98%)  last vol 0.6x avg  high@11-02 04:00 low@11-03 04:00
notable  11-03 08:00 largest gain +0.29%  |  11-02 04:00 largest drop -0.65%  |  11-02 12:00 heaviest volume 1.7x avg
(* = bar still forming)

earlier, compressed (coarser bars before the chart above)
tf  from         open   high    low  close   chg%  |74897..87374|
1w  2026-08-24  77726  81520  76664  77640  -0.11  |   -░░------         |
1w  2026-08-31  77640  82280  76204  80300  +3.43  |  --██████---        |
…
1d  2026-11-01  77521  78388  77465  78309  +1.02  |    ██-              |
1d  2026-11-02  78309  78831  76667  76796  -1.93  |   ░░░-              |

swings (1d, latest of the last year, reversals ≥ 3 ATR = 5688; last point still forming)
  … → 2026-09-21 H 87374 (+16.7%) → 2026-09-21 L 80822 (-7.5%) → 2026-09-23 H 87245 (+7.9%) → 2026-10-30 L 76260 (-12.6%)
levels (daily swing zones)
  above: 79387 (2x, last 2026-08-21, +1.2 ATR) · 80712 (2x, last 2026-09-21, +1.9 ATR) · 82540 (2x, last 2026-09-03, +2.9 ATR)
  below: 75984 (1x, last 2026-03-17, -0.5 ATR) · 75277 (2x, last 2026-09-15, -0.9 ATR) · 73620 (3x, last 2026-03-05, -1.8 ATR)
  previous day 11-02 H 78831 L 76667 · previous week H 81944 L 76260
range  1w -4.77%  1m -10.94%  3m +20.20%  6m -4.76%  1y -27.70%  ·  52w position 36%  ·  from 52w high -30.45%
state  trend 1h mixed · 4h down · 1d down  ·  RSI(14,1d) 36  ·  ATR(14,1d) 1896 (2.46%, 12th pct of 1y)  ·  20d high +4.1 ATR, low -0.4 ATR
positioning  funding 0.0001 (+0 24h)  ·  OI 2.6B (+19.3M 24h)  ·  long/short 2.75 (+0.51 24h)
events (USD, next 3d, UTC)  Tue 15:16 USD Congressional Elections
related (60d corr with the default board)  ETH +0.92 · SOL +0.86 · NQ +0.63 · ES +0.60
data quality  1 bar(s) span more than 8x the median range (daily)
```

**`why`** finds the move, measures what else moved in the same window, and pulls dated news.
Here it splits BTC's crash into two legs: after-hours tech earnings (the 24/7 Nasdaq perp
moved with it), then the exchange hack.

```
$ autorekt why BTC
why BTC  window 2026-10-29 20:00 .. 2026-10-30 00:00 UTC  move -3.53%
…
10-29 20:00  80824  80826  78233  78808  -2.49  22.9k  |      --░░░░░░░░    |
10-29 21:00  78808  79766  78533  79002  +0.25  12.7k  |       -█---        |
10-29 22:00  79002  79247  78607  78984  -0.02   7.7k  |       -░-          |
10-29 23:00  78984  79000  77896  77967  -1.29  10.1k  |     ░░░░           |
10-30 00:00  77967  78258  76739  77230  -0.95  20.4k  |--░░░░-             |
…
what else moved in the window (z = move / typical move of that length over the prior 20d):
id       change     z
ETH      -3.58%  -3.7  <- unusual
NDX247   -1.52%  -3.7  <- unusual
NQ       -1.54%  -3.3  <- unusual
COPPER   -0.51%  -1.1
DXY      -0.15%  -1.1
…
news around the window (Google News: bitcoin after:2026-10-28 before:2026-10-31):
10-29 23:06 !!! google/Bitcoin Magazine Bitcoin Price Slides Below $79,000 as Binance Loses $5.8 Billion in Massive Hack [BTC]
10-30 00:32 !!! google/CNBC    Bitcoin tumbles as Binance hack rattles crypto markets [BTC]
10-30 02:54 !!! google/TRM Labs North Korea Steals USD 6.36 Billion From Binance in the Largest Hack in History
…
```

With several ids, `why` gives the correlation matrix of their returns and the pairs that broke
from their usual relationship: the last five bars of one against what a fit on the other
implies, in standard deviations.

**Boards** put many instruments side by side. Rates and spreads move in basis points, vol
indices in points, everything else in percent.

```
$ autorekt show sectors
id      last      1h     day       5d      30d  trend                     flags
SPY   752.10  -0.29%  -0.48%   -2.62%   -2.28%  █▇▇▆█▇▇▅▆▆▄▅▅▆▄▄▄▄▅▆▅▂▃▁  market closed, below 20d low
RSP   219.98  -0.04%  +0.76%   +1.15%   +4.89%  ▃▁▁▂▃▃▃▂▄▅▄▃▅▅▆▆▆▆▆▆▇▇██  market closed
QQQ   681.09  -0.46%  -1.36%   -6.12%   -9.14%  █▇█▇▇▇▆▅▅▄▄▄▄▅▃▃▄▄▅▅▄▂▂▁  market closed, below 20d low
SMH   508.31  -0.65%  -2.71%  -10.53%  -19.39%  █▇▇▆▆▆▅▄▃▃▃▃▃▄▂▂▂▃▄▄▄▂▂▁  market closed, below 20d low
XLP    90.83  +0.10%  +1.58%   +4.57%  +12.79%  ▁▁▂▂▂▃▃▄▅▆▅▅▅▅▇▆▆▆▅▅▅▇▇█  market closed, above 20d high
…

$ autorekt show fear
id         last      1h     day      5d     30d  trend                     flags
VIX       21.15   -0.39   +1.73   +4.26   +5.84  ▄▃▂▂▁▃▅▅▅▅▄▄▃▃▂▂▂▂▁▄█▄▄▇  market closed, may be delayed, above 20d high
VIX9D     22.46   -0.03   +3.21   +7.00  +10.40  ▂▂▂▂▁▁▂▃▃▂▆▅▄▄▅▆▅▅▅▄▃▆▆█  market closed, may be delayed, above 20d high
VVIX     110.47   +0.86   +4.04  +14.37  +23.45  ▁▂▂▂▂▂▂▄▄▃█▇▅▄▅█▆▆▅▅▄█▆█  market closed, may be delayed, above 20d high
OVX       62.69   +0.06   +4.21   +4.10  +11.69  ▄▄▅▄▃▃▃▁▁▁▁▁▁▂▅▇▆▆▅▅▅▇▇█  market closed, may be delayed, above 20d high
DVOL      38.74   +0.48   -0.94   +1.38   +2.24  ▂▃▃▃▁▂▆▅▄▆███▇▇▆▅▆▇▅▄▆▆▅
CNNFNG    22.73       -   +0.00  -11.67  -15.47  ▅▆▅▄▄▆▇█▇▇██▄▅▂▄▄▂▁▅▅▄▃▂  extreme fear
…

$ autorekt show liquidity
id               last  1h  day     5d      30d     trend                     flags
SOFR           3.9500   -  +1bp    +3bp    +5bp    ▁▁▂▃▄▄▃▂▃▄▄▂▂▄▃▃▂▃▃▆▇█▇█  as of 2026-10-30, daily, published with lag
fred:IORB      3.9000   -  +0bp    +0bp    +0bp    ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄  as of 2026-11-03, daily, published with lag
RESERVES      2893770   -  -2.32%  -2.32%  -1.24%  ▇█▁▅▇█▆▆▅▃▃█▆▄▄▂▂▁▄▂▃▅▅▁  as of 2026-10-28, daily, published with lag, USD mn
HYOAS          3.4400   -  +18bp   +6bp    +32bp   ▁▁▂▃▄▅▅▅▅▄▅▅▅▅▆▇▇▇▆▇▇▆▅█  as of 2026-10-30, daily, published with lag
…
```

**Odds and positioning** from prediction markets and derivatives venues:

```
$ autorekt show kalshi:KXFEDDECISION-26DEC
…
   27.0%  #####                 Fed maintains rate  bid 26.0% ask 27.0%
   72.0%  ##############        Hike 25bps  bid 72.0% ask 73.0%
…

$ autorekt show funding BTC ETH SOL
coin  okx                  hyperliquid          deribit             binance  spread
BTC   +11.0% [0.0100%/8h]  +11.0% [0.0013%/1h]  +5.1% [0.0047%/8h]        -  5.8pp
ETH   +11.0% [0.0100%/8h]  +11.0% [0.0013%/1h]  +7.3% [0.0067%/8h]        -  3.6pp
SOL   +11.0% [0.0100%/8h]  +10.4% [0.0012%/1h]  -                         -  0.6pp
```

**Rules** are backtested before they are trusted, then the daemon wakes the agent:

```
$ autorekt watch 'pct(BTC, "1h") < -1.5' --test --tf 5m -r 14d --cooldown 6h
backtest adhoc  expr: pct(BTC, "1h") < -1.5  tf 5m  range 14d  for 0  cooldown 6h  ->  2 firing(s) over 4032 bars
time                value  BTC +1h     +4h    +24h
2026-10-22 02:15  -1.5401   +0.00%  -0.52%  -1.17%
2026-10-29 20:40  -2.4493   +0.25%  -2.00%  -2.46%
+1h: mean +0.13% median +0.25% up 2/2   +4h: mean -1.26% median -0.52% up 0/2   +24h: mean -1.81% median -1.17% up 0/2

$ autorekt watch --wait
2026-11-03 03:19 [rule/macro_releases] sev2 In 10m: AUD Cash Rate (forecast 4.85%, previous 4.60%)
```

## Install

Give your agent this prompt:

```
Install autorekt from https://github.com/autorekt/autorekt: build it with `cargo build --release`
(or download the static Linux binary from the latest build artifact), put the binary on PATH,
run `autorekt show` to check it works, and learn the commands from `autorekt --help`.
```

## License

MIT
