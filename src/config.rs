//! Runtime configuration: `$AUTOREKT_HOME` (default `~/.autorekt`) holds config.yaml,
//! aliases.yaml, rules/, monitors/, the SQLite store and the daemon socket.

use chrono::Datelike;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct Config {
    /// Exchange used when an alias like `BTC` names a crypto asset. Default: okx.
    pub default_exchange: Option<String>,
    /// Per-provider base URL overrides, e.g. `binance_futures: https://fapi.binance.com`.
    #[serde(default)]
    pub base_urls: BTreeMap<String, String>,
    /// Optional HTTP(S) proxy for all requests.
    pub proxy: Option<String>,
    /// Boards for `autorekt show NAME`.
    #[serde(default)]
    pub boards: BTreeMap<String, Vec<String>>,
    /// Port of the chart server (0 = pick a free one).
    pub share_port: Option<u16>,
    /// Public base URL for shared charts if you run your own tunnel/reverse proxy.
    pub share_base_url: Option<String>,
    /// Disable starting a cloudflared quick tunnel for `share`.
    #[serde(default)]
    pub share_no_tunnel: bool,
    /// Create a shared chart for each rule firing and pass it to hooks as AUTOREKT_CHART_URL.
    #[serde(default)]
    pub hook_chart_url: bool,
    /// Hook run for monitor alerts that do not name their own hook.
    pub default_hook: Option<String>,
    /// Series the daemon samples periodically (for metrics with no public history).
    /// Defaults to a small set (Hyperliquid OI, Deribit put/call and skew) when absent.
    pub record: Option<Vec<RecordSpec>>,
    /// Prediction markets shown by `brief` (ids; Kalshi series prefixed `series:` pick the next event).
    pub brief_odds: Option<Vec<String>>,
    /// OKX swap families whose liquidations the daemon collects (default BTC, ETH, SOL).
    pub liquidations: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RecordSpec {
    pub id: String,
    #[serde(default = "five_min")]
    pub every: String,
}

fn five_min() -> String {
    "5m".into()
}

pub fn home() -> PathBuf {
    if let Ok(h) = std::env::var("AUTOREKT_HOME") {
        return PathBuf::from(h);
    }
    std::env::home_dir().expect("no home directory found; set AUTOREKT_HOME").join(".autorekt")
}

pub fn get() -> &'static Config {
    static CFG: OnceLock<Config> = OnceLock::new();
    CFG.get_or_init(|| {
        let path = home().join("config.yaml");
        match std::fs::read_to_string(&path) {
            Ok(t) => serde_yaml::from_str(&t).unwrap_or_else(|e| {
                eprintln!("error: invalid {}: {e}", path.display());
                std::process::exit(2)
            }),
            Err(_) => Config::default(),
        }
    })
}

pub fn default_exchange() -> String {
    std::env::var("AUTOREKT_EXCHANGE").ok().or_else(|| get().default_exchange.clone()).unwrap_or_else(|| "okx".into())
}

pub fn base_url(key: &str, default: &str) -> String {
    get().base_urls.get(key).cloned().unwrap_or_else(|| default.to_string())
}

