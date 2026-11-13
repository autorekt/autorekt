# autorekt

English | [简体中文](README.zh-CN.md)

**Make your AI a Wall Street analyst.**

Models are already geniuses in a data center. In markets, what holds them back is what they can see.
autorekt hands your agent the whole tape: prices across every asset class, dollar liquidity, swap
spreads, positioning, options gamma, China's money markets, prediction-market odds, news and the
macro calendar, as compact text it can read. So you and your agent can follow global capital as it
moves, and get woken when something happens.

<details>
<summary>The briefing an agent reads before it forms a view (<code>autorekt show</code>, full output)</summary>

```
$ autorekt show
briefing  2026-11-05 05:26 UTC  — numbers first; interpret with the caveats at the end

## 1. Cross-asset
id        last  1h      day     5d       30d      trend                     flags
BTC      80405  -0.10%  +3.87%  +4.14%   -5.97%   ▂▂▂▂▃▃▃▃▃▃▄▄▃▂▁▁▂▃▂▃▄███
ETH     2421.6  -0.08%  +6.15%  +8.07%   -10.20%  ▁▁▁▁▁▂▂▂▂▂▂▃▃▂▂▁▂▃▂▃▄███
SOL      95.81  -0.20%  +3.69%  -0.03%   -20.60%  █▅█▇▇▇▆▇▆▅▅▆▄▂▁▁▂▃▃▃▄▇▇▇
HYPE     83.52  -0.34%  -0.92%  -5.46%   -9.13%   █▇▆▅▄▅▅▅▅▅▅▅▃▂▂▁▂▄▂▂▄▂▁▁
SPX     7688.6  -0.18%  +1.58%  +1.42%   -1.10%   ▇████▅▆▅▆▆▅▇▄▃▅▆▆▆▅▁▃▂▄▅  market closed, may be delayed
NDX      28437  -0.17%  +0.91%  +0.18%   -8.49%   ██▇▇▇▅▅▄▅▅▄▅▃▃▄▅▅▅▄▁▂▁▂▂  market closed, may be delayed
ES      7703.0  -0.28%  -0.23%  +1.17%   -2.17%   ████▇██▆▅▅▃▄▅▄▄▂▁▂▃▁▃▆▇▆  may be delayed
NQ       28262  -0.94%  -1.07%  -0.95%   -10.23%  ███▇▇▇▇▅▅▄▃▄▄▄▃▂▂▂▃▁▂▃▃▂  may be delayed
VIX      16.61  -0.17   -4.22   -2.83    +1.09    ▄▃▅▆▆▅▃▃▂▂▂▁▂▁▄▅▅▄██▆▃▂▁  market closed, may be delayed
MOVE     93.89  +0.00   -6.09   -6.29    -19.71   █▆▅▄▅▄▆▅▆▆▆▃█▆▇▅▁▂▁▄▅▅▂▁  market closed, may be delayed
US10Y   4.8940  +1bp    -6bp    -3bp     -42bp    ██▇▇▇▇▅▅▅▅▅▅▃▃▂▃▃▄▃▂▁▂▂▁  market closed, may be delayed, below 20d low
DXY     101.24  -0.24%  -0.26%  -0.71%   -0.58%   █▇▇▆▇▇▅▅▄▃▅▅▄▃▄▄▄▄▅▂▂▂▃▁  below 20d low
USDJPY  157.81  -0.08%  -0.42%  +0.18%   -0.31%   ▄▆▃▃▄▄▃▂▁▂▄▄▄▄▄▄▆█▇▅▆▇▅▄
GOLD    4142.6  +0.54%  +1.71%  +6.25%   -1.06%   ▆▆▅▆▅▅▅▅▄▃▁▁▁▁▃▃▁▃▂▂▅▅▆█  may be delayed
SILVER   52.94  +0.43%  +1.56%  +7.43%   -13.46%  ██▇█▇▆▆▅▄▃▁▂▃▂▃▃▁▃▂▁▅▅▆▆  may be delayed
COPPER  6.4160  +0.33%  +0.20%  -1.02%   -2.71%   ▇▇▇█▇▆▇▆▆▅▄▅▅▅▆▅▂▄▂▁▃▃▃▃  may be delayed
WTI      86.92  -1.27%  -1.36%  -16.52%  -2.82%   ▇▇▇▇▇▇▆████▇▇▆▅▅▅▅▅▄▅▄▂▁  may be delayed, below 20d low
HYG      77.52  -0.02%  +0.70%  +0.13%   +0.70%   ▃▄▄▅▃▁▃▂▂▂▁▂▃▂▄▆██▇▅▃▂▅▇  market closed
NIKKEI   61242  +0.13%  -1.52%  -2.92%   -13.36%  █▇▇▇▆▆▄▄▃▄▃▄▃▂▂▃▃▃▄▃▃▁▂▁  may be delayed, below 20d low
HSI      24613  -0.19%  +2.90%  +5.66%   +1.37%   ▆▅▆▆▆▇▆▆▆▅▄▁▁▁▂▁▂▂▃▃▃▃▅█  may be delayed, above 20d high
DAX      26438  -0.27%  +0.00%  +2.40%   +4.69%   ▂▂▄▄▂▁▂▃▂▃▁▂▃▃▃▄▅▆▅▅▄▅▇█  market closed, may be delayed, above 20d high
FNG      50.00  -       +16.00  +20.00   -23.00   █▇▆▆█▆▇▇▆▅▆▇█▅▅▅▄▅▅▂▂▃▁▅  neutral
CNNFNG   27.31  -       +0.00   +0.99    -19.71   ▅▄▄▆▇█▇▇██▄▁▂▄▄▁▄▅▅▃▂▃▃▃  fear
DVOL     37.76  -0.27   -0.99   -2.84    +1.63    ▅████▇▇▆▅▇▇▃▄▇▅▅▆▇▆▄▁▃▃▂
top movers (day): ETH +6.15%, BTC +3.87%, SOL +3.69%, HSI +2.90%, GOLD +1.71%, SPX +1.58%

## 2. Volatility & sentiment
id                          last     1h     day      5d     30d  trend                     flags
VIX9D                      15.19  -0.23   -6.77   -4.48   +2.34  ▁▁▁▁▁▃▃▃▄▄▄▃▅▆▅▄▄▃▃█▆▇▄▃  market closed, may be delayed
VIX                        16.61  -0.17   -4.22   -2.83   +1.09  ▄▃▅▆▆▅▃▃▂▂▂▁▂▁▄▅▅▄██▆▃▂▁  market closed, may be delayed
VIX3M                      18.80  -0.06   -2.47   -2.11   +0.80  ▁▁▁▁▁▂▂▂▃▃▃▂▅▅▄▃▃▃▃█▅▆▃▂  market closed, may be delayed
VVIX                       90.41  -0.25  -16.99  -17.03   +4.95  ▁▁▁▁▁▃▂▃▄▄▃▃▅▆▄▄▃▃▃█▅▅▃▂  market closed, may be delayed
MOVE                       93.89  +0.00   -6.09   -6.29  -19.71  █▆▅▄▅▄▆▅▆▆▆▃█▆▇▅▁▂▁▄▅▅▂▁  market closed, may be delayed
OVX                        58.81  -0.00   -2.88   +0.33  +10.16  ▅▄▃▃▃▂▁▁▂▁▁▄▅▇▇▆▅▆▅▇██▆▆  market closed, may be delayed
DVOL                       37.76  -0.27   -0.99   -2.84   +1.63  ▅████▇▇▆▅▇▇▃▄▇▅▅▆▇▆▄▁▃▃▂
CNNFNG                     27.31      -   +0.00   +0.99  -19.71  ▅▄▄▆▇█▇▇██▄▁▂▄▄▁▄▅▅▃▂▃▃▃  fear
FNG                        50.00      -  +16.00  +20.00  -23.00  █▇▆▆█▆▇▇▆▅▆▇█▅▅▅▄▅▅▂▂▃▁▅  neutral
SPX_GEX                    53.96      -   +0.00       -       -  ▄                         USD bn per 1% move
cboe:SPX#skew_25d         4.5500      -   +0.00       -       -  ▄
cboe:SPX#put_call_volume  1.2169      -   +0.00       -       -  ▄
cboe:SPX#zero_dte_pct      65.18      -   +0.00       -       -  ▄
VIX curve: 9D 15.19 / 30D 16.61 / 3M 18.80 → contango (normal)
SPX dealer gamma +54.0 $bn per 1% move → dealers dampen moves

## 3. Dollar liquidity (NY Fed same day; FRED, DTCC and Treasury lag 1-5 days)
nyfed:SOFR - fred:IORB               +0.070  (1w ago +0.040)  ▃▂▁▂▂▃▂▁▁▁▂▂▂▁▂▃▄▅▇▆▇▇██  repo above IORB: reserves tight
nyfed:SOFR#p99 - fred:IORB           +0.150  (1w ago +0.120)  ▄▂▂▂▂▃▂▁▁▁▂▁▁▁▂▃▅▅▇▆▇▇██  repo above IORB: reserves tight
(dtcc:SOFR10Y - fred:DGS10) * 100    -41.642  (1w ago -40.800)  ▆▄▆▃▄▆▅▇▇▄▁▆▇▅█▅▃▄█▃▄▆▅▃  swap spread, bp: more negative = dealers' balance sheets strained by Treasury supply
(dtcc:SOFR30Y - fred:DGS30) * 100    -67.100  (1w ago -67.900)  ▂▁▂▂▁▂▂▃▃▁▃▅▄▄▇▄▄▇█▄▃▅▄▃  swap spread, bp: more negative = dealers' balance sheets strained by Treasury supply
id              last  1h  day     5d        30d      trend                     flags
SRF             0.00   -  -0.0bn  +0.0bn    -0.0bn   ▁▁▁▁█▁▁▁▁▁▁▁▂▁▁▁▁▁▁▁▁▁▁▁  as of 2026-11-04, daily, same day, USD bn
RRP          0.22500   -  -0.2bn  -4.2bn    -0.8bn   ▁▁▁██▁▂▁▂▁▁▁▂▁▁▁▁▁▁▁▁▄▄▁  as of 2026-11-04, daily, published with lag, USD bn
RESERVES     2893770   -  -2.32%  -2.32%    -1.24%   ▇█▁▅▇█▆▆▅▃▃█▆▄▄▂▂▁▄▂▃▅▅▁  as of 2026-10-28, daily, published with lag, USD mn, no new bar for 7 days
TGA           938.21   -  +7.5bn  -102.0bn  +67.0bn  ▄▄▃▄▃▅▁▁▁▁▁▃▁▃▄▅▄▄▅█▆▇▄▃  as of 2026-11-03, daily, one business day behind, USD bn
DEALER_UST    425643   -  -4.03%  -4.03%    -6.36%   ▄▂█▅▆▅▇▂▄▃▁               as of 2026-10-21, weekly, about ten days behind, USD mn, no new bar for 14 days
HYOAS         3.4000   -  -3bp    +14bp     +30bp    ▁▂▃▄▅▆▅▄▅▅▅▅▆▆▇▇▆█▇▅▅▆██  as of 2026-11-03, daily, published with lag
IGOAS        0.93000   -  -1bp    +4bp      +8bp     ▁▂▃▄▄▅▄▃▃▃▃▃▄▄▄▅▆▆▇▆▅▆██  as of 2026-11-03, daily, published with lag
NFCI        -0.45900   -  +0.01   +0.01     +0.05    ▇▆▅▄▄▄▄▄▄▄▄▃▂▂▁▁▁▂▂▄▅▆▇█  as of 2026-10-30, daily, published with lag, no new bar for 5 days
STABLES         295B   -  -0.17%  -1.45%    -4.27%   ██▇▇▇▇▇▇▆▆▆▆▆▆▅▅▅▅▄▃▃▃▂▁  daily, USD

## 4. Global rates, China and Hong Kong (fed funds futures = implied average rate per month)
id                last     1h      day     5d       30d      trend                     flags
US10Y             4.8940   +1bp    -6bp    -3bp     -42bp    ██▇▇▇▇▅▅▅▅▅▅▃▃▂▃▃▄▃▂▁▂▂▁  market closed, may be delayed, below 20d low
SWAP10Y           4.5336   -       -1bp    -2bp     -39bp    ▄▅▆█▇▇▆██▇▇▇▄▄▅▅▃▂▂▃▂▂▁▁  as of 2026-11-04, %, daily median of reported trades, previous business day
EA10Y             3.3466   -       +2bp    -3bp     -12bp    ▆▇██▇▇▄▅▆▆▅▅▅▂▄▃▃▂▂▂▂▂▁▁  as of 2026-11-03, %, daily, published next morning
JGB10Y            2.8550   -       +4bp    +4bp     -23bp    ▅▅▇▇▇▇█▇██▇▆▆▄▄▄▄▃▁▁▂▂▁▂  as of 2026-11-04, %, daily, Tokyo business days
JGB30Y            3.9790   -       +4bp    +7bp     -19bp    ▅▅▇▇▇▆▇██▇▇▅▆▄▅▃▃▂▁▁▂▃▁▃  as of 2026-11-04, %, daily, Tokyo business days
CGB10Y            1.6077   -       -1bp    -4bp     -7bp     ▇▇▇▇▇▇▆███▆▅▅▄▅▄▄▄▄▃▂▅▃▁  as of 2026-11-04, %, daily, after the close
CNYFIX            6.7156   -       -0.04%  +0.03%   -0.29%   █▇▇▆▇▅▅▅▄▄▄▃▂▂▂▂▂▁▂▁▁▁▁▁  as of 2026-11-05, daily, Beijing business days
CNH_FIX_GAP_PIPS  -486.90  -3.40   -42.20  +135.80  -153.20  ▆▅▅▅▅▄▄▄▃▄▃▁▂▂▅██▇▆█▇█▇▆  expr
SHIBOR3M          1.3980   -       -0bp    -0bp     -3bp     ██████████████▆▃▃▃▃▂▂▂▂▁  as of 2026-11-05, daily, Beijing business days
CN_MARGIN         2300.4   -       +0.07%  -1.82%   -10.48%  ██████▇▆▆▆▆▆▅▅▅▅▄▄▃▃▃▂▂▁  as of 2026-11-04, daily, after the close, CNY bn
SOUTHBOUND        -3.2772  -       -26.70  +0.28    -10.14   ▄▅▅▅█▅▅▄▆▆▄▄▄▂▃▃▁▃▃▄▃▃▃▃  as of 2026-11-04, daily, after the close, CNY bn
HIBOR             ERR                                                                  HTTP 405 from api.hkma.gov.hk
HK_AGGBAL         ERR                                                                  HTTP 405 from api.hkma.gov.hk
FF_NOV26          3.8825   +0.06%  +0.06%  +0.06%   -1.21%   █▇▄▆▆▆▂▂▁▁▁▁▁▁▁▂▁▁▁▂▁▁▁▂  expr
FF_DEC26          4.0250   +0.00%  +0.00%  +0.50%   -1.23%   ▇▇▇█▇▅▅▅▃▁▂▂▁▂▃▂▃▃▃▃▃▄▃▄  expr
FF_JAN27          4.0950   +0.12%  +0.00%  +0.74%   -1.21%   ▇▇▇█▆▅▆▅▂▁▂▂▂▃▃▂▂▃▃▂▃▃▅▅  expr

## 5. Crypto positioning (OKX perps, Deribit options, US spot ETF flows)
id                                  last  1h       day      5d           30d  trend                     flags
BTC#funding_rate              0.00010000  +0.25bp  +0.12bp  +0.00bp        -  ██▃▃▁▄▆▂▄▁▄▃▆█▃▅█▅██▅▆██
BTC#open_interest                   2.7B  +0.01%   -0.17%   +0.27%    +4.49%  ▆▆▆▆▄▅▄▃▃▂▃▃▃▄▂▂▁▁███▆▅▅
BTC#long_short_ratio              1.5200  +0.01    -0.80    -1.25      +0.54  ▆▇█▇▇▇▇▆▆▅▅▅▅▆▇▇▇▆▅▅▃▁▁▁
BTC#top_long_short_ratio          1.0913  +0.01    -0.41    -0.53      +0.23  ▆▅▅▅▅▄▄▄▅▆▇██▇▆▆▅▇▄▂▁▁▁▁
BTC#taker_buy_sell_ratio          1.3096  +0.33    +0.24    +0.43      +0.27  ▇▇▅▃▃▇▃▄▂▄▄▃▅▁▃▅▄▆▃▄▂█▃▇  above 20d high
ETH#funding_rate             0.000048885  -0.25bp  -0.17bp  +1.35bp        -  ▆▇▅▅▄▇▇▅▅▄▅▄▆▅▇▆▇█▅█▁▄▇▆
deribit:BTC#skew_10pct            2.8500  -        +0.00    +0.00          -  ▄
deribit:BTC#put_call_volume      0.44768  -        +0.00    +0.00          -  ▄
CB_PREMIUM_BP                     -10.46  +0.97    -0.57    +3.49      -8.81  ▃▃▂▅▄▅▄▄▂▃▄▃▄▃▁▁▂▅▃▄▅█▇▇  expr
BTC_ETF_FLOW                       74.60  -        +190.20  +717.80  +164.40  ▆▆▅▄▅▆▄▂▃▅▄▃▅▄██▁▄▆▆▅▁▂▅  as of 2026-11-04, daily net flow, USD mn
ETH_ETF_FLOW                       53.00  -        +138.30  +208.70  +103.80  ▇█▆▄▄▄▄▁▃▄▃▂▄▄▃▆▂▆▃▅▃▁▂▇  as of 2026-11-04, daily net flow, USD mn
BTC liquidations collected (OKX, since daemon start, last 24h): $39.2M

## 6. Key odds
kalshi:KXFEDDECISION-26DEC  Fed decision in Dec 2026?  [vol24h 155k, ends 2026-12-09]
    1.0%                        Cut >25bps  bid 0.0% ask 1.0%
    1.0%                        Cut 25bps  bid 1.0% ask 2.0%
   21.0%  ####                  Fed maintains rate  bid 20.0% ask 21.0%
   81.0%  ################      Hike 25bps  bid 80.0% ask 81.0%
    1.0%                        Hike >25bps  bid 1.0% ask 2.0%

## 7. Calendar (high impact, -1d..+3d)
Thu 11-05 12:00 GBP  BOE Monetary Policy Report
Thu 11-05 12:00 GBP  Monetary Policy Summary
Thu 11-05 12:00 GBP  MPC Official Bank Rate Votes  (forecast 7-0-2, previous 3-0-6)
Thu 11-05 12:00 GBP  Official Bank Rate  (forecast 4.00%, previous 3.75%)
Thu 11-05 12:30 GBP  BOE Gov Bailey Speaks
Fri 11-06 13:30 CAD  Employment Change  (forecast 14.6K, previous -68.3K)
Fri 11-06 13:30 CAD  Unemployment Rate  (forecast 6.5%, previous 6.5%)
Fri 11-06 13:30 USD  Average Hourly Earnings m/m  (forecast 0.3%, previous 0.1%)
Fri 11-06 13:30 USD  Non-Farm Employment Change  (forecast 65K, previous 29K)
Fri 11-06 13:30 USD  Unemployment Rate  (forecast 4.2%, previous 4.2%)
Fri 11-06 16:00 JPY  BOJ Gov Ueda Speaks
Fri 11-06 21:45 NZD  Parliamentary Elections

## 8. Headlines (severity ≥2, last 12h)
11-05 04:27 !!  bloomberg      China Bank Funding Costs Hit Record Low on PBOC Rate Shift
11-05 03:01 !!  ft             Oil tumbles below $100 on post-election peace hopes [WTI]
11-05 01:51 !!! cointelegraph  India warns AI-powered hackers targeting crypto exchanges at ‘machine speed’
11-04 23:44 !!! bloomberg      Australia Chain David Jones Collapses After Almost 200 Years
11-04 23:18 !!  cnbc           Jim Cramer warns investors may be reading too much into Wall Street's post-election rally [SPX]
11-04 20:46 !!! decrypt        Coinbase Says 99% of Customer Crypto Is Offline and Out of Hackers’ Reach
11-04 20:14 !!  cnbc           Crude oil prices fall below $90 a barrel as Vance blames GOP election defeat on Iran war [WTI]
11-04 20:09 !!! theblock       Bitcoin rebounds past $80,000 as markets shrug off midterms and Binance hack [BTC]
11-04 19:37 !!  cnbc           Hyperscalers and Apple surge after election results remove investor uncertainty
11-04 18:02 !!  cointelegraph  Circle’s ARC token sale lifts earnings as lower yields weigh on USDC income [US10Y]

caveats: Yahoo and Cboe are ~15m delayed; SPX gamma assumes dealers long calls and short puts; FRED lags; swap rates are medians of reported trades; crypto positioning is OKX-only; headlines are keyword-scored. Drill down with candles/metric/news --about.
```

