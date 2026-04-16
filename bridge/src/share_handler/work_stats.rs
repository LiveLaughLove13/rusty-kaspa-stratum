#[cfg(feature = "rkstratum_cpu_miner")]
use crate::rkstratum_cpu_miner::InternalMinerMetrics;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[derive(Clone)]
pub struct WorkStats {
    pub blocks_found: Arc<Mutex<i64>>,
    pub shares_found: Arc<Mutex<i64>>,
    pub shares_diff: Arc<Mutex<f64>>,
    pub stale_shares: Arc<Mutex<i64>>,
    pub invalid_shares: Arc<Mutex<i64>>,
    pub worker_name: Arc<Mutex<String>>,
    pub start_time: Instant,
    pub last_share: Arc<Mutex<Instant>>,
    pub var_diff_start_time: Arc<Mutex<Option<Instant>>>,
    pub var_diff_shares_found: Arc<Mutex<i64>>,
    pub var_diff_window: Arc<Mutex<usize>>,
    pub min_diff: Arc<Mutex<f64>>,
}

impl WorkStats {
    pub fn new(worker_name: String) -> Self {
        Self {
            blocks_found: Arc::new(Mutex::new(0)),
            shares_found: Arc::new(Mutex::new(0)),
            shares_diff: Arc::new(Mutex::new(0.0)),
            stale_shares: Arc::new(Mutex::new(0)),
            invalid_shares: Arc::new(Mutex::new(0)),
            worker_name: Arc::new(Mutex::new(worker_name)),
            start_time: Instant::now(),
            last_share: Arc::new(Mutex::new(Instant::now())),
            var_diff_start_time: Arc::new(Mutex::new(None)),
            var_diff_shares_found: Arc::new(Mutex::new(0)),
            var_diff_window: Arc::new(Mutex::new(0)),
            min_diff: Arc::new(Mutex::new(0.0)),
        }
    }
}

pub(crate) struct StatsPrinterEntry {
    pub instance_id: String,
    pub inst_short: String,
    pub target_spm: f64,
    pub start: Instant,
    pub stats: Arc<Mutex<HashMap<String, WorkStats>>>,
    pub overall: Arc<WorkStats>,
}

pub(crate) static STATS_PRINTER_REGISTRY: Lazy<Mutex<Vec<StatsPrinterEntry>>> =
    Lazy::new(|| Mutex::new(Vec::new()));
pub static STATS_PRINTER_STARTED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "rkstratum_cpu_miner")]
pub static RKSTRATUM_CPU_MINER_METRICS: Lazy<
    parking_lot::Mutex<Option<Arc<InternalMinerMetrics>>>,
> = Lazy::new(|| parking_lot::Mutex::new(None));

#[cfg(feature = "rkstratum_cpu_miner")]
pub fn set_rkstratum_cpu_miner_metrics(metrics: Arc<InternalMinerMetrics>) {
    *RKSTRATUM_CPU_MINER_METRICS.lock() = Some(metrics);
}

pub(crate) fn format_hashrate(ghs: f64) -> String {
    if ghs < 1.0 {
        format!("{:.2}MH/s", ghs * 1000.0)
    } else if ghs < 1000.0 {
        format!("{:.2}GH/s", ghs)
    } else {
        format!("{:.2}TH/s", ghs / 1000.0)
    }
}
