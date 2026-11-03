//! Runtime configuration: `$AUTOREKT_HOME` (default `~/.autorekt`) holds config.yaml,
//! aliases.yaml, rules/, monitors/, the SQLite store and the daemon socket.

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
    /// Watchlists for `autorekt board --set NAME`.
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
    let base = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    base.join(".autorekt")
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
        "crypto" => &["BTC", "ETH", "SOL", "XRP", "BNB", "DOGE", "HYPE", "SUI", "FNG", "DVOL", "STABLES", "CB_PREMIUM_BP|(coinbase:BTC/USD / okx:BTC/USDT - 1) * 10000", "IBIT", "MSTR", "COIN"],
        "macro" => &["SPX", "NDX", "RUT", "VIX", "MOVE", "US3M", "US10Y", "US30Y", "DXY", "USDJPY", "EURUSD", "GOLD", "WTI", "COPPER", "HYG", "TLT"],
        "global" => &["SPX", "STOXX50", "DAX", "FTSE", "NIKKEI", "TOPIX", "HSI", "CSI300", "KOSPI", "NIFTY", "ASX", "USDJPY", "USDCNH", "EURUSD"],
        "fear" => &["VIX", "VIX9D", "VIX3M", "VVIX", "MOVE", "SKEW", "OVX", "GVZ", "DVOL", "ETHDVOL", "FNG", "CNNFNG", "HYG", "JNK", "GOLD"],
        "liquidity" => &["SOFR", "EFFR", "fred:IORB", "fred:SOFR99", "RRP", "RESERVES", "TGA", "WALCL", "HYOAS", "IGOAS", "NFCI", "STLFSI", "T10Y2Y", "T10Y3M"],
        "rates" => &["US3M", "US5Y", "US10Y", "US30Y", "ZT", "ZN", "ZB", "TLT", "HYG", "LQD", "DXY"],
        "commodities" => &["GOLD", "SILVER", "PLATINUM", "COPPER", "WTI", "BRENT", "NATGAS", "WHEAT", "CORN"],
        "positioning" => &["COT_ES", "COT_NQ", "COT_ZN", "COT_GOLD", "COT_WTI", "COT_JPY", "COT_EUR", "COT_BTC", "COT_VIX", "BTC#long_short_ratio", "BTC#funding_rate", "finra:SPY", "finra:QQQ", "finra:NVDA"],
        "sectors" => &["SPY", "RSP", "QQQ", "IWM", "XLK", "SMH", "XLC", "XLY", "XLF", "XLE", "XLI", "XLV", "XLP", "XLU", "XLRE", "XLB"],
        "megacaps" => &["AAPL", "MSFT", "NVDA", "GOOGL", "AMZN", "META", "TSLA", "AVGO", "TSM", "ORCL", "INTC", "AMD"],
        "asia" => &["NIKKEI", "TOPIX", "HSI", "HSTECH", "CSI300", "SSEC", "KOSPI", "TAIEX", "NIFTY", "ASX", "USDJPY", "USDCNH", "USDKRW"],
        _ => return None,
    };
    Some(preset.iter().map(|s| s.to_string()).collect())
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
    ]
    .iter()
    .map(|(i, e)| RecordSpec { id: i.to_string(), every: e.to_string() })
    .collect()
}

pub fn liquidation_families() -> Vec<String> {
    get().liquidations.clone().unwrap_or_else(|| vec!["BTC-USDT".into(), "ETH-USDT".into(), "SOL-USDT".into()])
}
