//! Prometheus metrics, worker counters, and HTTP dashboard (`/metrics`, `/api/*`, static files).
//! Implementation is split across `metrics` and `http` (`static_files`, `stats_json`, `config_api`, `serve`).

mod http;
mod metrics;

pub use http::{set_web_config_path, set_web_status_config, start_prom_server, start_web_server_all};
pub use metrics::*;
