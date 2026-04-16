use prometheus::proto::MetricFamily;
#[cfg(feature = "rkstratum_cpu_miner")]
use prometheus::{Counter, register_counter};
use prometheus::{
    CounterVec, Gauge, GaugeVec, register_counter_vec, register_gauge, register_gauge_vec,
};
use std::collections::HashMap;
#[cfg(feature = "rkstratum_cpu_miner")]
use std::collections::VecDeque;
use std::sync::OnceLock;
use std::time::Instant;

/// Worker labels for Prometheus metrics
const WORKER_LABELS: &[&str] = &["instance", "worker", "miner", "wallet", "ip"];

/// Invalid share type labels
const INVALID_LABELS: &[&str] = &["instance", "worker", "miner", "wallet", "ip", "type"];

/// Block labels
const BLOCK_LABELS: &[&str] = &[
    "instance",
    "worker",
    "miner",
    "wallet",
    "ip",
    "nonce",
    "bluescore",
    "timestamp",
    "hash",
];

/// Error labels
const ERROR_LABELS: &[&str] = &["instance", "wallet", "error"];

/// Balance labels
const BALANCE_LABELS: &[&str] = &["instance", "wallet"];

/// Share counter - number of valid shares found by worker
static SHARE_COUNTER: OnceLock<CounterVec> = OnceLock::new();

/// Share difficulty counter - total difficulty of shares found by worker
static SHARE_DIFF_COUNTER: OnceLock<CounterVec> = OnceLock::new();

/// Invalid share counter - number of invalid/stale/duplicate/weak shares
static INVALID_COUNTER: OnceLock<CounterVec> = OnceLock::new();

/// Block counter - number of blocks mined
static BLOCK_COUNTER: OnceLock<CounterVec> = OnceLock::new();

static BLOCK_ACCEPTED_COUNTER: OnceLock<CounterVec> = OnceLock::new();

static BLOCK_NOT_CONFIRMED_BLUE_COUNTER: OnceLock<CounterVec> = OnceLock::new();

/// Block gauge - unique instances per block mined
static BLOCK_GAUGE: OnceLock<GaugeVec> = OnceLock::new();

/// Disconnect counter - number of disconnects by worker
static DISCONNECT_COUNTER: OnceLock<CounterVec> = OnceLock::new();

/// Job counter - number of jobs sent to miner
static JOB_COUNTER: OnceLock<CounterVec> = OnceLock::new();

/// Balance gauge - wallet balance for connected workers
static BALANCE_GAUGE: OnceLock<GaugeVec> = OnceLock::new();

/// Error counter - errors by worker
static ERROR_BY_WALLET: OnceLock<CounterVec> = OnceLock::new();

/// Estimated network hashrate gauge
static ESTIMATED_NETWORK_HASHRATE: OnceLock<Gauge> = OnceLock::new();

/// Network difficulty gauge
static NETWORK_DIFFICULTY: OnceLock<Gauge> = OnceLock::new();

/// Network block count gauge
static NETWORK_BLOCK_COUNT: OnceLock<Gauge> = OnceLock::new();

/// Worker start time gauge (Unix timestamp in seconds)
static WORKER_START_TIME: OnceLock<GaugeVec> = OnceLock::new();

/// Worker current difficulty gauge (current mining difficulty assigned to worker)
static WORKER_CURRENT_DIFFICULTY: OnceLock<GaugeVec> = OnceLock::new();

/// Worker last activity time - tracks when each worker last submitted a share
/// Key: "instance:worker:wallet", Value: Instant of last activity
pub(crate) static WORKER_LAST_ACTIVITY: OnceLock<parking_lot::Mutex<HashMap<String, Instant>>> =
    OnceLock::new();

/// Bridge start time - tracks when the bridge started (for uptime calculation)
pub(crate) static BRIDGE_START_TIME: OnceLock<Instant> = OnceLock::new();

