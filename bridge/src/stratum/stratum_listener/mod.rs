//! Stratum TCP listener: accept loop, per-client read/framing, JSON-RPC dispatch.
//!
//! Internal modules: `types` (config + handler types), `listen` (bind/accept), `client_io/` (per-client read loop).

mod client_io;
mod listen;
mod types;

pub use types::{EventHandler, StateGenerator, StratumClientListener, StratumListenerConfig, StratumStats};

use crate::jsonrpc_event::JsonRpcEvent;
use crate::stratum_context::StratumContext;
use std::sync::Arc;

/// Stratum TCP listener
pub struct StratumListener {
    config: types::StratumListenerConfig,
    stats: Arc<parking_lot::Mutex<types::StratumStats>>,
    shutting_down: Arc<std::sync::atomic::AtomicBool>,
}

impl StratumListener {
    /// Create a new Stratum listener
    pub fn new(config: types::StratumListenerConfig) -> Self {
        Self {
            config,
            stats: Arc::new(parking_lot::Mutex::new(types::StratumStats::default())),
            shutting_down: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start listening for connections
    pub async fn listen(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.listen_impl(None).await
    }

    pub async fn listen_with_shutdown(
        &self,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.listen_impl(Some(shutdown_rx)).await
    }

    async fn listen_impl(
        &self,
        shutdown_rx: Option<tokio::sync::watch::Receiver<bool>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        listen::listen_impl(&self.config, &self.stats, &self.shutting_down, shutdown_rx).await
    }

    /// Handle an event
    pub fn handle_event(
        &self,
        _ctx: Arc<StratumContext>,
        event: JsonRpcEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(_handler) = self.config.handler_map.get(&event.method) {
            // Note: This is a sync wrapper - actual handlers should be async
            // For now, we'll handle this in spawn_client_listener
            Ok(())
        } else {
            Ok(())
        }
    }
}
