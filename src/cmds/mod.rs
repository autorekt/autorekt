//! The five commands (show, search, why, watch, daemon) and the views and helpers behind them.

mod alerts;
mod analysis;
mod brief;
mod context;
mod crypto;
mod monitor;
mod report;
mod rule;
mod search;
mod service;
mod share;
mod show;
mod view;
mod watch;

pub use analysis::why;
pub use search::search;
pub use service::daemon;
pub use show::show;
pub use watch::watch;

use crate::cli::Global;
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
