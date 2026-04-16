use super::{BIG_JOB_REGEX, CLIENT_TIMEOUT, send_client_diff};
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
use tracing::{debug, error, warn};

pub(crate) async fn new_block_job_task<T: KaspaApiTrait + Send + Sync + 'static>(
    client_clone: Arc<StratumContext>,
    kaspa_api_clone: Arc<T>,
    share_handler: Arc<ShareHandler>,
    min_diff: f64,
    instance_id: String,
) {
    let state = GetMiningState(&client_clone);

    // Check if client has wallet address
    let wallet_addr_str = {
        let id = client_clone.identity.lock();
        if id.wallet_addr.is_empty() {
            let connect_time = state.connect_time();
            if let Ok(elapsed) = connect_time.elapsed()
                && elapsed > CLIENT_TIMEOUT
            {
                warn!("client misconfigured, no miner address specified - disconnecting");
                let wallet_str = id.wallet_addr.clone();
                record_worker_error(&instance_id, &wallet_str, crate::errors::ErrorShortCode::NoMinerAddress.as_str());
                drop(id); // Drop before disconnect
                client_clone.disconnect();
            }
            debug!("new_block_available: client {} has no wallet address yet, skipping", client_clone.remote_addr);
            return;
        }
        id.wallet_addr.clone()
    };

    debug!("new_block_available: fetching block template for client {} (wallet: {})", client_clone.remote_addr, wallet_addr_str);

    // Get block template
    let (wallet_addr, remote_app, canxium_addr) = {
        let id = client_clone.identity.lock();
        (id.wallet_addr.clone(), id.remote_app.clone(), id.canxium_addr.clone())
    };

    let template_result = kaspa_api_clone.get_block_template(&wallet_addr, &remote_app, &canxium_addr).await;

    let block = match template_result {
        Ok(block) => {
            debug!("new_block_available: successfully fetched block template for client {}", client_clone.remote_addr);
            block
        }
        Err(e) => {
            if e.to_string().contains("Could not decode address") {
                record_worker_error(&instance_id, &wallet_addr, crate::errors::ErrorShortCode::InvalidAddressFmt.as_str());
                error!("failed fetching new block template from kaspa, malformed address: {}", e);
                client_clone.disconnect();
            } else {
                record_worker_error(&instance_id, &wallet_addr, crate::errors::ErrorShortCode::FailedBlockFetch.as_str());
                error!("failed fetching new block template from kaspa: {}", e);
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
            error!("failed to serialize block header: {}", error_msg);

            // Log block header details for debugging
            debug!("Block header version: {}", block.header.version);
            debug!("Block header timestamp: {}", block.header.timestamp);
            debug!("Block header bits: {}", block.header.bits);
            debug!("Block header daa_score: {}", block.header.daa_score);
            debug!("Block header blue_score: {}", block.header.blue_score);
            debug!("Block header parents_by_level expanded_len: {}", block.header.parents_by_level.expanded_len());

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
        "[JOB CREATION] new_block_available: created job ID {} for client {} (counter: {}, stored IDs: {:?})",
        job_id, client_clone.remote_addr, counter_after, stored_ids
    );

    // Initialize state if first time (per-client state initialization)
    if !state.is_initialized() {
        state.set_initialized(true);
        let use_big_job = BIG_JOB_REGEX.is_match(&remote_app);
        state.set_use_big_job(use_big_job);

        // Send initial difficulty
        use crate::hasher::KaspaDiff;
        let mut stratum_diff = KaspaDiff::new();
        // Use miner-specific calculation (IceRiver uses different formula)
        let remote_app = client_clone.identity.lock().remote_app.clone();
        stratum_diff.set_diff_value_for_miner(min_diff, &remote_app);
        state.set_stratum_diff(stratum_diff);

        // Update worker difficulty metric
        let wallet_addr = client_clone.identity.lock().wallet_addr.clone();
        let worker_name = client_clone.identity.lock().worker_name.clone();
        update_worker_difficulty(
            &WorkerContext {
                instance_id: instance_id.clone(),
                worker_name: worker_name.clone(),
                miner: remote_app.clone(),
                wallet: wallet_addr.clone(),
                ip: format!("{}:{}", client_clone.remote_addr(), client_clone.remote_port()),
            },
            min_diff,
        );

        let target = state.stratum_diff().map(|d| d.target_value.clone()).unwrap_or_else(BigUint::zero);
        let target_bytes = target.to_bytes_be();
        debug!(
            "Initialized per-client MiningState with difficulty: {}, target: {:x} ({} bytes, {} bits)",
            min_diff,
            target,
            target_bytes.len(),
            target_bytes.len() * 8
        );
        send_client_diff(&instance_id, &client_clone, &state, min_diff);
        share_handler.set_client_vardiff(&client_clone, min_diff);
    } else {
        // Check for vardiff update
        if let Some(mut stratum_diff) = state.stratum_diff() {
            let current_diff = stratum_diff.diff_value;
            let mut var_diff = share_handler.get_client_vardiff(&client_clone);

            // Recover from stale/recreated stats entries that can report 0.0 diff.
            // Seed back to current state diff so UI/terminal does not stick at zero.
            if var_diff <= 0.0 && current_diff > 0.0 {
                share_handler.set_client_vardiff(&client_clone, current_diff);
                share_handler.start_client_vardiff(&client_clone);
                var_diff = current_diff;
            }

            if var_diff != current_diff {
                debug!("changing diff from {} to {}", current_diff, var_diff);
                // Use miner-specific calculation (IceRiver uses different formula)
                let remote_app = client_clone.identity.lock().remote_app.clone();
                stratum_diff.set_diff_value_for_miner(var_diff, &remote_app);
                state.set_stratum_diff(stratum_diff);

                // Update worker difficulty metric
                let wallet_addr = client_clone.identity.lock().wallet_addr.clone();
                let worker_name = client_clone.identity.lock().worker_name.clone();
                update_worker_difficulty(
                    &WorkerContext {
                        instance_id: instance_id.clone(),
                        worker_name: worker_name.clone(),
                        miner: remote_app.clone(),
                        wallet: wallet_addr.clone(),
                        ip: format!("{}:{}", client_clone.remote_addr(), client_clone.remote_port()),
                    },
                    var_diff,
                );

                send_client_diff(&instance_id, &client_clone, &state, var_diff);
                share_handler.start_client_vardiff(&client_clone);
            }
        }
    }

    // Build job params
    // Check if this is an IceRiver or Bitmain miner - they need single hex string format
    let remote_app = client_clone.identity.lock().remote_app.clone();
    let remote_app_lower = remote_app.to_lowercase();
    let is_iceriver =
        remote_app_lower.contains("iceriver") || remote_app_lower.contains("icemining") || remote_app_lower.contains("icm");
    let is_bitmain =
        remote_app_lower.contains("godminer") || remote_app_lower.contains("bitmain") || remote_app_lower.contains("antminer");

    debug!(
        "[JOB] new_block_available: client {}, is_iceriver: {}, is_bitmain: {}, use_big_job: {}",
        client_clone.remote_addr,
        is_iceriver,
        is_bitmain,
        state.use_big_job()
    );

    let mut job_params = vec![serde_json::Value::String(job_id.to_string())];
    if is_iceriver {
        // IceRiver format - single hex string (uses Hash::to_string() to match working stratum code)
        // This matches Ghostpool and other working implementations
        debug!("[JOB] new_block_available: Generating IceRiver format job params");
        let iceriver_params = generate_iceriver_job_params(&pre_pow_hash, block.header.timestamp);
        debug!("[JOB] new_block_available: IceRiver job_data length: {} (expected 80)", iceriver_params.len());
        job_params.push(serde_json::Value::String(iceriver_params));
    } else if state.use_big_job() && !is_iceriver {
        // BzMiner format - single hex string (big endian hash)
        // Convert Hash to bytes for BzMiner format
        debug!("[JOB] new_block_available: Generating BzMiner format job params");
        let header_bytes = pre_pow_hash.as_bytes();
        let large_params = generate_large_job_params(&header_bytes, block.header.timestamp);
        debug!("[JOB] new_block_available: BzMiner job_data length: {} (expected 80)", large_params.len());
        job_params.push(serde_json::Value::String(large_params));
    } else {
        // Legacy format - array + number (for Bitmain and other miners)
        debug!("[JOB] new_block_available: Using Legacy format (array + timestamp)");
        let header_bytes = pre_pow_hash.as_bytes();
        let job_header = generate_job_header(&header_bytes);
        job_params.push(serde_json::Value::Array(job_header.iter().map(|&v| serde_json::Value::Number(v.into())).collect()));
        job_params.push(serde_json::Value::Number(block.header.timestamp.into()));
    }

    // IceRiver expects minimal notification format (method + params only, no id or jsonrpc)
    // This matches StratumNotification format used by the stratum crate
    let (is_iceriver_client, is_bitmain_client) = {
        let app = client_clone.identity.lock().remote_app.clone();
        let lower = app.to_lowercase();
        (app.contains("IceRiver"), lower.contains("godminer") || lower.contains("bitmain") || lower.contains("antminer"))
    };

    debug!(
        "new_block_available: sending job ID {} to client {} (params count: {}, is_iceriver: {}, is_bitmain: {})",
        job_id,
        client_clone.remote_addr,
        job_params.len(),
        is_iceriver_client,
        is_bitmain_client
    );

    // Send job ID in mining.notify
    // })
    let send_result = if is_iceriver_client {
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
            warn!("new_block_available: failed to send job {} - client disconnected", job_id);
        } else {
            record_worker_error(&instance_id, &wallet_addr, crate::errors::ErrorShortCode::FailedSendWork.as_str());
            error!("failed sending work packet {}: {}", job_id, e);
            error!("new_block_available: failed to send job {} to client {}: {}", job_id, client_clone.remote_addr, e);
        }
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
        debug!("new_block_available: successfully sent job ID {} to client {}", job_id, client_clone.remote_addr);
    }
}
