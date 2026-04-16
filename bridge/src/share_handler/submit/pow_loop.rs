//! PoW validation, network block submit, pool difficulty, job-ID workaround.

use super::super::ShareHandler;
use super::super::kaspa_api_trait::KaspaApiTrait;
use super::block_submit::{BlockSubmitFlowResult, run_block_found_submit_flow};
use super::error::SubmitRunError;
use super::parse::PreparedSubmit;
use super::pow_math;
use super::pow_step::evaluate_job_pow;
use crate::{log_colors::LogColors, mining_state::GetMiningState, stratum_context::StratumContext};
use num_traits::ToPrimitive;
use num_traits::Zero;
use std::sync::Arc;
use tracing::{debug, warn};

pub(super) enum PowDone {
    /// `mining.submit` already answered (stale/bad block path).
    AlreadyFinished,
    /// Run weak-share vs accepted-share finishing in `finish`.
    Continue { invalid_share: bool },
}

pub(super) async fn run_pow_validation_loop(
    handler: &ShareHandler,
    ctx: Arc<StratumContext>,
    event: &crate::jsonrpc_event::JsonRpcEvent,
    kaspa_api: Arc<dyn KaspaApiTrait + Send + Sync>,
    prep: &PreparedSubmit,
) -> Result<PowDone, SubmitRunError> {
    let state = GetMiningState(ctx.as_ref());
    let nonce_val = prep.nonce_val;
    // PoW validation with job ID workaround
    // Go validates the submitted job first, then tries previous jobs if share doesn't meet pool difficulty
    // This workaround handles IceRiver/Bitmain ASICs that submit jobs with incorrect IDs
    let mut current_job_id = prep.job_id;
    let mut current_job = prep.job.clone();
    let mut invalid_share = false;
    let mut pow_passed;
    let mut pow_value;
    let max_jobs = state.max_jobs() as u64;

    debug!("[SUBMIT] Starting PoW validation for job_id: {} (max_jobs: {})", current_job_id, max_jobs);

    loop {
        // DIAGNOSTIC: Run full diagnostic on first share
        static DIAGNOSTIC_RUN: std::sync::Once = std::sync::Once::new();
        let header = &current_job.block.header;
        let mut header_clone = (**header).clone();

        DIAGNOSTIC_RUN.call_once(|| {
            debug!("{}", LogColors::block("===== RUNNING POW DIAGNOSTIC ====="));
            crate::pow_diagnostic::diagnose_pow_issue(&header_clone, nonce_val);
            debug!("{}", LogColors::block("===== DIAGNOSTIC COMPLETE ====="));
        });

        // DEBUG: Compare what we sent to ASIC vs what we're validating (moved to debug level)
        debug!("{} {}", LogColors::validation("[DEBUG]"), LogColors::label("===== VALIDATION DEBUG ====="));
        debug!(
            "{} {} {}",
            LogColors::validation("[DEBUG]"),
            LogColors::label("Job we sent to ASIC:"),
            format!("job_id={}, timestamp={}", current_job_id, current_job.block.header.timestamp)
        );
        debug!(
            "{} {} {}",
            LogColors::validation("[DEBUG]"),
            LogColors::label("ASIC submitted:"),
            format!("job_id={}, nonce=0x{:x}", current_job_id, nonce_val)
        );
        debug!(
            "{} {} {}",
            LogColors::validation("[DEBUG]"),
            LogColors::label("Header we're validating:"),
            format!("timestamp={}, nonce={}, bits=0x{:08x}", header_clone.timestamp, header_clone.nonce, header_clone.bits)
        );

        // Set the nonce in the header
        header_clone.nonce = nonce_val;

        debug!(
            "{} {} {}",
            LogColors::validation("[DEBUG]"),
            LogColors::label("After setting nonce:"),
            format!("timestamp={}, nonce=0x{:x}, bits=0x{:08x}", header_clone.timestamp, header_clone.nonce, header_clone.bits)
        );

        let snapshot = evaluate_job_pow(&header_clone, nonce_val);
        pow_value = snapshot.pow_value;
        let network_target = snapshot.network_target;
        let meets_network_target = snapshot.meets_network_target;
        // IMPORTANT: Use kaspa_pow's own compact-target handling as the source of truth.
        // This avoids any potential mismatch in our BigUint conversion/comparison path.
        pow_passed = snapshot.check_passed;

        debug!(
            "{} {} {}",
            LogColors::validation("[DEBUG]"),
            LogColors::label("PowState result:"),
            format!("check_passed={}, pow_value={:x}", snapshot.check_passed, pow_value)
        );

        let pow_value_bytes = pow_value.to_bytes_be();
        let network_target_bytes = network_target.to_bytes_be();

        debug!("[SUBMIT] Target comparison:");
        debug!("[SUBMIT]   - pow_value: {:x} ({} bytes)", pow_value, pow_value_bytes.len());
        debug!("[SUBMIT]   - network_target: {:x} ({} bytes)", network_target, network_target_bytes.len());
        debug!("[SUBMIT]   - meets_network_target(BigUint): {}", meets_network_target);
        debug!("[SUBMIT]   - check_passed(kaspa_pow): {}", pow_passed);

        debug!(
            "[SUBMIT] PoW check result: passed={}, pow_value={:x}, network_target={:x}, header.bits={}",
            pow_passed, pow_value, network_target, snapshot.header_bits
        );

        // Log detailed validation information with colors (moved to debug level)
        debug!(
            "{} {} {}",
            LogColors::validation("[VALIDATION]"),
            LogColors::label("PoW Validation -"),
            format!(
                "Nonce: {:x}, Pow Value: {:x} ({} bytes), Network Target: {:x} ({} bytes)",
                nonce_val,
                pow_value,
                pow_value_bytes.len(),
                network_target,
                network_target_bytes.len()
            )
        );
        debug!(
            "{} {} {}",
            LogColors::validation("[VALIDATION]"),
            LogColors::label("Comparison:"),
            format!("pow_value <= network_target = {} (lower hash is better)", meets_network_target)
        );
        debug!(
            "{} {} {}",
            LogColors::validation("[VALIDATION]"),
            LogColors::label("PowState.check_pow() result:"),
            format!("passed={}, Header bits: {}", pow_passed, snapshot.header_bits)
        );

        // On devnet, network difficulty is very low, so we should see blocks being found
        // Log at debug level (detailed validation logs moved to debug)
        if pow_passed {
            debug!(
                "{} {} {}",
                LogColors::validation("[VALIDATION]"),
                LogColors::block("*** NETWORK TARGET PASSED ***"),
                format!("pow_value={:x} <= network_target={:x}", pow_value, network_target)
            );
        } else if !network_target.is_zero() {
            let ratio = if !pow_value.is_zero() {
                let target_f64 = network_target.to_f64().unwrap_or(0.0);
                let pow_f64 = pow_value.to_f64().unwrap_or(1.0);
                if pow_f64 > 0.0 { (target_f64 / pow_f64) * 100.0 } else { 0.0 }
            } else {
                0.0
            };
            debug!(
                "{} {} {}",
                LogColors::validation("[VALIDATION]"),
                LogColors::label("Network target NOT met -"),
                format!("pow_value={:x} > network_target={:x} ({}% of target)", pow_value, network_target, ratio)
            );
        } else {
            warn!("{} {}", LogColors::validation("[VALIDATION]"), LogColors::error("Network target is ZERO - cannot validate!"));
        }

        // Check network target (block)
        // Use meets_network_target (not pow_passed) for network target validation
        // Go code compares: powValue.Cmp(&powState.Target) <= 0 where Target is network target from header.bits
        // We calculate network_target directly from current job's header.bits (not from stored state)
        // This ensures we use the correct target for each job, as different jobs may have different header.bits
        if meets_network_target {
            match run_block_found_submit_flow(
                handler,
                &ctx,
                event,
                &kaspa_api,
                prep,
                header_clone,
                &current_job,
                current_job_id,
                nonce_val,
                &pow_value,
                &network_target,
            )
            .await?
            {
                BlockSubmitFlowResult::Break { invalid_share: inv } => {
                    invalid_share = inv;
                    break;
                }
                BlockSubmitFlowResult::Finished => return Ok(PowDone::AlreadyFinished),
            }
        }

        // Check pool difficulty
        let pool_target = pow_math::pool_target_or_zero(state.stratum_diff().map(|d| d.target_value.clone()));

        // Compare FULL pow_value against pool_target (not just lower bits)
        // Compare full 256-bit values
        let pow_bytes = pow_value.to_bytes_be();
        let target_bytes = pool_target.to_bytes_be();

        // Log difficulty check for debugging
        if pool_target.is_zero() {
            warn!("stratum_diff target is zero! pow_value: {:x}, pool_target: {:x}", pow_value, pool_target);
        } else {
            let pow_len = pow_bytes.len();
            let target_len = target_bytes.len();

            debug!(
                "difficulty check: nonce: {:x} ({}), pow_value (full): {:x} ({} bytes), pool_target: {:x} ({} bytes), diff_value: {:?}, pow_value <= pool_target = {}",
                nonce_val,
                nonce_val,
                pow_value,
                pow_len,
                pool_target,
                target_len,
                state.stratum_diff().map(|d| d.diff_value),
                !pow_math::share_too_weak_for_pool(&pow_value, &pool_target)
            );
            debug!(
                "Full comparison - pow_value: {:x} ({} bytes), pool_target: {:x} ({} bytes)",
                pow_value, pow_len, pool_target, target_len
            );
        }

        // Check pool difficulty (stratum target)
        // If pow_value >= pool_target, share doesn't meet pool difficulty
        // Higher hash value means worse share
        if pow_math::share_too_weak_for_pool(&pow_value, &pool_target) {
            // Share doesn't meet pool difficulty - might be wrong job ID (moved to debug to keep terminal clean)
            let worker_name = ctx.identity.lock().worker_name.clone();
            debug!(
                "{} {} {}",
                LogColors::validation("INVALID SHARE (too high)"),
                LogColors::label("worker:"),
                format!(
                    "{}, nonce: {:x}, pow_value: {:x}, pool_target: {:x}, pow_ge_pool_target: true",
                    worker_name, nonce_val, pow_value, pool_target
                )
            );

            if current_job_id == prep.job_id {
                debug!("low diff share... checking for bad job ID ({})", current_job_id);
                invalid_share = true;
            }

            // Job ID workaround for Bitmain/IceRiver ASICs - try previous jobs
            // Validate job ID: jobId == 1 || jobId%maxJobs == submitInfo.jobId%maxJobs+1
            match pow_math::weak_share_job_advance(current_job_id, prep.job_id, max_jobs) {
                pow_math::WeakShareJobAdvance::Exhausted => {
                    debug!("Job ID loop exhausted: current_job_id={}, job_id={}, max_jobs={}", current_job_id, prep.job_id, max_jobs);
                    break;
                }
                pow_math::WeakShareJobAdvance::NoPreviousJob => break,
                pow_math::WeakShareJobAdvance::RetryPreviousJob { job_id: prev_job_id } => {
                    if let Some(prev_job) = state.get_job(prev_job_id) {
                        current_job_id = prev_job_id;
                        current_job = prev_job;
                        debug!("Trying previous job ID: {} (submitted as {})", current_job_id, prep.job_id);
                        continue;
                    } else {
                        debug!("Previous job ID {} doesn't exist, exiting loop", prev_job_id);
                        break;
                    }
                }
            }
        } else {
            // Valid share (pow_value < pool_target) - moved to debug to keep terminal clean
            let worker_name = ctx.identity.lock().worker_name.clone();
            debug!(
                "{} {} {}",
                LogColors::validation("VALID SHARE"),
                LogColors::label("worker:"),
                format!(
                    "{}, nonce: {:x}, pow_value: {:x}, pool_target: {:x}, pow_lt_pool_target: true",
                    worker_name, nonce_val, pow_value, pool_target
                )
            );

            if invalid_share {
                debug!("found correct job ID: {} (submitted as {})", current_job_id, prep.job_id);
            }
            invalid_share = false;
            break;
        }
    }

    Ok(PowDone::Continue { invalid_share })
}
