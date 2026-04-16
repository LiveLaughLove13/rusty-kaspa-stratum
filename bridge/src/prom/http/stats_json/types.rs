use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub(crate) struct InternalCpuStats {
    pub(crate) hashrateGhs: f64,
    pub(crate) hashesTried: u64,
    pub(crate) blocksSubmitted: u64,
    pub(crate) blocksAccepted: u64,
    pub(crate) shares: u64,
    pub(crate) stale: u64,
    pub(crate) invalid: u64,
    pub(crate) wallet: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub(crate) struct StatsResponse {
    pub(crate) totalBlocks: u64,
    #[serde(default)]
    pub(crate) totalBlocksAcceptedByNode: u64,
    #[serde(default)]
    pub(crate) totalBlocksNotConfirmedBlue: u64,
    pub(crate) totalShares: u64,
    pub(crate) networkHashrate: u64,
    pub(crate) networkDifficulty: f64,
    pub(crate) networkBlockCount: u64,
    pub(crate) activeWorkers: usize,
    pub(crate) internalCpu: Option<InternalCpuStats>,
    pub(crate) blocks: Vec<BlockInfo>,
    pub(crate) workers: Vec<WorkerInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bridgeUptime: Option<u64>, // Bridge uptime in seconds
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BlockInfo {
    pub(crate) instance: String,
    pub(crate) worker: String,
    pub(crate) wallet: String,
    pub(crate) timestamp: String,
    pub(crate) hash: String,
    pub(crate) nonce: String,
    pub(crate) bluescore: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct WorkerInfo {
    pub(crate) instance: String,
    pub(crate) worker: String,
    pub(crate) wallet: String,
    pub(crate) hashrate: f64,
    pub(crate) shares: u64,
    pub(crate) stale: u64,
    pub(crate) invalid: u64,
    #[serde(default, rename = "duplicateShares")]
    pub(crate) duplicate_shares: u64,
    #[serde(default, rename = "weakShares")]
    pub(crate) weak_shares: u64,
    pub(crate) blocks: u64,
    #[serde(default, rename = "disconnects")]
    pub(crate) disconnects: u64,
    #[serde(default, rename = "jobs")]
    pub(crate) jobs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "balanceKas")]
    pub(crate) balance_kas: Option<f64>,
    #[serde(default, rename = "errors")]
    pub(crate) errors: u64,
    #[serde(default, rename = "blocksAcceptedByNode")]
    pub(crate) blocks_accepted_by_node: u64,
    #[serde(default, rename = "blocksNotConfirmedBlue")]
    pub(crate) blocks_not_confirmed_blue: u64,
    #[serde(skip_serializing_if = "Option::is_none", rename = "lastSeen")]
    pub(crate) last_seen: Option<u64>, // Unix timestamp in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>, // "online", "offline", or "idle"
    #[serde(skip_serializing_if = "Option::is_none", rename = "currentDifficulty")]
    pub(crate) current_difficulty: Option<f64>, // Current mining difficulty assigned to this worker
    #[serde(skip_serializing_if = "Option::is_none", rename = "sessionUptime")]
    pub(crate) session_uptime: Option<u64>, // Session uptime in seconds (time since last connection)
}
