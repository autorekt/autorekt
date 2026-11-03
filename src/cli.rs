//! Command-line interface: argument definitions (clap) and dispatch to `cmds`.
//! Help texts here are what agents read first, so they carry the id conventions.

use crate::render::{self, Opts};
use crate::{cmds, daemon};
use anyhow::Result;
use clap::{Args, Parser, Subcommand};

const ABOUT: &str = "autorekt — market observation for agents (and the humans they report to).
New here? Run `autorekt guide`.

Ids are <provider>:<symbol>[#metric]. Exchanges use CCXT symbols:
  okx:BTC/USDT:USDT (perp)  okx:BTC/USDT (spot)  hyperliquid:BTC/USDC:USDC  hyperliquid:xyz:TSLA (HIP-3)
Others use native ids: yahoo:^VIX  yahoo:GC=F  fred:DGS10  polymarket:<slug>  kalshi:<TICKER>  alt:fng
Aliases: BTC ETH SOL ... (default exchange perp), SPX NDX VIX MOVE DXY US10Y GOLD WTI HYG FNG ...
Metrics: BTC#funding_rate  BTC#open_interest  BTC#long_short_ratio  BTC#taker_buy_sell_ratio  BTC#basis  BTC#liquidations
Vol/fear: VIX VIX9D VIX3M MOVE OVX DVOL (deribit) CNNFNG FNG   Liquidity: fred:SOFR fred:IORB fred:WRESBAL HYOAS NFCI
Expressions anywhere a series is expected: 'fred:SOFR - fred:IORB'  'ta.rsi(BTC, 14)'  'VIX - VIX3M'
Timeframes: 1m 5m 15m 1h 4h 1d 1w 1M";

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
    /// Output CSV (series commands).
    #[arg(long, global = true)]
    pub csv: bool,
    /// Time zone for display (IANA name, or UTC/ET/HK/CN/tokyo).
    #[arg(long, global = true, env = "AUTOREKT_TZ", default_value = "UTC")]
    tz: String,
    /// Width of picture columns in characters.
    #[arg(long, global = true, default_value_t = 32)]
    width: usize,
    /// Glyphs for pictures: ascii (= up, : down) or block (█ ░ and ▁▂▃ sparklines).
    #[arg(long, global = true, default_value = "block")]
    glyphs: String,
}

