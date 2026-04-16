use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use prometheus::gather;

use super::parse::{
    new_worker_info, parse_instance_wallet_labels, parse_worker_labels,
    sum_prometheus_counter_family,
};
use super::types::{BlockInfo, InternalCpuStats, StatsResponse, WorkerInfo};

use crate::prom::metrics::{
    BRIDGE_START_TIME, WORKER_LAST_ACTIVITY, filter_metric_families_for_instance,
};
#[cfg(feature = "rkstratum_cpu_miner")]
use crate::prom::metrics::{INTERNAL_CPU_MINING_ADDRESS, INTERNAL_CPU_RECENT_BLOCKS};

/// Get stats as JSON (optionally filtered to a single instance id)
pub(crate) async fn get_stats_json_filtered(instance_id: Option<&str>) -> StatsResponse {
    // NOTE: Instance filtering removes metrics that don't carry an `instance` label.
    // The dashboard expects global network gauges too, so we always gather unfiltered
    // for those, and then optionally filter for per-worker/per-block metrics.
    let all_families = gather();
    let families_for_workers_and_blocks = match instance_id {
        Some(id) => filter_metric_families_for_instance(all_families.clone(), id),
        None => all_families.clone(),
    };
    let mut stats = StatsResponse {
        totalBlocks: 0,
        totalBlocksAcceptedByNode: 0,
        totalBlocksNotConfirmedBlue: 0,
        totalShares: 0,
        networkHashrate: 0,
        networkDifficulty: 0.0,
        networkBlockCount: 0,
        activeWorkers: 0,
        internalCpu: None,
        blocks: Vec::new(),
        workers: Vec::new(),
        bridgeUptime: None,
    };

    let mut worker_stats: HashMap<String, WorkerInfo> = HashMap::new();
    let mut worker_hash_values: HashMap<String, f64> = HashMap::new(); // Store hash values for hashrate calculation
    let mut worker_start_times: HashMap<String, f64> = HashMap::new(); // Store start times for hashrate calculation
    let mut worker_difficulties: HashMap<String, f64> = HashMap::new(); // Store current difficulty for each worker
    let mut balance_by_instance_wallet: HashMap<String, f64> = HashMap::new();
    let mut errors_by_instance_wallet: HashMap<String, u64> = HashMap::new();
    let mut block_set: HashSet<String> = HashSet::new();

    // Parse global network gauges from the unfiltered set.
    // Also pick up internal CPU miner metrics (if present).
    let mut internal_cpu_hashrate_ghs: Option<f64> = None;
    let mut internal_cpu_hashes_tried: Option<u64> = None;
    let mut internal_cpu_blocks_submitted: Option<u64> = None;
    let mut internal_cpu_blocks_accepted: Option<u64> = None;

    for family in all_families.iter() {
        let name = family.get_name();

        if name == "ks_estimated_network_hashrate_gauge"
            && let Some(metric) = family.get_metric().first()
        {
            stats.networkHashrate = metric.get_gauge().get_value() as u64;
        }

        if name == "ks_network_difficulty_gauge"
            && let Some(metric) = family.get_metric().first()
        {
            stats.networkDifficulty = metric.get_gauge().get_value();
        }

        // Network height / block count gauge. Historical name is "ks_network_block_count".
        // Accept both just in case we rename later.
        if (name == "ks_network_block_count" || name == "ks_network_block_count_gauge")
            && let Some(metric) = family.get_metric().first()
        {
            stats.networkBlockCount = metric.get_gauge().get_value() as u64;
        }

        // Internal CPU miner metrics (exported when the bridge is built with `rkstratum_cpu_miner`
        // and the internal miner is enabled at runtime).
        if name == "ks_internal_cpu_hashrate_ghs"
            && let Some(metric) = family.get_metric().first()
        {
            internal_cpu_hashrate_ghs = Some(metric.get_gauge().get_value());
        }
        if name == "ks_internal_cpu_hashes_tried_total"
            && let Some(metric) = family.get_metric().first()
        {
            internal_cpu_hashes_tried = Some(metric.get_counter().get_value().max(0.0) as u64);
        }
        if name == "ks_internal_cpu_blocks_submitted_total"
            && let Some(metric) = family.get_metric().first()
        {
            internal_cpu_blocks_submitted = Some(metric.get_counter().get_value().max(0.0) as u64);
        }
        if name == "ks_internal_cpu_blocks_accepted_total"
            && let Some(metric) = family.get_metric().first()
        {
            internal_cpu_blocks_accepted = Some(metric.get_counter().get_value().max(0.0) as u64);
        }
    }

    {
        let blocks_submitted = internal_cpu_blocks_submitted.unwrap_or(0);
        let blocks_accepted = internal_cpu_blocks_accepted.unwrap_or(0);
        let hashes_tried = internal_cpu_hashes_tried.unwrap_or(0);
        let hashrate_ghs = internal_cpu_hashrate_ghs.unwrap_or(0.0);

        // Only surface internal CPU miner data when it is actually enabled/active.
        // Otherwise a build that includes the feature would always show an "InternalCPU" row with zeros.
        #[cfg(feature = "rkstratum_cpu_miner")]
        let wallet = INTERNAL_CPU_MINING_ADDRESS
            .get()
            .cloned()
            .unwrap_or_default();
        #[cfg(not(feature = "rkstratum_cpu_miner"))]
        let wallet = String::new();

        let should_show_internal_cpu = !wallet.is_empty()
            || blocks_submitted > 0
            || blocks_accepted > 0
            || hashes_tried > 0
            || hashrate_ghs > 0.0;

        if should_show_internal_cpu {
            let stale = blocks_submitted.saturating_sub(blocks_accepted);
            stats.internalCpu = Some(InternalCpuStats {
                hashrateGhs: hashrate_ghs,
                hashesTried: hashes_tried,
                blocksSubmitted: blocks_submitted,
                blocksAccepted: blocks_accepted,
                // Expose these so the UI can fill Shares/Stale/Invalid columns for InternalCPU.
                // Internal CPU mining doesn't produce Stratum shares; blocks are the closest analogue.
                shares: blocks_accepted,
                stale,
                invalid: 0,
                wallet,
            });
        }
    }

    for family in &families_for_workers_and_blocks {
        let name = family.get_name();

        // Parse block gauge
        if name == "ks_mined_blocks_gauge" {
            for metric in family.get_metric() {
                if metric.get_gauge().get_value() > 0.0 {
                    let labels = metric.get_label();
                    let mut instance = String::new();
                    let mut worker = String::new();
                    let mut wallet = String::new();
                    let mut timestamp = String::new();
                    let mut hash = String::new();
                    let mut nonce = String::new();
                    let mut bluescore = String::new();

                    for label in labels {
                        match label.get_name() {
                            "instance" => instance = label.get_value().to_string(),
                            "worker" => worker = label.get_value().to_string(),
                            "wallet" => wallet = label.get_value().to_string(),
                            "timestamp" => timestamp = label.get_value().to_string(),
                            "hash" => hash = label.get_value().to_string(),
                            "nonce" => nonce = label.get_value().to_string(),
                            "bluescore" => bluescore = label.get_value().to_string(),
                            _ => {}
                        }
                    }

                    if !hash.is_empty() && !block_set.contains(&hash) {
                        block_set.insert(hash.clone());
                        stats.blocks.push(BlockInfo {
                            instance,
                            worker: worker.clone(),
                            wallet: wallet.clone(),
                            timestamp,
                            hash,
                            nonce,
                            bluescore,
                        });
                        stats.totalBlocks += 1;
                    }
                }
            }
        }

        // Parse block counter
        if name == "ks_blocks_mined" {
            for metric in family.get_metric() {
                let (instance, worker_key, wallet) = parse_worker_labels(metric.get_label());

                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let count = metric.get_counter().get_value() as u64;
                    let entry = worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));
                    // Aggregate across multiple time series for the same (instance,worker,wallet)
                    entry.blocks = entry.blocks.saturating_add(count);
                }
            }
        }

        // Parse share diff counter (for hashrate calculation)
        if name == "ks_valid_share_diff_counter" {
            for metric in family.get_metric() {
                let (instance, worker_key, wallet) = parse_worker_labels(metric.get_label());

                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let total_hash_value = metric.get_counter().get_value();
                    // Store hash value for hashrate calculation (aggregate across label variants)
                    *worker_hash_values.entry(key.clone()).or_insert(0.0) += total_hash_value;
                    // Ensure worker exists in stats
                    worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));
                }
            }
        }

        // Parse share counter
        if name == "ks_valid_share_counter" {
            for metric in family.get_metric() {
                let (instance, worker_key, wallet) = parse_worker_labels(metric.get_label());

                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let count = metric.get_counter().get_value() as u64;
                    let entry = worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));
                    entry.shares = entry.shares.saturating_add(count);
                    stats.totalShares = stats.totalShares.saturating_add(count);
                }
            }
        }

        // Parse invalid share counter
        if name == "ks_invalid_share_counter" {
            for metric in family.get_metric() {
                let mut share_type = String::new();

                let labels = metric.get_label();
                let (instance, worker_key, wallet) = parse_worker_labels(labels);
                for label in labels {
                    if label.get_name() == "type" {
                        share_type = label.get_value().to_string();
                    }
                }

                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let count = metric.get_counter().get_value() as u64;
                    let worker = worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));

                    if share_type == "stale" {
                        worker.stale = worker.stale.saturating_add(count);
                    } else if share_type == "invalid" {
                        worker.invalid = worker.invalid.saturating_add(count);
                    } else if share_type == "duplicate" {
                        worker.duplicate_shares = worker.duplicate_shares.saturating_add(count);
                    } else if share_type == "weak" {
                        worker.weak_shares = worker.weak_shares.saturating_add(count);
                    }
                }
            }
        }

        if name == "ks_worker_disconnect_counter" {
            for metric in family.get_metric() {
                let (instance, worker_key, wallet) = parse_worker_labels(metric.get_label());
                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let count = metric.get_counter().get_value().max(0.0) as u64;
                    let entry = worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));
                    entry.disconnects = entry.disconnects.saturating_add(count);
                }
            }
        }

        if name == "ks_worker_job_counter" {
            for metric in family.get_metric() {
                let (instance, worker_key, wallet) = parse_worker_labels(metric.get_label());
                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let count = metric.get_counter().get_value().max(0.0) as u64;
                    let entry = worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));
                    entry.jobs = entry.jobs.saturating_add(count);
                }
            }
        }

        if name == "ks_blocks_accepted_by_node" {
            for metric in family.get_metric() {
                let (instance, worker_key, wallet) = parse_worker_labels(metric.get_label());
                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let count = metric.get_counter().get_value().max(0.0) as u64;
                    let entry = worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));
                    entry.blocks_accepted_by_node =
                        entry.blocks_accepted_by_node.saturating_add(count);
                }
            }
        }

        if name == "ks_blocks_not_confirmed_blue" {
            for metric in family.get_metric() {
                let (instance, worker_key, wallet) = parse_worker_labels(metric.get_label());
                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let count = metric.get_counter().get_value().max(0.0) as u64;
                    let entry = worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));
                    entry.blocks_not_confirmed_blue =
                        entry.blocks_not_confirmed_blue.saturating_add(count);
                }
            }
        }

        if name == "ks_balance_by_wallet_gauge" {
            for metric in family.get_metric() {
                let (instance, wallet) = parse_instance_wallet_labels(metric.get_label());
                if !wallet.is_empty() {
                    let map_key = format!("{instance}:{wallet}");
                    balance_by_instance_wallet.insert(map_key, metric.get_gauge().get_value());
                }
            }
        }

        if name == "ks_worker_errors" {
            for metric in family.get_metric() {
                let (instance, wallet) = parse_instance_wallet_labels(metric.get_label());
                if !wallet.is_empty() {
                    let map_key = format!("{instance}:{wallet}");
                    let count = metric.get_counter().get_value().max(0.0) as u64;
                    let e = errors_by_instance_wallet.entry(map_key).or_insert(0);
                    *e = e.saturating_add(count);
                }
            }
        }

        // Parse network hashrate
        if name == "ks_estimated_network_hashrate"
            && let Some(metric) = family.get_metric().first()
        {
            stats.networkHashrate = metric.get_gauge().get_value() as u64;
        }

        // Parse worker start time
        if name == "ks_worker_start_time" {
            for metric in family.get_metric() {
                let (instance, worker_key, wallet) = parse_worker_labels(metric.get_label());

                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let start_time_secs = metric.get_gauge().get_value();
                    // Use earliest start time across multiple label variants
                    worker_start_times
                        .entry(key.clone())
                        .and_modify(|v| {
                            if start_time_secs > 0.0 && (*v <= 0.0 || start_time_secs < *v) {
                                *v = start_time_secs;
                            }
                        })
                        .or_insert(start_time_secs);
                    // Ensure worker exists in stats
                    worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));
                }
            }
        }

        // Parse worker current difficulty
        if name == "ks_worker_current_difficulty" {
            for metric in family.get_metric() {
                let (instance, worker_key, wallet) = parse_worker_labels(metric.get_label());

                if !worker_key.is_empty() {
                    let key = format!("{}:{}:{}", instance, worker_key, wallet);
                    let difficulty = metric.get_gauge().get_value();
                    // Use the most recent difficulty value (if multiple label variants exist)
                    if difficulty > 0.0 {
                        worker_difficulties.insert(key.clone(), difficulty);
                    }
                    // Ensure worker exists in stats
                    worker_stats
                        .entry(key.clone())
                        .or_insert_with(|| new_worker_info(instance, worker_key, wallet));
                }
            }
        }
    }

    stats.totalBlocksAcceptedByNode = sum_prometheus_counter_family(
        families_for_workers_and_blocks.as_slice(),
        "ks_blocks_accepted_by_node",
    );
    stats.totalBlocksNotConfirmedBlue = sum_prometheus_counter_family(
        families_for_workers_and_blocks.as_slice(),
        "ks_blocks_not_confirmed_blue",
    );

    for (key, w) in worker_stats.iter_mut() {
        let mut it = key.splitn(3, ':');
        let inst = it.next().unwrap_or("");
        let _worker_name = it.next();
        let wal = it.next().unwrap_or("");
        if !wal.is_empty() {
            let iw = format!("{inst}:{wal}");
            if let Some(&bal) = balance_by_instance_wallet.get(&iw) {
                w.balance_kas = Some(bal);
            }
            if let Some(&err) = errors_by_instance_wallet.get(&iw) {
                w.errors = err;
            }
        }
    }

    // Calculate hashrate for workers using share_diff_counter and start_time
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as f64;

    let mut total_worker_hashrate_ghs = 0.0;

    // Calculate hashrate for each worker
    for (key, worker) in worker_stats.iter_mut() {
        if let (Some(&total_hash_value), Some(&start_time_secs)) =
            (worker_hash_values.get(key), worker_start_times.get(key))
        {
            let elapsed = current_time - start_time_secs;
            // Calculate hashrate: total_hash_value / elapsed_time (in GH/s)
            // Matches console stats: hashrate = shares_diff / elapsed
            // Formula: hashrate = total_hash_value / elapsed (already in GH/s units)
            if elapsed > 0.0 && total_hash_value > 0.0 {
                worker.hashrate = total_hash_value / elapsed;
                total_worker_hashrate_ghs += worker.hashrate;
            }
        }
    }

    // If network hashrate is 0 or unavailable, use total worker hashrate as fallback
    // Convert from GH/s to H/s for network hashrate display
    if stats.networkHashrate == 0 && total_worker_hashrate_ghs > 0.0 {
        stats.networkHashrate = (total_worker_hashrate_ghs * 1e9) as u64;
    }

    // Filter out inactive workers (no activity in the last 5 minutes)
    const WORKER_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
    const WORKER_IDLE_THRESHOLD: Duration = Duration::from_secs(60); // 1 minute for idle status
    let now = Instant::now();
    let activity_map = WORKER_LAST_ACTIVITY.get_or_init(|| parking_lot::Mutex::new(HashMap::new()));
    let current_time_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Clean up old entries and filter active workers
    let mut active_workers: Vec<WorkerInfo> = Vec::new();
    {
        let mut activity = activity_map.lock();
        for (key, mut worker) in worker_stats.into_iter() {
            // Populate difficulty and session uptime from collected metrics
            if let Some(&difficulty) = worker_difficulties.get(&key)
                && difficulty > 0.0
            {
                worker.current_difficulty = Some(difficulty);
            }

            // Calculate session uptime from start time
            if let Some(&start_time_secs) = worker_start_times.get(&key)
                && start_time_secs > 0.0
            {
                let start_time_u64 = start_time_secs as u64;
                let session_uptime_secs = current_time_secs.saturating_sub(start_time_u64);
                worker.session_uptime = Some(session_uptime_secs);
            }

            // Check if worker has been active recently
            if let Some(&last_activity) = activity.get(&key) {
                // Check if duration is valid (handles clock adjustments)
                if let Some(duration) = now.checked_duration_since(last_activity) {
                    if duration <= WORKER_INACTIVITY_TIMEOUT {
                        // Calculate last seen timestamp
                        let last_seen_secs = current_time_secs.saturating_sub(duration.as_secs());
                        worker.last_seen = Some(last_seen_secs);

                        // Determine status based on last activity
                        if duration <= WORKER_IDLE_THRESHOLD {
                            worker.status = Some("online".to_string());
                        } else {
                            worker.status = Some("idle".to_string());
                        }

                        active_workers.push(worker);
                    } else {
                        // Remove stale entries (no activity for > 5 minutes)
                        activity.remove(&key);
                    }
                } else {
                    // Clock went backwards or instant is in the future - treat as active
                    // Update to current time to prevent issues
                    activity.insert(key.clone(), now);
                    worker.last_seen = Some(current_time_secs);
                    worker.status = Some("online".to_string());
                    active_workers.push(worker);
                }
            } else {
                // No activity record exists - this means the worker hasn't submitted any shares
                // since the last stats collection. If they have shares, they might be disconnected.
                // Only include them if they have very recent activity (check worker start time)
                if let Some(&start_time_secs) = worker_start_times.get(&key) {
                    let start_time_secs_u64 = start_time_secs as u64;
                    let elapsed_secs = current_time_secs.saturating_sub(start_time_secs_u64);
                    // If worker started less than 1 minute ago and has shares, they might be active
                    // Otherwise, assume they're disconnected
                    if elapsed_secs < 60 && worker.shares > 0 {
                        // Very new worker - give them a chance
                        activity.insert(key.clone(), now);
                        worker.last_seen = Some(current_time_secs);
                        worker.status = Some("online".to_string());
                        active_workers.push(worker);
                    }
                    // Otherwise, don't include them (they're likely disconnected)
                }
            }
        }
    }

    stats.workers = active_workers;
    // Active workers are the number of Stratum workers, plus the internal CPU miner if present.
    stats.activeWorkers = stats.workers.len() + stats.internalCpu.as_ref().map(|_| 1).unwrap_or(0);

    // Fold internal CPU miner counts into summary totals so the dashboard top-cards reflect
    // internal mining even when no ASICs are connected.
    if let Some(icpu) = stats.internalCpu.as_ref() {
        stats.totalBlocks = stats.totalBlocks.saturating_add(icpu.blocksAccepted);
        // Internal CPU mining doesn't produce shares in the Stratum sense; however, blocks are
        // "shares too" operationally (they represent successful work). Counting accepted blocks
        // here prevents the Total Shares card from staying at 0 for CPU-only runs.
        stats.totalShares = stats.totalShares.saturating_add(icpu.blocksAccepted);
    }

    // Add internal CPU recent blocks into the unified blocks list so the donut chart and
    // recent blocks table populate even in CPU-only runs.
    #[cfg(feature = "rkstratum_cpu_miner")]
    if let Some(icpu) = stats.internalCpu.as_ref()
        && let Some(q) = INTERNAL_CPU_RECENT_BLOCKS.get()
    {
        let wallet = icpu.wallet.clone();
        let guard = q.lock();
        for b in guard.iter() {
            let hash = b.hash.clone();
            if hash.is_empty() || block_set.contains(&hash) {
                continue;
            }
            block_set.insert(hash.clone());
            stats.blocks.push(BlockInfo {
                instance: "-".to_string(),
                worker: "InternalCPU".to_string(),
                wallet: wallet.clone(),
                timestamp: b.timestamp_unix.to_string(),
                hash,
                nonce: b.nonce.to_string(),
                bluescore: b.bluescore.to_string(),
            });
        }
    }

    // Sort blocks by bluescore (newest first)
    stats.blocks.sort_by(|a, b| {
        let a_score: u64 = a.bluescore.parse().unwrap_or(0);
        let b_score: u64 = b.bluescore.parse().unwrap_or(0);
        b_score.cmp(&a_score)
    });

    // Sort workers by blocks (most blocks first)
    stats.workers.sort_by_key(|w| Reverse(w.blocks));

    // Calculate bridge uptime
    if let Some(&start_time) = BRIDGE_START_TIME.get() {
        let uptime_secs = now.duration_since(start_time).as_secs();
        stats.bridgeUptime = Some(uptime_secs);
    }

    stats
}

pub(crate) async fn get_stats_json(instance_id: &str) -> StatsResponse {
    get_stats_json_filtered(Some(instance_id)).await
}

pub(crate) async fn get_stats_json_all() -> StatsResponse {
    get_stats_json_filtered(None).await
}
