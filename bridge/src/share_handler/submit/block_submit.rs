//! Network-target block found: build block, submit to node, confirm task, stale/bad handling.

use super::super::ShareHandler;
use super::super::duplicate_submit::DuplicateSubmitOutcome;
use super::super::kaspa_api_trait::KaspaApiTrait;
use super::error::{BlockSubmitRejection, SubmitRunError, classify_block_submit_error_message};
use super::parse::PreparedSubmit;
use crate::{
    log_colors::LogColors,
    prom::{
        record_block_accepted_by_node, record_block_found, record_block_not_confirmed_blue, record_invalid_share, record_stale_share,
    },
    stratum_context::StratumContext,
};
use kaspa_consensus_core::block::Block;
use kaspa_consensus_core::header::Header;
use num_bigint::BigUint;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

pub(super) const BLOCK_CONFIRM_RETRY_DELAY: Duration = Duration::from_secs(2);
pub(super) const BLOCK_CONFIRM_MAX_ATTEMPTS: usize = 30;

/// How the PoW loop should continue after [`run_block_found_submit_flow`].
pub(super) enum BlockSubmitFlowResult {
    /// Leave the job loop (`break` with `invalid_share`).
    Break { invalid_share: bool },
    /// `mining.submit` was already answered (stale / bad block).
    Finished,
}