pub fn default_board() -> Vec<String> {
    if let Some(b) = get().boards.get("default") {
        return b.clone();
    }
    [
        "BTC", "ETH", "SOL", "HYPE", "SPX", "NDX", "ES", "NQ", "VIX", "MOVE", "US10Y", "DXY", "USDJPY", "GOLD", "SILVER",
        "COPPER", "WTI", "HYG", "NIKKEI", "HSI", "DAX", "FNG", "CNNFNG", "DVOL",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn board(name: &str) -> Option<Vec<String>> {
    if name == "default" {
        return Some(default_board());
    }
    if let Some(b) = get().boards.get(name) {
        return Some(b.clone());
    }
    let preset: &[&str] = match name {
        "crypto" => &["BTC", "ETH", "SOL", "XRP", "BNB", "DOGE", "HYPE", "SUI", "FNG", "DVOL", "STABLES", "CB_PREMIUM_BP|(coinbase:BTC/USD / okx:BTC/USDT - 1) * 10000", "BTC_ETF_FLOW", "ETH_ETF_FLOW", "IBIT", "MSTR", "COIN"],
        "macro" => &["SPX", "NDX", "RUT", "VIX", "MOVE", "US3M", "US10Y", "US30Y", "DXY", "USDJPY", "EURUSD", "GOLD", "WTI", "COPPER", "HYG", "TLT", "GDPNOW"],
        "global" => &["SPX", "STOXX50", "DAX", "FTSE", "NIKKEI", "TOPIX", "HSI", "CSI300", "KOSPI", "NIFTY", "ASX", "USDJPY", "USDCNH", "EURUSD", "US10Y", "EA10Y", "JGB10Y", "CGB10Y"],
        "fear" => &["VIX", "VIX9D", "VIX3M", "VVIX", "MOVE", "SKEW", "OVX", "GVZ", "DVOL", "ETHDVOL", "FNG", "CNNFNG", "SPX_GEX", "cboe:SPX#skew_25d", "HYG", "JNK", "GOLD"],
        "liquidity" => &[
            "SOFR", "nyfed:SOFR#p99", "EFFR", "fred:IORB", "SRF", "RRP", "RESERVES", "TGA", "WALCL", "DEALER_UST", SWAP_SPREAD_10Y, SWAP_SPREAD_30Y, "HYOAS", "IGOAS", "NFCI", "STLFSI",
            "T10Y2Y", "T10Y3M",
        ],
        "rates" => {
            let mut v: Vec<String> = ["US3M", "US2Y", "US5Y", "US10Y", "US30Y", "SWAP2Y", "SWAP10Y", "SWAP30Y", SWAP_SPREAD_10Y, SWAP_SPREAD_30Y, "treasury:AUCTION_10Y", "ESTR", "EA10Y", "JGB10Y", "JGB30Y", "CGB10Y", "ZT", "ZN", "ZB", "TLT", "HYG", "LQD", "DXY"].iter().map(|s| s.to_string()).collect();
            v.extend(fed_funds_strip(4));
            return Some(v);
        }
        "commodities" => &["GOLD", "SILVER", "PLATINUM", "COPPER", "WTI", "BRENT", "NATGAS", "WHEAT", "CORN", "CRUDE_STOCKS"],
        "positioning" => &[
            "COT_ES", "COT_NQ", "COT_ZN", "COT_GOLD", "COT_WTI", "COT_JPY", "COT_EUR", "COT_BTC", "COT_VIX", "SPX_GEX", "cboe:SPX#put_call_oi", "cboe:SPX#skew_25d", "cboe:SPX#zero_dte_pct",
            "BTC_ETF_FLOW", "ETH_ETF_FLOW", "BTC#long_short_ratio", "BTC#funding_rate", "finra:SPY", "finra:QQQ", "finra:NVDA",
        ],
        "sectors" => &["SPY", "RSP", "QQQ", "IWM", "XLK", "SMH", "XLC", "XLY", "XLF", "XLE", "XLI", "XLV", "XLP", "XLU", "XLRE", "XLB"],
        "megacaps" => &["AAPL", "MSFT", "NVDA", "GOOGL", "AMZN", "META", "TSLA", "AVGO", "TSM", "ORCL", "INTC", "AMD"],
        "asia" => &[
            "NIKKEI", "TOPIX", "HSI", "HSTECH", "CSI300", "SSEC", "KOSPI", "TAIEX", "NIFTY", "ASX", "USDJPY", "USDCNH", "CNYFIX", CNH_FIX_GAP, "USDKRW", "JGB10Y", "CGB10Y", "SHIBOR3M",
            "CN_MARGIN", "SOUTHBOUND", "CN_BREADTH", "HIBOR", "HK_AGGBAL",
        ],
        _ => return None,
    };
    Some(preset.iter().map(|s| s.to_string()).collect())
}

pub const SWAP_SPREAD_10Y: &str = "SWAP_SPREAD_10Y_BP|(dtcc:SOFR10Y - fred:DGS10) * 100";
pub const SWAP_SPREAD_30Y: &str = "SWAP_SPREAD_30Y_BP|(dtcc:SOFR30Y - fred:DGS30) * 100";
/// Offshore yuan minus the PBoC's fixing, in pips: how far the market sits from the official rate.
pub const CNH_FIX_GAP: &str = "CNH_FIX_GAP_PIPS|(USDCNH - cfets:USDCNY) * 10000";

/// Implied average fed funds rate for the next `n` months, from CBOT 30-day fed funds futures.
pub fn fed_funds_strip(n: u32) -> Vec<String> {
    const CODES: &[u8] = b"FGHJKMNQUVXZ";
    let now = chrono::Utc::now();
    (0..n)
        .map(|i| {
            let m0 = now.month0() + i;
            let (y, m) = (now.year() + (m0 / 12) as i32, (m0 % 12) as usize);
            let mon = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"][m];
            format!("FF_{mon}{:02}|100 - yahoo:ZQ{}{:02}.CBT", y % 100, CODES[m] as char, y % 100)
        })
        .collect()
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// The series of each briefing section; the daemon keeps all of them fresh.
pub struct Sections {
    pub main: Vec<String>,
    pub fear: Vec<String>,
    pub liq: Vec<String>,
    pub world: Vec<String>,
    pub crypto: Vec<String>,
}

pub fn brief_sections() -> Sections {
    let mut world = strs(&["US10Y", "SWAP10Y", "EA10Y", "JGB10Y", "JGB30Y", "CGB10Y", "CNYFIX", CNH_FIX_GAP, "SHIBOR3M", "CN_MARGIN", "SOUTHBOUND"]);
    world.extend(fed_funds_strip(3));
    Sections {
        main: default_board(),
        fear: strs(&["VIX9D", "VIX", "VIX3M", "VVIX", "MOVE", "OVX", "DVOL", "CNNFNG", "FNG", "SPX_GEX", "cboe:SPX#skew_25d", "cboe:SPX#put_call_volume", "cboe:SPX#zero_dte_pct"]),
        liq: strs(&[
            "nyfed:SOFR - fred:IORB", "nyfed:SOFR#p99 - fred:IORB", "(dtcc:SOFR10Y - fred:DGS10) * 100", "(dtcc:SOFR30Y - fred:DGS30) * 100", "SRF", "RRP", "RESERVES", "TGA",
            "DEALER_UST", "HYOAS", "IGOAS", "NFCI", "STABLES",
        ]),
        world,
        crypto: strs(&[
            "BTC#funding_rate", "BTC#open_interest", "BTC#long_short_ratio", "BTC#top_long_short_ratio", "BTC#taker_buy_sell_ratio",
            "ETH#funding_rate", "deribit:BTC#skew_10pct", "deribit:BTC#put_call_volume", "CB_PREMIUM_BP|(coinbase:BTC/USD / okx:BTC/USDT - 1) * 10000", "BTC_ETF_FLOW", "ETH_ETF_FLOW",
        ]),
    }
}

pub fn board_names() -> Vec<String> {
    let mut v: Vec<String> = ["default", "crypto", "macro", "global", "fear", "liquidity", "rates", "commodities", "positioning", "sectors", "megacaps", "asia"].iter().map(|s| s.to_string()).collect();
    for k in get().boards.keys() {
        if !v.contains(k) {
            v.push(k.clone());
        }
    }
    v
}

pub fn record_specs() -> Vec<RecordSpec> {
    if let Some(r) = &get().record {
        return r.clone();
    }
    [
        ("hyperliquid:BTC#open_interest", "5m"),
        ("hyperliquid:ETH#open_interest", "5m"),
        ("deribit:BTC#put_call_oi", "15m"),
        ("deribit:BTC#put_call_volume", "15m"),
        ("deribit:BTC#skew_10pct", "15m"),
        ("deribit:BTC#atm_iv", "15m"),
        ("cboe:SPX#gex", "1h"),
        ("cboe:SPX#put_call_volume", "1h"),
        ("cboe:SPX#skew_25d", "1h"),
        ("eastmoney:BREADTH", "30m"),
    ]
    .iter()
    .map(|(i, e)| RecordSpec { id: i.to_string(), every: e.to_string() })
    .collect()
}

pub fn liquidation_families() -> Vec<String> {
    get().liquidations.clone().unwrap_or_else(|| vec!["BTC-USDT".into(), "ETH-USDT".into(), "SOL-USDT".into()])
}
