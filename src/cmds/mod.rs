//! Command implementations, one file per concern. `cli.rs` parses arguments and calls these.

mod analysis;
mod brief;
mod crypto;
mod monitor;
mod rule;
mod service;
mod share;
mod view;
mod watch;

pub use analysis::{corr, why};
pub use brief::brief;
pub use crypto::{funding, prelaunch};
pub use monitor::{monitor_add, monitor_ls, monitor_new, monitor_rm, monitor_sdk};
pub use rule::rule;
pub use service::{service, tail};
pub use share::share;
pub use view::{board, calendar, candles, markets, metric, news, prob_search, prob_show, prob_trending, quote, resolve, sources};
pub use watch::{diff, events, wait};

use crate::cli::{Global, RuleOp};
use crate::model::*;
use crate::providers::{self, Range};
use crate::render::{self, board, Opts};
use crate::{config, daemon, expr, ids, rules, store};
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::path::Path;

/// Print any serializable value as pretty JSON (the `--json` output of every command).
pub fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

pub(crate) fn ago(ms: i64) -> String {
    let d = (now_ms() - ms) / 1000;
    match d {
        d if d < 0 => "in future".into(),
        d if d < 90 => format!("{d}s ago"),
        d if d < 5400 => format!("{}m ago", d / 60),
        d if d < 172_800 => format!("{}h ago", d / 3600),
        d => format!("{}d ago", d / 86400),
    }
}
