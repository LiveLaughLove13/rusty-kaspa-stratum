//! `stratum-bridge` binary entrypoint. All mining, Stratum listeners, and kaspad handling run in
//! `kaspa_stratum_bridge::runner::run` (the same entrypoint as `rkstratum-bridge-desktop`).

use clap::Parser;
use kaspa_alloc::init_allocator_with_default_settings;
use kaspa_stratum_bridge::cli::Cli;
use kaspa_stratum_bridge::runner::run;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init_allocator_with_default_settings();
    run(Cli::parse()).await
}
