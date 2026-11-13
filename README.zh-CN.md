# autorekt

[English](README.md) | 简体中文

**让你的 AI 成为华尔街分析师。**

数据中心里已经坐满了天才。在金融市场上，限制它们的是能看到什么。autorekt 把整盘行情交给你的 agent：所有资产类别的价格、美元流动性、互换利差、市场仓位、期权伽马、中国的资金面、预测市场赔率、新闻和宏观日历，全部变成它读得懂的紧凑文本。你和你的 agent 可以随时看清全球资本的流向，并在出事时第一时间被叫醒。

<details>
<summary>agent 形成判断之前读的简报（<code>autorekt show</code>，完整输出）</summary>

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

> *autorekt*（名词）：自动爆仓器。它把世上所有的 K 线、资金费率、信用利差和预测市场都交给你的 agent，让它能以前所未有的自信犯错。以前的人只能手动爆仓。本项目不构成投资建议；如果构成，它会起一个更让人放心的名字。

## 安装

把这段提示词交给你的 agent：

```text
安装 autorekt（https://autorekt.com，源码在 https://github.com/autorekt/autorekt）。
1. 先装预编译的二进制：Linux 或 macOS 上运行 `curl -fsSL https://autorekt.com/install.sh | sh`；
   Windows 上在 PowerShell 里运行 `irm https://autorekt.com/install.ps1 | iex`，或在 CMD 里运行
   `curl -fsSL https://autorekt.com/install.cmd -o install.cmd && install.cmd && del install.cmd`。
2. 如果失败，就从源码编译：克隆仓库，运行 `cargo build --release`，把生成的二进制放到 PATH 上。
   这台机器上已有的 Rust 工具链够新就直接用它。缺少或版本太旧的 Rust 工具链、C 编译器和其他
   编译依赖，按人在这个系统上的正常做法安装：官方安装程序或系统的包管理器（例如 https://rustup.rs
   的 rustup、build-essential、Xcode 命令行工具、Visual Studio Build Tools）。
3. 遇到任何问题都自己排查，直到一切正常；必要时可以修改代码，并告诉我改了什么。
4. 运行 `autorekt show` 确认能用，然后从 `autorekt --help` 学习命令。
```

安装之后，autorekt 完全在你自己的机器上运行，从不连接我们的任何服务器：不需要账号，没有遥测，不会向我们发送任何东西。数据直接从各个公开数据源本身获取（交易所、央行、统计机构、新闻源），只有在你分享图表时才会开一个 Cloudflare 隧道。它访问的每个网络地址都明文写在源码里，很容易验证；下面第二段提示词就会让你的 agent 去验证这一点。

如果你非常在乎安全，可以改用下面这段：agent 会先审计整个仓库，把有风险的地方告诉你，再从审计过的源码编译安装。这会让安装变得很慢，也会消耗大量 token。

```text
安装 autorekt 之前先审计它。克隆 https://github.com/autorekt/autorekt，读完整个仓库：Rust 源码、
编译脚本、Cargo.toml 和 Cargo.lock 里的依赖、site/install/ 里的安装脚本，以及 GitHub 工作流。
找出所有有风险的地方：它访问哪些网络地址、发送了什么；它在自己的目录（~/.autorekt）之外读写哪些
文件；它会运行哪些命令（钩子、监控脚本、下载的 cloudflared）；本地 HTTP 服务暴露了什么；公网隧道；
以及不常见或无人维护的依赖。特别核实“它从不连接作者运营的服务器”这一说法。把每个风险连同文件和行号、严重程度、能否关闭一起告诉我，然后等我同意。
同意后用审计过的源码运行 `cargo build --release` 编译（需要时按正常方式安装 Rust 工具链），把二进制
放到 PATH 上，再运行 `autorekt show` 确认能用。
```

支持技能（skill）的 agent，比如 Claude Code，还可以加载 [skills/autorekt/SKILL.md](https://github.com/autorekt/autorekt/blob/main/skills/autorekt/SKILL.md)，它会告诉 agent 什么时候、怎样使用 autorekt。

autorekt 只观察，从不交易，任何能执行 shell 命令的 agent 都能用。只用免费数据，在你自己的机器上从公开接口获取：不需要注册账号，也不需要 API 密钥。

## 瓶颈在数据，不在智力

问一个模型"市场为什么动了"，它只能翻出过时的训练数据和几条新闻标题。好的分析师不会靠猜：形成判断之前，他们会先看利率和资金面、资金流向和仓位、期权和赔率，以及同一时间还有什么在动。这些数据分散在几十个来源里，格式、单位和发布时滞各不相同，大部分从来不上新闻。

autorekt 把它们汇集成一个为模型设计的统一视图：数字优先，每个数字都标明新鲜程度，并附上分析师本来就要自己算的统计量。下面这些问题，你的 agent 现在可以用数据而不是猜测来回答（数据来自 2026 年 11 月 5 日）：

- **美元流动性在收紧吗？** 隔夜回购利率比美联储付给准备金的利率高 7 个基点，其 99 分位高 15 个基点；10 年期互换利差为 −42 个基点；旁边还有财政部现金余额、常备回购便利的用量和一级交易商的持仓。`autorekt show liquidity`
- **北京在托人民币吗？** 离岸人民币比人民银行每天公布的中间价强约 490 个点，旁边是 SHIBOR、中国国债收益率、两融余额和南向资金。`autorekt show asia`
- **交易商会平抑下一次波动，还是放大它？** 标普 500 的交易商伽马为每 1% 波动 +540 亿美元，25 delta 偏度为 4.6 个波动率点，当天标普期权成交量的 65% 来自当日到期的合约。`autorekt show cboe:SPX#gex`
- **市场给美联储定价了什么？** Kalshi 给 12 月加息 80% 的概率；联邦基金期货隐含 11 月 3.88%、12 月 4.03%。`autorekt show kalshi:KXFEDDECISION-26DEC`
- **刚才为什么会动？** 11 月 4 日以太坊在四小时内上涨 5.3%，同一时间原油下跌 6.4%（6 个标准差），标普 500 上涨 1.8%，高收益债走强：这是宏观的风险偏好回升，不是加密圈的新闻。`autorekt show ETH --view explain`
- **谁在买？** 现货 ETF 资金流、按交易者类型划分的期货仓位、场外卖空成交、加密资金费率和多空比。`autorekt show positioning`