</details>

> *autorekt* (n.): an automatic liquidation device. It hands your agent every chart, funding rate,
> credit spread and prediction market there is, so it can be wrong with unprecedented confidence.
> People used to get rekt by hand. Not financial advice; if it were, it would have a more
> reassuring name.

## Install

Give your agent this prompt:

```text
Install autorekt (https://autorekt.com, source at https://github.com/autorekt/autorekt).
1. Install the prebuilt binary: on Linux or macOS run `curl -fsSL https://autorekt.com/install.sh | sh`; on Windows run `irm https://autorekt.com/install.ps1 | iex` in PowerShell, or `curl -fsSL https://autorekt.com/install.cmd -o install.cmd && install.cmd && del install.cmd` in CMD.
2. If that fails, build from source: clone the repository, run `cargo build --release` and put the binary on PATH. Use the Rust toolchain already on this machine if it is new enough. Install a missing or outdated Rust toolchain, C compiler or other build dependency the way a person would on this system, with the official installer or the system package manager (for example rustup from https://rustup.rs, build-essential, the Xcode command line tools, Visual Studio Build Tools).
3. Debug any problem until everything works, changing the code if you must, and tell me what you changed.
4. Check it works with `autorekt show`, then learn the commands from `autorekt --help`.
```

- **Nothing leaves your machine** No account, no API keys, no telemetry. Data comes straight from the public sources, and a Cloudflare tunnel opens only when you share a page.
- **Easy to check** Every address autorekt contacts is written out in the source.
- **Audit first, if you like** The prompt below has your agent read the whole repository and report every risk before it builds. Slower, and it uses many tokens.

```text
Before installing autorekt, audit it. Clone https://github.com/autorekt/autorekt and read the whole repository: the Rust source, build scripts, the dependencies in Cargo.toml and Cargo.lock, the install scripts in site/install/ and the GitHub workflows. Find everything risky: which network addresses it contacts and what it sends; which files it reads or writes outside its own directory (~/.autorekt); which commands it runs (hooks, monitors, the downloaded cloudflared); what the local HTTP server exposes; the public tunnel; and dependencies that are unusual or unmaintained. Check in particular the claim that it never contacts a server run by its authors. Report each risk to me with its file and line, how serious it is and whether it can be turned off, then wait for my go-ahead. Then build from the audited source with `cargo build --release` (installing a Rust toolchain the usual way if needed), put the binary on PATH, and check it with `autorekt show`.
```

Agents that load skills, such as Claude Code, can also take
[skills/autorekt/SKILL.md](https://github.com/autorekt/autorekt/blob/main/skills/autorekt/SKILL.md), which tells
them when and how to reach for autorekt. autorekt observes; it never trades.

## The bottleneck is data, not intelligence

Ask a model why the market moved and it reaches for stale training data and a couple of headlines.
A good analyst does not guess. Before forming a view they check rates and funding, flows and
positioning, options and odds, and what else moved at the same time. That data is scattered
across dozens of sources, each with its own format, units and publication lag, and most of it
never makes the news.

autorekt gathers it into one consistent view built for models: exact numbers first, how fresh
every number is, and the statistics an analyst would compute anyway. Questions your agent can
answer with data instead of guesses (numbers from November 5, 2026):

- **Is dollar liquidity tightening?** Overnight repo trades 7bp above the rate the Fed pays on reserves, its 99th percentile 15bp above; 10-year swap spreads sit at −42bp; the Treasury's cash balance, the standing repo facility and primary dealer positions sit alongside. `autorekt show liquidity`
- **Is Beijing leaning on the yuan?** Offshore yuan trades about 490 pips stronger than the PBoC's daily fixing, next to SHIBOR, China's bond yields, margin debt and southbound flows. `autorekt show asia`
- **Will dealers calm the next move or feed it?** SPX dealer gamma is +$54bn per 1% move, 25-delta skew 4.6 vol points, and 65% of the session's SPX option volume was in same-day expiries. `autorekt show cboe:SPX#gex`
- **What is priced for the Fed?** Kalshi gives 80% to a December hike; fed funds futures imply 3.88% for November and 4.03% for December. `autorekt show kalshi:KXFEDDECISION-26DEC`
- **Why did that just move?** Ether rose 5.3% in four hours on November 4 while oil fell 6.4% (6 standard deviations), the S&P 500 rose 1.8% and high yield rallied: a macro risk-on move, not crypto news. `autorekt show ETH --view explain`
- **Who is buying?** Spot ETF flows, futures positioning by trader type, off-exchange short volume, crypto funding and long/short ratios. `autorekt show positioning`

## What your agent sees

Real output, unedited. Each view leads with numbers, uses a picture only for shape, and ends
with the context an agent would otherwise have to fetch itself.

### A chart

One line per candle with exact numbers, then the longer view: coarser bars further back, swing
points and the price zones they keep returning to, trend state on several timeframes,
positioning, upcoming releases, the most correlated instruments and data-quality checks.
`--with` adds indicator columns in Pine Script style, where `$` is the series.

<details>
<summary><code>autorekt show BTC -i 4h -n 8 --with 'ta.rsi($,14)'</code></summary>

```
$ autorekt show BTC -i 4h -n 8 --with 'ta.rsi($,14)'
quotes  UTC  chg vs 24h ago (crypto) or previous close (TradFi)
id    last    chg    chg%    bid    ask    low   high  vol(quote)  time         notes
BTC  80400  +2953  +3.81%  80400  80400  77334  81200        9.2B  11-05 05:20
okx:BTC/USDT:USDT (BTC)  4h  UTC  11-04 00:00..11-05 04:00  scale 75690..81200 (1 char = 177.74)
time           open   high    low  close   chg%    vol  ta.rsi($,14)  |75690                      81200|
11-04 00:00   77439  77530  75690  76450  -1.28  29.2k         35.38  |----░░░░░░░                     |
11-04 04:00   76450  78346  76389  78105  +2.16  27.3k         51.70  |    ███████████-                |
11-04 08:00   78105  78691  78105  78440  +0.43  11.7k         54.22  |              ██--              |
11-04 12:00   78440  81200  77976  80274  +2.34  53.1k         64.99  |             --████████████-----|
11-04 16:00   80274  80700  79903  79952  -0.40  14.0k         62.22  |                        ░░░--   |
11-04 20:00   79952  80941  79915  80466  +0.64   9.5k         64.80  |                        ████--- |
11-05 00:00   80466  80886  80215  80518  +0.06   8.6k         65.06  |                         --█--  |
11-05 04:00*  80518  80709  80336  80400  -0.15   2.2k         63.90  |                          ░░-   |
summary  chg +3.82%  range 75690..81200 (7.28%)  ATR(7) 1233 (1.53%)  last vol 0.1x avg  high@11-04 12:00 low@11-04 00:00
notable  11-04 12:00 largest gain +2.34%  |  11-04 00:00 largest drop -1.28%  |  11-04 12:00 heaviest volume 2.7x avg
(* = bar still forming)

earlier, compressed (coarser bars before the chart above)
tf  from         open   high    low  close   chg%  |74897..87374|
1w  2026-08-24  77726  81520  76664  77640  -0.11  |   -░░------         |
1w  2026-08-31  77640  82280  76204  80300  +3.43  |  --██████---        |
1w  2026-09-07  80300  80405  75866  76794  -4.37  |  -░░░░░░░           |
1w  2026-09-14  76793  81930  74897  81144  +5.67  |---████████-         |
1w  2026-09-21  81144  87374  80822  84441  +4.06  |         -██████-----|
1w  2026-09-28  84441  87239  82501  86485  +2.42  |            ---█████-|
1w  2026-10-05  86485  86964  80351  83716  -3.20  |         -----░░░░░░ |
1w  2026-10-12  83716  84348  79725  82070  -1.97  |        ---░░░░-     |
1d  2026-10-19  82070  83150  81682  81910  -0.19  |           ░--       |
1d  2026-10-20  81910  83437  81166  82984  +1.31  |          -███-      |
1d  2026-10-21  82985  84876  82588  84050  +1.28  |            -███-    |
1d  2026-10-22  84050  84320  80900  81381  -3.18  |          ░░░░░░     |
1d  2026-10-23  81381  82048  80457  81150  -0.28  |         -░-         |
1d  2026-10-24  81150  81176  79168  80850  -0.37  |       ---░          |
1d  2026-10-25  80850  81008  79773  80119  -0.90  |        ░░░          |
1d  2026-10-26  80118  80823  79323  80125  +0.01  |       -█-           |
1d  2026-10-27  80125  81279  79478  80881  +0.94  |       -███          |
1d  2026-10-28  80881  81787  80350  81310  +0.53  |         -█-         |
1d  2026-10-29  81310  81944  77896  77967  -4.11  |     ░░░░░░-         |
1d  2026-10-30  77967  78767  76260  77287  -0.87  |  --░░-              |
1d  2026-10-31  77287  77734  77050  77521  +0.30  |   -█-               |
1d  2026-11-01  77521  78388  77465  78309  +1.02  |    ██-              |
1d  2026-11-02  78309  78831  76667  76796  -1.93  |   ░░░-              |
1d  2026-11-03  76796  78049  76360  77439  +0.84  |  -██-               |

swings (1d, latest of the last year, reversals ≥ 3 ATR = 5950.3; last point still forming)
  2026-08-19 L 64142 → 2026-08-21 H 79603 (+24.1%) → 2026-08-21 L 72984 (-8.3%) → 2026-09-03 H 82280 (+12.7%) → 2026-09-15 L 74897 (-9.0%) → 2026-09-21 H 87374 (+16.7%) → 2026-09-21 L 80822 (-7.5%) → 2026-09-23 H 87245 (+7.9%) → 2026-11-04 L 75690 (-13.2%)
levels (daily swing zones)
  above: 80712 (2x, last 2026-09-21, +0.2 ATR) · 82540 (2x, last 2026-09-03, +1.1 ATR) · 83770 (3x, last 2026-01-31, +1.7 ATR)
  below: 79387 (2x, last 2026-08-21, -0.5 ATR) · 75984 (1x, last 2026-03-17, -2.2 ATR) · 75277 (2x, last 2026-09-15, -2.6 ATR)
  previous day 11-04 H 81200 L 75690 · previous week H 81944 L 76260
range  1w +3.12%  1m -5.98%  3m +25.04%  6m +0.53%  1y -22.57%  ·  52w position 46%  ·  from 52w high -25.21%
state  trend 1h up · 4h up · 1d mixed  ·  RSI(14,1d) 50  ·  ATR(14,1d) 1983.4 (2.47%, 18th pct of 1y)  ·  20d high +2.3 ATR, low -2.4 ATR
positioning  funding 0.0001 (+0.000012447 24h)  ·  OI 2.7B (-3.6M 24h)  ·  long/short 1.52 (-0.8 24h)
events (USD, next 3d, UTC)  Fri 13:30 USD Average Hourly Earnings m/m · Fri 13:30 USD Non-Farm Employment Change · Fri 13:30 USD Unemployment Rate
related (60d corr with the default board)  ETH +0.92 · SOL +0.86 · ES +0.64 · NQ +0.63
data quality  1 bar(s) span more than 8x the median range (daily)
```

</details>

### Why did it move?

`--view explain` finds the move, scores how unusual every other market's move was in the same window, and
puts dated news next to it. A move shared by many assets is macro; a move alone is idiosyncratic.

<details>
<summary><code>autorekt show ETH --view explain</code></summary>

```
$ autorekt show ETH --view explain
explain ETH  window 2026-11-04 13:00 .. 2026-11-04 17:00 UTC  move +5.25%
okx:ETH/USDT:USDT  1h  UTC  11-04 09:00..11-04 18:00  scale 2308.5..2451.1 (1 char = 4.6023)
time     open    high     low   close   chg%    vol  |2308.5                    2451.1|
09:00  2322.4  2323.9  2314.5  2321.6  -0.03  82.0k  | --░                            |
10:00  2321.6  2334.7  2320.0  2327.4  +0.25   117k  |   ██--                         |
11:00  2327.4  2338.9  2325.2  2326.8  -0.03   112k  |    ░---                        |
12:00  2326.7  2328.5  2308.5  2312.7  -0.60   170k  |-░░░░                           |
13:00  2312.7  2321.0  2312.7  2318.1  +0.24  69.1k  | ██-                            |
14:00  2318.2  2388.0  2311.6  2378.2  +2.59   627k  | -██████████████--              |
15:00  2378.2  2451.1  2377.3  2425.8  +2.00   975k  |               ███████████------|
16:00  2425.8  2446.1  2425.8  2434.2  +0.34   198k  |                         ███--- |
17:00  2434.1  2435.0  2419.0  2424.0  -0.42   101k  |                        -░░░    |
18:00  2424.0  2436.2  2419.0  2422.4  -0.07  95.4k  |                        -░---   |
summary  chg +4.31%  range 2308.5..2451.1 (6.18%)  ATR(9) 28.9 (1.19%)  last vol 0.4x avg  high@15:00 low@12:00
notable  14:00 largest gain +2.59%  |  12:00 largest drop -0.60%  |  15:00 heaviest volume 3.8x avg

what else moved in the window (z = move / typical move of that length over the prior 20d):
id       change     z
OIL      -6.39%  -6.0  <- unusual
BTC      +2.85%  +4.1  <- unusual
HYG      +0.63%  +3.6  <- unusual
NDX247   +1.72%  +3.4  <- unusual
SPX      +1.84%  +3.0  <- unusual
DXY      +0.33%  +2.9  <- unusual
NQ       +1.73%  +2.8  <- unusual
VIX      -1.570  -1.4
NDX      +1.58%  +1.4
US10Y    -0.050  -1.3
DAX      +0.83%  +1.3
GOLD247  +0.52%  +1.2
XAU      +0.56%  +1.1
USDJPY   +0.12%  +0.8
XAG      +0.39%  +0.4
COPPER   -0.22%  -0.4
N225     +0.00%  +0.0
HSI      +0.00%  +0.0

news around the window (Google News: ethereum after:2026-11-03 before:2026-11-05):
11-04 01:31     google/TradingView Bitcoin, Ethereum, XRP, Dogecoin Surrender Early Gains as Midterm Polls Close—Analyst Sees ‘Sell the Rumor, Buy the News’ Play [BTC,ETH]
11-04 04:14     google/FXStreet Top 3 Price Predictions: Bitcoin, Ethereum, Ripple – BTC tests critical support, ETH struggles below resistance, XRP extends losses [BTC,ETH]
11-04 04:41     google/The Defiant Bullish Picks Solana, Aptos and Ethereum for Tokenized Shares [ETH,SOL]
11-04 04:55     google/Yahoo Finance Why Bitmine Immersion Technologies Stock Is In Focus After Its Ethereum Buying Spree [ETH]
11-04 05:39     google/The Defiant Justin Drake Targets December 2028 for Ethereum’s Post-Quantum Transition [ETH]
11-04 11:35 !!  google/Coinpedia Ethereum Price Surges Over 7%: $236M Liquidations Fuel ETH Recovery – What Happens Next? [ETH]
11-04 13:42 !   google/Chicago Star Media Ethereum's second-term surge, and how Washington's shift on crypto is changing the game [ETH]
11-04 14:36 !!! google/BigGo Finance Six Altcoins Flash Buy Signals After October Rout, Analyst Says Ethereum Correction May Be Near an End [ETH]
11-04 16:09 !   google/TradingView Bitcoin Pumps to $80,000, Ethereum, XRP, Solana Surge up to 6% [BTC,ETH,SOL]
11-04 17:00 !   google/AMBCrypto Ethereum price prediction: $2.4K recovery or another liquidation driven retreat? [ETH]
11-04 17:04 !!  google/KuCoin  Ethereum Price Surges 7% Amid $236M Liquidations [ETH]
11-04 17:17 !!! google/24/7 Wall St. Hackers Who Allegedly Took $133 Million From a French Bank Swapped Into Ethereum and Monero. Is That Behind Monero's 17% Monthly Gain? [ETH]
11-05 00:56 !   google/OX Security PolinRider Returns: Hijacked npm Packages Use Ethereum Blockchain to Conceal Malware Infrastructure [ETH]
11-05 02:04 !   google/Benzinga Bitcoin Tops $81,000, Ethereum, XRP, Dogecoin Also Jump After Midterm Elections: Analyst Says BTC About t [BTC,ETH]

read: a move shared by many assets with high |z| is macro (rates, dollar, risk appetite); a move alone in ETH is idiosyncratic — look for asset-specific news.
```

</details>

### Boards and odds

Boards put many series side by side. Rates and spreads change in basis points, volatility indices
in points, everything else in percent, and every row says how fresh it is. Prediction markets are
first-class series: chart them, correlate them, alert on them.

<details>
<summary><code>autorekt show positioning</code> and <code>autorekt show kalshi:KXFEDDECISION-26DEC</code></summary>

```
$ autorekt show positioning
board positioning  2026-11-05 05:20 UTC  (day = 24h for crypto, vs previous close for TradFi; rates/spreads in bp, vol & sentiment indices in points, odds in pp; trend = last 6d hourly, or 30d for daily series)
id                           last  1h       day      5d       30d      trend                     flags
COT_ES                    -134368  -        +8303    +8303    -1140    ▇▄▄▅▃▃▂▁▁▇▇▇██▇█▆▆▆▄▃▃▃▄  non-commercial net, contracts, weekly, as of Tuesday, no new bar for 8 days
COT_NQ                      48839  -        -1887    -1887    -7311    ▄▄▃▃▁▂▂▂▂▂▂▃▃▃▁▂▃▅▄█▇█▇▇  non-commercial net, contracts, weekly, as of Tuesday, no new bar for 8 days
COT_ZN                    -981538  -        -17245   -17245   -169786  ▇▇█▇█▆█▅▄▆▇▆▅▅▁▂▆▄▆▇▄▄▁▁  non-commercial net, contracts, weekly, as of Tuesday, no new bar for 8 days
COT_GOLD                   186451  -        -13414   -13414   -39402   ▁▂▂▂▂▁▁▃▃▃▄▄▃▃▄▆█▇▇▇▆▅▅▄  non-commercial net, contracts, weekly, as of Tuesday, no new bar for 8 days
COT_WTI                     74343  -        -14982   -14982   -66763   ██▇▇▆▆▆▄▄▄▃▁▂▄▃▄▄▄▅▅▃▃▃▂  non-commercial net, contracts, weekly, as of Tuesday, no new bar for 8 days
COT_JPY                    101591  -        +31713   +31713   +29609   ▃▃▃▄▃▃▂▁▁▁▁▂▁▁▄▄▄▃▆▇▇▇▇█  non-commercial net, contracts, weekly, as of Tuesday, no new bar for 8 days
COT_EUR                   -143894  -        -9370    -9370    -91560   ▆▇█████▇██▆▆▅▄▄▄▅▅▅▄▄▃▁▁  non-commercial net, contracts, weekly, as of Tuesday, no new bar for 8 days
COT_BTC                    2720.0  -        +302     +302     -36      ▅▄▄▃▂▄▄▆▇▇█▆▆██▅▄▁▃▅▅▅▅▅  non-commercial net, contracts, weekly, as of Tuesday, no new bar for 8 days
COT_VIX                    -77466  -        +19043   +19043   +1814    ██▇█▅▅▅▃▄▄▄▄▃▄▄▂▃▂▁▃▃▁▁▃  non-commercial net, contracts, weekly, as of Tuesday, no new bar for 8 days
SPX_GEX                     53.96  -        +0.00    -        -        ▄                         USD bn per 1% move
cboe:SPX#put_call_oi       1.4255  -        +0.00    -        -        ▄
cboe:SPX#skew_25d          4.5500  -        +0.00    -        -        ▄
cboe:SPX#zero_dte_pct      7.5570  -        +0.00    -        -        ▄
BTC_ETF_FLOW                74.60  -        +190.20  +717.80  +164.40  ▆▆▅▄▅▆▄▂▃▅▄▃▅▄██▁▄▆▆▅▁▂▅  as of 2026-11-04, daily net flow, USD mn
ETH_ETF_FLOW                53.00  -        +138.30  +208.70  +103.80  ▇█▆▄▄▄▄▁▃▄▃▂▄▄▃▆▂▆▃▅▃▁▂▇  as of 2026-11-04, daily net flow, USD mn
BTC#long_short_ratio       1.5100  +0.00    -0.81    -1.26    +0.53    ▆▇█▇▇▇▇▆▆▅▅▅▅▆▇▇▇▆▅▅▃▁▁▁
BTC#funding_rate       0.00010000  +0.25bp  +0.12bp  +0.00bp  -        ██▃▃▁▄▆▂▄▁▄▃▆█▃▅█▅██▅▆██
finra:SPY                   60.4%  -        +18.3pp  -3.2pp   +7.9pp   ▂▄▇▇▄▄▄▁▃▄▅█▃▅▂▃▅▅▃▆▇▇▅▆  short volume ratio, off-exchange only, T+0 evening
finra:QQQ                   61.8%  -        +10.3pp  -9.7pp   -5.3pp   ▁▃▇▅▇▄▆▅▇▂▇█▂█▂▁▆▂▆▁▇█▅▄  short volume ratio, off-exchange only, T+0 evening
finra:NVDA                  29.9%  -        +2.6pp   -7.9pp   -27.5pp  ▃▃▂▅▇▅█▇▇▆▄▇▃▄▁▂▄▁▁▁▄▃▁▁  short volume ratio, off-exchange only, T+0 evening
```

```
$ autorekt show kalshi:KXFEDDECISION-26DEC
kalshi:KXFEDDECISION-26DEC-C26  Will the Federal Reserve Cut rates by >25bps at their December 2026 meeting?  [vol24h 1.1k, 24h +0.0pp, ends 2026-12-09]
    1.0%                        Cut >25bps  bid 0.0% ask 1.0%
kalshi:KXFEDDECISION-26DEC-C25  Will the Federal Reserve Cut rates by 25bps at their December 2026 meeting?  [vol24h 14.1k, 24h +0.0pp, ends 2026-12-09]
    1.0%                        Cut 25bps  bid 1.0% ask 2.0%
kalshi:KXFEDDECISION-26DEC-H0  Will the Federal Reserve Hike rates by 0bps at their December 2026 meeting?  [vol24h 44.8k, 24h -5.0pp, ends 2026-12-09]
   21.0%  ####                  Fed maintains rate  bid 20.0% ask 21.0%
kalshi:KXFEDDECISION-26DEC-H25  Will the Federal Reserve Hike rates by 25bps at their December 2026 meeting?  [vol24h 79.5k, 24h +4.0pp, ends 2026-12-09]
   80.0%  ################      Hike 25bps  bid 80.0% ask 81.0%
kalshi:KXFEDDECISION-26DEC-H26  Will the Federal Reserve Hike rates by >25bps at their December 2026 meeting?  [vol24h 15.7k, 24h +0.0pp, ends 2026-12-09]
    1.0%                        Hike >25bps  bid 1.0% ask 2.0%
```

</details>

### Rules that wake the agent

A rule is an expression over any series. `--test` replays it over history with its cooldown and
shows what followed each firing. Saved rules run in the daemon, which runs a hook command or
releases `watch --wait` for agents without hooks.

<details>
<summary><code>autorekt watch 'pct(BTC, "1h") &lt; -1.5' --test</code></summary>

```
$ autorekt watch 'pct(BTC, "1h") < -1.5' --test -i 5m -r 14d --cooldown 6h
backtest adhoc  expr: pct(BTC, "1h") < -1.5  tf 5m  range 14d  for 0  cooldown 6h  ->  2 firing(s) over 4032 bars
time                value  BTC +1h     +4h    +24h
2026-10-29 20:40  -2.4493   +0.25%  -2.00%  -2.46%
2026-11-04 03:05  -1.6259   +1.17%  +2.16%  +5.32%
+1h: mean +0.71% median +1.17% up 2/2   +4h: mean +0.08% median +2.16% up 1/2   +24h: mean +1.43% median +5.32% up 1/2   
```

</details>

## Three verbs

Agents learn a tool by imitating how it was used before, so the interface stays small: one verb
per job, the same options everywhere, and help and error messages that say what to do next.

- **`show`** Read anything: the briefing, boards, charts, tables, a journal, odds, news, the calendar, your alerts. `autorekt show liquidity`
- **`search`** Find anything: instrument ids across every source, prediction markets, headlines. `autorekt search swap --kind instruments`
- **`watch`** Get woken: rules in plain expressions, backtested before they are trusted, run by a background daemon. `autorekt watch --wait`

## Any table in one line

Several ids, a board or a list make a table. Options pick its columns, keep the rows you want and
put them in order.

- **Columns** Change over any window or between two dates, distance from the high, drawdown, volatility. `autorekt show sectors --with 'chg(5d),dd(1y)'`
- **Filter and sort** Keep the rows that pass, order them, take the top and the bottom. `autorekt show sectors --where 'chg(ytd) > 15' --sort 'chg(5d)' --top 5`
- **Company figures** P/E, P/B, dividend yield, payout and growth for Hong Kong, A-shares and the US. `autorekt show hk.southbound --with 'pe,pb,dy' --where 'dy > 6'`
- **Against a benchmark** Beta, correlation and upside and downside capture, dividends included. `autorekt show 0788.HK 2601.HK --vs 3033.HK --with 'beta(),upcap(),downcap()' -i 1M -r 5y --adjust`
- **China's boards and flows** Every A-share industry or concept board with its fund flows. `autorekt show cn.industry --sort 'flow(5d)' --top 10`
- **Restriction lists** Whether a stock is on OFAC's NS-CMIC list or the Pentagon's 1260H list. `autorekt show 9988.HK 0788.HK --with lists`
- **Filings and dates** A stock's exchange filings and headlines, or its next earnings and dividends. `autorekt show calendar NVDA AAPL`
- **Why it moved** The biggest move, what moved with it, and the news; or how assets correlate. `autorekt show BTC --view explain`
- **Your own lists** Ids from a file, one per line, anywhere an id goes. `autorekt show @watchlist.txt --with 'chg(5d),dy'`
- **Export** Every bar of each series as CSV, or any table as Markdown. `autorekt show SPY QQQ -i 1d -r 1y --csv`
- **Any span** A start and an end date, for a chart or for the change a table shows. `autorekt show SPY QQQ --from 2026-11-06 --to 2026-11-12`
- **Total return** Prices with dividends added back, in charts, tables and CSV. `autorekt show 0788.HK -i 1w -r 5y --adjust`

## Every free source worth reading

Public endpoints only, fetched on your own machine, cached and rate-limited. When a source fails,
autorekt says so instead of quietly showing something else.

- **Crypto** OKX, Binance, Hyperliquid (including 24/7 equity perps), Deribit options and DVOL, Coinbase, DefiLlama stablecoins, spot ETF flows from Farside
- **Markets** Yahoo Finance for stocks, ETFs, indices, futures, FX and commodities; Cboe option chains for dealer gamma, skew, put/call and same-day expiries
- **Dollar liquidity** The New York Fed for the SOFR distribution, repo facility use and dealer positions; the Treasury's daily cash balance and auctions; SOFR swap rates from reported trades (DTCC)
- **Rates** FRED, the ECB, Japan's Ministry of Finance
- **Positioning** CFTC Commitments of Traders, FINRA off-exchange short volume, funding rates, open interest, long/short ratios and liquidations
- **China and Hong Kong** Yuan fixings and SHIBOR (CFETS); bond yields, margin debt, Stock Connect flows, quotes, boards and fund flows (Eastmoney); HIBOR (HKMA)
- **Companies** Fundamentals and filings (Eastmoney, HKEXnews, SEC EDGAR), earnings and dividend dates (Nasdaq), US restriction lists (OFAC, the Federal Register)
- **Odds, news and the calendar** Polymarket, Kalshi, financial and crypto news feeds, Google News search, the ForexFactory calendar, CNN and crypto Fear & Greed

## Share what it found

An agent can publish its analysis as a web page: `autorekt show report.md --share` turns markdown
with show blocks into a page with live, interactive charts, served from your machine through a
Cloudflare tunnel.

````markdown
## Dollar liquidity is tightening

Repo is trading above the rate the Fed pays on reserves.

```show
nyfed:SOFR - fred:IORB -i 1d -r 90d
(dtcc:SOFR10Y - fred:DGS10) * 100 -i 1d -r 180d
```
````

- **Links that last** Links survive restarts, and sharing the same file again updates the same link. `autorekt show shares`
- **Expire or remove** `--ttl 30d` sets how long a link lives; `--ttl 0` removes it. `autorekt show report.md --share --ttl 0`
- **A fixed address** The tunnel keeps its address while the daemon restarts; a Cloudflare named tunnel keeps one for good (`autorekt daemon --help`). `autorekt daemon tunnel`

## Keep a journal

A journal is a markdown document that keeps your records in pipe tables, next to your notes.
autorekt reads it and shows it live.

````markdown
## Hong Kong dividends

```trades account=company
| date       | action | id      | qty | price | ccy | fee |
| 2026-11-12 | buy    | 0788.HK | 500 | 9.05  | HKD | 27  |
```

```orders account=company
| ref     | side | id      | qty | limit | placed     |
| tower-2 | buy  | 0788.HK | 500 | 8.60  | 2026-11-11 |
```

```show
@this --with 'chg(5d),dy,lists'
```
````

- **Live holdings** Costs, cash, gains and weights follow from the trades, in one currency with `--in`, or as of any day with `--at`. `autorekt show journal.md --in CNY`
- **What needs attention** Reached orders and this week's dates come first; `--changes` shows only what is new, and `watch` turns them into alerts. `autorekt show journal.md --changes`
- **Paper trading** Orders in an `account=sim` table fill, stop and take profit from daily bars. `autorekt show sim.md`
- **Private when shared** A shared journal leaves out quantities and amounts unless you add `--show-amounts`. `autorekt show journal.md --share`

## License

MIT
