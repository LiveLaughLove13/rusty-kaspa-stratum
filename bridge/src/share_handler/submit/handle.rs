//! Orchestrate `mining.submit`: parse → duplicate guard → PoW / block / pool diff → finish.

use super::super::ShareHandler;
use super::super::kaspa_api_trait::KaspaApiTrait;
use super::duplicate;
use super::error::SubmitRunError;
use super::finish;
use super::parse;
use super::pow_loop::{self, PowDone};
use crate::jsonrpc_event::JsonRpcEvent;
use crate::stratum_context::StratumContext;
use std::sync::Arc;

pub(super) async fn handle_submit(
    handler: &ShareHandler,
    ctx: Arc<StratumContext>,
    event: JsonRpcEvent,
    kaspa_api: Arc<dyn KaspaApiTrait + Send + Sync>,
) -> Result<(), SubmitRunError> {
    let prep = parse::prepare(handler, ctx.as_ref(), &event)?;

    if duplicate::respond_on_duplicate(handler, ctx.as_ref(), &event, &prep.submit_key).await? {
        return Ok(());
    }

    match pow_loop::run_pow_validation_loop(handler, Arc::clone(&ctx), &event, kaspa_api, &prep).await? {
        PowDone::AlreadyFinished => Ok(()),
        PowDone::Continue { invalid_share } => finish::after_pow_loop(handler, ctx, &event, &prep, invalid_share).await,
    }
}
