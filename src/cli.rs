//! Command-line interface: four verbs for agents (show, search, why, watch) plus `daemon`.
//! The first argument says what; options mean the same thing in every command.

use crate::cmds;
use crate::render::{self, Opts};
use anyhow::Result;
use clap::{Args, Parser, Subcommand};

const ABOUT: &str = "autorekt — market observation for agents. Start with: autorekt show

  show    read anything: briefing, boards, charts, odds, news, calendar, alerts
  search  find anything: ids, tickers, prediction markets, news
  why     explain: a move (one id), or how assets relate and which pairs broke from it (several ids)
  watch   get alerted: rules, backtests, waiting, Python monitors
  daemon  background service (starts by itself when needed)

Ids are <provider>:<symbol>[#metric]:
  exchanges use CCXT symbols: okx:BTC/USDT:USDT (perp)  okx:BTC/USDT (spot)  hyperliquid:xyz:TSLA (24/7)
  others use native ids: yahoo:^VIX  yahoo:GC=F  fred:DGS10  polymarket:<slug>  kalshi:<TICKER>
  funding & rates: nyfed:SOFR#p99 nyfed:SRF treasury:TGA treasury:AUCTION_10Y dtcc:SOFR10Y mof:JGB10Y ecb:EA10Y
  China & HK: cfets:USDCNY cfets:SHIBOR_3M eastmoney:CGB10Y eastmoney:MARGIN eastmoney:SOUTHBOUND hkma:HIBOR_1M
  positioning & flows: cboe:SPX#gex #skew_25d #put_call_volume #zero_dte_pct  farside:BTC (ETF flows)  cftc:13874A
  aliases: BTC ETH SOL SPX NDX VIX MOVE DXY US10Y GOLD WTI HYG DVOL FNG CNNFNG COT_ES STABLES SWAP10Y JGB10Y CGB10Y CNYFIX SPX_GEX ...
  metrics: BTC#funding_rate #open_interest #long_short_ratio #taker_buy_sell_ratio #basis #liquidations
  expressions work wherever an id does: 'fred:SOFR - fred:IORB'  'ta.rsi(BTC, 14)'  'RSP / SPY'
Timeframes: 1m 5m 15m 1h 4h 1d 1w.  Durations: 30m 6h 7d 90d.  Times: 2026-10-30T20:00:00Z";

const SHOW_HELP: &str = "WHAT is one of:
  (nothing)        the briefing: cross-asset board, volatility, liquidity, crypto positioning, odds, calendar, headlines
  a board          default crypto macro global fear liquidity rates commodities positioning sectors megacaps asia
  funding [COINS]  funding across venues, annualized          prelaunch    pre-launch and pre-IPO perps
  odds             trending prediction markets                 news [TOPIC] headlines (markets, macro, crypto)
  calendar [CCY]   macro releases                              alerts       this machine's rule and monitor events
  ID               one series: text candles (or sparkline + table for single-value series),
                   then compressed earlier history, swings, levels, state, events and data checks
  ID ID ...        one row per id: last, changes, trend, flags
  FILE.md          a markdown report; each line of a ```chart block is a chart in show syntax
                   (several lines = one chart with tabs). Prints a text preview; --share publishes
                   it as a page with live interactive charts.

Examples:
  autorekt show
  autorekt show fear
  autorekt show BTC -i 4h -n 30
  autorekt show GOLD -i 1d --with 'ta.rsi($,14)' --with 'ta.sma($,50)'
  autorekt show 'fred:SOFR - fred:IORB' -i 1d -r 90d
  autorekt show NQ -i 15m --at 2026-10-30T20:00:00Z
  autorekt show BTC ETH 'RSP / SPY' --changes
  autorekt show BTC -i 15m --share
  autorekt show report.md --share

A report chart block:
  ```chart
  BTC -i 1h -r 7d --overlay BTC#open_interest
  RSP / SPY -i 1d -r 120d
  ```";

const SEARCH_HELP: &str = "Searches instruments (OKX, Hyperliquid, Yahoo, CFTC and the statistics sources: nyfed,
treasury, dtcc, cfets, eastmoney, mof, ecb, hkma, farside), prediction markets (Polymarket, Kalshi) and news. Without a query: the sources and their status (--check probes them).

Examples:
  autorekt search fed december
  autorekt search anthropic --kind instruments
  autorekt search binance hack --kind news -r 3d
  autorekt search --check";

const WHY_HELP: &str = "One id: the biggest move in the range (or the move --at a time), what else moved in the
same window as z-scores, and dated news. Several ids: their correlation matrix. No ids: the
correlation matrix of the main cross-asset set.

Examples:
  autorekt why BTC
  autorekt why GOLD --at 2026-10-30T14:00:00Z --window 6h
  autorekt why SPX NDX BTC GOLD US10Y -r 30d
  autorekt why";

const WATCH_HELP: &str = "Rules are expressions that fire when true; the daemon evaluates them and runs your hook.

Examples:
  autorekt watch                                              list rules and monitors
  autorekt watch btc_dump --expr 'pct(BTC, \"1h\") < -3' --hook 'notify.sh'
  autorekt watch btc_80k --expr 'BTC > 80000' --cooldown 6h
  autorekt watch vix_inv --expr 'VIX - VIX3M > 1.5' --tf 15m --for 30m
  autorekt watch hourly --every 1h --hook '...'
  autorekt watch us_open --cron '30 13 * * 1-5' --hook '...'
  autorekt watch macro --calendar 15m --countries USD,EUR --hook '...'
  autorekt watch 'pct(BTC, \"1h\") < -3' --test -r 30d            backtest an expression (or a rule name)
  autorekt watch btc_dump --test                               evaluate now
  autorekt watch --wait --timeout 1h                           block until something fires
  autorekt watch basis --python                                create a Python monitor
  autorekt watch btc_dump --rm

Expressions: ids are closes at --tf; close/open/high/low/volume(id, tf), funding_rate(id),
open_interest(id), prob(id, outcome), pct(id, \"1h\"), stale(id, \"2h\"), atr(id, tf, n),
ta.sma ema rma wma rsi stdev highest lowest sum zscore change roc crossover crossunder cross
rising falling, abs min max sqrt log nz hour dayofweek, + - * / > < >= <= == != and or not.
Message templates: {{value}} {{rhs}} {{name}} {{expr}} {{time}} {{labels.KEY}}.
Hook environment: AUTOREKT_RULE AUTOREKT_MESSAGE AUTOREKT_VALUE AUTOREKT_RHS AUTOREKT_TIME
AUTOREKT_SEVERITY AUTOREKT_EXPR AUTOREKT_CONTEXT (recent candles) AUTOREKT_CHART_URL AUTOREKT_LABEL_<KEY>.";

const DAEMON_HELP: &str = "Actions: status (default) start stop restart logs [MONITOR] install (systemd user service)
run (foreground) tunnel (download cloudflared for --share links).

Files in $AUTOREKT_HOME (default ~/.autorekt): config.yaml, aliases.yaml (ALIAS: id), rules/,
monitors/ (Python monitors), autorekt.db, logs/, daemon.json.

config.yaml keys:
  default_exchange: okx          where bare crypto aliases resolve
  proxy: http://host:port        proxy for all requests
  base_urls: {binance_futures: https://fapi.binance.com}
  boards: {mine: [BTC, ETH, 'RSP / SPY', 'LABEL|expression']}
  record: [{id: 'hyperliquid:BTC#open_interest', every: 5m}]   sample metrics with no public history
  liquidations: [BTC-USDT, ETH-USDT]                            OKX swap families to collect
  brief_odds: ['series:KXFEDDECISION', 'polymarket:<slug>']     odds shown by `show`
  default_hook: 'command'        hook for monitor alerts without their own
  hook_chart_url: true           pass AUTOREKT_CHART_URL to hooks
  share_base_url / share_port / share_no_tunnel                 serving shared charts";

#[derive(Parser)]
#[command(name = "autorekt", version, about = ABOUT, long_about = ABOUT)]
struct Cli {
    #[command(flatten)]
    g: Global,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Args, Clone)]
pub struct Global {
    /// Output JSON instead of text.
    #[arg(long, global = true)]
    pub json: bool,
    /// Output CSV (single series).
    #[arg(long, global = true)]
    pub csv: bool,
    /// Time zone for display (IANA name, or UTC/ET/HK/CN/tokyo).
    #[arg(long, global = true, env = "AUTOREKT_TZ", default_value = "UTC")]
    tz: String,
    /// Width of picture columns in characters.
    #[arg(long, global = true, default_value_t = 32)]
    width: usize,
    /// Glyphs for pictures: block (█ ░ ▁▂▃) or ascii (= up, : down).
    #[arg(long, global = true, default_value = "block")]
    glyphs: String,
}

#[derive(Subcommand)]
enum Cmd {
    /// Read anything: briefing, boards, charts, odds, news, calendar, alerts.
    #[command(after_help = SHOW_HELP)]
    Show(ShowArgs),
    /// Find anything: ids, tickers, prediction markets, news.
    #[command(after_help = SEARCH_HELP)]
    Search(SearchArgs),
    /// Explain a move, or how assets relate.
    #[command(after_help = WHY_HELP)]
    Why(WhyArgs),
    /// Get alerted: rules, backtests, waiting, Python monitors.
    #[command(after_help = WATCH_HELP)]
    Watch(WatchArgs),
    /// Background service: status, start, stop, restart, logs, install, run, tunnel.
    #[command(after_help = DAEMON_HELP)]
    Daemon(DaemonArgs),
}

#[derive(Args)]
pub struct ShowArgs {
    /// What to show (see below).
    pub what: Vec<String>,
    /// Timeframe of bars.
    #[arg(short = 'i', long)]
    pub interval: Option<String>,
    /// Number of bars or items.
    #[arg(short = 'n', long)]
    pub limit: Option<usize>,
    /// How far back, e.g. 12h, 7d, 90d.
    #[arg(short = 'r', long)]
    pub range: Option<String>,
    /// Look at the past: the view ends at this time.
    #[arg(long)]
    pub at: Option<String>,
    /// Only what changed since this agent last looked.
    #[arg(long)]
    pub changes: bool,
    /// Agent name for --changes (default: env AUTOREKT_AGENT).
    #[arg(long = "as", env = "AUTOREKT_AGENT", default_value = "default")]
    pub agent: String,
    /// Minimum severity 0-3 for news, calendar and alerts.
    #[arg(long, default_value_t = 0)]
    pub min_severity: u8,
    /// Overlay matching headlines on a chart.
    #[arg(long)]
    pub news: bool,
    /// Extra column for a chart: an expression where $ is the series, e.g. 'ta.rsi($,14)'.
    #[arg(long)]
    pub with: Vec<String>,
    /// Print a public link to an interactive chart (or a FILE.md report) instead of text.
    #[arg(long)]
    pub share: bool,
    /// With --share: extra series drawn on their own scale.
    #[arg(long)]
    pub overlay: Vec<String>,
    /// With --share: link lifetime.
    #[arg(long, default_value = "7d")]
    pub ttl: String,
    /// With --share: link for this machine only (no tunnel).
    #[arg(long)]
    pub local: bool,
}

#[derive(Args)]
pub struct SearchArgs {
    /// Words to search for.
    pub query: Vec<String>,
    /// Restrict to instruments, odds or news.
    #[arg(long)]
    pub kind: Option<String>,
    /// Results per section.
    #[arg(short = 'n', long, default_value_t = 8)]
    pub limit: usize,
    /// News only from this far back.
    #[arg(short = 'r', long)]
    pub range: Option<String>,
    /// Without a query: probe each source.
    #[arg(long)]
    pub check: bool,
}

#[derive(Args)]
pub struct WhyArgs {
    pub ids: Vec<String>,
    /// Timeframe of bars (default 1h for a move, 1d for correlations).
    #[arg(short = 'i', long)]
    pub interval: Option<String>,
    /// How far back to look (default 7d for a move, 60d for correlations).
    #[arg(short = 'r', long)]
    pub range: Option<String>,
    /// Time of the move to explain (default: the biggest move in the range).
    #[arg(long)]
    pub at: Option<String>,
    /// Window around the move.
    #[arg(long, default_value = "4h")]
    pub window: String,
}

#[derive(Args)]
pub struct WatchArgs {
    /// Rule or monitor name; with --test also an expression.
    pub target: Option<String>,
    /// Condition expression: the rule fires when it is true.
    #[arg(long)]
    pub expr: Option<String>,
    /// Timer: fire every duration.
    #[arg(long)]
    pub every: Option<String>,
    /// Timer: cron schedule (5 fields, UTC).
    #[arg(long)]
    pub cron: Option<String>,
    /// Fire this long before each macro release.
    #[arg(long)]
    pub calendar: Option<String>,
    /// Calendar currencies, e.g. USD,EUR.
    #[arg(long)]
    pub countries: Option<String>,
    /// Timeframe for ids in the expression (default 1m).
    #[arg(long)]
    pub tf: Option<String>,
    /// The condition must hold this long before firing.
    #[arg(long = "for")]
    pub for_: Option<String>,
    /// Minimum time between firings while true (default 1h).
    #[arg(long)]
    pub cooldown: Option<String>,
    /// Evaluate closed bars only.
    #[arg(long)]
    pub bar_close: bool,
    /// Message template.
    #[arg(long)]
    pub message: Option<String>,
    /// Shell command run when the rule fires.
    #[arg(long)]
    pub hook: Option<String>,
    /// Severity of the rule's events, 0-3 (default 2).
    #[arg(long)]
    pub severity: Option<u8>,
    /// Labels as key=value.
    #[arg(long)]
    pub label: Vec<String>,
    /// Also emit an event when the condition clears.
    #[arg(long)]
    pub notify_resolved: bool,
    /// Keep the rule but stop evaluating it.
    #[arg(long)]
    pub disable: bool,
    /// Evaluate a disabled rule again.
    #[arg(long)]
    pub enable: bool,
    /// Remove the rule or monitor.
    #[arg(long)]
    pub rm: bool,
    /// Evaluate now; with -r, backtest over that range.
    #[arg(long)]
    pub test: bool,
    /// Backtest range.
    #[arg(short = 'r', long)]
    pub range: Option<String>,
    /// Backtest: instrument whose forward returns are reported.
    #[arg(long)]
    pub fwd: Option<String>,
    /// Block until a rule fires or a monitor alerts (only TARGET if given).
    #[arg(long)]
    pub wait: bool,
    /// With --wait: give up after this long (exit code 2).
    #[arg(long)]
    pub timeout: Option<String>,
    /// Minimum severity: events to wait for, or calendar releases to fire on.
    #[arg(long)]
    pub min_severity: Option<u8>,
    /// Create a Python monitor named TARGET from the template.
    #[arg(long)]
    pub python: bool,
    /// Skip evaluating a new rule once before saving it.
    #[arg(long)]
    pub no_check: bool,
    /// Owner of a new rule (default: env AUTOREKT_AGENT).
    #[arg(long = "as", env = "AUTOREKT_AGENT")]
    pub agent: Option<String>,
}

#[derive(Args)]
pub struct DaemonArgs {
    /// status, start, stop, restart, logs, install, run, tunnel.
    #[arg(default_value = "status")]
    pub action: String,
    /// For logs: a monitor name.
    pub name: Option<String>,
    /// For logs: number of lines.
    #[arg(short = 'n', long, default_value_t = 40)]
    pub lines: usize,
}

/// Parse one line of a ```chart block ("BTC -i 1h -r 7d --overlay BTC#open_interest") with the
/// same options as `show`; quotes group words as in a shell.
pub fn parse_chart_line(line: &str) -> Result<ShowArgs> {
    let mut words: Vec<String> = vec!["autorekt".into(), "show".into()];
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    match Cli::try_parse_from(words) {
        Ok(Cli { cmd: Cmd::Show(a), .. }) => Ok(a),
        Ok(_) => anyhow::bail!("not a chart line"),
        Err(e) => anyhow::bail!("{}", e.to_string().lines().next().unwrap_or("invalid chart line")),
    }
}

fn opts(g: &Global) -> Result<Opts> {
    Ok(Opts { tz: render::parse_tz(&g.tz)?, width: g.width, ascii: g.glyphs != "block" })
}

pub async fn run() -> i32 {
    let cli = Cli::parse();
    let result = match opts(&cli.g) {
        Ok(o) => match cli.cmd {
            Cmd::Show(a) => cmds::show(a, &cli.g, &o).await,
            Cmd::Search(a) => cmds::search(a, &cli.g, &o).await,
            Cmd::Why(a) => cmds::why(a, &cli.g, &o).await,
            Cmd::Watch(a) => cmds::watch(a, &cli.g, &o).await,
            Cmd::Daemon(a) => cmds::daemon(a, &cli.g).await,
        },
        Err(e) => Err(e),
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    }
}
