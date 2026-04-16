//! Kaspa Stratum bridge library.
//!
//! Implementation files are grouped under `util/`, `jsonrpc/`, `mining/`, `stratum/`, `config/`,
//! `kaspa/`, `host/`, and `cpu_miner/`. Submodules such as `stratum_line_codec`, `kaspaapi`, and
//! `pow_diagnostic` keep stable paths. The crate root lists re-exports explicitly (no glob
//! re-exports). YAML/config `BridgeConfig` comes from `app_config`; per-instance stratum settings
//! use `StratumServerBridgeConfig` (the stratum listener’s `BridgeConfig`). Internal CPU miner prom
//! helpers are only on `prom` when built with `rkstratum_cpu_miner`.

mod util {
    pub mod errors;
    pub mod log_colors;
    pub mod net_utils;
}

mod jsonrpc {
    pub mod jsonrpc_event;
}

mod mining {
    pub mod hasher;
    pub mod mining_state;
    pub mod pow_diagnostic;
}

mod stratum {
    pub mod client_handler;
    pub mod default_client;
    pub mod stratum_context;
    pub mod stratum_line_codec;
    pub mod stratum_listener;
    pub mod stratum_server;
}

mod config {
    pub mod app_config;
}

mod kaspa {
    pub mod kaspaapi;
}

mod host {
    pub mod host_metrics;
}

#[cfg(feature = "rkstratum_cpu_miner")]
mod cpu_miner {
    pub mod rkstratum_cpu_miner;
}

// Public module paths unchanged for downstream / tests.
pub use config::app_config;
pub use host::host_metrics;
pub use jsonrpc::jsonrpc_event;
pub use kaspa::kaspaapi;
pub use mining::hasher;
pub use mining::mining_state;
pub use mining::pow_diagnostic;
pub use stratum::client_handler;
pub use stratum::default_client;
pub use stratum::stratum_context;
pub use stratum::stratum_line_codec;
pub use stratum::stratum_listener;
pub use stratum::stratum_server;
pub use util::errors;
pub use util::log_colors;
pub use util::net_utils;

pub mod prom;
pub mod share_handler;

pub mod app_dirs;
pub mod cli;
pub mod health_check;
pub mod inprocess_node;
pub mod runner;
pub mod tracing_setup;

#[cfg(test)]
mod tests;

mod bridge_error;

#[cfg(feature = "rkstratum_cpu_miner")]
pub use cpu_miner::rkstratum_cpu_miner;

pub use app_config::{BridgeConfig, InstanceConfig};
pub use bridge_error::BridgeError;
pub use client_handler::ClientHandler;
pub use default_client::{default_handlers, default_logger};
pub use errors::ErrorShortCode;
pub use hasher::{
    KaspaDiff, big_diff_to_little, calculate_target, diff_to_hash, diff_to_target, diff_to_target_alternative,
    generate_iceriver_job_params, generate_job_header, generate_large_job_params, serialize_block_header,
    stratum_difficulty_to_target_kaspa,
};
pub use jsonrpc_event::{JsonRpcEvent, JsonRpcResponse, StratumMethod, unmarshal_event, unmarshal_response};
pub use kaspaapi::{KaspaApi, NODE_STATUS, NodeStatusApi, NodeStatusSnapshot, network_display_from_id, node_status_for_api};
pub use log_colors::LogColors;
pub use mining_state::{GetMiningState, Job, MiningState};
pub use net_utils::{bind_addr_for_operator_http, bind_addr_from_port, normalize_port};
pub use prom::{
    WorkerContext, init_metrics, init_worker_counters, record_balances, record_block_accepted_by_node, record_block_found,
    record_block_not_confirmed_blue, record_disconnect, record_dupe_share, record_invalid_share, record_network_stats, record_new_job,
    record_share_found, record_stale_share, record_weak_share, record_worker_error, set_web_config_path, set_web_status_config,
    start_prom_server, start_web_server_all, update_worker_difficulty,
};
#[cfg(feature = "rkstratum_cpu_miner")]
pub use rkstratum_cpu_miner::{InternalCpuMinerConfig, InternalMinerMetrics, spawn_internal_cpu_miner};
pub use share_handler::{KaspaApiTrait, STATS_PRINTER_STARTED, ShareHandler, SubmitError, SubmitRunError, WorkStats};
#[cfg(feature = "rkstratum_cpu_miner")]
pub use share_handler::{RKSTRATUM_CPU_MINER_METRICS, set_rkstratum_cpu_miner_metrics};
pub use stratum_context::{ClientIdentity, ContextSummary, ErrorDisconnected, StratumContext};
pub use stratum_line_codec::{line_looks_like_http, push_lossy_and_drain_lines, strip_nul_bytes};
pub use stratum_listener::{
    EventHandler, StateGenerator, StratumClientListener, StratumListener, StratumListenerConfig, StratumStats,
};
/// Per-instance stratum listener settings (distinct from `BridgeConfig` in `app_config`).
pub use stratum_server::BridgeConfig as StratumServerBridgeConfig;
pub use stratum_server::{listen_and_serve, listen_and_serve_with_shutdown, start_block_template_listener_with_api};

pub use runner::{config_yaml_candidate_paths, default_dashboard_iframe_url, request_bridge_shutdown, run};
