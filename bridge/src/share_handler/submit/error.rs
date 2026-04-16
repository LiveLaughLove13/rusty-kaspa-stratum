//! Typed errors for the `mining.submit` pipeline (`parse` → duplicate → PoW → finish).

use crate::stratum_context::ErrorDisconnected;
use thiserror::Error;

/// Synchronous validation / parsing failures before PoW runs.
#[derive(Debug, Error)]
pub enum SubmitError {
    #[error("malformed event, expected at least 3 params")]
    TooFewParams,
    #[error("job id must be a string or number")]
    JobIdWrongType,
    #[error("job id is not parsable as a number: {0}")]
    JobIdParse(String),
    #[error("job id number is out of range")]
    JobIdOutOfRange,
    #[error("job does not exist (stale)")]
    StaleJob,
    #[error("nonce must be a string")]
    NonceNotString,
    #[error("failed parsing nonce as hex: {0}")]
    NonceHexParse(String),
}

/// Full async submit flow: wraps parse errors, Stratum I/O, and reply failures.
#[derive(Debug, Error)]
pub enum SubmitRunError {
    #[error(transparent)]
    Validation(#[from] SubmitError),
    #[error(transparent)]
    StratumDisconnected(#[from] ErrorDisconnected),
    #[error("failed to send JSON-RPC reply: {0}")]
    ReplyFailed(String),
}

/// Kaspa node outcome when `submit_block` returns an error (stringly-typed RPC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockSubmitRejection {
    /// Duplicate / already-known block (e.g. `ErrDuplicateBlock` in RPC text).
    DuplicateBlockStale,
    /// Any other validation or transport failure.
    Other,
}

/// Classifies `submit_block` failures without changing node semantics.
///
/// The node still exposes this as text; we keep a single place for the substring match.
pub(crate) fn classify_block_submit_error_message(message: &str) -> BlockSubmitRejection {
    if message.contains("ErrDuplicateBlock") {
        BlockSubmitRejection::DuplicateBlockStale
    } else {
        BlockSubmitRejection::Other
    }
}

#[cfg(test)]
mod block_submit_classify_tests {
    use super::*;

    #[test]
    fn duplicate_block_detected() {
        assert_eq!(
            classify_block_submit_error_message("rpc error: ErrDuplicateBlock: ..."),
            BlockSubmitRejection::DuplicateBlockStale
        );
    }

    #[test]
    fn other_errors_not_duplicate() {
        assert_eq!(
            classify_block_submit_error_message("stale header"),
            BlockSubmitRejection::Other
        );
        assert_eq!(
            classify_block_submit_error_message("duplicate tx"),
            BlockSubmitRejection::Other
        );
    }
}
