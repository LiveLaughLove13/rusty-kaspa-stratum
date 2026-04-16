use super::ShareHandler;
use super::duplicate_submit::DuplicateSubmitGuard;
use super::vardiff::{VAR_DIFF_THREAD_SLEEP, vardiff_compute_next_diff};
#[cfg(feature = "rkstratum_cpu_miner")]
use super::work_stats::RKSTRATUM_CPU_MINER_METRICS;
use super::work_stats::{
    STATS_PRINTER_REGISTRY, STATS_PRINTER_STARTED, StatsPrinterEntry, WorkStats, format_hashrate,
};
use crate::kaspaapi::NODE_STATUS;
use crate::mining_state::GetMiningState;
use crate::prom::*;
use crate::stratum_context::StratumContext;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tracing::{debug, info};

const STATS_PRUNE_INTERVAL: Duration = Duration::from_secs(60);
const STATS_PRINT_INTERVAL: Duration = Duration::from_secs(10);

impl ShareHandler {
    pub fn new(instance_id: String) -> Self {
        Self {
            tip_blue_score: Arc::new(parking_lot::Mutex::new(0)),
            stats: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            overall: Arc::new(WorkStats::new("overall".to_string())),
            instance_id,
            duplicate_submit_guard: Arc::new(parking_lot::Mutex::new(DuplicateSubmitGuard::new(
                Duration::from_secs(180),
                50_000,
            ))),
        }
    }

    pub fn get_create_stats(&self, ctx: &StratumContext) -> WorkStats {
        let mut stats_map = self.stats.lock();

        let worker_id = {
            let id = ctx.identity.lock();
            if !id.worker_name.is_empty() {
                id.worker_name.clone()
            } else {
                ctx.remote_addr().to_string()
            }
        };

        if let Some(stats) = stats_map.get(&worker_id) {
            return stats.clone();
        }

        let stats = WorkStats::new(worker_id.clone());
        // Seed per-worker displayed diff from current mining state so recreated
        // entries do not start at 0.0 and get stuck in terminal/UI.
        let seeded_diff = GetMiningState(ctx)
            .stratum_diff()
            .map(|d| d.diff_value)
            .unwrap_or(0.0);
        if seeded_diff > 0.0 {
            *stats.min_diff.lock() = seeded_diff;
        }
        stats_map.insert(worker_id.clone(), stats.clone());
        drop(stats_map);

        // Initialize worker counters
        let wallet_addr = ctx.identity.lock().wallet_addr.clone();
        let worker_name = stats.worker_name.lock().clone();
        init_worker_counters(&crate::prom::WorkerContext {
            instance_id: self.instance_id.clone(),
            worker_name: worker_name.clone(),
            miner: String::new(),
            wallet: wallet_addr.clone(),
            ip: format!("{}:{}", ctx.remote_addr(), ctx.remote_port()),
        });

        stats
    }

    pub fn set_client_vardiff(&self, ctx: &StratumContext, min_diff: f64) -> f64 {
        let stats = self.get_create_stats(ctx);
        let previous = *stats.min_diff.lock();
        *stats.min_diff.lock() = min_diff;
        *stats.var_diff_start_time.lock() = Some(Instant::now());
        *stats.var_diff_shares_found.lock() = 0;
        *stats.var_diff_window.lock() = 0;
        previous
    }

    pub fn get_client_vardiff(&self, ctx: &StratumContext) -> f64 {
        let stats = self.get_create_stats(ctx);
        *stats.min_diff.lock()
    }

    pub fn start_client_vardiff(&self, ctx: &StratumContext) {
        let stats = self.get_create_stats(ctx);
        if stats.var_diff_start_time.lock().is_none() {
            *stats.var_diff_start_time.lock() = Some(Instant::now());
            *stats.var_diff_shares_found.lock() = 0;
        }
    }

    pub fn start_prune_stats_thread(&self) {
        self.start_prune_stats_thread_impl(None);
    }

    pub fn start_prune_stats_thread_with_shutdown(&self, shutdown_rx: watch::Receiver<bool>) {
        self.start_prune_stats_thread_impl(Some(shutdown_rx));
    }

