//! Instrument ids: `<provider>:<provider-native id>[#metric]`.
//!
//! Exchanges use CCXT unified symbols (`BTC/USDT`, `BTC/USDT:USDT`, `BTC/USD:BTC-261226`).
//! Other providers use the id their own community uses (`yahoo:^VIX`, `fred:DGS10`,
//! `polymarket:<slug>`, `kalshi:<ticker>`). Bare aliases (`BTC`, `VIX`) resolve via an alias
//! table, and TradingView-style ids (`BINANCE:BTCUSDT.P`) are normalized.

use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

pub const PROVIDERS: &[&str] = &[
    "okx", "binance", "hyperliquid", "deribit", "cftc", "finra", "llama", "bitget", "coinbase", "yahoo", "fred", "nyfed", "treasury", "dtcc", "cboe", "cfets",
    "eastmoney", "mof", "ecb", "hkma", "farside", "polymarket", "kalshi", "alt", "local",
];

/// Providers of statistics with one value per day or week (rates, balances, flows, positioning),
/// as opposed to tradable prices.
pub const POINT_PROVIDERS: &[&str] = &["fred", "alt", "cftc", "finra", "llama", "local", "nyfed", "treasury", "dtcc", "cboe", "cfets", "eastmoney", "mof", "ecb", "hkma", "farside"];

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
pub struct InstrumentId {
    pub provider: String,
    pub symbol: String,
    pub metric: Option<String>,
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.symbol)?;
        if let Some(m) = &self.metric {
            write!(f, "#{m}")?;
        }
        Ok(())
    }
}

impl InstrumentId {
    pub fn base(&self) -> InstrumentId {
        InstrumentId { metric: None, ..self.clone() }
    }
    pub fn with_metric(&self, m: &str) -> InstrumentId {
        InstrumentId { metric: Some(m.to_string()), ..self.clone() }
    }
}

/// Parsed CCXT unified symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ccxt {
    pub base: String,
    pub quote: String,
    pub settle: Option<String>,
    pub expiry: Option<String>,
}

impl Ccxt {
    pub fn parse(s: &str) -> Result<Ccxt> {
        let (pair, rest) = match s.split_once(':') {
            Some((p, r)) => (p, Some(r)),
            None => (s, None),
        };
        let Some((base, quote)) = pair.split_once('/') else {
            bail!("'{s}' is not a CCXT symbol (expected BASE/QUOTE, BASE/QUOTE:SETTLE or BASE/QUOTE:SETTLE-YYMMDD)")
        };
        let (settle, expiry) = match rest {
            None => (None, None),
            Some(r) => match r.split_once('-') {
                Some((s, e)) => (Some(s.to_string()), Some(e.to_string())),
                None => (Some(r.to_string()), None),
            },
        };
        Ok(Ccxt { base: base.to_uppercase(), quote: quote.to_uppercase(), settle, expiry })
    }
    pub fn is_spot(&self) -> bool {
        self.settle.is_none()
    }
    pub fn is_swap(&self) -> bool {
        self.settle.is_some() && self.expiry.is_none()
    }
}

/// ISO codes of the currencies Yahoo quotes; six letters naming two of them form an FX pair.
pub const CURRENCIES: &[&str] = &[
    "USD", "EUR", "JPY", "GBP", "AUD", "NZD", "CAD", "CHF", "CNY", "CNH", "HKD", "SGD", "KRW", "INR", "MXN", "SEK", "NOK", "DKK", "PLN",
    "HUF", "CZK", "TRY", "ZAR", "BRL", "THB", "TWD", "ILS",
];

/// Coins that a bare alias resolves to a crypto perp rather than a Yahoo ticker.
const CRYPTO: &[&str] = &[
    "BTC", "ETH", "SOL", "XRP", "BNB", "DOGE", "ADA", "TRX", "AVAX", "LINK", "DOT", "TON", "SUI", "HYPE", "LTC",
    "BCH", "NEAR", "APT", "ARB", "OP", "PEPE", "WIF", "SHIB", "UNI", "AAVE", "ENA", "ONDO", "TAO", "XLM", "HBAR",
    "FIL", "ATOM", "INJ", "SEI", "TIA", "JUP", "WLD", "ETC", "XMR", "ZEC", "CRV", "LDO", "PENGU", "TRUMP", "FARTCOIN",
    "BONK", "POL", "ICP", "RENDER", "FET", "VIRTUAL", "KAITO", "MKR", "PUMP", "WLFI", "ASTER", "XPL",
];

