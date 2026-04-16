use crate::{jsonrpc_event::JsonRpcEvent, mining_state::MiningState, prom::*, stratum_context::StratumContext};
use tracing::{debug, error};

/// Send `mining.set_difficulty` to a client (spawned).
pub fn send_client_diff(instance_id: &str, client: &StratumContext, _state: &MiningState, diff: f64) {
    debug!("[DIFFICULTY] Building difficulty message for {}", client.remote_addr);

    let instance_id = instance_id.to_string();

    let diff_value =
        serde_json::Value::Number(serde_json::Number::from_f64(diff).unwrap_or_else(|| serde_json::Number::from(diff as u64)));

    let client_clone = client.clone();
    tokio::spawn(async move {
        debug!("[DIFFICULTY] Sending mining.set_difficulty to {}", client_clone.remote_addr);

        let diff_event = JsonRpcEvent {
            jsonrpc: "2.0".to_string(),
            method: "mining.set_difficulty".to_string(),
            id: None,
            params: vec![diff_value],
        };

        let send_result = client_clone.send(diff_event).await;

        if let Err(e) = send_result {
            let wallet_addr = client_clone.identity.lock().wallet_addr.clone();
            record_worker_error(&instance_id, &wallet_addr, crate::errors::ErrorShortCode::FailedSetDiff.as_str());
            error!("[DIFFICULTY] ERROR: Failed sending difficulty: {}", e);
            return;
        }
        debug!("[DIFFICULTY] Successfully sent difficulty {} to {}", diff, client_clone.remote_addr);
    });
}
