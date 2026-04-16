//! Extranonce assignment after miner type is detected (`mining.subscribe`).

use crate::stratum_context::StratumContext;
use std::sync::atomic::{AtomicI32, Ordering};
use tracing::{debug, warn};

static GLOBAL_NEXT_EXTRANONCE: AtomicI32 = AtomicI32::new(0);

/// Assign extranonce to a client based on detected miner type.
/// Called from `handle_subscribe` after miner type is detected.
pub fn assign_extranonce_for_miner(ctx: &StratumContext, remote_app: &str) {
    let remote_app_lower = remote_app.to_lowercase();
    let is_bitmain = remote_app_lower.contains("godminer")
        || remote_app_lower.contains("bitmain")
        || remote_app_lower.contains("antminer");

    let required_extranonce_size = if is_bitmain { 0 } else { 2 };

    let extranonce = if required_extranonce_size > 0 {
        let max_extranonce = (2_f64.powi(16) - 1.0) as i32;

        let extranonce_val =
            match GLOBAL_NEXT_EXTRANONCE.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |val| {
                if val < max_extranonce {
                    Some(val + 1)
                } else {
                    Some(0)
                }
            }) {
                Ok(prev) => {
                    if prev >= max_extranonce {
                        warn!("wrapped extranonce! new clients may be duplicating work...");
                    }
                    if prev < max_extranonce { prev + 1 } else { 0 }
                }
                Err(_) => 0,
            };
        let extranonce_str = format!(
            "{:0width$x}",
            extranonce_val,
            width = (required_extranonce_size * 2) as usize
        );
        debug!(
            "[AUTO-EXTRANONCE] Assigned extranonce '{}' (value: {}, size: {} bytes) to {} miner '{}'",
            extranonce_str,
            extranonce_val,
            required_extranonce_size,
            if is_bitmain {
                "Bitmain"
            } else {
                "IceRiver/BzMiner/Goldshell"
            },
            remote_app
        );
        extranonce_str
    } else {
        debug!(
            "[AUTO-EXTRANONCE] Assigned empty extranonce (size: 0 bytes) to Bitmain miner '{}'",
            remote_app
        );
        String::new()
    };

    *ctx.extranonce.lock() = extranonce.clone();

    debug!(
        "[AUTO-EXTRANONCE] Client {} extranonce set to '{}' (detected miner: '{}', type: {})",
        ctx.remote_addr,
        extranonce,
        remote_app,
        if is_bitmain {
            "Bitmain"
        } else {
            "IceRiver/BzMiner/Goldshell"
        }
    );
}