pub fn default_aliases() -> BTreeMap<String, String> {
    let pairs: &[(&str, &str)] = &[
        // US indices and index futures
        ("SPX", "yahoo:^GSPC"), ("SP500", "yahoo:^GSPC"), ("NDX", "yahoo:^NDX"), ("NASDAQ", "yahoo:^IXIC"),
        ("DJI", "yahoo:^DJI"), ("RUT", "yahoo:^RUT"), ("ES", "yahoo:ES=F"), ("NQ", "yahoo:NQ=F"), ("YM", "yahoo:YM=F"),
        ("RTY", "yahoo:RTY=F"),
        // Global indices
        ("DAX", "yahoo:^GDAXI"), ("FTSE", "yahoo:^FTSE"), ("STOXX50", "yahoo:^STOXX50E"), ("CAC", "yahoo:^FCHI"),
        ("NIKKEI", "yahoo:^N225"), ("N225", "yahoo:^N225"), ("TOPIX", "yahoo:1306.T"), ("HSI", "yahoo:^HSI"), ("HSTECH", "yahoo:HSTECH.HK"),
        ("SSEC", "yahoo:000001.SS"), ("CSI300", "yahoo:000300.SS"), ("KOSPI", "yahoo:^KS11"), ("NIFTY", "yahoo:^NSEI"),
        ("TAIEX", "yahoo:^TWII"), ("ASX", "yahoo:^AXJO"),
        // Volatility / fear
        ("VIX", "yahoo:^VIX"), ("VIX3M", "yahoo:^VIX3M"), ("VIX9D", "yahoo:^VIX9D"), ("VVIX", "yahoo:^VVIX"),
        ("MOVE", "yahoo:^MOVE"), ("SKEW", "yahoo:^SKEW"), ("OVX", "yahoo:^OVX"), ("GVZ", "yahoo:^GVZ"),
        ("FNG", "alt:fng"), ("CRYPTOFNG", "alt:fng"), ("CNNFNG", "alt:cnn_fng"),
        ("STABLES", "llama:stablecoins"), ("USDT_SUPPLY", "llama:USDT"), ("USDC_SUPPLY", "llama:USDC"),
        ("DVOL", "deribit:BTC-DVOL"), ("BTCDVOL", "deribit:BTC-DVOL"), ("ETHDVOL", "deribit:ETH-DVOL"),
        // Rates
        ("US3M", "yahoo:^IRX"), ("US5Y", "yahoo:^FVX"), ("US10Y", "yahoo:^TNX"), ("US30Y", "yahoo:^TYX"),
        ("US2Y", "fred:DGS2"), ("ZN", "yahoo:ZN=F"), ("ZB", "yahoo:ZB=F"), ("ZT", "yahoo:ZT=F"), ("TLT", "yahoo:TLT"),
        ("HYG", "yahoo:HYG"), ("LQD", "yahoo:LQD"), ("JNK", "yahoo:JNK"),
        ("HYOAS", "fred:BAMLH0A0HYM2"), ("IGOAS", "fred:BAMLC0A0CM"), ("SOFR", "fred:SOFR"), ("EFFR", "fred:EFFR"),
        ("RRP", "fred:RRPONTSYD"), ("WALCL", "fred:WALCL"), ("TGA", "treasury:TGA"), ("RESERVES", "fred:WRESBAL"),
        ("T10Y2Y", "fred:T10Y2Y"), ("T10Y3M", "fred:T10Y3M"), ("NFCI", "fred:NFCI"), ("STLFSI", "fred:STLFSI4"),
        // FX
        ("DXY", "yahoo:DX-Y.NYB"), ("EURUSD", "yahoo:EURUSD=X"), ("USDJPY", "yahoo:JPY=X"), ("GBPUSD", "yahoo:GBPUSD=X"),
        ("USDCNH", "yahoo:CNH=X"), ("USDCHF", "yahoo:CHF=X"), ("AUDUSD", "yahoo:AUDUSD=X"), ("USDCAD", "yahoo:CAD=X"),
        ("USDMXN", "yahoo:MXN=X"), ("USDKRW", "yahoo:KRW=X"), ("EURJPY", "yahoo:EURJPY=X"),
        // Commodities
        ("GOLD", "yahoo:GC=F"), ("XAU", "yahoo:GC=F"), ("SILVER", "yahoo:SI=F"), ("XAG", "yahoo:SI=F"),
        ("COPPER", "yahoo:HG=F"), ("WTI", "yahoo:CL=F"), ("OIL", "yahoo:CL=F"), ("BRENT", "yahoo:BZ=F"),
        ("NATGAS", "yahoo:NG=F"), ("PLATINUM", "yahoo:PL=F"), ("WHEAT", "yahoo:ZW=F"), ("CORN", "yahoo:ZC=F"),
        // CFTC Commitments of Traders (weekly futures positioning)
        ("COT_ES", "cftc:13874A"), ("COT_NQ", "cftc:209742"), ("COT_ZN", "cftc:043602"), ("COT_ZT", "cftc:042601"),
        ("COT_ZB", "cftc:020601"), ("COT_GOLD", "cftc:088691"), ("COT_SILVER", "cftc:084691"), ("COT_COPPER", "cftc:085692"),
        ("COT_WTI", "cftc:067651"), ("COT_JPY", "cftc:097741"), ("COT_EUR", "cftc:099741"), ("COT_BTC", "cftc:133741"),
        ("COT_VIX", "cftc:1170E1"),
        // Funding markets, swaps and global yields
        ("SRF", "nyfed:SRF"), ("DEALER_UST", "nyfed:PDPOSGST-TOT"),
        ("SWAP2Y", "dtcc:SOFR2Y"), ("SWAP5Y", "dtcc:SOFR5Y"), ("SWAP10Y", "dtcc:SOFR10Y"), ("SWAP30Y", "dtcc:SOFR30Y"),
        ("JGB2Y", "mof:JGB2Y"), ("JGB10Y", "mof:JGB10Y"), ("JGB30Y", "mof:JGB30Y"), ("ESTR", "ecb:ESTR"), ("EA10Y", "ecb:EA10Y"),
        // China and Hong Kong
        ("CNYFIX", "cfets:USDCNY"), ("SHIBOR3M", "cfets:SHIBOR_3M"), ("CGB10Y", "eastmoney:CGB10Y"), ("CGB30Y", "eastmoney:CGB30Y"),
        ("CN_MARGIN", "eastmoney:MARGIN"), ("SOUTHBOUND", "eastmoney:SOUTHBOUND"), ("CN_BREADTH", "eastmoney:BREADTH"),
        ("HIBOR", "hkma:HIBOR_1M"), ("HK_AGGBAL", "hkma:AGGREGATE_BALANCE"),
        // Options positioning, ETF flows, oil inventories, GDP nowcast
        ("SPX_GEX", "cboe:SPX#gex"), ("BTC_ETF_FLOW", "farside:BTC"), ("ETH_ETF_FLOW", "farside:ETH"),
        ("CRUDE_STOCKS", "fred:WCESTUS1"), ("GDPNOW", "fred:GDPNOW"),
        // 24/7 perps on Hyperliquid HIP-3 (trade while cash markets are closed)
        ("NDX247", "hyperliquid:xyz:XYZ100"), ("GOLD247", "hyperliquid:xyz:GOLD"),
        // Crypto proxies in TradFi
        ("IBIT", "yahoo:IBIT"), ("MSTR", "yahoo:MSTR"), ("COIN", "yahoo:COIN"),
    ];
    pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

/// Load user aliases from `$AUTOREKT_HOME/aliases.yaml` (map ALIAS: id), merged over the defaults.
pub fn aliases() -> BTreeMap<String, String> {
    let mut m = default_aliases();
    let path = crate::config::home().join("aliases.yaml");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let user: BTreeMap<String, String> = serde_yaml::from_str(&text).unwrap_or_else(|e| {
            eprintln!("error: invalid {}: {e}", path.display());
            std::process::exit(2)
        });
        for (k, v) in user {
            m.insert(k.to_uppercase(), v);
        }
    }
    m
}