## 你的 agent 看到什么

以下都是未经修改的真实输出。每个视图都以数字开头，图形只用来表现形态，最后附上 agent 本来需要自己去查的背景信息。

### K 线图

每根 K 线一行，给出精确数字；然后是更长周期的视角：更早的粗粒度 K 线、波段高低点和价格反复回到的区域、多个周期上的趋势状态、仓位、即将公布的数据、相关性最高的品种，以及数据质量检查。`--with` 用 Pine Script 风格的写法加上指标列，其中 `$` 代表这条序列。

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

### 为什么会动？

`--view explain` 找到这次波动，衡量同一时间窗口里其他每个市场的波动有多不寻常，并把带时间的新闻放在旁边。
很多资产一起动，是宏观因素；只有它自己在动，是个别因素。

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

### 看板和赔率

看板把很多序列并排放在一起。利率和利差的变化以基点计，波动率指数以点计，其他以百分比计，每一行都标明数据的新鲜程度。预测市场是一等公民：可以画图、算相关性、设提醒。

<details>
<summary><code>autorekt show positioning</code> 和 <code>autorekt show kalshi:KXFEDDECISION-26DEC</code></summary>

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

### 唤醒 agent 的规则

规则是基于任何序列的表达式。`--test` 会带着冷却时间在历史数据上回放这条规则，并显示每次触发之后发生了什么。保存的规则由守护进程运行：触发时执行钩子命令，或者让没有钩子的 agent 从
`watch --wait` 中返回。

<details>
<summary><code>autorekt watch 'pct(BTC, "1h") &lt; -1.5' --test</code></summary>

```
$ autorekt watch 'pct(BTC, "1h") < -1.5' --test --tf 5m -r 14d --cooldown 6h
backtest adhoc  expr: pct(BTC, "1h") < -1.5  tf 5m  range 14d  for 0  cooldown 6h  ->  2 firing(s) over 4032 bars
time                value  BTC +1h     +4h    +24h
2026-10-29 20:40  -2.4493   +0.25%  -2.00%  -2.46%
2026-11-04 03:05  -1.6259   +1.17%  +2.16%  +5.32%
+1h: mean +0.71% median +1.17% up 2/2   +4h: mean +0.08% median +2.16% up 1/2   +24h: mean +1.43% median +5.32% up 1/2   
```

</details>

## 三个动词

agent 学用一个工具，靠的是模仿它以前被怎么用。所以接口尽量小：一件事一个动词，所有命令的选项含义一致，帮助和错误信息会告诉你下一步该做什么。