#[derive(Subcommand)]
enum Cmd {
    /// One-shot market briefing for an agent: cross-asset board, fear, liquidity, crypto
    /// positioning, key odds, calendar, headlines, local events. Start an analysis here.
    Brief {
        /// Hours of headlines/events to include.
        #[arg(long, default_value_t = 12)]
        hours: i64,
    },
    /// Overview: one line per instrument (changes, trend, flags). Sets: default crypto macro global fear liquidity rates commodities sectors megacaps asia, or a comma list of ids.
    Board {
        /// Board name or a comma-separated list of ids.
        #[arg(default_value = "default")]
        set: String,
    },
    /// Last price and 24h / previous-close change.
    Quote { ids: Vec<String> },
    /// Text candle chart (one line per bar).
    Candles(SeriesArgs),
    /// A metric or any series as sparkline + table (funding, OI, odds, FRED...).
    Metric(SeriesArgs),
    /// Prediction markets (Polymarket, Kalshi).
    #[command(subcommand)]
    Prob(ProbCmd),
    /// News headlines (RSS feeds), optionally filtered or searched.
    News {
        /// Only items newer than this (e.g. 2h, 1d).
        #[arg(long)]
        since: Option<String>,
        /// Search Google News for this query instead of the feed list.
        #[arg(long)]
        about: Option<String>,
        /// Topic filter for the feed list: markets, macro, crypto.
        #[arg(long)]
        topic: Option<String>,
        /// Minimum severity 0-3.
        #[arg(long, default_value_t = 0)]
        min_severity: u8,
        #[arg(short = 'n', long, default_value_t = 40)]
        limit: usize,
        /// Show summaries.
        #[arg(long)]
        body: bool,
    },
    /// Macro calendar (ForexFactory): this week and next.
    Calendar {
        /// Days ahead to show (negative values show the past too).
        #[arg(long, default_value_t = 7)]
        days: i64,
        /// Include this many days back.
        #[arg(long, default_value_t = 1)]
        back: i64,
        /// Minimum impact: 1 low, 2 medium, 3 high.
        #[arg(long, default_value_t = 2)]
        min_impact: u8,
        /// Country/currency filter, e.g. USD,JPY.
        #[arg(long)]
        country: Option<String>,
    },
    /// List markets of a provider (optionally filtered), e.g. `markets hyperliquid xyz`.
    Markets { provider: String, filter: Option<String> },
    /// Providers, what they offer, and whether they are reachable.
    Sources {
        /// Probe each provider now.
        #[arg(long)]
        check: bool,
    },
    /// Show how an id or alias resolves.
    Resolve { ids: Vec<String> },
    /// Print the guide for agents (start here).
    Guide,
    /// Rules: Pine-style conditions in Prometheus-style rules, plus timers.
    #[command(subcommand)]
    Rule(RuleCmd),
    /// Block until a rule fires or a monitor alerts; prints the event(s) and exits.
    Wait {
        /// Only these rules (comma-separated names); default any.
        #[arg(long)]
        rule: Option<String>,
        /// Give up after this long (exit code 2), e.g. 30m.
        #[arg(long)]
        timeout: Option<String>,
        /// Minimum severity.
        #[arg(long, default_value_t = 0)]
        min_severity: u8,
    },
    /// Local event log: rule firings, timers, monitor alerts.
    Events {
        #[arg(long, default_value = "24h")]
        since: String,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// What changed since this agent last looked: events, board moves, major headlines.
    Diff {
        /// Agent name for the cursor (or env AUTOREKT_AGENT).
        #[arg(long = "as", env = "AUTOREKT_AGENT", default_value = "default")]
        agent: String,
        /// Board to compare.
        #[arg(long, default_value = "default")]
        set: String,
        /// Peek without moving the cursor.
        #[arg(long)]
        peek: bool,
    },
    /// Share an interactive chart with a human: prints a URL (served by the daemon, via a tunnel).
    Share {
        /// Id or expression. Omit with --install-tunnel.
        id: Option<String>,
        #[arg(short = 'i', long, default_value = "1h")]
        interval: String,
        #[arg(short = 'n', long, default_value_t = 200)]
        limit: usize,
        /// candles or line.
        #[arg(long, default_value = "candles")]
        kind: String,
        /// Extra line series drawn on their own scale.
        #[arg(long)]
        overlay: Vec<String>,
        #[arg(long)]
        title: Option<String>,
        /// Link lifetime, e.g. 7d.
        #[arg(long, default_value = "7d")]
        ttl: String,
        /// Local link only, no tunnel.
        #[arg(long)]
        local: bool,
        /// Download cloudflared into ~/.autorekt/bin.
        #[arg(long)]
        install_tunnel: bool,
    },
    /// Manage the background daemon.
    #[command(subcommand)]
    Daemon(DaemonCmd),
    /// Python monitors (event-driven scripts the daemon supervises).
    #[command(subcommand)]
    Monitor(MonitorCmd),
    /// Pre-launch / pre-IPO perps (OKX pre-market, Hyperliquid HIP-3 dexes): price, 24h change, volume, funding.
    Prelaunch,
    /// Explain a move: find it (or use --at), show what else moved abnormally in the same
    /// window, and the news from that window.
    Why {
        id: String,
        /// Time of the move (ISO or Unix ms); default: the largest move in --lookback.
        #[arg(long)]
        at: Option<String>,
        /// Window around the move.
        #[arg(long, default_value = "4h")]
        window: String,
        /// Where to search for the largest move.
        #[arg(long, default_value = "7d")]
        lookback: String,
        /// Bar size used to find and measure moves.
        #[arg(short = 'i', long, default_value = "1h")]
        interval: String,
    },
    /// Funding across venues for coins, annualized (OKX 8h, Hyperliquid 1h, Deribit 8h, Binance).
    Funding {
        /// Coins (default BTC ETH SOL XRP DOGE HYPE).
        coins: Vec<String>,
    },
    /// Correlation matrix of returns across instruments.
    Corr {
        /// Ids (default: a cross-asset set).
        ids: Vec<String>,
        #[arg(short = 'i', long, default_value = "1d")]
        interval: String,
        #[arg(short = 'n', long, default_value_t = 60)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum RuleCmd {
    /// Add or replace a rule. Example: rule add btc_dump --expr 'pct(BTC, "1h") < -3' --hook 'notify.sh'
    Add {
        name: String,
        #[arg(long)]
        expr: Option<String>,
        /// Shorthand: --above / --below on an id (with --id).
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        above: Option<f64>,
        #[arg(long)]
        below: Option<f64>,
        #[arg(long)]
        every: Option<String>,
        #[arg(long)]
        schedule: Option<String>,
        /// Fire this long before each high-impact macro release (e.g. 15m).
        #[arg(long)]
        calendar_before: Option<String>,
        /// Calendar currencies, e.g. USD,EUR.
        #[arg(long)]
        calendar_countries: Option<String>,
        #[arg(long)]
        calendar_min_impact: Option<u8>,
        #[arg(long)]
        tf: Option<String>,
        #[arg(long = "for")]
        for_: Option<String>,
        #[arg(long)]
        cooldown: Option<String>,
        /// update or bar_close.
        #[arg(long)]
        eval: Option<String>,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        hook: Option<String>,
        #[arg(long)]
        severity: Option<u8>,
        /// key=value labels.
        #[arg(long)]
        label: Vec<String>,
        #[arg(long)]
        notify_resolved: bool,
        /// Skip the immediate evaluation check.
        #[arg(long)]
        no_check: bool,
        #[arg(long = "as", env = "AUTOREKT_AGENT")]
        owner: Option<String>,
    },
    /// List rules with their state.
    Ls,
    /// Show one rule (YAML) and its state.
    Show { name: String },
    /// Remove a rule.
    Rm { name: String },
    /// Evaluate a rule (or an ad-hoc expression) once, now.
    Check {
        /// Rule name or expression.
        target: String,
        #[arg(long)]
        tf: Option<String>,
    },
    /// Backtest: when would this rule have fired over a past range, and what happened next?
    Test {
        /// Rule name or expression.
        target: String,
        #[arg(long, default_value = "30d")]
        range: String,
        #[arg(long)]
        tf: Option<String>,
        #[arg(long = "for")]
        for_: Option<String>,
        #[arg(long)]
        cooldown: Option<String>,
        /// Instrument whose forward returns are reported (default: the rule's tradable
        /// instrument, or SPX for macro series).
        #[arg(long)]
        fwd: Option<String>,
    },
    /// Enable / disable without deleting.
    Enable { name: String },
    Disable { name: String },
}

#[derive(Subcommand)]
enum DaemonCmd {
    /// Run in the foreground.
    Run,
    /// Start in the background (commands that need it also auto-start it).
    Start,
    Stop,
    Restart,
    Status,
    /// Tail the daemon log.
    Logs {
        #[arg(short = 'n', long, default_value_t = 40)]
        lines: usize,
    },
    /// Install a systemd user service so the daemon survives reboots.
    Install,
}

#[derive(Subcommand)]
enum MonitorCmd {
    /// List monitors and their process state.
    Ls,
    /// Create a monitor from the template (then edit ~/.autorekt/monitors/NAME.py).
    New { name: String },
    /// Install a monitor script (copied into ~/.autorekt/monitors).
    Add { file: String },
    Rm { name: String },
    Logs {
        name: String,
        #[arg(short = 'n', long, default_value_t = 40)]
        lines: usize,
    },
    /// Print the Python SDK path and docs.
    Sdk,
}

#[derive(Args)]
pub struct SeriesArgs {
    pub id: String,
    /// Timeframe.
    #[arg(short = 'i', long, default_value = "1h")]
    pub interval: String,
    /// Number of bars.
    #[arg(short = 'n', long)]
    pub limit: Option<usize>,
    /// Lookback instead of -n, e.g. 7d.
    #[arg(short = 'r', long)]
    pub range: Option<String>,
    /// End time (ISO date/time or Unix ms); default now.
    #[arg(long)]
    pub end: Option<String>,
    /// Rows of the table (metric view).
    #[arg(long, default_value_t = 24)]
    pub rows: usize,
    /// Overlay headlines on bars (candles view).
    #[arg(long)]
    pub news: bool,
}

#[derive(Subcommand)]
enum ProbCmd {
    /// Search markets by text.
    Search {
        query: Vec<String>,
        /// polymarket, kalshi or all.
        #[arg(long, default_value = "all")]
        provider: String,
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
    },
    /// Show a market, or all markets of an event.
    Show { id: String },
    /// Highest 24h-volume markets.
    Trending {
        #[arg(long, default_value = "polymarket")]
        provider: String,
        #[arg(short = 'n', long, default_value_t = 15)]
        limit: usize,
    },
}

fn opts(g: &Global) -> Result<Opts> {
    Ok(Opts { tz: render::parse_tz(&g.tz)?, width: g.width, ascii: g.glyphs != "block" })
}

pub async fn run() -> i32 {
    let cli = Cli::parse();
    match dispatch(cli).await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    }
}

async fn dispatch(cli: Cli) -> Result<()> {
    let g = cli.g.clone();
    let o = opts(&g)?;
    match cli.cmd {
        Cmd::Brief { hours } => cmds::brief(hours, &g, &o).await,
        Cmd::Board { set } => cmds::board(&set, &g, &o).await,
        Cmd::Quote { ids } => cmds::quote(ids, &g, &o).await,
        Cmd::Candles(a) => cmds::candles(a, &g, &o).await,
        Cmd::Metric(a) => cmds::metric(a, &g, &o).await,
        Cmd::Prob(ProbCmd::Search { query, provider, limit }) => cmds::prob_search(query, provider, limit, &g).await,
        Cmd::Prob(ProbCmd::Show { id }) => cmds::prob_show(id, &g).await,
        Cmd::Prob(ProbCmd::Trending { provider, limit }) => cmds::prob_trending(provider, limit, &g).await,
        Cmd::News { since, about, topic, min_severity, limit, body } => cmds::news(since, about, topic, min_severity, limit, body, &g, &o).await,
        Cmd::Calendar { days, back, min_impact, country } => cmds::calendar(days, back, min_impact, country, &g, &o).await,
        Cmd::Markets { provider, filter } => cmds::markets(provider, filter, &g).await,
        Cmd::Sources { check } => cmds::sources(check).await,
        Cmd::Resolve { ids } => cmds::resolve(ids),
        Cmd::Guide => {
            print!("{}", include_str!("../GUIDE.md"));
            Ok(())
        }
        Cmd::Rule(rc) => cmds::rule(rc_into(rc), &g, &o).await,
        Cmd::Wait { rule, timeout, min_severity } => cmds::wait(rule, timeout, min_severity, &g, &o).await,
        Cmd::Events { since, limit } => cmds::events(&since, limit, &g, &o),
        Cmd::Diff { agent, set, peek } => cmds::diff(&agent, &set, peek, &g, &o).await,
        Cmd::Share { id, interval, limit, kind, overlay, title, ttl, local, install_tunnel } => {
            cmds::share(id, &interval, limit, &kind, overlay, title, &ttl, local, install_tunnel, &g).await
        }
        Cmd::Daemon(DaemonCmd::Logs { lines }) => cmds::tail(&daemon::logs().join("daemon.log"), lines),
        Cmd::Daemon(d) => {
            let action = match d {
                DaemonCmd::Run => "run",
                DaemonCmd::Start => "start",
                DaemonCmd::Stop => "stop",
                DaemonCmd::Restart => "restart",
                DaemonCmd::Status => "status",
                DaemonCmd::Install => "install",
                DaemonCmd::Logs { .. } => unreachable!("handled above"),
            };
            cmds::service(action, &g).await
        }
        Cmd::Monitor(MonitorCmd::Ls) => cmds::monitor_ls(&g).await,
        Cmd::Monitor(MonitorCmd::New { name }) => cmds::monitor_new(&name),
        Cmd::Monitor(MonitorCmd::Add { file }) => cmds::monitor_add(&file),
        Cmd::Monitor(MonitorCmd::Rm { name }) => cmds::monitor_rm(&name),
        Cmd::Monitor(MonitorCmd::Logs { name, lines }) => cmds::tail(&daemon::logs().join(format!("monitor-{name}.log")), lines),
        Cmd::Monitor(MonitorCmd::Sdk) => cmds::monitor_sdk(),
        Cmd::Why { id, at, window, lookback, interval } => cmds::why(&id, at, &window, &lookback, &interval, &g, &o).await,
        Cmd::Funding { coins } => cmds::funding(coins, &g).await,
        Cmd::Prelaunch => cmds::prelaunch(&g).await,
        Cmd::Corr { ids, interval, limit } => cmds::corr(ids, &interval, limit, &g, &o).await,
    }
}


pub enum RuleOp {
    Add(crate::rules::Rule, bool),
    Ls,
    Show(String),
    Rm(String),
    Check(String, Option<String>),
    Test(String, String, Option<String>, Option<String>, Option<String>, Option<String>),
    Enable(String, bool),
}

fn rc_into(rc: RuleCmd) -> RuleOp {
    match rc {
        RuleCmd::Add { name, expr, id, above, below, every, schedule, calendar_before, calendar_countries, calendar_min_impact, tf, for_, cooldown, eval, message, hook, severity, label, notify_resolved, no_check, owner } => {
            let expr = match (expr, id, above, below) {
                (Some(e), _, _, _) => Some(e),
                (None, Some(i), Some(a), None) => Some(format!("\"{i}\" > {a}")),
                (None, Some(i), None, Some(b)) => Some(format!("\"{i}\" < {b}")),
                (None, Some(i), Some(a), Some(b)) => Some(format!("\"{i}\" > {a} or \"{i}\" < {b}")),
                _ => None,
            };
            let labels = label.iter().filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
            RuleOp::Add(crate::rules::Rule { name, expr, every, schedule, calendar_before, calendar_countries: calendar_countries.map(|c| c.split(',').map(|x| x.trim().to_uppercase()).collect()).unwrap_or_default(), calendar_min_impact, tf, for_, cooldown, eval, labels, message, hook, severity, notify_resolved, disabled: false, owner }, !no_check)
        }
        RuleCmd::Ls => RuleOp::Ls,
        RuleCmd::Show { name } => RuleOp::Show(name),
        RuleCmd::Rm { name } => RuleOp::Rm(name),
        RuleCmd::Check { target, tf } => RuleOp::Check(target, tf),
        RuleCmd::Test { target, range, tf, for_, cooldown, fwd } => RuleOp::Test(target, range, tf, for_, cooldown, fwd),
        RuleCmd::Enable { name } => RuleOp::Enable(name, true),
        RuleCmd::Disable { name } => RuleOp::Enable(name, false),
    }
}