const QUOTES: &[&str] = &["USDT", "USDC", "FDUSD", "USD", "BUSD", "EUR", "BTC", "ETH"];

/// Normalize a TradingView id like `BINANCE:BTCUSDT.P` or `OKX:BTCUSDT`.
fn from_tradingview(exchange: &str, sym: &str) -> Option<InstrumentId> {
    let provider = exchange.to_lowercase();
    if !["binance", "okx", "bitget", "coinbase", "bybit"].contains(&provider.as_str()) {
        return None;
    }
    let (raw, perp) = match sym.strip_suffix(".P") {
        Some(r) => (r, true),
        None => (sym, false),
    };
    let quote = QUOTES.iter().find(|q| raw.len() > q.len() && raw.ends_with(*q))?;
    let base = &raw[..raw.len() - quote.len()];
    let symbol = if perp { format!("{base}/{quote}:{quote}") } else { format!("{base}/{quote}") };
    Some(InstrumentId { provider, symbol, metric: None })
}

/// Accept common non-CCXT spellings on exchanges and rewrite them to CCXT symbols:
/// `BTC-USDT-SWAP` (OKX native) → `BTC/USDT:USDT`, `BTC-USDT` → `BTC/USDT`,
/// `BTCUSDT` → `BTC/USDT`, `BTCUSDT.P` / `BTCUSDT-PERP` → `BTC/USDT:USDT`.
pub fn normalize_exchange_symbol(provider: &str, sym: &str) -> String {
    if !matches!(provider, "okx" | "binance" | "bitget" | "coinbase") || sym.contains('/') {
        return sym.to_string();
    }
    let up = sym.to_uppercase();
    let parts: Vec<&str> = up.split('-').collect();
    match parts.as_slice() {
        [b, q, "SWAP"] => return format!("{b}/{q}:{}", if *q == "USD" { *b } else { *q }),
        [b, q, e] if e.chars().all(|c| c.is_ascii_digit()) => return format!("{b}/{q}:{}-{e}", if *q == "USD" { *b } else { *q }),
        [b, q] if QUOTES.contains(q) => return format!("{b}/{q}"),
        _ => {}
    }
    let (raw, perp) = if let Some(r) = up.strip_suffix(".P").or_else(|| up.strip_suffix("-PERP")).or_else(|| up.strip_suffix("PERP")) {
        (r.to_string(), true)
    } else {
        (up.clone(), false)
    };
    if let Some(q) = QUOTES.iter().find(|q| raw.len() > q.len() && raw.ends_with(*q)) {
        let b = &raw[..raw.len() - q.len()];
        return if perp { format!("{b}/{q}:{q}") } else { format!("{b}/{q}") };
    }
    sym.to_string()
}