// ---------------------------
// Internal CPU miner metrics (feature-gated)
// ---------------------------
#[cfg(feature = "rkstratum_cpu_miner")]
static INTERNAL_CPU_HASHES_TRIED_TOTAL: OnceLock<Counter> = OnceLock::new();
#[cfg(feature = "rkstratum_cpu_miner")]
static INTERNAL_CPU_BLOCKS_SUBMITTED_TOTAL: OnceLock<Counter> = OnceLock::new();
#[cfg(feature = "rkstratum_cpu_miner")]
static INTERNAL_CPU_BLOCKS_ACCEPTED_TOTAL: OnceLock<Counter> = OnceLock::new();
#[cfg(feature = "rkstratum_cpu_miner")]
static INTERNAL_CPU_HASHRATE_GHS: OnceLock<Gauge> = OnceLock::new();
#[cfg(feature = "rkstratum_cpu_miner")]
pub(crate) static INTERNAL_CPU_MINING_ADDRESS: OnceLock<String> = OnceLock::new();
#[cfg(feature = "rkstratum_cpu_miner")]
pub(crate) static INTERNAL_CPU_RECENT_BLOCKS: OnceLock<
    parking_lot::Mutex<VecDeque<InternalCpuBlock>>,
> = OnceLock::new();
#[cfg(feature = "rkstratum_cpu_miner")]
const INTERNAL_CPU_RECENT_BLOCKS_LIMIT: usize = 256;

/// Initialize Prometheus metrics
pub fn init_metrics() {
    // Record bridge start time for uptime calculation
    BRIDGE_START_TIME.get_or_init(Instant::now);
    SHARE_COUNTER.get_or_init(|| {
        register_counter_vec!(
            "ks_valid_share_counter",
            "Number of shares found by worker over time",
            WORKER_LABELS
        )
        .unwrap()
    });

    SHARE_DIFF_COUNTER.get_or_init(|| {
        register_counter_vec!(
            "ks_valid_share_diff_counter",
            "Total difficulty of shares found by worker over time",
            WORKER_LABELS
        )
        .unwrap()
    });

    INVALID_COUNTER.get_or_init(|| {
        register_counter_vec!(
            "ks_invalid_share_counter",
            "Number of stale shares found by worker over time",
            INVALID_LABELS
        )
        .unwrap()
    });

    BLOCK_COUNTER.get_or_init(|| {
        register_counter_vec!(
            "ks_blocks_mined",
            "Number of blocks mined over time",
            WORKER_LABELS
        )
        .unwrap()
    });

    BLOCK_ACCEPTED_COUNTER.get_or_init(|| {
        register_counter_vec!(
            "ks_blocks_accepted_by_node",
            "Number of blocks accepted by the connected Kaspa node (may later be red)",
            WORKER_LABELS
        )
        .unwrap()
    });

    BLOCK_NOT_CONFIRMED_BLUE_COUNTER.get_or_init(|| {
        register_counter_vec!(
            "ks_blocks_not_confirmed_blue",
            "Number of node-accepted blocks that were not confirmed blue within the confirmation window",
            WORKER_LABELS
        )
        .unwrap()
    });

    BLOCK_GAUGE.get_or_init(|| {
        register_gauge_vec!(
            "ks_mined_blocks_gauge",
            "Gauge containing 1 unique instance per block mined",
            BLOCK_LABELS
        )
        .unwrap()
    });

    DISCONNECT_COUNTER.get_or_init(|| {
        register_counter_vec!(
            "ks_worker_disconnect_counter",
            "Number of disconnects by worker",
            WORKER_LABELS
        )
        .unwrap()
    });

    JOB_COUNTER.get_or_init(|| {
        register_counter_vec!(
            "ks_worker_job_counter",
            "Number of jobs sent to the miner by worker over time",
            WORKER_LABELS
        )
        .unwrap()
    });

    BALANCE_GAUGE.get_or_init(|| {
        register_gauge_vec!(
            "ks_balance_by_wallet_gauge",
            "Gauge representing the wallet balance for connected workers",
            BALANCE_LABELS
        )
        .unwrap()
    });

    ERROR_BY_WALLET.get_or_init(|| {
        register_counter_vec!(
            "ks_worker_errors",
            "Gauge representing errors by worker",
            ERROR_LABELS
        )
        .unwrap()
    });

    ESTIMATED_NETWORK_HASHRATE.get_or_init(|| {
        register_gauge!(
            "ks_estimated_network_hashrate_gauge",
            "Gauge representing the estimated network hashrate"
        )
        .unwrap()
    });

    NETWORK_DIFFICULTY.get_or_init(|| {
        register_gauge!(
            "ks_network_difficulty_gauge",
            "Gauge representing the network difficulty"
        )
        .unwrap()
    });

    NETWORK_BLOCK_COUNT.get_or_init(|| {
        register_gauge!(
            "ks_network_block_count",
            "Gauge representing the network block count"
        )
        .unwrap()
    });

    WORKER_START_TIME.get_or_init(|| {
        register_gauge_vec!(
            "ks_worker_start_time",
            "Unix timestamp (seconds) when worker first connected",
            WORKER_LABELS
        )
        .unwrap()
    });

    WORKER_CURRENT_DIFFICULTY.get_or_init(|| {
        register_gauge_vec!(
            "ks_worker_current_difficulty",
            "Current mining difficulty assigned to worker",
            WORKER_LABELS
        )
        .unwrap()
    });

    // Internal CPU miner metrics (no labels; there is only one internal miner per process)
    #[cfg(feature = "rkstratum_cpu_miner")]
    {
        INTERNAL_CPU_HASHES_TRIED_TOTAL.get_or_init(|| {
            register_counter!(
                "ks_internal_cpu_hashes_tried_total",
                "Total hashes tried by the internal CPU miner since process start"
            )
            .unwrap()
        });
        INTERNAL_CPU_BLOCKS_SUBMITTED_TOTAL.get_or_init(|| {
            register_counter!(
                "ks_internal_cpu_blocks_submitted_total",
                "Total blocks submitted by the internal CPU miner since process start"
            )
            .unwrap()
        });
        INTERNAL_CPU_BLOCKS_ACCEPTED_TOTAL.get_or_init(|| {
            register_counter!(
                "ks_internal_cpu_blocks_accepted_total",
                "Total blocks accepted by the connected Kaspa node from the internal CPU miner since process start"
            )
            .unwrap()
        });
        INTERNAL_CPU_HASHRATE_GHS.get_or_init(|| {
            register_gauge!(
                "ks_internal_cpu_hashrate_ghs",
                "Internal CPU miner hashrate (GH/s)"
            )
            .unwrap()
        });
    }
}

