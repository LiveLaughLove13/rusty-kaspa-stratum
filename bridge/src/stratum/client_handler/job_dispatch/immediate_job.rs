use super::{BIG_JOB_REGEX, send_client_diff};
use crate::{
    hasher::{calculate_target, generate_iceriver_job_params, generate_job_header, generate_large_job_params, serialize_block_header},
    jsonrpc_event::JsonRpcEvent,
    mining_state::{GetMiningState, Job},
    prom::*,
    share_handler::{KaspaApiTrait, ShareHandler},
    stratum_context::StratumContext,
};
use num_bigint::BigUint;
use num_traits::Zero;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, warn};

pub(crate) async fn send_immediate_job_task<T: KaspaApiTrait + Send + Sync + ?Sized + 'static>(
    client_clone: Arc<StratumContext>,
    kaspa_api_clone: Arc<T>,
    share_handler: Arc<ShareHandler>,
    min_diff: f64,
    instance_id: String,
) {
    // Get per-client mining state from context
    let state = GetMiningState(&client_clone);

    // Get client info
    let (wallet_addr, remote_app, canxium_addr) = {
        let wallet = client_clone.identity.lock().wallet_addr.clone();
        let app = client_clone.identity.lock().remote_app.clone();
        let canx = client_clone.identity.lock().canxium_addr.clone();
        (wallet, app, canx)
    };

    debug!("send_immediate_job: fetching block template for client {} (wallet: {})", client_clone.remote_addr, wallet_addr);

    // Get block template
    let template_result = kaspa_api_clone.get_block_template(&wallet_addr, &remote_app, &canxium_addr).await;

    let block = match template_result {
        Ok(block) => {
            debug!("send_immediate_job: successfully fetched block template for client {}", client_clone.remote_addr);

            // === LOG NEW BLOCK TEMPLATE HEADER === (moved to debug level)
            debug!("=== NEW BLOCK TEMPLATE RECEIVED ===");
            debug!("  blue_score: {}", block.header.blue_score);
            debug!("  bits: {} (0x{:08x})", block.header.bits, block.header.bits);
            debug!("  timestamp: {}", block.header.timestamp);
            debug!("  version: {}", block.header.version);
            debug!("  daa_score: {}", block.header.daa_score);

            // Track and log what changed from previous header
            if let Some(old_header) = state.get_last_header() {
                debug!("=== HEADER CHANGES ===");
                debug!("  blue_score_changed: {}", old_header.blue_score != block.header.blue_score);
                debug!("    old: {}, new: {}", old_header.blue_score, block.header.blue_score);
                debug!("  bits_changed: {}", old_header.bits != block.header.bits);
                debug!("    old: 0x{:08x}, new: 0x{:08x}", old_header.bits, block.header.bits);
                debug!("  timestamp_changed: {}", old_header.timestamp != block.header.timestamp);
                debug!("    delta: {} ms", block.header.timestamp - old_header.timestamp);
                debug!("  daa_score_changed: {}", old_header.daa_score != block.header.daa_score);
                debug!("  version_changed: {}", old_header.version != block.header.version);
            } else {
                debug!("=== FIRST HEADER === (no previous header to compare)");
            }

            // Store this header for next comparison
            state.set_last_header((*block.header).clone());

            block
        }
        Err(e) => {
            if e.to_string().contains("Could not decode address") {
                record_worker_error(&instance_id, &wallet_addr, crate::errors::ErrorShortCode::InvalidAddressFmt.as_str());
                error!("send_immediate_job: failed fetching block template, malformed address: {}", e);
                client_clone.disconnect();
            } else {
                record_worker_error(&instance_id, &wallet_addr, crate::errors::ErrorShortCode::FailedBlockFetch.as_str());
                error!("send_immediate_job: failed fetching block template: {}", e);
            }
            return;
        }
    };

    // Calculate target
    let big_diff = calculate_target(block.header.bits as u64);
    state.set_big_diff(big_diff);

    // Serialize header - now returns Hash type directly
    // The "Odd number of digits" error typically indicates a malformed hex string
    // in one of the hash fields. This can happen if the block data from the node
    // contains an invalid hash representation.
    let pre_pow_hash = match serialize_block_header(&block) {
        Ok(h) => h,
        Err(e) => {
            let error_msg = e.to_string();
            record_worker_error(&instance_id, &wallet_addr, crate::errors::ErrorShortCode::BadDataFromMiner.as_str());
            error!("send_immediate_job: failed to serialize block header: {}", error_msg);

            // Log block header details for debugging
            debug!("Block header version: {}", block.header.version);
            debug!("Block header timestamp: {}", block.header.timestamp);
            debug!("Block header bits: {}", block.header.bits);

            // Skip this block and continue - the next block template should work
            return;
        }
    };

    // Create Job struct with both block and pre_pow_hash
    let job = Job { block: block.clone(), pre_pow_hash };

    // Add job
    let job_id = state.add_job(job);
    let counter_after = state.current_job_counter();
    let stored_ids = state.get_stored_job_ids();
    debug!(
        "[JOB CREATION] send_immediate_job: created job ID {} for client {} (counter: {}, stored IDs: {:?})",
        job_id, client_clone.remote_addr, counter_after, stored_ids
    );

    // Initialize state if first time
    if !state.is_initialized() {
        state.set_initialized(true);
        let use_big_job = BIG_JOB_REGEX.is_match(&remote_app);
        state.set_use_big_job(use_big_job);

        // Initialize stratum diff
        use crate::hasher::KaspaDiff;
        let mut stratum_diff = KaspaDiff::new();
        let remote_app_clone = remote_app.clone();
        stratum_diff.set_diff_value_for_miner(min_diff, &remote_app_clone);
        state.set_stratum_diff(stratum_diff);

        // Update worker difficulty metric
        let wallet_addr = client_clone.identity.lock().wallet_addr.clone();
        let worker_name = client_clone.identity.lock().worker_name.clone();
        update_worker_difficulty(
            &WorkerContext {
                instance_id: instance_id.clone(),
                worker_name: worker_name.clone(),
                miner: remote_app_clone.clone(),
                wallet: wallet_addr.clone(),
                ip: format!("{}:{}", client_clone.remote_addr(), client_clone.remote_port()),
            },
            min_diff,
        );

        let target = state.stratum_diff().map(|d| d.target_value.clone()).unwrap_or_else(BigUint::zero);
        let target_bytes = target.to_bytes_be();
        debug!(
            "send_immediate_job: Initialized MiningState with difficulty: {}, target: {:x} ({} bytes, {} bits)",
            min_diff,
            target,
            target_bytes.len(),
            target_bytes.len() * 8
        );
    }

    // CRITICAL: Always send difficulty to each client (IceRiver expects this on every connection)
    // Even if state is already initialized, we need to send difficulty to this specific client
    // Use the actual current difficulty from state if available, otherwise use min_diff
    let current_diff = state.stratum_diff().map(|d| d.diff_value).unwrap_or(min_diff);

    // Update metric to ensure displayed difficulty matches what we're sending
    // (This handles the case where state was already initialized but metric wasn't updated)
    let wallet_addr = client_clone.identity.lock().wallet_addr.clone();
    let worker_name = client_clone.identity.lock().worker_name.clone();
    let remote_app = client_clone.identity.lock().remote_app.clone();
    update_worker_difficulty(
        &WorkerContext {
            instance_id: instance_id.clone(),
            worker_name: worker_name.clone(),
            miner: remote_app.clone(),
            wallet: wallet_addr.clone(),
            ip: format!("{}:{}", client_clone.remote_addr(), client_clone.remote_port()),
        },
        current_diff,
    );

    debug!("[DIFFICULTY] ===== SENDING DIFFICULTY TO {} =====", client_clone.remote_addr);
    debug!("[DIFFICULTY] Difficulty value: {} (from state: {})", current_diff, state.stratum_diff().is_some());
    send_client_diff(&instance_id, &client_clone, &state, current_diff);
    share_handler.set_client_vardiff(&client_clone, min_diff);
    debug!("[DIFFICULTY] ===== DIFFICULTY SENT TO {} =====", client_clone.remote_addr);

    // Small delay to ensure difficulty is sent before job
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Build job params - check if this is an IceRiver or Bitmain miner
    let remote_app_lower = remote_app.to_lowercase();
    let is_iceriver =
        remote_app_lower.contains("iceriver") || remote_app_lower.contains("icemining") || remote_app_lower.contains("icm");
    let is_bitmain =
        remote_app_lower.contains("godminer") || remote_app_lower.contains("bitmain") || remote_app_lower.contains("antminer");

    debug!("[JOB] ===== BUILDING JOB FOR {} =====", client_clone.remote_addr);
    debug!("[JOB] Job ID: {}", job_id);
    debug!("[JOB] Remote app: '{}'", remote_app);
    debug!("[JOB] Is IceRiver: {}, Is Bitmain: {}, use_big_job: {}", is_iceriver, is_bitmain, state.use_big_job());
    debug!("[JOB] Pre-PoW hash: {}", pre_pow_hash);
    debug!("[JOB] Block timestamp: {}", block.header.timestamp);

    let mut job_params = vec![serde_json::Value::String(job_id.to_string())];
    debug!("[JOB] Job params initialized with job_id: {}", job_id);
    if state.use_big_job() && !is_iceriver {
        // BzMiner format - single hex string (big endian hash)
        // Convert Hash to bytes for BzMiner format
        debug!("[JOB] Generating BzMiner format job params");
        let header_bytes = pre_pow_hash.as_bytes();
        let large_params = generate_large_job_params(&header_bytes, block.header.timestamp);
        debug!("[JOB] BzMiner job_data length: {} (expected 80)", large_params.len());
        debug!("[JOB] BzMiner job_data (first 20 chars): {}", &large_params[..large_params.len().min(20)]);
        debug!("[JOB] BzMiner job_data (full): {}", large_params);
        job_params.push(serde_json::Value::String(large_params));
    } else if is_iceriver {
        // IceRiver format - single hex string (uses Hash::to_string() to match working stratum code)
        // This matches Ghostpool and other working implementations
        debug!("[JOB] Generating IceRiver format job params");
        let iceriver_params = generate_iceriver_job_params(&pre_pow_hash, block.header.timestamp);
        debug!("[JOB] IceRiver job_data length: {} (expected 80)", iceriver_params.len());
        debug!("[JOB] IceRiver job_data (first 20 chars): {}", &iceriver_params[..iceriver_params.len().min(20)]);
        debug!("[JOB] IceRiver job_data (full): {}", iceriver_params);
        job_params.push(serde_json::Value::String(iceriver_params));
    } else {
        // Legacy format - array + number (for Bitmain and other miners)
        let header_bytes = pre_pow_hash.as_bytes();
        let job_header = generate_job_header(&header_bytes);
        debug!("send_immediate_job: using Legacy format, array size: {}", job_header.len());
        job_params.push(serde_json::Value::Array(job_header.iter().map(|&v| serde_json::Value::Number(v.into())).collect()));
        job_params.push(serde_json::Value::Number(block.header.timestamp.into()));
    }

    debug!("[JOB] ===== SENDING MINING.NOTIFY TO {} =====", client_clone.remote_addr);
    debug!("[JOB] Method: mining.notify");
    debug!("[JOB] Params count: {}", job_params.len());

    // Also log the raw job data for verification
    if let Some(serde_json::Value::String(job_data)) = job_params.get(1) {
        debug!("[JOB] Job data string length: {} chars", job_data.len());
        if job_data.len() == 80 {
            let hash_part = &job_data[..64];
            let timestamp_part = &job_data[64..];
            debug!("[JOB] Hash part (64 hex): {}", hash_part);
            debug!("[JOB] Timestamp part (16 hex): {}", timestamp_part);
            debug!("[JOB] Full job_data: {}", job_data);
        } else {
            let expected_for = if is_iceriver {
                "IceRiver"
            } else if is_bitmain {
                "Bitmain"
            } else {
                "standard"
            };
            warn!("[JOB] WARNING - job_data length is {} (expected 80 for {})", job_data.len(), expected_for);
        }
    }

    let format_name = if is_iceriver {
        "IceRiver"
    } else if state.use_big_job() {
        "BzMiner"
    } else {
        "Legacy"
    };
    debug!("[JOB] Sending job ID {} to {} (format: {}, params: {})", job_id, client_clone.remote_addr, format_name, job_params.len());

    // IceRiver expects minimal notification format (method + params only, no id or jsonrpc)
    // Send job ID in mining.notify
    let send_result = if is_iceriver {
        // IceRiver expects minimal notification format (method + params only, no id or jsonrpc)
        client_clone.send_notification("mining.notify", job_params.clone()).await
    } else {
        // For non-IceRiver, use standard JSON-RPC format with job ID
        let notify_event = JsonRpcEvent {
            jsonrpc: "2.0".to_string(),
            method: "mining.notify".to_string(),
            id: Some(serde_json::Value::Number(job_id.into())),
            params: job_params.clone(),
        };
        client_clone.send(notify_event).await
    };

    if let Err(e) = send_result {
        if e.to_string().contains("disconnected") {
            record_worker_error(&instance_id, &wallet_addr, crate::errors::ErrorShortCode::Disconnected.as_str());
            error!("[JOB] ERROR: Failed to send job {} - client disconnected", job_id);
        } else {
            record_worker_error(&instance_id, &wallet_addr, crate::errors::ErrorShortCode::FailedSendWork.as_str());
            error!("[JOB] ERROR: Failed sending work packet {}: {}", job_id, e);
        }
        debug!("[JOB] ===== JOB SEND FAILED FOR {} =====", client_clone.remote_addr);
    } else {
        let wallet_addr_str = wallet_addr.clone();
        let worker_name = client_clone.identity.lock().worker_name.clone();
        record_new_job(&crate::prom::WorkerContext {
            instance_id: instance_id.clone(),
            worker_name: worker_name.clone(),
            miner: String::new(),
            wallet: wallet_addr_str.clone(),
            ip: format!("{}:{}", client_clone.remote_addr(), client_clone.remote_port()),
        });
        debug!("[JOB] Successfully sent job ID {} to client {}", job_id, client_clone.remote_addr);
        debug!("[JOB] ===== JOB SENT SUCCESSFULLY TO {} =====", client_clone.remote_addr);
    }
}
