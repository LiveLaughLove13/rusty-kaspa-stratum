use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Serialize;

#[derive(Clone, Debug, Default)]
pub struct NodeStatusSnapshot {
    pub last_updated: Option<std::time::Instant>,
    /// Wall clock ms since UNIX epoch when the snapshot was last refreshed (for dashboards).
    pub last_updated_unix_ms: Option<u64>,
    pub is_connected: bool,
    pub is_synced: Option<bool>,
    pub network_id: Option<String>,
    pub server_version: Option<String>,
    pub virtual_daa_score: Option<u64>,
    pub sink_blue_score: Option<u64>,
    pub block_count: Option<u64>,
    pub header_count: Option<u64>,
    pub difficulty: Option<f64>,
    pub tip_hash: Option<String>,
    pub peers: Option<usize>,
    pub mempool_size: Option<u64>,
}

pub static NODE_STATUS: Lazy<Mutex<NodeStatusSnapshot>> =
    Lazy::new(|| Mutex::new(NodeStatusSnapshot::default()));

/// JSON-friendly node snapshot for `/api/status` (camelCase matches prior dashboard conventions for nested objects).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatusApi {
    pub is_connected: bool,
    pub is_synced: Option<bool>,
    pub network_id: Option<String>,
    pub network_display: Option<String>,
    pub server_version: Option<String>,
    pub virtual_daa_score: Option<u64>,
    pub sink_blue_score: Option<u64>,
    pub block_count: Option<u64>,
    pub header_count: Option<u64>,
    /// DAG difficulty from the node (RPC); distinct from Prometheus-estimated network difficulty on the dashboard.
    pub difficulty: Option<f64>,
    pub tip_hash: Option<String>,
    pub peers: Option<usize>,
    pub mempool_size: Option<u64>,
    pub last_updated_unix_ms: Option<u64>,
}

/// Short network label for UI (same parsing idea as the `[NODE]` log line).
pub fn network_display_from_id(network_id: Option<&str>) -> Option<String> {
    let net = network_id?.trim();
    if net.is_empty() || net == "-" {
        return None;
    }
    let mut network_type = None;
    let mut suffix = None;
    if let Some(pos) = net.find("network_type:") {
        let s = &net[pos + "network_type:".len()..];
        let s = s.trim_start();
        network_type = s.split(&[',', '}'][..]).next().map(|v| v.trim());
    }
    if let Some(pos) = net.find("suffix:") {
        let s = &net[pos + "suffix:".len()..];
        let s = s.trim_start();
        let raw = s.split(&[',', '}'][..]).next().map(|v| v.trim());
        if raw != Some("None") {
            suffix = raw;
        }
    }
    Some(match (network_type, suffix) {
        (Some(nt), Some(suf)) => format!("{}-{}", nt, suf),
        (Some(nt), None) => nt.to_string(),
        _ => net.to_string(),
    })
}

pub fn node_status_for_api() -> NodeStatusApi {
    let s = NODE_STATUS.lock();
    NodeStatusApi {
        is_connected: s.is_connected,
        is_synced: s.is_synced,
        network_id: s.network_id.clone(),
        network_display: network_display_from_id(s.network_id.as_deref()),
        server_version: s.server_version.clone(),
        virtual_daa_score: s.virtual_daa_score,
        sink_blue_score: s.sink_blue_score,
        block_count: s.block_count,
        header_count: s.header_count,
        difficulty: s.difficulty,
        tip_hash: s.tip_hash.clone(),
        peers: s.peers,
        mempool_size: s.mempool_size,
        last_updated_unix_ms: s.last_updated_unix_ms,
    }
}