/// Update internal CPU miner metrics from a snapshot.
/// Values should be monotonically increasing counts; this function converts them to Prometheus counters.
#[cfg(feature = "rkstratum_cpu_miner")]
pub fn record_internal_cpu_miner_snapshot(
    hashes_tried: u64,
    blocks_submitted: u64,
    blocks_accepted: u64,
    hashrate_ghs: f64,
) {
    // Ensure metrics are registered even if the prom server hasn't started yet.
    init_metrics();

    if let Some(c) = INTERNAL_CPU_HASHES_TRIED_TOTAL.get() {
        let current = c.get() as u64;
        if hashes_tried > current {
            c.inc_by((hashes_tried - current) as f64);
        }
    }
    if let Some(c) = INTERNAL_CPU_BLOCKS_SUBMITTED_TOTAL.get() {
        let current = c.get() as u64;
        if blocks_submitted > current {
            c.inc_by((blocks_submitted - current) as f64);
        }
    }
    if let Some(c) = INTERNAL_CPU_BLOCKS_ACCEPTED_TOTAL.get() {
        let current = c.get() as u64;
        if blocks_accepted > current {
            c.inc_by((blocks_accepted - current) as f64);
        }
    }
    if let Some(g) = INTERNAL_CPU_HASHRATE_GHS.get() {
        let v = if hashrate_ghs.is_finite() && hashrate_ghs >= 0.0 {
            hashrate_ghs
        } else {
            0.0
        };
        g.set(v);
    }
}

/// Store the internal CPU miner reward address for display in `/api/stats`.
/// Best-effort: if called multiple times, only the first value is kept.
#[cfg(feature = "rkstratum_cpu_miner")]
pub fn set_internal_cpu_mining_address(addr: String) {
    let addr = addr.trim().to_string();
    if addr.is_empty() {
        return;
    }
    let _ = INTERNAL_CPU_MINING_ADDRESS.set(addr);
}

#[cfg(feature = "rkstratum_cpu_miner")]
#[derive(Clone, Debug)]
pub(crate) struct InternalCpuBlock {
    pub(crate) timestamp_unix: u64,
    pub(crate) bluescore: u64,
    pub(crate) nonce: u64,
    pub(crate) hash: String,
}