- **`show`** 查看一切：简报、看板、K 线、某项指标、赔率、新闻、日历、你的提醒，或者一份 markdown 报告。`autorekt show liquidity`。`--view explain` 找到一次波动、同时在动的市场和相关新闻（`autorekt show BTC --view explain`）；`--view corr` 衡量资产之间如何联动。`--with` 选择表格的列（`autorekt show sectors --with 'chg(5d),chg(ytd),from_high(52w)'`），`--from`/`--to` 指定明确的区间，`--csv` 输出每个序列的每一根 K 线（`autorekt show SPY QQQ -i 1d -r 1y --csv`），`--md` 把表格输出为 Markdown。`--where`、`--sort`、`--top`、`--bottom` 筛选和排序表格；`@watchlist.txt` 从文件读取代码。公司基本面也可以作为列：`autorekt show hk.southbound --with 'pe,pb,dy' --where 'dy > 6 and pb < 0.8'`（港股和 A 股来自东方财富，美股来自 SEC 申报文件）。`--adjust` 使用含股息的复权价格，`--vs` 指定 `beta()`、`corr()`、`upcap()`、`downcap()` 的基准（`autorekt show 0788.HK 2601.HK --vs 3033.HK --with 'beta(),upcap(),downcap()' -i 1M -r 5y --adjust`）。
- **`search`** 查找一切：所有来源的品种代码、预测市场、新闻标题。`autorekt search swap --kind instruments`
- **`watch`** 被叫醒：用普通表达式写规则，先回测再信任，由后台守护进程运行。`autorekt watch --wait`

## 所有值得读的免费数据源

只用公开接口，在你自己的机器上获取，带缓存和限速。某个来源出错时，autorekt 会直接说出来，而不是悄悄换成别的数据。

- **加密货币** OKX、Binance、Hyperliquid（包括 7×24 小时交易的美股永续合约）、Deribit 期权和 DVOL、Coinbase、DefiLlama 稳定币、Farside 的现货 ETF 资金流
- **市场行情** Yahoo Finance 的股票、ETF、指数、期货、外汇和大宗商品；Cboe 期权链，用于交易商伽马、偏度、看跌看涨比和当日到期期权
- **利率和美元流动性** FRED；纽约联储的 SOFR 分布、回购便利用量和一级交易商持仓；美国财政部的每日现金余额和国债拍卖；来自公开申报的互换交易的 SOFR 互换利率（DTCC）；欧洲央行；日本财务省
- **市场仓位** CFTC 持仓报告、FINRA 场外卖空成交、资金费率、未平仓合约、多空比和爆仓
- **中国和香港** 人民币中间价和 SHIBOR（中国外汇交易中心）；国债收益率、两融余额、沪深港通资金和涨跌家数、A 股和港股行情、行业和概念板块及其资金流向（东方财富，`autorekt show cn.industry --sort 'flow(5d)' --top 10`）；港元拆息和银行体系总结余（香港金管局）
- **赔率、新闻和日历** Polymarket、Kalshi、财经和加密新闻源、Google 新闻搜索、ForexFactory 日历、CNN 和加密恐惧贪婪指数

## 分享分析结果

agent 可以把分析发布成网页：`autorekt show report.md --share` 会把带图表块的 markdown 变成一个带实时交互图表的页面，通过 Cloudflare 隧道从你的机器上提供访问。

````markdown
## Dollar liquidity is tightening

Repo is trading above the rate the Fed pays on reserves.

```chart
nyfed:SOFR - fred:IORB -i 1d -r 90d
(dtcc:SOFR10Y - fred:DGS10) * 100 -i 1d -r 180d
```
````

链接在重启后仍然有效。再次分享同一个文件会更新同一个链接；`--name` 另开一个链接，`--ttl 30d` 设定有效期，`--ttl 0` 删除链接。`autorekt show shares` 列出所有有效链接及其访问次数和到期时间。

隧道作为独立进程运行，所以守护进程重启时随机的 `trycloudflare.com` 地址保持不变（机器重启后会变）。如果需要永不改变的地址，在 Cloudflare 上为你域名下的一个主机名创建命名隧道，指向 `http://localhost:8765`，把 token 存进一个文件，然后设置：

```text
# ~/.autorekt/config.yaml
share_port: 8765
share_base_url: https://charts.example.com
share_tunnel_token_file: ~/.autorekt/tunnel.token
```

`autorekt daemon tunnel` 显示隧道状态；`autorekt daemon tunnel stop` 停止隧道。

## 许可证

MIT
