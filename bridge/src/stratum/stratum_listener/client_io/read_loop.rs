use crate::log_colors::LogColors;
use crate::stratum_context::StratumContext;
use crate::stratum_line_codec::{
    line_looks_like_http, push_lossy_and_drain_lines, strip_nul_bytes,
};
use hex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tracing::{debug, error, info, warn};

use super::super::types::EventHandler;

pub(crate) async fn spawn_client_listener(
    ctx: Arc<StratumContext>,
    handler_map: &Arc<HashMap<String, EventHandler>>,
) {
    debug!(
        "[CLIENT_LISTENER] Starting client listener for {}:{}",
        ctx.remote_addr, ctx.remote_port
    );
    let mut buffer = [0u8; 1024];
    let mut line_buffer = String::new();
    let mut first_message = true;

    loop {
        // Check if disconnected
        if !ctx.connected() {
            debug!(
                "[CLIENT_LISTENER] Client {}:{} disconnected",
                ctx.remote_addr, ctx.remote_port
            );
            break;
        }

        // Get read half for reading (must drop guard before await)
        let read_half_opt = {
            let mut read_guard = ctx.get_read_half();
            read_guard.take()
        };

        let read_result = if let Some(mut read_half) = read_half_opt {
            // Set read deadline
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

            let result = tokio::time::timeout_at(deadline, read_half.read(&mut buffer)).await;

            // Put read half back
            {
                let mut read_guard = ctx.get_read_half();
                *read_guard = Some(read_half);
            }

            result
        } else {
            // Read half is None, disconnect
            warn!(
                "[CONNECTION] Read half is None for {}, disconnecting",
                ctx.remote_addr
            );
            break;
        };

        match read_result {
            Ok(Ok(0)) => {
                // EOF - client closed connection
                let (worker_name, remote_app) = {
                    let id = ctx.identity.lock();
                    (id.worker_name.clone(), id.remote_app.clone())
                };
                let pending_buffer_bytes = line_buffer.len();
                let is_pre_handshake = worker_name.is_empty() && remote_app.is_empty();
                if is_pre_handshake && first_message && pending_buffer_bytes == 0 {
                    debug!(
                        "[CONNECTION] Client {}:{} closed connection (EOF) worker='{}' app='{}' first_message={} pending_buffer_bytes={}",
                        ctx.remote_addr,
                        ctx.remote_port,
                        worker_name,
                        remote_app,
                        first_message,
                        pending_buffer_bytes
                    );
                } else {
                    info!(
                        "[CONNECTION] Client {}:{} closed connection (EOF) worker='{}' app='{}' first_message={} pending_buffer_bytes={}",
                        ctx.remote_addr,
                        ctx.remote_port,
                        worker_name,
                        remote_app,
                        first_message,
                        pending_buffer_bytes
                    );
                }
                break;
            }
            Ok(Ok(n)) => {
                debug!(
                    "[CLIENT_LISTENER] Read {} bytes from {}:{}",
                    n, ctx.remote_addr, ctx.remote_port
                );

                // Remove null bytes and process
                let data: Vec<u8> = strip_nul_bytes(&buffer[..n]);

                if first_message {
                    let (wallet_addr, worker_name, remote_app) = {
                        let id = ctx.identity.lock();
                        (
                            id.wallet_addr.clone(),
                            id.worker_name.clone(),
                            id.remote_app.clone(),
                        )
                    };
                    let message_str = String::from_utf8_lossy(&data);

                    // Check for HTTP/2/gRPC protocol in first message (before logging)
                    let first_line = message_str.lines().next().unwrap_or("").trim();
                    if line_looks_like_http(first_line) {
                        error!(
                            "{}",
                            LogColors::error("========================================")
                        );
                        error!(
                            "{}",
                            LogColors::error(
                                "===== PROTOCOL MISMATCH DETECTED (FIRST MESSAGE) ===== "
                            )
                        );
                        error!(
                            "{}",
                            LogColors::error("========================================")
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("Client Information:")
                        );
                        error!(
                            "{} {} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("  - IP Address:"),
                            format!("{}:{}", ctx.remote_addr, ctx.remote_port)
                        );
                        error!(
                            "{} {} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("  - Protocol Detected:"),
                            "HTTP/2 or HTTP (gRPC)"
                        );
                        error!(
                            "{} {} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("  - Expected Protocol:"),
                            "Plain TCP/JSON-RPC (Stratum)"
                        );
                        error!(
                            "{} {} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("  - First Message (hex):"),
                            hex::encode(&data)
                        );
                        error!(
                            "{} {} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("  - First Message (string):"),
                            first_line
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("Action:")
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            "  * Rejecting connection - Stratum port only accepts JSON-RPC over plain TCP"
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            "  * HTTP/2/gRPC connections should use the Kaspa node port (16110), not the bridge port (5555)"
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            "  * Closing connection immediately"
                        );
                        error!(
                            "{}",
                            LogColors::error("========================================")
                        );

                        // Close connection
                        ctx.disconnect();
                        break;
                    }

                    debug!(
                        "{}",
                        LogColors::asic_to_bridge("========================================")
                    );
                    debug!(
                        "{}",
                        LogColors::asic_to_bridge("===== FIRST MESSAGE FROM ASIC ===== ")
                    );
                    debug!(
                        "{}",
                        LogColors::asic_to_bridge("========================================")
                    );
                    debug!(
                        "{} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("Connection Information:")
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - IP Address:"),
                        format!("{}:{}", ctx.remote_addr, ctx.remote_port)
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Wallet Address:"),
                        format!("'{}'", wallet_addr)
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Worker Name:"),
                        format!("'{}'", worker_name)
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Miner Application:"),
                        format!("'{}'", remote_app)
                    );
                    debug!(
                        "{} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("First Message Data:")
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Raw Bytes (hex):"),
                        hex::encode(&data)
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Raw Bytes Length:"),
                        format!("{} bytes", data.len())
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Message as String:"),
                        message_str
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - String Length:"),
                        format!("{} characters", message_str.len())
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - String Length:"),
                        format!("{} bytes (UTF-8)", message_str.len())
                    );
                    // Show byte-by-byte breakdown for first 100 bytes
                    if data.len() <= 100 {
                        debug!(
                            "{} {} {}",
                            LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                            LogColors::label("  - Byte Breakdown:"),
                            format!("{:?}", data)
                        );
                    } else {
                        debug!(
                            "{} {} {}",
                            LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                            LogColors::label("  - First 100 Bytes:"),
                            format!("{:?}", &data[..100.min(data.len())])
                        );
                    }
                    debug!(
                        "{}",
                        LogColors::asic_to_bridge("========================================")
                    );
                    first_message = false;
                }

                let chunk = String::from_utf8_lossy(&data);
                let drained = push_lossy_and_drain_lines(&mut line_buffer, &chunk);

                for line in drained {
                    // Get client context for detailed logging
                    let (wallet_addr, worker_name, remote_app) = {
                        let id = ctx.identity.lock();
                        (
                            id.wallet_addr.clone(),
                            id.worker_name.clone(),
                            id.remote_app.clone(),
                        )
                    };

                    // Detect HTTP/2/gRPC connections early and reject them
                    if line_looks_like_http(&line) {
                        error!(
                            "{}",
                            LogColors::error("========================================")
                        );
                        error!(
                            "{}",
                            LogColors::error("===== PROTOCOL MISMATCH DETECTED ===== ")
                        );
                        error!(
                            "{}",
                            LogColors::error("========================================")
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("Client Information:")
                        );
                        error!(
                            "{} {} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("  - IP Address:"),
                            format!("{}:{}", ctx.remote_addr, ctx.remote_port)
                        );
                        error!(
                            "{} {} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("  - Protocol Detected:"),
                            "HTTP/2 or HTTP (gRPC)"
                        );
                        error!(
                            "{} {} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("  - Expected Protocol:"),
                            "Plain TCP/JSON-RPC (Stratum)"
                        );
                        error!(
                            "{} {} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("  - Received Message:"),
                            &line
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            LogColors::label("Action:")
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            "  * Rejecting connection - Stratum port only accepts JSON-RPC over plain TCP"
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            "  * HTTP/2/gRPC connections should use the Kaspa node port (16110), not the bridge port (5555)"
                        );
                        error!(
                            "{} {}",
                            LogColors::error("[ERROR]"),
                            "  * Closing connection immediately"
                        );
                        error!(
                            "{}",
                            LogColors::error("========================================")
                        );

                        // Close connection
                        ctx.disconnect();
                        break;
                    }

                    // Log raw incoming message from ASIC at DEBUG level (verbose details)
                    debug!(
                        "{}",
                        LogColors::asic_to_bridge("========================================")
                    );
                    debug!(
                        "{}",
                        LogColors::asic_to_bridge("===== RECEIVED MESSAGE FROM ASIC ===== ")
                    );
                    debug!(
                        "{}",
                        LogColors::asic_to_bridge("========================================")
                    );
                    debug!(
                        "{} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("Client Information:")
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - IP Address:"),
                        format!("{}:{}", ctx.remote_addr, ctx.remote_port)
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Wallet Address:"),
                        format!("'{}'", wallet_addr)
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Worker Name:"),
                        format!("'{}'", worker_name)
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Miner Application:"),
                        format!("'{}'", remote_app)
                    );
                    debug!(
                        "{} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("Raw Message Data:")
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Raw Message:"),
                        line
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Message Length:"),
                        format!("{} bytes", line.len())
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Message Length:"),
                        format!("{} characters", line.chars().count())
                    );
                    debug!(
                        "{} {} {}",
                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                        LogColors::label("  - Raw Bytes (hex):"),
                        hex::encode(line.as_bytes())
                    );

                    match crate::jsonrpc_event::unmarshal_event(&line) {
                        Ok(event) => {
                            let params_str = serde_json::to_string(&event.params)
                                .unwrap_or_else(|_| "[]".to_string());

                            // Log parsed event details at DEBUG level (detailed logs moved to debug)
                            debug!(
                                "{}",
                                LogColors::asic_to_bridge("===== PARSING SUCCESSFUL ===== ")
                            );
                            debug!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("Parsed Event Structure:")
                            );
                            debug!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Method:"),
                                format!("'{}'", event.method)
                            );
                            debug!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Event ID:"),
                                format!("{:?}", event.id)
                            );
                            debug!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - JSON-RPC Version:"),
                                format!("'{}'", event.jsonrpc)
                            );
                            debug!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("Parameters:")
                            );
                            debug!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Params Count:"),
                                event.params.len()
                            );
                            debug!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Params JSON:"),
                                params_str
                            );
                            debug!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Params Length:"),
                                format!("{} characters", params_str.len())
                            );
                            // Log each param individually with type information
                            for (idx, param) in event.params.iter().enumerate() {
                                let param_str = serde_json::to_string(param)
                                    .unwrap_or_else(|_| "N/A".to_string());
                                let param_type = if param.is_string() {
                                    let s = param.as_str().unwrap_or("");
                                    format!("String (length: {}, value: '{}')", s.len(), s)
                                } else if param.is_number() {
                                    format!("Number (value: {})", param)
                                } else if let Some(arr) = param.as_array() {
                                    format!(
                                        "Array (length: {}, items: {:?})",
                                        arr.len(),
                                        arr.iter()
                                            .take(5)
                                            .map(|v| serde_json::to_string(v)
                                                .unwrap_or_else(|_| "?".to_string()))
                                            .collect::<Vec<_>>()
                                    )
                                } else if param.is_object() {
                                    "Object".to_string()
                                } else if param.is_boolean() {
                                    format!("Boolean (value: {})", param.as_bool().unwrap_or(false))
                                } else {
                                    "Null".to_string()
                                };
                                debug!(
                                    "{} {} {}",
                                    LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                    LogColors::label(&format!("  - Param[{}]:", idx)),
                                    format!("{} (type: {})", param_str, param_type)
                                );
                            }

                            if let Some(handler) = handler_map.get(&event.method) {
                                debug!(
                                    "{}",
                                    LogColors::asic_to_bridge("===== PROCESSING MESSAGE ===== ")
                                );
                                debug!(
                                    "{} {} {}",
                                    LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                    LogColors::label("  - Handler Found:"),
                                    "YES"
                                );
                                debug!(
                                    "{} {} {}",
                                    LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                    LogColors::label("  - Method:"),
                                    format!("'{}'", event.method)
                                );
                                debug!(
                                    "{} {}",
                                    LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                    "  - Starting handler execution..."
                                );
                                if let Err(e) = handler(ctx.clone(), event).await {
                                    let error_msg = e.to_string();
                                    if error_msg.contains("stale")
                                        || error_msg.contains("job does not exist")
                                    {
                                        // Log stale job errors as debug (expected behavior, not important)
                                        debug!(
                                            "{}",
                                            LogColors::asic_to_bridge(
                                                "===== HANDLER EXECUTION RESULT ===== "
                                            )
                                        );
                                        debug!(
                                            "{} {} {}",
                                            LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                            LogColors::validation("  - Result:"),
                                            "STALE JOB (expected - job no longer exists)"
                                        );
                                        debug!(
                                            "{} {} {}",
                                            LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                            LogColors::label("  - Error Message:"),
                                            error_msg
                                        );
                                    } else if error_msg.contains("job id is not parsable") {
                                        // Log parsing errors as warnings
                                        warn!(
                                            "{} {} {}",
                                            LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                            LogColors::error("  - Result:"),
                                            "ERROR (job ID parsing failed)"
                                        );
                                        warn!(
                                            "{} {} {}",
                                            LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                            LogColors::label("  - Error Message:"),
                                            error_msg
                                        );
                                    } else {
                                        error!(
                                            "{} {} {}",
                                            LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                            LogColors::error("  - Result:"),
                                            "ERROR (handler execution failed)"
                                        );
                                        error!(
                                            "{} {} {}",
                                            LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                            LogColors::label("  - Error Message:"),
                                            error_msg
                                        );
                                    }
                                } else {
                                    debug!(
                                        "{}",
                                        LogColors::asic_to_bridge(
                                            "===== HANDLER EXECUTION RESULT ===== "
                                        )
                                    );
                                    debug!(
                                        "{} {} {}",
                                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                        LogColors::label("  - Result:"),
                                        "SUCCESS"
                                    );
                                    debug!(
                                        "{} {}",
                                        LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                        "  - Message processed successfully"
                                    );
                                }
                                debug!(
                                    "{}",
                                    LogColors::asic_to_bridge(
                                        "========================================"
                                    )
                                );
                            }
                        }
                        Err(e) => {
                            error!(
                                "{}",
                                LogColors::asic_to_bridge(
                                    "========================================"
                                )
                            );
                            error!("{}", LogColors::error("===== ERROR PARSING MESSAGE ===== "));
                            error!(
                                "{}",
                                LogColors::asic_to_bridge(
                                    "========================================"
                                )
                            );
                            error!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("Client Information:")
                            );
                            error!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - IP Address:"),
                                format!("{}:{}", ctx.remote_addr, ctx.remote_port)
                            );
                            error!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Wallet Address:"),
                                format!("'{}'", wallet_addr)
                            );
                            error!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Worker Name:"),
                                format!("'{}'", worker_name)
                            );
                            error!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Miner Application:"),
                                format!("'{}'", remote_app)
                            );
                            error!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("Failed Message:")
                            );
                            error!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Raw Message:"),
                                line
                            );
                            error!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Message Length:"),
                                format!("{} bytes", line.len())
                            );
                            error!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Raw Bytes (hex):"),
                                hex::encode(line.as_bytes())
                            );
                            error!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("Parse Error Details:")
                            );
                            error!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Error Type:"),
                                "JSON Parsing Failed"
                            );
                            error!(
                                "{} {} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::error("  - Error Message:"),
                                e
                            );
                            error!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                LogColors::label("  - Possible Causes:")
                            );
                            error!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                "    * Malformed JSON syntax"
                            );
                            error!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                "    * Protocol mismatch"
                            );
                            error!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                "    * Incomplete message"
                            );
                            error!(
                                "{} {}",
                                LogColors::asic_to_bridge("[ASIC->BRIDGE]"),
                                "    * Encoding issue"
                            );
                            error!(
                                "{}",
                                LogColors::asic_to_bridge(
                                    "========================================"
                                )
                            );
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                // Check if it's a connection closed error (expected when client disconnects)
                let error_msg = e.to_string();
                if error_msg.contains("forcibly closed")
                    || error_msg.contains("Connection reset")
                    || error_msg.contains("Broken pipe")
                    || e.kind() == std::io::ErrorKind::ConnectionReset
                    || e.kind() == std::io::ErrorKind::BrokenPipe
                {
                    let (worker_name, remote_app) = {
                        let id = ctx.identity.lock();
                        (id.worker_name.clone(), id.remote_app.clone())
                    };
                    let is_pre_handshake = worker_name.is_empty() && remote_app.is_empty();
                    if is_pre_handshake {
                        debug!(
                            "[CONNECTION] Client {}:{} disconnected (reset/broken pipe) kind={:?} worker='{}' app='{}' msg='{}'",
                            ctx.remote_addr,
                            ctx.remote_port,
                            e.kind(),
                            worker_name,
                            remote_app,
                            error_msg
                        );
                    } else {
                        info!(
                            "[CONNECTION] Client {}:{} disconnected (reset/broken pipe) kind={:?} worker='{}' app='{}' msg='{}'",
                            ctx.remote_addr,
                            ctx.remote_port,
                            e.kind(),
                            worker_name,
                            remote_app,
                            error_msg
                        );
                    }
                } else {
                    error!("error reading from socket: {}", e);
                }
                break;
            }
            Err(_) => {
                // Timeout - continue
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                continue;
            }
        }
    }

    ctx.disconnect();
}
