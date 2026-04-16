mod duplicate_submit;
mod kaspa_api_trait;
mod lifecycle;
mod submit;
mod vardiff;
mod work_stats;

pub use kaspa_api_trait::KaspaApiTrait;
pub use submit::{SubmitError, SubmitRunError};
#[cfg(feature = "rkstratum_cpu_miner")]
pub use work_stats::{RKSTRATUM_CPU_MINER_METRICS, set_rkstratum_cpu_miner_metrics};
pub use work_stats::{STATS_PRINTER_STARTED, WorkStats};

use duplicate_submit::DuplicateSubmitGuard;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ShareHandler {
    #[allow(dead_code)]
    tip_blue_score: Arc<Mutex<u64>>,
    stats: Arc<Mutex<HashMap<String, WorkStats>>>,
    overall: Arc<WorkStats>,
    instance_id: String, // Instance identifier for logging
    duplicate_submit_guard: Arc<Mutex<DuplicateSubmitGuard>>,
}

impl ShareHandler {
    pub fn log_prefix(&self) -> String {
        format!("[{}]", self.instance_id)
    }
}