/// Record a recently submitted internal CPU miner block so the dashboard can display it
/// without relying on high-cardinality Prometheus labels.
#[cfg(feature = "rkstratum_cpu_miner")]
pub fn record_internal_cpu_recent_block(hash: String, nonce: u64, bluescore: u64) {
    use std::time::{SystemTime, UNIX_EPOCH};

    if hash.trim().is_empty() {
        return;
    }

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut q = INTERNAL_CPU_RECENT_BLOCKS
        .get_or_init(|| {
            parking_lot::Mutex::new(VecDeque::with_capacity(INTERNAL_CPU_RECENT_BLOCKS_LIMIT))
        })
        .lock();

    // De-dupe by hash
    if q.iter().any(|b| b.hash == hash) {
        return;
    }

    q.push_front(InternalCpuBlock {
        timestamp_unix: ts,
        bluescore,
        nonce,
        hash,
    });
    if q.len() > INTERNAL_CPU_RECENT_BLOCKS_LIMIT {
        q.truncate(INTERNAL_CPU_RECENT_BLOCKS_LIMIT);
    }
}

pub struct WorkerContext {
    pub instance_id: String,
    pub worker_name: String,
    pub miner: String,
    pub wallet: String,
    pub ip: String,
}

impl WorkerContext {
    pub fn labels(&self) -> Vec<&str> {
        vec![
            &self.instance_id,
            &self.worker_name,
            &self.miner,
            &self.wallet,
            &self.ip,
        ]
    }
}

pub fn record_block_accepted_by_node(worker: &WorkerContext) {
    if let Some(counter) = BLOCK_ACCEPTED_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc();
    }
}

pub fn record_block_not_confirmed_blue(worker: &WorkerContext) {
    if let Some(counter) = BLOCK_NOT_CONFIRMED_BLUE_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc();
    }
}

/// Record a valid share found
pub fn record_share_found(worker: &WorkerContext, share_diff: f64) {
    if let Some(counter) = SHARE_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc();
    }
    if let Some(counter) = SHARE_DIFF_COUNTER.get() {
        counter
            .with_label_values(&worker.labels())
            .inc_by(share_diff);
    }
    // Update last activity time for this worker
    update_worker_activity(worker);
}

/// Record a stale share
pub fn record_stale_share(worker: &WorkerContext) {
    if let Some(counter) = INVALID_COUNTER.get() {
        let mut labels = worker.labels();
        labels.push("stale");
        counter.with_label_values(&labels).inc();
    }
    // Update activity time - worker is still connected even if share is stale
    update_worker_activity(worker);
}

/// Record a duplicate share
pub fn record_dupe_share(worker: &WorkerContext) {
    if let Some(counter) = INVALID_COUNTER.get() {
        let mut labels = worker.labels();
        labels.push("duplicate");
        counter.with_label_values(&labels).inc();
    }
    // Update activity time - worker is still connected even if share is duplicate
    update_worker_activity(worker);
}

/// Record an invalid share
pub fn record_invalid_share(worker: &WorkerContext) {
    if let Some(counter) = INVALID_COUNTER.get() {
        let mut labels = worker.labels();
        labels.push("invalid");
        counter.with_label_values(&labels).inc();
    }
    // Update activity time - worker is still connected even if share is invalid
    update_worker_activity(worker);
}

/// Record a weak share
pub fn record_weak_share(worker: &WorkerContext) {
    if let Some(counter) = INVALID_COUNTER.get() {
        let mut labels = worker.labels();
        labels.push("weak");
        counter.with_label_values(&labels).inc();
    }
    // Update activity time - worker is still connected even if share is weak
    update_worker_activity(worker);
}

/// Helper function to update worker activity time
fn update_worker_activity(worker: &WorkerContext) {
    let key = format!(
        "{}:{}:{}",
        worker.instance_id, worker.worker_name, worker.wallet
    );
    let activity_map = WORKER_LAST_ACTIVITY.get_or_init(|| parking_lot::Mutex::new(HashMap::new()));
    activity_map.lock().insert(key, Instant::now());
}

/// Record a block found
pub fn record_block_found(worker: &WorkerContext, nonce: u64, bluescore: u64, hash: String) {
    if let Some(counter) = BLOCK_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc();
    }
    if let Some(gauge) = BLOCK_GAUGE.get() {
        let mut labels = worker.labels();
        let nonce_str = nonce.to_string();
        let bluescore_str = bluescore.to_string();
        let timestamp_str = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();
        labels.push(&nonce_str);
        labels.push(&bluescore_str);
        labels.push(&timestamp_str);
        labels.push(&hash);
        gauge.with_label_values(&labels).set(1.0);
    }
}

