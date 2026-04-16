//! In-flight / cached duplicate submit guard and Stratum responses.

use super::super::ShareHandler;
use super::super::duplicate_submit::DuplicateSubmitOutcome;
use super::error::SubmitRunError;
use crate::jsonrpc_event::{JsonRpcEvent, JsonRpcResponse};
use crate::stratum_context::StratumContext;
use std::time::Instant;

/// Register in-flight or return early with JSON-RPC reply if duplicate rules apply.
/// Returns `Ok(true)` if the request is fully handled (caller should return `Ok(())`).
pub(super) async fn respond_on_duplicate(
    handler: &ShareHandler,
    ctx: &StratumContext,
    event: &JsonRpcEvent,
    submit_key: &str,
) -> Result<bool, SubmitRunError> {
    let duplicate_outcome = {
        let now = Instant::now();
        let mut guard = handler.duplicate_submit_guard.lock();
        if let Some(outcome) = guard.get(submit_key, now) {
            Some(outcome)
        } else {
            guard.insert_inflight(submit_key.to_string(), now);
            None
        }
    };

    if let Some(outcome) = duplicate_outcome {
        match outcome {
            DuplicateSubmitOutcome::Accepted | DuplicateSubmitOutcome::InFlight => {
                ctx.reply(JsonRpcResponse { id: event.id.clone(), result: Some(serde_json::Value::Bool(true)), error: None }).await?;
                return Ok(true);
            }
            DuplicateSubmitOutcome::Stale => {
                ctx.reply_stale_share(event.id.clone()).await?;
                return Ok(true);
            }
            DuplicateSubmitOutcome::LowDiff => {
                if let Some(id) = &event.id {
                    let _ = ctx.reply_low_diff_share(id).await;
                }
                return Ok(true);
            }
            DuplicateSubmitOutcome::Bad => {
                ctx.reply_bad_share(event.id.clone()).await?;
                return Ok(true);
            }
        }
    }

    Ok(false)
}
