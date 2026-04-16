//! Crate-wide error type for boundaries that still box into [`std::error::Error`] (e.g. Stratum `EventHandler`).

use crate::share_handler::SubmitRunError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error(transparent)]
    Submit(#[from] SubmitRunError),
}

impl BridgeError {
    pub fn into_boxed_stratum(self) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(self)
    }
}
