use crate::jsonrpc_event::JsonRpcEvent;
use crate::stratum_context::StratumContext;
use std::collections::HashMap;
use std::sync::Arc;

/// Event handler function type
pub type EventHandler = Arc<
    dyn Fn(
            Arc<StratumContext>,
            JsonRpcEvent,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<(), Box<dyn std::error::Error + Send + Sync>>,
                    > + Send,
            >,
        > + Send
        + Sync,
>;

/// Client listener trait
pub trait StratumClientListener: Send + Sync {
    fn on_connect(&self, ctx: Arc<StratumContext>);
    fn on_disconnect(&self, ctx: Arc<StratumContext>);
}

/// State generator function type
pub type StateGenerator = Box<dyn Fn() -> Arc<dyn std::any::Any + Send + Sync> + Send + Sync>;

/// Stratum listener statistics
#[derive(Debug, Default)]
pub struct StratumStats {
    pub disconnects: u64,
}

/// Configuration for the Stratum listener
pub struct StratumListenerConfig {
    pub handler_map: Arc<HashMap<String, EventHandler>>,
    pub on_connect: Arc<dyn Fn(Arc<StratumContext>) + Send + Sync>,
    pub on_disconnect: Arc<dyn Fn(Arc<StratumContext>) + Send + Sync>,
    pub port: String,
}
