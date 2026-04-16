//! Stratum `mining.submit`: parse job/nonce, duplicate guard, PoW / pool diff, block pipeline.
//!
//! Submodules: [`parse`], [`duplicate`], [`pow_loop`], [`finish`]; [`handle`] wires them in order.

mod block_submit;
mod duplicate;
mod error;
pub use error::{SubmitError, SubmitRunError};
mod finish;
mod handle;
mod parse;
mod pow_loop;
mod pow_math;
mod pow_step;

use super::ShareHandler;
use super::kaspa_api_trait::KaspaApiTrait;
use crate::jsonrpc_event::JsonRpcEvent;
use crate::stratum_context::StratumContext;
use kaspa_consensus_core::block::Block;
use std::sync::Arc;

impl ShareHandler {
    pub async fn handle_submit(
        &self,
        ctx: Arc<StratumContext>,
        event: JsonRpcEvent,
        kaspa_api: Arc<dyn KaspaApiTrait + Send + Sync>,
    ) -> Result<(), SubmitRunError> {
        handle::handle_submit(self, ctx, event, kaspa_api).await
    }

    #[allow(dead_code)]
    async fn submit_block(
        &self,
        _ctx: &StratumContext,
        _block: Block,
        _nonce: u64,
        _event_id: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Block submission is handled at the HandleSubmit level
        // This method is kept for compatibility but actual submission
        // happens when PoW passes network target in handle_submit
        Ok(())
    }
}