/// Logs, builds the block, submits, spawns blue-confirm task, or handles duplicate / RPC errors.
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_block_found_submit_flow(
    handler: &ShareHandler,
    ctx: &Arc<StratumContext>,
    event: &crate::jsonrpc_event::JsonRpcEvent,
    kaspa_api: &Arc<dyn KaspaApiTrait + Send + Sync>,
    prep: &PreparedSubmit,
    mut header_clone: Header,
    current_job: &crate::mining_state::Job,
    current_job_id: u64,
    nonce_val: u64,
    pow_value: &BigUint,
    network_target: &BigUint,
) -> Result<BlockSubmitFlowResult, SubmitRunError> {
    let wallet_addr = ctx.identity.lock().wallet_addr.clone();
    let worker_name = ctx.identity.lock().worker_name.clone();
    let prefix = handler.log_prefix();

    info!(
        "{} {} {}",
        prefix,
        LogColors::block("===== BLOCK FOUND! ====="),
        format!("Worker: {}, Wallet: {}, Nonce: {:x}", worker_name, wallet_addr, nonce_val)
    );
    debug!(
        "{} {} {} {}",
        prefix,
        LogColors::block("[BLOCK]"),
        LogColors::label("ACCEPTANCE REASON:"),
        format!("pow_value ({:x}) <= network_target ({:x})", pow_value, network_target)
    );
    debug!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::label("Pow Value:"), format!("{:x}", pow_value));

    let header_bits = header_clone.bits;
    let header_version = header_clone.version;
    let original_timestamp = header_clone.timestamp;

    header_clone.nonce = nonce_val;

    let current_time_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let timestamp_age_ms = current_time_ms.saturating_sub(original_timestamp);
    let timestamp_age_sec = timestamp_age_ms / 1000;

    debug!(
        "{} {} {}",
        LogColors::block("[BLOCK]"),
        LogColors::label("Header Verification:"),
        "Using REAL header from Kaspa node block template"
    );
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("  - Header Version:"), header_version);
    debug!(
        "{} {} {}",
        LogColors::block("[BLOCK]"),
        LogColors::label("  - Header Bits:"),
        format!("{} (0x{:x})", header_bits, header_bits)
    );
    debug!(
        "{} {} {}",
        LogColors::block("[BLOCK]"),
        LogColors::label("  - Timestamp:"),
        format!("{} (age: {}s, preserved from template)", original_timestamp, timestamp_age_sec)
    );
    debug!(
        "{} {} {}",
        LogColors::block("[BLOCK]"),
        LogColors::label("  - Nonce:"),
        format!("{:x} (set from ASIC submission)", nonce_val)
    );

    if timestamp_age_sec > 60 {
        warn!(
            "{} {} {}",
            LogColors::block("[BLOCK]"),
            LogColors::error("Timestamp is old:"),
            format!("{} seconds old - block template may be stale", timestamp_age_sec)
        );
    }

    let transactions_vec = current_job.block.transactions.iter().cloned().collect();
    let block = Block::from_arcs(Arc::new(header_clone), Arc::new(transactions_vec));
    let blue_score = block.header.blue_score;

    use kaspa_consensus_core::hashing::header;
    let block_hash = header::hash(&block.header).to_string();

    info!("{} {} {}", prefix, LogColors::block("BLOCK FOUND!"), format!("Hash: {}", block_hash));
    debug!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::label("Worker:"), worker_name);
    debug!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::label("Wallet:"), wallet_addr);
    debug!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::label("Nonce:"), format!("{:x}", nonce_val));

    debug!("{} {}", LogColors::block("[BLOCK]"), LogColors::block("=== SUBMITTING BLOCK TO NODE ==="));
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Worker:"), worker_name);
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Nonce:"), format!("{:x} (0x{:016x})", nonce_val, nonce_val));
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Bits:"), format!("{} (0x{:08x})", header_bits, header_bits));
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Timestamp:"), format!("{}", original_timestamp));
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Blue Score:"), blue_score);
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Pow Value:"), format!("{:x}", pow_value));
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Network Target:"), format!("{:x}", network_target));
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Job ID:"), current_job_id);
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Wallet:"), wallet_addr);
    debug!(
        "{} {} {}",
        LogColors::block("[BLOCK]"),
        LogColors::label("Client:"),
        format!("{}:{}", ctx.remote_addr(), ctx.remote_port())
    );
    debug!("{} {} {}", LogColors::block("[BLOCK]"), LogColors::label("Block Hash:"), block_hash);
    debug!("{} {}", LogColors::block("[BLOCK]"), "Calling kaspa_api.submit_block()...");

    let block_submit_result = kaspa_api.submit_block(block.clone()).await;

    match block_submit_result {
        Ok(response) => {
            if !response.report.is_success() {
                let prefix = handler.log_prefix();
                warn!("{} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::error("Block rejected by node"));
                warn!(
                    "{} {} {} {}",
                    prefix,
                    LogColors::block("[BLOCK]"),
                    LogColors::label("REJECTION REASON:"),
                    format!("{:?}", response.report)
                );
                return Ok(BlockSubmitFlowResult::Break { invalid_share: true });
            }

            let prefix = handler.log_prefix();
            info!(
                "{} {} {}",
                prefix,
                LogColors::block("[BLOCK]"),
                LogColors::block(&format!("Block submitted successfully! Hash: {}", block_hash))
            );
            info!(
                "{} {} {}",
                prefix,
                LogColors::block("[BLOCK]"),
                LogColors::block(&format!("BLOCK ACCEPTED BY NODE! Hash: {}", block_hash))
            );
            info!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::label("  - Worker:"), worker_name);
            info!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::label("  - Nonce:"), format!("{:x}", nonce_val));

            let stats = handler.get_create_stats(ctx.as_ref());
            let overall = handler.overall.clone();
            let instance_id = handler.instance_id.clone();
            let prom_worker = crate::prom::WorkerContext {
                instance_id: handler.instance_id.clone(),
                worker_name: worker_name.clone(),
                miner: String::new(),
                wallet: wallet_addr.clone(),
                ip: format!("{}:{}", ctx.remote_addr(), ctx.remote_port()),
            };

            record_block_accepted_by_node(&prom_worker);

            let kaspa_api = Arc::clone(kaspa_api);
            let block_hash_for_confirm = block_hash.clone();

            tokio::spawn(async move {
                for _ in 0..BLOCK_CONFIRM_MAX_ATTEMPTS {
                    match kaspa_api.get_current_block_color(&block_hash_for_confirm).await {
                        Ok(true) => {
                            *stats.blocks_found.lock() += 1;
                            *overall.blocks_found.lock() += 1;
                            record_block_found(&prom_worker, nonce_val, blue_score, block_hash_for_confirm.clone());
                            info!(
                                "[{}] {} {}",
                                instance_id,
                                LogColors::block("[BLOCK]"),
                                LogColors::block(&format!("Block confirmed BLUE in DAG! Hash: {}", block_hash_for_confirm))
                            );
                            return;
                        }
                        Ok(false) => {
                            tokio::time::sleep(BLOCK_CONFIRM_RETRY_DELAY).await;
                        }
                        Err(_) => {
                            tokio::time::sleep(BLOCK_CONFIRM_RETRY_DELAY).await;
                        }
                    }
                }

                record_block_not_confirmed_blue(&prom_worker);
                info!(
                    "[{}] {} {}",
                    instance_id,
                    LogColors::block("[BLOCK]"),
                    LogColors::label(&format!(
                        "Block not confirmed blue after {} attempts (not counted as Blocks). Hash: {}",
                        BLOCK_CONFIRM_MAX_ATTEMPTS, block_hash_for_confirm
                    ))
                );
            });

            Ok(BlockSubmitFlowResult::Break { invalid_share: false })
        }
        Err(e) => {
            let prefix = handler.log_prefix();
            let error_str = e.to_string();
            error!("{} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::error("Block submission FAILED"));
            error!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::label("Worker:"), worker_name);
            error!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::label("Blockhash:"), block_hash);
            error!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::error("Error:"), error_str);

            if classify_block_submit_error_message(&error_str) == BlockSubmitRejection::DuplicateBlockStale {
                warn!("{} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::error("block rejected, stale"));
                warn!(
                    "{} {} {} {}",
                    prefix,
                    LogColors::block("[BLOCK]"),
                    LogColors::label("REJECTION REASON:"),
                    "Block was already submitted to the network (stale/duplicate)"
                );

                {
                    let now = Instant::now();
                    let mut guard = handler.duplicate_submit_guard.lock();
                    guard.set_outcome(&prep.submit_key, now, DuplicateSubmitOutcome::Stale);
                }

                let stats = handler.get_create_stats(ctx.as_ref());
                *stats.stale_shares.lock() += 1;
                *handler.overall.stale_shares.lock() += 1;

                record_stale_share(&crate::prom::WorkerContext {
                    instance_id: handler.instance_id.clone(),
                    worker_name: worker_name.clone(),
                    miner: String::new(),
                    wallet: wallet_addr.clone(),
                    ip: format!("{}:{}", ctx.remote_addr(), ctx.remote_port()),
                });
                ctx.reply_stale_share(event.id.clone()).await?;
                return Ok(BlockSubmitFlowResult::Finished);
            }

            warn!(
                "{} {} {}",
                prefix,
                LogColors::block("[BLOCK]"),
                LogColors::error("block rejected, unknown issue (probably bad pow)")
            );
            error!(
                "{} {} {} {}",
                prefix,
                LogColors::block("[BLOCK]"),
                LogColors::label("REJECTION REASON:"),
                "Block failed node validation (probably bad pow)"
            );
            error!("{} {} {} {}", prefix, LogColors::block("[BLOCK]"), LogColors::error("Error:"), error_str);

            let stats = handler.get_create_stats(ctx.as_ref());
            *stats.invalid_shares.lock() += 1;
            *handler.overall.invalid_shares.lock() += 1;

            record_invalid_share(&crate::prom::WorkerContext {
                instance_id: handler.instance_id.clone(),
                worker_name: worker_name.clone(),
                miner: String::new(),
                wallet: wallet_addr.clone(),
                ip: format!("{}:{}", ctx.remote_addr(), ctx.remote_port()),
            });

            {
                let now = Instant::now();
                let mut guard = handler.duplicate_submit_guard.lock();
                guard.set_outcome(&prep.submit_key, now, DuplicateSubmitOutcome::Bad);
            }
            ctx.reply_bad_share(event.id.clone()).await?;
            Ok(BlockSubmitFlowResult::Finished)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockSubmitRejection, classify_block_submit_error_message};
    use kaspa_rpc_core::{SubmitBlockReport, SubmitBlockResponse};

    #[test]
    fn submit_block_report_success_is_accepted() {
        let r = SubmitBlockResponse { report: SubmitBlockReport::Success };
        assert!(r.report.is_success());
    }

    #[test]
    fn duplicate_block_error_string_classifies() {
        assert_eq!(
            classify_block_submit_error_message("rpc error: ErrDuplicateBlock: ..."),
            BlockSubmitRejection::DuplicateBlockStale
        );
    }
}