/// Record a disconnect
pub fn record_disconnect(worker: &WorkerContext) {
    if let Some(counter) = DISCONNECT_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc();
    }

    // Remove worker from activity tracking immediately on disconnect
    let key = format!(
        "{}:{}:{}",
        worker.instance_id, worker.worker_name, worker.wallet
    );
    let activity_map = WORKER_LAST_ACTIVITY.get_or_init(|| parking_lot::Mutex::new(HashMap::new()));
    activity_map.lock().remove(&key);
}

/// Record a new job sent
pub fn record_new_job(worker: &WorkerContext) {
    if let Some(counter) = JOB_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc();
    }
}

/// Record network stats
pub fn record_network_stats(hashrate: u64, block_count: u64, difficulty: f64) {
    if let Some(gauge) = ESTIMATED_NETWORK_HASHRATE.get() {
        gauge.set(hashrate as f64);
    }
    if let Some(gauge) = NETWORK_BLOCK_COUNT.get() {
        gauge.set(block_count as f64);
    }
    if let Some(gauge) = NETWORK_DIFFICULTY.get() {
        gauge.set(difficulty);
    }
}
/// Record a worker error
pub fn record_worker_error(instance_id: &str, wallet: &str, error: &str) {
    if let Some(counter) = ERROR_BY_WALLET.get() {
        counter
            .with_label_values(&[instance_id, wallet, error])
            .inc();
    }
}

/// Record wallet balances
pub fn record_balances(instance_id: &str, balances: &[(String, u64)]) {
    if let Some(gauge) = BALANCE_GAUGE.get() {
        for (address, balance) in balances {
            // Convert from sompi to KAS (divide by 100000000)
            let balance_kas = *balance as f64 / 100_000_000.0;
            gauge
                .with_label_values(&[instance_id, address])
                .set(balance_kas);
        }
    }
}

fn metric_matches_instance(metric: &prometheus::proto::Metric, instance_id: &str) -> bool {
    metric
        .get_label()
        .iter()
        .any(|label| label.get_name() == "instance" && label.get_value() == instance_id)
}

pub(crate) fn filter_metric_families_for_instance(
    metric_families: Vec<MetricFamily>,
    instance_id: &str,
) -> Vec<MetricFamily> {
    let mut out = Vec::with_capacity(metric_families.len());

    for family in metric_families {
        let has_instance_label = family.get_metric().iter().any(|metric| {
            metric
                .get_label()
                .iter()
                .any(|label| label.get_name() == "instance")
        });

        if !has_instance_label {
            out.push(family);
            continue;
        }

        let mut filtered_family = family.clone();
        filtered_family
            .mut_metric()
            .retain(|metric| metric_matches_instance(metric, instance_id));
        if !filtered_family.get_metric().is_empty() {
            out.push(filtered_family);
        }
    }

    out
}

/// Initialize worker counters (set to 0 to create the metric)
pub fn init_worker_counters(worker: &WorkerContext) {
    if let Some(counter) = SHARE_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc_by(0.0);
    }
    if let Some(counter) = SHARE_DIFF_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc_by(0.0);
    }
    if let Some(counter) = INVALID_COUNTER.get() {
        for error_type in &["stale", "duplicate", "invalid", "weak"] {
            let mut labels = worker.labels();
            labels.push(error_type);
            counter.with_label_values(&labels).inc_by(0.0);
        }
    }
    if let Some(counter) = BLOCK_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc_by(0.0);
    }
    if let Some(counter) = BLOCK_ACCEPTED_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc_by(0.0);
    }
    if let Some(counter) = BLOCK_NOT_CONFIRMED_BLUE_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc_by(0.0);
    }
    if let Some(counter) = DISCONNECT_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc_by(0.0);
    }
    if let Some(counter) = JOB_COUNTER.get() {
        counter.with_label_values(&worker.labels()).inc_by(0.0);
    }
    // Set worker start time (Unix timestamp in seconds)
    if let Some(gauge) = WORKER_START_TIME.get() {
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as f64;
        gauge.with_label_values(&worker.labels()).set(start_time);
    }
    // Initialize worker difficulty to 0 (will be updated when difficulty is set)
    if let Some(gauge) = WORKER_CURRENT_DIFFICULTY.get() {
        gauge.with_label_values(&worker.labels()).set(0.0);
    }
}

/// Update the current mining difficulty for a worker
pub fn update_worker_difficulty(worker: &WorkerContext, difficulty: f64) {
    if let Some(gauge) = WORKER_CURRENT_DIFFICULTY.get() {
        gauge.with_label_values(&worker.labels()).set(difficulty);
    }
}