/// Parse a user-facing id or alias.
pub fn parse(input: &str) -> Result<InstrumentId> {
    let input = input.trim();
    if input.is_empty() {
        bail!("empty instrument id");
    }
    if input.contains(char::is_whitespace) {
        bail!("'{input}' contains spaces — it looks like an expression, not an id (expressions work in show, why and watch)");
    }
    let (main, metric) = match input.split_once('#') {
        Some((a, m)) => (a, Some(m.to_string())),
        None => (input, None),
    };
    // provider:symbol
    if let Some((p, rest)) = main.split_once(':') {
        let pl = p.to_lowercase();
        if PROVIDERS.contains(&pl.as_str()) && p == pl {
            if rest.is_empty() {
                bail!("missing symbol after '{p}:'");
            }
            return Ok(InstrumentId { symbol: normalize_exchange_symbol(&pl, rest), provider: pl, metric });
        }
        if let Some(mut id) = from_tradingview(p, rest) {
            id.metric = metric;
            return Ok(id);
        }
        if !PROVIDERS.contains(&pl.as_str()) && p == pl {
            bail!("unknown provider '{p}' (known: {})", PROVIDERS.join(", "));
        }
    }
    let upper = main.to_uppercase();
    let al = aliases();
    if let Some(target) = al.get(&upper) {
        let mut id = parse(target)?;
        if metric.is_some() {
            id.metric = metric;
        }
        return Ok(id);
    }
    // Bare CCXT symbol: assume the default crypto exchange.
    if main.contains('/') {
        return Ok(InstrumentId { provider: crate::config::default_exchange(), symbol: main.to_uppercase(), metric });
    }
    if CRYPTO.contains(&upper.as_str()) {
        return Ok(InstrumentId {
            provider: crate::config::default_exchange(),
            symbol: format!("{upper}/USDT:USDT"),
            metric,
        });
    }
    if upper.len() == 6 && upper.is_ascii() && CURRENCIES.contains(&&upper[..3]) && CURRENCIES.contains(&&upper[3..]) {
        return Ok(InstrumentId { provider: "yahoo".into(), symbol: format!("{upper}=X"), metric });
    }
    // Anything else: treat as a Yahoo ticker (stocks, ETFs, ^indices, =F futures, =X FX).
    Ok(InstrumentId { provider: "yahoo".into(), symbol: upper, metric })
}

