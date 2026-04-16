//! Parse Stratum `mining.submit` params: job id, nonce / extranonce, dedupe key.

use super::super::ShareHandler;
use super::error::SubmitError;
use crate::{
    errors::ErrorShortCode,
    jsonrpc_event::JsonRpcEvent,
    mining_state::{GetMiningState, Job},
    prom::record_worker_error,
    stratum_context::StratumContext,
};
use serde_json::Value;
use tracing::{debug, error, warn};

pub(super) struct PreparedSubmit {
    pub job_id: u64,
    pub job: Job,
    pub nonce_val: u64,
    /// Hex nonce after extranonce merge (same tail as in `submit_key`).
    #[allow(dead_code)]
    pub final_nonce_str: String,
    pub submit_key: String,
}

/// Validate params, resolve job, parse nonce; build `submit_key` for duplicate guard.
pub(super) fn prepare(handler: &ShareHandler, ctx: &StratumContext, event: &JsonRpcEvent) -> Result<PreparedSubmit, SubmitError> {
    let state = GetMiningState(ctx);
    let prefix = handler.log_prefix();
    debug!("{} [SUBMIT] ===== SHARE SUBMISSION FROM {} =====", prefix, ctx.remote_addr);
    debug!("{} [SUBMIT] Event ID: {:?}", prefix, event.id);
    debug!("{} [SUBMIT] Params count: {}", prefix, event.params.len());
    debug!("{} [SUBMIT] Full params: {:?}", prefix, event.params);

    let _max_jobs = state.max_jobs() as u64;
    let current_counter = state.current_job_counter();
    let stored_ids = state.get_stored_job_ids();
    debug!("{} [SUBMIT] Retrieved MiningState - counter: {}, stored IDs: {:?}", prefix, current_counter, stored_ids);

    if event.params.len() < 3 {
        error!("{} [SUBMIT] ERROR: Expected at least 3 params, got {}", prefix, event.params.len());
        let wallet_addr = ctx.identity.lock().wallet_addr.clone();
        record_worker_error(&handler.instance_id, &wallet_addr, ErrorShortCode::BadDataFromMiner.as_str());
        return Err(SubmitError::TooFewParams);
    }

    let prefix = handler.log_prefix();
    debug!("{} [SUBMIT] Params[0] (address/identity): {:?}", prefix, event.params.first());
    debug!("{} [SUBMIT] Params[1] (job_id): {:?}", prefix, event.params.get(1));
    debug!("{} [SUBMIT] Params[2] (nonce-ish): {:?}", prefix, event.params.get(2));

    if let Some(Value::String(submitted_identity)) = event.params.first() {
        let wallet_addr = ctx.identity.lock().wallet_addr.clone();

        let parts: Vec<&str> = submitted_identity.split('.').collect();
        let submitted_address = parts[0];

        let submitted_clean = submitted_address.trim_start_matches("kaspa:").trim_start_matches("kaspatest:");
        let authorized_clean = wallet_addr.trim_start_matches("kaspa:").trim_start_matches("kaspatest:");

        if submitted_clean.to_lowercase() != authorized_clean.to_lowercase() {
            debug!(
                "Submit params[0] address mismatch: submitted '{}' vs authorized '{}' (using authorized)",
                submitted_identity, wallet_addr
            );
        } else {
            debug!("Submit params[0] matches authorized address: {}", submitted_identity);
        }
    }

    let job_id = match &event.params[1] {
        serde_json::Value::String(s) => {
            debug!("[SUBMIT] Job ID is string: '{}'", s);
            s.parse::<u64>().map_err(|e| SubmitError::JobIdParse(e.to_string()))?
        }
        serde_json::Value::Number(n) => {
            debug!("[SUBMIT] Job ID is number: {}", n);
            n.as_u64().ok_or(SubmitError::JobIdOutOfRange)?
        }
        _ => {
            error!("[SUBMIT] ERROR: Job ID must be string or number, got: {:?}", event.params[1]);
            return Err(SubmitError::JobIdWrongType);
        }
    };

    debug!("[SUBMIT] Parsed job_id: {}", job_id);

    let current_job_counter = state.current_job_counter();
    debug!(
        "[SUBMIT] Current job counter: {}, submitted job_id: {} (diff: {})",
        current_job_counter,
        job_id,
        if job_id > current_job_counter {
            format!("+{}", job_id - current_job_counter)
        } else {
            format!("-{}", current_job_counter - job_id)
        }
    );

    let job = state.get_job(job_id);
    let current_counter = state.current_job_counter();
    let prefix = handler.log_prefix();
    let job = match job {
        Some(j) => {
            debug!("{} [SUBMIT] Found job ID {} (current counter: {})", prefix, job_id, current_counter);
            j
        }
        None => {
            let stored_job_ids = state.get_stored_job_ids();
            warn!(
                "[SUBMIT] Job ID {} not found at slot {} (current counter: {}, stored IDs: {:?})",
                job_id,
                job_id % 300,
                current_counter,
                stored_job_ids
            );
            let wallet_addr = ctx.identity.lock().wallet_addr.clone();
            record_worker_error(&handler.instance_id, &wallet_addr, ErrorShortCode::MissingJob.as_str());
            return Err(SubmitError::StaleJob);
        }
    };

    let nonce_param_idx = if event.params.len() >= 5 { 4 } else { 2 };
    let nonce_str = event.params[nonce_param_idx].as_str().ok_or(SubmitError::NonceNotString)?;
    debug!("[SUBMIT] Raw nonce string: '{}'", nonce_str);

    let nonce_str = nonce_str.replace("0x", "");
    debug!("[SUBMIT] Nonce after removing 0x: '{}' (length: {} hex chars)", nonce_str, nonce_str.len());

    let mut final_nonce_str = nonce_str.clone();
    {
        let extranonce = ctx.extranonce.lock();
        if !extranonce.is_empty() {
            let extranonce_val = extranonce.clone();
            let extranonce2_len = 16 - extranonce_val.len();

            if nonce_str.len() <= extranonce2_len {
                final_nonce_str = format!("{}{:0>width$}", extranonce_val, nonce_str, width = extranonce2_len);
                debug!(
                    "[SUBMIT] Extranonce prepended: '{}' = '{}' + '{:0>width$}'",
                    final_nonce_str,
                    extranonce_val,
                    nonce_str,
                    width = extranonce2_len
                );
            }
        }
    }

    debug!("[SUBMIT] Final nonce string: '{}'", final_nonce_str);
    let nonce_val = {
        let prefix = handler.log_prefix();
        u64::from_str_radix(&final_nonce_str, 16).map_err(|e| {
            error!("{} [SUBMIT] ERROR: Failed to parse nonce '{}' as hex: {}", prefix, final_nonce_str, e);
            SubmitError::NonceHexParse(e.to_string())
        })?
    };

    debug!("[SUBMIT] Parsed nonce value (u64): {}", nonce_val);
    debug!("[SUBMIT] Nonce hex: {:016x}", nonce_val);

    let worker_id = {
        let id = ctx.identity.lock();
        if !id.worker_name.is_empty() { id.worker_name.clone() } else { format!("{}:{}", ctx.remote_addr(), ctx.remote_port()) }
    };
    let submit_key = format!("{}|{}|{}", worker_id, job_id, final_nonce_str);

    Ok(PreparedSubmit { job_id, job, nonce_val, final_nonce_str, submit_key })
}
