//! After PoW loop: weak / low-diff share vs accepted share, metrics, `mining.submit` OK reply.

use super::super::ShareHandler;
use super::super::duplicate_submit::DuplicateSubmitOutcome;
use super::error::SubmitRunError;
use super::parse::PreparedSubmit;
use crate::{
    jsonrpc_event::{JsonRpcEvent, JsonRpcResponse},
    mining_state::GetMiningState,
    prom::{WorkerContext, record_share_found, record_weak_share},
    stratum_context::StratumContext,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::debug;

pub(super) async fn after_pow_loop(
    handler: &ShareHandler,
    ctx: Arc<StratumContext>,
    event: &JsonRpcEvent,
    prep: &PreparedSubmit,
    invalid_share: bool,
) -> Result<(), SubmitRunError> {
    let state = GetMiningState(ctx.as_ref());

    let stats = handler.get_create_stats(ctx.as_ref());

    if invalid_share {
        debug!("low diff share confirmed");
        *stats.invalid_shares.lock() += 1;
        *handler.overall.invalid_shares.lock() += 1;

        let wallet_addr = ctx.identity.lock().wallet_addr.clone();
        let worker_name = ctx.identity.lock().worker_name.clone();
        record_weak_share(&WorkerContext {
            instance_id: handler.instance_id.clone(),
            worker_name: worker_name.clone(),
            miner: String::new(),
            wallet: wallet_addr.clone(),
            ip: format!("{}:{}", ctx.remote_addr(), ctx.remote_port()),
        });

        if let Some(id) = &event.id {
            let _ = ctx.reply_low_diff_share(id).await;
        }

        {
            let now = Instant::now();
            let mut guard = handler.duplicate_submit_guard.lock();
            guard.set_outcome(&prep.submit_key, now, DuplicateSubmitOutcome::LowDiff);
        }
        return Ok(());
    }

    let stats = handler.get_create_stats(ctx.as_ref());
    *stats.shares_found.lock() += 1;
    *stats.var_diff_shares_found.lock() += 1;

    let hash_value = state.stratum_diff().map(|d| d.hash_value).unwrap_or(0.0);

    *stats.shares_diff.lock() += hash_value;
    *stats.last_share.lock() = Instant::now();
    *handler.overall.shares_found.lock() += 1;

    let wallet_addr = ctx.identity.lock().wallet_addr.clone();
    let worker_name = ctx.identity.lock().worker_name.clone();
    record_share_found(
        &WorkerContext {
            instance_id: handler.instance_id.clone(),
            worker_name: worker_name.clone(),
            miner: String::new(),
            wallet: wallet_addr.clone(),
            ip: format!("{}:{}", ctx.remote_addr(), ctx.remote_port()),
        },
        hash_value,
    );

    {
        let now = Instant::now();
        let mut guard = handler.duplicate_submit_guard.lock();
        guard.set_outcome(&prep.submit_key, now, DuplicateSubmitOutcome::Accepted);
    }

    ctx.reply(JsonRpcResponse {
        id: event.id.clone(),
        result: Some(serde_json::Value::Bool(true)),
        error: None,
    })
    .await
    .map_err(|e| SubmitRunError::ReplyFailed(e.to_string()))?;
    Ok(())
}
