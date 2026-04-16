//! Kaspa gRPC client (`KaspaApi`), node status snapshot for dashboards, and coinbase tag helpers.
//!
//! Split across `coinbase_tag`, `node_status`, and `api` (`block_submit_guard`, `streams`, `template_submit`).

mod api;
mod coinbase_tag;
mod node_status;

pub use api::KaspaApi;
pub use node_status::{NODE_STATUS, NodeStatusApi, NodeStatusSnapshot, network_display_from_id, node_status_for_api};