/// True when `s` names an alias or a known crypto asset (case-insensitive).
pub fn is_known_alias(s: &str) -> bool {
    let u = s.to_uppercase();
    aliases().contains_key(&u) || CRYPTO.contains(&u.as_str())
}

/// Short display label: the alias if one maps to this id, else the id itself.
pub fn label(id: &InstrumentId) -> String {
    let full = id.base().to_string();
    // Prefer the shortest alias (SPX over SP500, FNG over CRYPTOFNG).
    let best = aliases().into_iter().filter(|(_, t)| *t == full).map(|(a, _)| a).min_by_key(|a| (a.len(), a.clone()));
    if let Some(a) = best {
        return match &id.metric {
            Some(m) => format!("{a}#{m}"),
            None => a,
        };
    }
    if id.provider == crate::config::default_exchange() {
        if let Ok(c) = Ccxt::parse(&id.symbol) {
            if c.is_swap() && c.quote == "USDT" {
                return match &id.metric {
                    Some(m) => format!("{}#{m}", c.base),
                    None => c.base,
                };
            }
        }
    }
    id.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_forms() {
        assert_eq!(parse("okx:BTC/USDT:USDT").unwrap().to_string(), "okx:BTC/USDT:USDT");
        assert_eq!(parse("BINANCE:BTCUSDT.P").unwrap().to_string(), "binance:BTC/USDT:USDT");
        assert_eq!(parse("VIX").unwrap().to_string(), "yahoo:^VIX");
        assert_eq!(parse("AAPL").unwrap().to_string(), "yahoo:AAPL");
        assert_eq!(parse("audjpy").unwrap().to_string(), "yahoo:AUDJPY=X");
        assert_eq!(parse("USDJPY").unwrap().to_string(), "yahoo:JPY=X");
        assert_eq!(parse("btc#funding_rate").unwrap().metric.as_deref(), Some("funding_rate"));
        assert_eq!(parse("yahoo:^GSPC").unwrap().symbol, "^GSPC");
        assert_eq!(parse("okx:BTC-USDT-SWAP").unwrap().symbol, "BTC/USDT:USDT");
        assert_eq!(parse("okx:BTCUSDT").unwrap().symbol, "BTC/USDT");
        assert_eq!(parse("binance:ETHUSDT.P").unwrap().symbol, "ETH/USDT:USDT");
        assert_eq!(parse("okx:BTC-USD-261226").unwrap().symbol, "BTC/USD:BTC-261226");
        let c = Ccxt::parse("BTC/USD:BTC-261226").unwrap();
        assert_eq!(c.expiry.as_deref(), Some("261226"));
    }
}

