//! autorekt — market observation for agents. See `autorekt --help`.

mod cli;
mod cmds;
mod columns;
mod config;
mod daemon;
mod expr;
mod freshness;
mod http;
mod ids;
mod model;
mod os;
mod providers;
mod render;
mod rules;
mod sets;
mod store;
mod structure;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime");
    let code = rt.block_on(cli::run());
    std::process::exit(code);
}