    fn start_prune_stats_thread_impl(&self, mut shutdown_rx: Option<watch::Receiver<bool>>) {
        let stats = Arc::clone(&self.stats);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(STATS_PRUNE_INTERVAL);
            loop {
                if let Some(ref mut rx) = shutdown_rx {
                    tokio::select! {
                        _ = rx.changed() => {
                            if *rx.borrow() {
                                break;
                            }
                        }
                        _ = interval.tick() => {
                            let mut stats_map = stats.lock();
                            let now = Instant::now();
                            stats_map.retain(|_, v| {
                                let last_share = *v.last_share.lock();
                                let shares = *v.shares_found.lock();
                                (shares > 0 || now.duration_since(v.start_time) < Duration::from_secs(180))
                                    && now.duration_since(last_share) < Duration::from_secs(600)
                            });
                        }
                    }
                } else {
                    interval.tick().await;
                    let mut stats_map = stats.lock();
                    let now = Instant::now();
                    stats_map.retain(|_, v| {
                        let last_share = *v.last_share.lock();
                        let shares = *v.shares_found.lock();
                        (shares > 0 || now.duration_since(v.start_time) < Duration::from_secs(180))
                            && now.duration_since(last_share) < Duration::from_secs(600)
                    });
                }
            }
        });
    }

    pub fn start_print_stats_thread(&self, target_spm: u32) {
        self.start_print_stats_thread_impl(target_spm, None);
    }

    pub fn start_print_stats_thread_with_shutdown(
        &self,
        target_spm: u32,
        shutdown_rx: watch::Receiver<bool>,
    ) {
        self.start_print_stats_thread_impl(target_spm, Some(shutdown_rx));
    }

    fn start_print_stats_thread_impl(
        &self,
        target_spm: u32,
        shutdown_rx: Option<watch::Receiver<bool>>,
    ) {
        let target_spm = if target_spm == 0 {
            20.0
        } else {
            target_spm as f64
        };
        let instance_id = self.instance_id.clone();
        let inst_short = {
            let digits: String = instance_id.chars().filter(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<u32>() {
                format!("Ins{:02}", n)
            } else {
                "Ins??".to_string()
            }
        };

        {
            let mut registry = STATS_PRINTER_REGISTRY.lock();
            if !registry.iter().any(|e| e.instance_id == instance_id) {
                registry.push(StatsPrinterEntry {
                    instance_id,
                    inst_short,
                    target_spm,
                    start: Instant::now(),
                    stats: Arc::clone(&self.stats),
                    overall: Arc::clone(&self.overall),
                });
            }
        }

        if STATS_PRINTER_STARTED.swap(true, Ordering::AcqRel) {
            return;
        }

        let mut shutdown_rx = shutdown_rx;
        tokio::spawn(async move {
            fn trunc<'a>(s: &'a str, max: usize) -> Cow<'a, str> {
                if s.len() <= max {
                    Cow::Borrowed(s)
                } else {
                    Cow::Owned(s.chars().take(max).collect())
                }
            }

            fn format_uptime(d: Duration) -> String {
                let total_secs = d.as_secs();
                let days = total_secs / 86_400;
                let hours = (total_secs % 86_400) / 3_600;
                let mins = (total_secs % 3_600) / 60;
                let secs = total_secs % 60;
                format!("{:02}:{:02}:{:02}:{:02}", days, hours, mins, secs)
            }

            const WORKER_W: usize = 16;
            const INST_W: usize = 5;
            const HASH_W: usize = 11;
            const DIFF_W: usize = 6;
            const SPM_W: usize = 11;
            const TRND_W: usize = 4;
            const ACC_W: usize = 12;
            const BLK_W: usize = 6;
            const TBLK_W: usize = 6;
            const TIME_W: usize = 11;

            fn border() -> String {
                format!(
                    "+-{}-+-{}-+-{}-+-{}-+-{}-+-{}-+-{}-+-{}-+-{}-+-{}-+",
                    "-".repeat(WORKER_W),
                    "-".repeat(INST_W),
                    "-".repeat(HASH_W),
                    "-".repeat(DIFF_W),
                    "-".repeat(SPM_W),
                    "-".repeat(TRND_W),
                    "-".repeat(ACC_W),
                    "-".repeat(BLK_W),
                    "-".repeat(TBLK_W),
                    "-".repeat(TIME_W)
                )
            }

            fn header() -> String {
                format!(
                    "| {:<WORKER_W$} | {:<INST_W$} | {:>HASH_W$} | {:>DIFF_W$} | {:>SPM_W$} | {:<TRND_W$} | {:>ACC_W$} | {:>BLK_W$} | {:>TBLK_W$} | {:>TIME_W$} |",
                    "Worker",
                    "Inst",
                    "Hash",
                    "Diff",
                    "SPM|TGT",
                    "Trnd",
                    "Acc|Stl|Inv",
                    "Blocks",
                    "Total",
                    "D|HR|M|S",
                )
            }

            let mut interval = tokio::time::interval(STATS_PRINT_INTERVAL);
            // Internal miner hashrate is based on hashes/sec (not Stratum shares), so we keep a
            // last-sample snapshot to compute a stable, accurate rate (matching the dashboard).
            #[cfg(feature = "rkstratum_cpu_miner")]
            let mut last_internal_hashes: Option<u64> = None;
            #[cfg(feature = "rkstratum_cpu_miner")]
            let mut last_internal_sample = Instant::now();
            loop {
                if let Some(ref mut rx) = shutdown_rx {
                    tokio::select! {
                        _ = rx.changed() => {
                            if *rx.borrow() {
                                break;
                            }
                        }
                        _ = interval.tick() => {}
                    }
                } else {
                    interval.tick().await;
                }

                let node_status = {
                    let s = NODE_STATUS.lock();
                    s.clone()
                };

                let entries = {
                    let registry = STATS_PRINTER_REGISTRY.lock();
                    registry
                        .iter()
                        .map(|e| {
                            (
                                e.inst_short.clone(),
                                e.target_spm,
                                e.start,
                                Arc::clone(&e.stats),
                                Arc::clone(&e.overall),
                            )
                        })
                        .collect::<Vec<_>>()
                };

                if entries.is_empty() {
                    continue;
                }

                let mut rows: Vec<(String, String)> = Vec::new();
                let mut total_rate = 0.0;
                let mut total_shares: i64 = 0;
                let mut total_stales: i64 = 0;
                let mut total_invalids: i64 = 0;
                let mut total_blocks: i64 = 0;
                let mut total_blocks_all_time: i64 = 0;

                let now = Instant::now();
                let start = entries
                    .iter()
                    .map(|(_, _, start, _, _)| *start)
                    .max_by_key(|t| t.elapsed())
                    .unwrap_or_else(Instant::now);
                let total_uptime_mins = now.duration_since(start).as_secs_f64() / 60.0;

                let mut total_target: Option<f64> = Some(entries[0].1);
                for (inst_short, target_spm, _, stats, overall) in entries.iter() {
                    if let Some(t) = total_target
                        && (t - *target_spm).abs() > 0.0001
                    {
                        total_target = None;
                    }

                    total_shares += *overall.shares_found.lock();
                    total_stales += *overall.stale_shares.lock();
                    total_invalids += *overall.invalid_shares.lock();
                    // overall.blocks_found includes blocks from all workers (even pruned ones)
                    // Accumulate for the "Total" column (all-time blocks)
                    total_blocks_all_time += *overall.blocks_found.lock();

                    let stats_map = stats.lock();
                    for (_, v) in stats_map.iter() {
                        let elapsed = v.start_time.elapsed().as_secs_f64();
                        let rate = if elapsed > 0.0 {
                            let total_hash_value = *v.shares_diff.lock();
                            total_hash_value / elapsed
                        } else {
                            0.0
                        };
                        total_rate += rate;

                        let shares = *v.shares_found.lock();
                        let stales = *v.stale_shares.lock();
                        let invalids = *v.invalid_shares.lock();
                        let blocks = *v.blocks_found.lock();
                        let min_diff = *v.min_diff.lock();

                        // Sum blocks from individual workers for "Blocks" column (online workers only)
                        total_blocks += blocks;

                        let spm = if elapsed > 0.0 {
                            (shares as f64) / (elapsed / 60.0)
                        } else {
                            0.0
                        };
                        let trend = if spm > *target_spm * 1.2 {
                            "up"
                        } else if spm < *target_spm * 0.8 {
                            "down"
                        } else {
                            "flat"
                        };

                        let worker = v.worker_name.lock().clone();

                        let spm_tgt = format!("{:>4.1}/{:<4.1}", spm, *target_spm);

                        // For individual workers, "Blocks" and "Total" are the same (they're currently online)
                        let line = format!(
                            "| {:<WORKER_W$} | {:<INST_W$} | {:>HASH_W$} | {:>DIFF_W$} | {:>SPM_W$} | {:<TRND_W$} | {:>ACC_W$} | {:>BLK_W$} | {:>TBLK_W$} | {:>TIME_W$} |",
                            trunc(&worker, WORKER_W),
                            inst_short,
                            format_hashrate(rate),
                            min_diff.round() as u64,
                            spm_tgt,
                            trend,
                            format!("{}/{}/{}", shares, stales, invalids),
                            blocks,
                            blocks, // Total blocks (same as Blocks for active workers)
                            format_uptime(v.start_time.elapsed())
                        );
                        let sort_key = format!("{}:{}", inst_short, worker);
                        rows.push((sort_key, line));
                    }
                }

                rows.sort_by(|a, b| a.0.cmp(&b.0));

                let top = border();
                let sep = border();
                let hdr = header();

                let mut out = Vec::new();

                let sync_str = match node_status.is_synced {
                    Some(true) => "synced".to_string(),
                    Some(false) => "syncing".to_string(),
                    None => "unknown".to_string(),
                };
                let conn_str = if node_status.is_connected {
                    "connected"
                } else {
                    "disconnected"
                };

                let net = node_status.network_id.as_deref().unwrap_or("-");
                let ver = node_status.server_version.as_deref().unwrap_or("-");
                let peers = node_status
                    .peers
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let vdaa = node_status
                    .virtual_daa_score
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let blocks = node_status
                    .block_count
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let headers = node_status
                    .header_count
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let diff = node_status
                    .difficulty
                    .map(|d| format!("{:.2e}", d))
                    .unwrap_or_else(|| "-".to_string());
                let tip = node_status.tip_hash.as_deref().unwrap_or("-");
                let mempool = node_status
                    .mempool_size
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string());

                let tip_short = if tip.len() > 28 {
                    format!("{}...{}", &tip[..16], &tip[tip.len() - 8..])
                } else {
                    tip.to_string()
                };

                let net_short = crate::kaspaapi::network_display_from_id(Some(net))
                    .unwrap_or_else(|| net.to_string());

                out.push(format!(
                    "[NODE] {}|{} | n={} | v={} | p={} | vd={} | blk={}/{} | d={} | mp={} | tip={}",
                    conn_str,
                    sync_str,
                    net_short,
                    ver,
                    peers,
                    vdaa,
                    blocks,
                    headers,
                    diff,
                    mempool,
                    tip_short
                ));

                out.push(top.clone());
                out.push(hdr);
                out.push(sep.clone());

                for (_, line) in rows.iter() {
                    out.push(line.clone());
                }

                out.push(sep.clone());

                // If present, we also fold the feature-gated internal miner into the TOTAL row.
                // Note: Internal CPU mining doesn't produce Stratum shares; we treat accepted/submitted blocks
                // as the closest analogue for the Acc/Stl columns (same as the InternalCPU row does).
                let internal_totals: Option<(f64, i64, i64, i64, i64)> = {
                    // Feature-gated internal miner row
                    #[cfg(feature = "rkstratum_cpu_miner")]
                    {
                        let mut internal_totals: Option<(f64, i64, i64, i64, i64)> = None; // (ghs, acc, stl, inv, blocks)
                        if let Some(metrics) = RKSTRATUM_CPU_MINER_METRICS.lock().as_ref() {
                            let hashes = metrics.hashes_tried.load(Ordering::Relaxed);
                            let submitted = metrics.blocks_submitted.load(Ordering::Relaxed);
                            let accepted = metrics.blocks_accepted.load(Ordering::Relaxed);

                            // Calculate hashrate based on hash delta
                            let hashrate_ghs = if let Some(last_hashes) = last_internal_hashes {
                                let dt = now
                                    .duration_since(last_internal_sample)
                                    .as_secs_f64()
                                    .max(0.000_001);
                                let dh = hashes.saturating_sub(last_hashes);
                                // Hashrate as GH/s (format_hashrate expects GH/s)
                                (dh as f64 / dt) / 1e9
                            } else {
                                // First iteration: initialize but show 0 hashrate
                                0.0
                            };

                            // Update tracking variables for next iteration
                            last_internal_hashes = Some(hashes);
                            last_internal_sample = now;
                            internal_totals = Some((
                                hashrate_ghs,
                                accepted as i64,
                                submitted.saturating_sub(accepted) as i64,
                                0,
                                accepted as i64,
                            ));
                            let internal_line = format!(
                                "| {:<WORKER_W$} | {:<INST_W$} | {:>HASH_W$} | {:>DIFF_W$} | {:>SPM_W$} | {:<TRND_W$} | {:>ACC_W$} | {:>BLK_W$} | {:>TBLK_W$} | {:>TIME_W$} |",
                                "InternalCPU",
                                "-",
                                format_hashrate(hashrate_ghs),
                                "-",
                                "-",
                                "-",
                                format!(
                                    "{}/{}/{}",
                                    accepted,
                                    submitted.saturating_sub(accepted),
                                    0
                                ),
                                accepted,
                                accepted, // Total blocks (same as Blocks for InternalCPU)
                                format_uptime(now.duration_since(start))
                            );
                            out.push(internal_line);
                            out.push(sep.clone());
                        }
                        internal_totals
                    }
                    #[cfg(not(feature = "rkstratum_cpu_miner"))]
                    {
                        None
                    }
                };

                if let Some((ghs, acc, stl, inv, blocks)) = internal_totals {
                    total_rate += ghs;
                    total_shares += acc;
                    total_stales += stl;
                    total_invalids += inv;
                    total_blocks += blocks;
                    total_blocks_all_time += blocks; // Also add to all-time total for the "Total" column
                }

                let overall_spm = if total_uptime_mins > 0.0 {
                    (total_shares as f64) / total_uptime_mins
                } else {
                    0.0
                };
                let total_spm_tgt = match total_target {
                    Some(t) => format!("{:>4.1}/{:<4.1}", overall_spm, t),
                    None => format!("{:>4.1}/-", overall_spm),
                };

                out.push(format!(
                    "| {:<WORKER_W$} | {:<INST_W$} | {:>HASH_W$} | {:>DIFF_W$} | {:>SPM_W$} | {:<TRND_W$} | {:>ACC_W$} | {:>BLK_W$} | {:>TBLK_W$} | {:>TIME_W$} |",
                    "TOTAL",
                    "ALL",
                    format_hashrate(total_rate),
                    "-",
                    total_spm_tgt,
                    "-",
                    format!("{}/{}/{}", total_shares, total_stales, total_invalids),
                    total_blocks,        // Blocks from online workers only
                    total_blocks_all_time, // Total blocks from all workers (including offline)
                    format_uptime(now.duration_since(start))
                ));

                out.push(top);
                info!("{}", out.join("\n"));
            }
        });
    }

    pub fn start_vardiff_thread(&self, _expected_share_rate: u32, _log_stats: bool, _clamp: bool) {
        self.start_vardiff_thread_impl(_expected_share_rate, _log_stats, _clamp, None);
    }

    pub fn start_vardiff_thread_with_shutdown(
        &self,
        expected_share_rate: u32,
        log_stats: bool,
        clamp: bool,
        shutdown_rx: watch::Receiver<bool>,
    ) {
        self.start_vardiff_thread_impl(expected_share_rate, log_stats, clamp, Some(shutdown_rx));
    }

    fn start_vardiff_thread_impl(
        &self,
        expected_share_rate: u32,
        log_stats: bool,
        clamp: bool,
        mut shutdown_rx: Option<watch::Receiver<bool>>,
    ) {
        let stats = Arc::clone(&self.stats);
        let prefix = self.log_prefix();

        tokio::spawn(async move {
            let expected_spm = expected_share_rate.max(1) as f64;
            let mut interval = tokio::time::interval(Duration::from_secs(VAR_DIFF_THREAD_SLEEP));

            if log_stats {
                info!(
                    "{} VarDiff enabled (target={} shares/min, tick={}s, pow2_clamp={})",
                    prefix, expected_spm, VAR_DIFF_THREAD_SLEEP, clamp
                );
            } else {
                debug!(
                    "{} VarDiff thread started (target={} shares/min, tick={}s, pow2_clamp={})",
                    prefix, expected_spm, VAR_DIFF_THREAD_SLEEP, clamp
                );
            }

            loop {
                if let Some(ref mut rx) = shutdown_rx {
                    tokio::select! {
                        _ = rx.changed() => {
                            if *rx.borrow() {
                                break;
                            }
                        }
                        _ = interval.tick() => {}
                    }
                } else {
                    interval.tick().await;
                }

                let mut stats_map = stats.lock();
                let now = Instant::now();

                for (_worker_id, v) in stats_map.iter_mut() {
                    let start_opt = *v.var_diff_start_time.lock();
                    let Some(start) = start_opt else { continue };

                    let elapsed = now.duration_since(start).as_secs_f64().max(0.0);
                    let shares = *v.var_diff_shares_found.lock() as f64;
                    let current = *v.min_diff.lock();
                    let next_opt =
                        vardiff_compute_next_diff(current, shares, elapsed, expected_spm, clamp);
                    let Some(next) = next_opt else { continue };

                    *v.min_diff.lock() = next;
                    *v.var_diff_start_time.lock() = Some(now);
                    *v.var_diff_shares_found.lock() = 0;
                    *v.var_diff_window.lock() = 0;

                    if log_stats {
                        let observed_spm = if elapsed > 0.0 {
                            (shares / elapsed) * 60.0
                        } else {
                            0.0
                        };
                        info!(
                            "{} VarDiff: {:.1} spm (target {:.1}), shares={}, window={:.0}s, diff {:.0} -> {:.0}",
                            prefix,
                            observed_spm,
                            expected_spm,
                            shares as i64,
                            elapsed,
                            current,
                            next
                        );
                    }
                }
            }
        });
    }
}