/// Series whose changes are read as differences (rates, spreads, indices of stress or
/// sentiment), not percentages. Returns the unit label for differences.
pub fn level_unit(id: &InstrumentId) -> Option<&'static str> {
    if let Some(m) = &id.metric {
        return match m.as_str() {
            "p1" | "p25" | "p75" | "p99" if id.provider == "nyfed" => Some("bp"),
            "volume" | "accepted" | "turnover" | "financing" if matches!(id.provider.as_str(), "nyfed" | "treasury" | "eastmoney") => None,
            "net_buy" => Some("CNYbn"),
            _ if id.provider == "farside" => Some("$mn"),
            // Rates expressed as fractions per interval: show differences in basis points.
            "funding_rate" | "funding" | "premium" => Some("rbp"),
            "open_interest" | "oi" | "liquidations" | "liquidations_long" | "liquidations_short" => None,
            _ if matches!(id.provider.as_str(), "polymarket" | "kalshi") => Some("pp"),
            _ => Some("pt"),
        };
    }
    let s = id.symbol.as_str();
    match id.provider.as_str() {
        "fred" => {
            const RATES: &[&str] = &["DGS", "SOFR", "EFFR", "IORB", "OBFR", "TGCR", "BGCR", "BAML", "T10Y", "T5YIE", "T10YIE", "DTB", "DCPF", "DCPN", "DFII", "MORTGAGE", "DPRIME", "RPONTSYD"];
            const IDX: &[&str] = &["NFCI", "ANFCI", "STLFSI", "VIXCLS"];
            if RATES.iter().any(|p| s.starts_with(p)) {
                Some(if s.starts_with("RPONTSYD") { "$bn" } else { "bp" })
            } else if IDX.iter().any(|p| s.starts_with(p)) {
                Some("pt")
            } else if s.starts_with("RRPONTSYD") {
                Some("$bn")
            } else {
                None
            }
        }
        "yahoo" => match s {
            "^TNX" | "^FVX" | "^TYX" | "^IRX" => Some("bp"),
            "^VIX" | "^VIX9D" | "^VIX3M" | "^VVIX" | "^MOVE" | "^SKEW" | "^OVX" | "^GVZ" | "^VXN" => Some("pt"),
            _ => None,
        },
        "nyfed" => match s {
            "SRF" | "RRP" => Some("$bn"),
            _ if s.starts_with("PD") => None,
            _ => Some("bp"),
        },
        "treasury" => Some(if s == "TGA" { "$bn" } else { "bp" }),
        "dtcc" | "mof" | "ecb" => Some("bp"),
        "cfets" if s.starts_with("SHIBOR") => Some("bp"),
        "eastmoney" => match s {
            "MARGIN" | "NORTHBOUND" => None,
            "SOUTHBOUND" => Some("CNYbn"),
            "BREADTH" => Some("pt"),
            _ => Some("bp"),
        },
        "hkma" if s.starts_with("HIBOR") => Some("bp"),
        "farside" => Some("$mn"),
        "cboe" => Some("pt"),
        "alt" => Some("pt"),
        "deribit" if s.ends_with("-DVOL") => Some("pt"),
        "polymarket" | "kalshi" => Some("pp"),
        "cftc" => Some("ct"),
        "finra" => Some("pp"),
        _ => None,
    }
}

/// Units of levels that are not prices or percent rates, for display.
pub fn unit_note(id: &InstrumentId) -> Option<&'static str> {
    let s = id.symbol.as_str();
    Some(match (id.provider.as_str(), id.metric.as_deref()) {
        ("fred", None) => match s {
            "WRESBAL" | "WTREGEN" | "WALCL" | "WLRRAL" | "WSHOTSL" => "USD mn",
            "RRPONTSYD" | "RPONTSYD" => "USD bn",
            _ => return None,
        },
        ("nyfed", None) if s == "SRF" || s == "RRP" => "USD bn",
        ("nyfed", None) if s.starts_with("PD") => "USD mn",
        ("treasury", None) if s == "TGA" => "USD bn",
        ("eastmoney", None) if matches!(s, "MARGIN" | "SOUTHBOUND" | "NORTHBOUND") => "CNY bn",
        ("eastmoney", Some("financing" | "net_buy" | "turnover")) => "CNY bn",
        ("hkma", None) if s == "AGGREGATE_BALANCE" => "HKD bn",
        ("farside", _) => "USD mn",
        ("cboe", None | Some("gex")) => "USD bn per 1% move",
        _ => return None,
    })
}
