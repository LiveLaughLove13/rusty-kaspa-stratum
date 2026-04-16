use std::sync::atomic::Ordering;
use std::time::Duration;

use hex;
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use super::{ErrorDisconnected, StratumContext};
use crate::jsonrpc_event::{JsonRpcEvent, JsonRpcResponse};
use crate::log_colors::LogColors;

impl StratumContext {
    /// Send a JSON-RPC response
    pub async fn reply(&self, response: JsonRpcResponse) -> Result<(), ErrorDisconnected> {
        if self.disconnecting.load(Ordering::Acquire) {
            return Err(ErrorDisconnected);
        }

        let json = serde_json::to_string(&response).map_err(|_| ErrorDisconnected)?;
        let data = format!("{}\n", json);

        // Get client context for detailed logging
        let (wallet_addr, worker_name, remote_app) = {
            let id = self.identity.lock();
            (
                id.wallet_addr.clone(),
                id.worker_name.clone(),
                id.remote_app.clone(),
            )
        };

        // Log outgoing response at DEBUG level (detailed logs moved to debug)
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("========================================")
        );
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("===== SENDING RESPONSE TO ASIC ===== ")
        );
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("========================================")
        );
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Client Information:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - IP Address:"),
            format!("{}:{}", self.remote_addr, self.remote_port)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Wallet Address:"),
            format!("'{}'", wallet_addr)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Worker Name:"),
            format!("'{}'", worker_name)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Miner Application:"),
            format!("'{}'", remote_app)
        );
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Response Details:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Response ID:"),
            format!("{:?}", response.id)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Response Type:"),
            "JSON-RPC Response"
        );
        if let Some(ref result) = response.result {
            let result_str = serde_json::to_string(result).unwrap_or_else(|_| "N/A".to_string());
            tracing::debug!(
                "{} {} {}",
                LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
                LogColors::label("  - Result:"),
                result_str
            );
            tracing::debug!(
                "{} {} {}",
                LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
                LogColors::label("  - Result Length:"),
                format!("{} characters", result_str.len())
            );
        }
        if let Some(ref error) = response.error {
            let error_str = serde_json::to_string(error).unwrap_or_else(|_| "N/A".to_string());
            tracing::debug!(
                "{} {} {}",
                LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
                LogColors::error("  - Error:"),
                error_str
            );
            tracing::debug!(
                "{} {} {}",
                LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
                LogColors::label("  - Error Length:"),
                format!("{} characters", error_str.len())
            );
        }
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Message Data:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Raw JSON:"),
            json
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - JSON Length:"),
            format!("{} characters", json.len())
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Total Bytes (with newline):"),
            format!("{} bytes", data.len())
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Raw Bytes (hex):"),
            hex::encode(data.as_bytes())
        );
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("========================================")
        );

        self.write_data(data.as_bytes()).await?;
        Ok(())
    }

    /// Send a JSON-RPC event
    pub async fn send(&self, event: JsonRpcEvent) -> Result<(), ErrorDisconnected> {
        if self.disconnecting.load(Ordering::Acquire) {
            return Err(ErrorDisconnected);
        }

        let json = serde_json::to_string(&event).map_err(|_| ErrorDisconnected)?;
        let data = format!("{}\n", json);

        // Get client context for detailed logging
        let (wallet_addr, worker_name, remote_app) = {
            let id = self.identity.lock();
            (
                id.wallet_addr.clone(),
                id.worker_name.clone(),
                id.remote_app.clone(),
            )
        };
        let params_str = serde_json::to_string(&event.params).unwrap_or_else(|_| "[]".to_string());

        // Log outgoing event at DEBUG level (detailed logs moved to debug)
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("========================================")
        );
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("===== SENDING EVENT TO ASIC ===== ")
        );
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("========================================")
        );
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Client Information:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - IP Address:"),
            format!("{}:{}", self.remote_addr, self.remote_port)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Wallet Address:"),
            format!("'{}'", wallet_addr)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Worker Name:"),
            format!("'{}'", worker_name)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Miner Application:"),
            format!("'{}'", remote_app)
        );
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Event Details:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Method:"),
            format!("'{}'", event.method)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Event ID:"),
            format!("{:?}", event.id)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - JSON-RPC Version:"),
            format!("'{}'", event.jsonrpc)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Format:"),
            "Standard JSON-RPC (with jsonrpc field)"
        );
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Parameters:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Params Count:"),
            event.params.len()
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Params JSON:"),
            params_str
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Params Length:"),
            format!("{} characters", params_str.len())
        );
        // Log each param individually
        for (idx, param) in event.params.iter().enumerate() {
            let param_str = serde_json::to_string(param).unwrap_or_else(|_| "N/A".to_string());
            let param_type = if param.is_string() {
                "String".to_string()
            } else if param.is_number() {
                "Number".to_string()
            } else if param.is_array() {
                "Array".to_string()
            } else if param.is_object() {
                "Object".to_string()
            } else if param.is_boolean() {
                "Boolean".to_string()
            } else {
                "Null".to_string()
            };
            tracing::debug!(
                "{} {} {}",
                LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
                LogColors::label(&format!("  - Param[{}]:", idx)),
                format!("{} (type: {})", param_str, param_type)
            );
        }
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Message Data:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Raw JSON:"),
            json
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - JSON Length:"),
            format!("{} characters", json.len())
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Total Bytes (with newline):"),
            format!("{} bytes", data.len())
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Raw Bytes (hex):"),
            hex::encode(data.as_bytes())
        );
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("========================================")
        );

        self.write_data(data.as_bytes()).await?;
        Ok(())
    }

    /// Send a minimal Stratum notification (method + params only, no id or jsonrpc)
    /// This matches the format used by the stratum crate and expected by IceRiver ASICs
    pub async fn send_notification(
        &self,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<(), ErrorDisconnected> {
        if self.disconnecting.load(Ordering::Acquire) {
            return Err(ErrorDisconnected);
        }

        // Manually construct JSON without id or jsonrpc fields (matches StratumNotification format)
        let notification = serde_json::json!({
            "method": method,
            "params": params
        });

        let json = serde_json::to_string(&notification).map_err(|_| ErrorDisconnected)?;
        let data = format!("{}\n", json);

        // Get client context for detailed logging
        let (wallet_addr, worker_name, remote_app) = {
            let id = self.identity.lock();
            (
                id.wallet_addr.clone(),
                id.worker_name.clone(),
                id.remote_app.clone(),
            )
        };
        let params_str = serde_json::to_string(&params).unwrap_or_else(|_| "[]".to_string());

        // Log outgoing notification at DEBUG level (detailed logs moved to debug)
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("========================================")
        );
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("===== SENDING NOTIFICATION TO ASIC ===== ")
        );
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("========================================")
        );
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Client Information:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - IP Address:"),
            format!("{}:{}", self.remote_addr, self.remote_port)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Wallet Address:"),
            format!("'{}'", wallet_addr)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Worker Name:"),
            format!("'{}'", worker_name)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Miner Application:"),
            format!("'{}'", remote_app)
        );
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Notification Details:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Method:"),
            format!("'{}'", method)
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Format:"),
            "Minimal Stratum (no id/jsonrpc fields)"
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Target:"),
            "IceRiver/BzMiner compatible"
        );
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Parameters:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Params Count:"),
            params.len()
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Params JSON:"),
            params_str
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Params Length:"),
            format!("{} characters", params_str.len())
        );
        // Log each param individually
        for (idx, param) in params.iter().enumerate() {
            let param_str = serde_json::to_string(param).unwrap_or_else(|_| "N/A".to_string());
            let param_type = if param.is_string() {
                format!(
                    "String (length: {})",
                    param.as_str().map(|s| s.len()).unwrap_or(0)
                )
            } else if param.is_number() {
                "Number".to_string()
            } else if param.is_array() {
                format!(
                    "Array (length: {})",
                    param.as_array().map(|a| a.len()).unwrap_or(0)
                )
            } else if param.is_object() {
                "Object".to_string()
            } else if param.is_boolean() {
                "Boolean".to_string()
            } else {
                "Null".to_string()
            };
            tracing::debug!(
                "{} {} {}",
                LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
                LogColors::label(&format!("  - Param[{}]:", idx)),
                format!("{} (type: {})", param_str, param_type)
            );
        }
        tracing::debug!(
            "{} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("Message Data:")
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Raw JSON:"),
            json
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - JSON Length:"),
            format!("{} characters", json.len())
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Total Bytes (with newline):"),
            format!("{} bytes", data.len())
        );
        tracing::debug!(
            "{} {} {}",
            LogColors::bridge_to_asic("[BRIDGE->ASIC]"),
            LogColors::label("  - Raw Bytes (hex):"),
            hex::encode(data.as_bytes())
        );
        tracing::debug!(
            "{}",
            LogColors::bridge_to_asic("========================================")
        );

        self.write_data(data.as_bytes()).await?;
        Ok(())
    }

    /// Write data to the connection with backoff
    async fn write_data(&self, data: &[u8]) -> Result<(), ErrorDisconnected> {
        // Check if already disconnected
        if self.disconnecting.load(Ordering::Acquire) {
            return Err(ErrorDisconnected);
        }

        for attempt in 0..3 {
            if self
                .write_lock
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                // Extract write half (drop guard before await)
                let write_half_opt = {
                    let mut write_guard = self.write_half.lock();
                    write_guard.take()
                };

                let result = if let Some(mut write_half) = write_half_opt {
                    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);

                    // Try to write directly (no need to wait for writable)
                    let write_result =
                        tokio::time::timeout_at(deadline, write_half.write_all(data)).await;

                    // Put write half back regardless of result
                    {
                        let mut write_guard = self.write_half.lock();
                        *write_guard = Some(write_half);
                    }

                    write_result
                } else {
                    self.write_lock.store(false, Ordering::Release);
                    return Err(ErrorDisconnected);
                };

                self.write_lock.store(false, Ordering::Release);

                match result {
                    Ok(Ok(_)) => return Ok(()),
                    Ok(Err(e)) => {
                        tracing::warn!("Write error: {}", e);
                        self.check_disconnect();
                        return Err(ErrorDisconnected);
                    }
                    Err(_) => {
                        // Timeout on write - try again if we have attempts left
                        if attempt < 2 {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            continue;
                        } else {
                            self.check_disconnect();
                            return Err(ErrorDisconnected);
                        }
                    }
                }
            } else {
                // Write blocked - wait and retry
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        Err(ErrorDisconnected)
    }

    /// Reply with stale share error
    pub async fn reply_stale_share(&self, id: Option<Value>) -> Result<(), ErrorDisconnected> {
        tracing::debug!(
            "[BRIDGE->ASIC] Preparing STALE SHARE response (Error Code: 21, Job not found)"
        );
        self.reply(JsonRpcResponse::error(id, 21, "Job not found", None))
            .await
    }

    /// Reply with duplicate share error
    pub async fn reply_dupe_share(&self, id: Option<Value>) -> Result<(), ErrorDisconnected> {
        tracing::debug!(
            "[BRIDGE->ASIC] Preparing DUPLICATE SHARE response (Error Code: 22, Duplicate share submitted)"
        );
        self.reply(JsonRpcResponse::error(
            id,
            22,
            "Duplicate share submitted",
            None,
        ))
        .await
    }

    /// Reply with bad share error
    pub async fn reply_bad_share(&self, id: Option<Value>) -> Result<(), ErrorDisconnected> {
        tracing::debug!(
            "[BRIDGE->ASIC] Preparing BAD SHARE response (Error Code: 20, Unknown problem)"
        );
        self.reply(JsonRpcResponse::error(id, 20, "Unknown problem", None))
            .await
    }

    /// Reply with low difficulty share error
    pub async fn reply_low_diff_share(
        &self,
        id: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::debug!(
            "[BRIDGE->ASIC] Preparing LOW DIFFICULTY SHARE response (Error Code: 23, Invalid difficulty)"
        );
        self.reply(JsonRpcResponse::error(
            Some(id.clone()),
            23,
            "Invalid difficulty",
            None,
        ))
        .await
        .map_err(|e| {
            Box::new(std::io::Error::other(e.to_string()))
                as Box<dyn std::error::Error + Send + Sync>
        })
    }

    /// Send a response (async)
    #[allow(dead_code)]
    async fn send_response(&self, response: JsonRpcResponse) -> Result<(), ErrorDisconnected> {
        let json = serde_json::to_string(&response).map_err(|_| ErrorDisconnected)?;
        let data = format!("{}\n", json);
        self.write_data(data.as_bytes()).await
    }
}
