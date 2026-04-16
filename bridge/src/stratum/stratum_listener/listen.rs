use crate::net_utils::bind_addr_from_port;
use crate::stratum_context::StratumContext;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tracing::{debug, error, info};

use super::client_io::spawn_client_listener;
use super::types::StratumListenerConfig;

pub(crate) async fn listen_impl(
    config: &StratumListenerConfig,
    stats: &Arc<parking_lot::Mutex<super::types::StratumStats>>,
    shutting_down: &Arc<std::sync::atomic::AtomicBool>,
    mut shutdown_rx: Option<watch::Receiver<bool>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    shutting_down.store(false, std::sync::atomic::Ordering::Release);

    // Ensure we bind to IPv4 (0.0.0.0) when given a bare port like ":5555" / "5555".
    let addr_str = bind_addr_from_port(&config.port);

    let listener = TcpListener::bind(&addr_str).await.map_err(|e| format!("failed listening to socket {}: {}", config.port, e))?;

    debug!("Stratum listener started on {}", config.port);

    let (disconnect_tx, mut disconnect_rx) = mpsc::unbounded_channel::<Arc<StratumContext>>();
    let disconnect_tx_clone = disconnect_tx.clone();
    let on_disconnect = Arc::clone(&config.on_disconnect);
    let disconnect_stats = stats.clone();

    let mut disconnect_shutdown_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        loop {
            if let Some(ref mut rx) = disconnect_shutdown_rx {
                tokio::select! {
                    _ = rx.changed() => {
                        if *rx.borrow() {
                            break;
                        }
                    }
                    maybe_ctx = disconnect_rx.recv() => {
                        let Some(ctx) = maybe_ctx else {
                            break;
                        };
                        info!("[CONNECTION] client disconnecting - {}", ctx.remote_addr);
                        info!("[CONNECTION] Disconnect event for {}:{}", ctx.remote_addr, ctx.remote_port);
                        disconnect_stats.lock().disconnects += 1;
                        on_disconnect(ctx);
                    }
                }
            } else {
                let Some(ctx) = disconnect_rx.recv().await else {
                    break;
                };
                info!("[CONNECTION] client disconnecting - {}", ctx.remote_addr);
                info!("[CONNECTION] Disconnect event for {}:{}", ctx.remote_addr, ctx.remote_port);
                disconnect_stats.lock().disconnects += 1;
                on_disconnect(ctx);
            }
        }
    });

    loop {
        if let Some(ref mut rx) = shutdown_rx {
            tokio::select! {
                _ = rx.changed() => {
                    if *rx.borrow() {
                        shutting_down.store(true, std::sync::atomic::Ordering::Release);
                        break;
                    }
                }
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                        let remote_addr = addr.ip().to_string();
                        let remote_port = addr.port();

                        debug!("[CONNECTION] new client connecting - {}:{}", remote_addr, remote_port);
                        debug!("[CONNECTION] ===== TCP CONNECTION ESTABLISHED =====");
                        debug!("[CONNECTION] Remote address: {}:{}", remote_addr, remote_port);
                        debug!("[CONNECTION] Local address: {:?}", stream.local_addr());
                        debug!("[CONNECTION] Connection accepted successfully");

                        // Create new MiningState for each client
                        // Each client gets its own isolated state, just like in Go
                        use crate::mining_state::MiningState;
                        let state = Arc::new(MiningState::new());

                        // Clone for logging after move
                        let remote_addr_for_log = remote_addr.clone();
                        let remote_port_for_log = remote_port;

                        debug!("[CONNECTION] Creating StratumContext for {}:{}", remote_addr_for_log, remote_port_for_log);
                        let ctx = StratumContext::new(
                            remote_addr,
                            remote_port,
                            stream,
                            state,
                            disconnect_tx_clone.clone(),
                        );
                        debug!("[CONNECTION] StratumContext created successfully");

                        debug!("[CONNECTION] Calling on_connect handler");
                        (config.on_connect)(ctx.clone());
                        debug!("[CONNECTION] on_connect handler completed");

                        // Spawn client handler
                        debug!("[CONNECTION] Spawning client listener task for {}:{}", remote_addr_for_log, remote_port_for_log);
                        let ctx_clone = ctx.clone();
                        let handler_map = config.handler_map.clone();
                        tokio::spawn(async move {
                            debug!("[CONNECTION] Client listener task started for {}:{}", ctx_clone.remote_addr, ctx_clone.remote_port);
                            spawn_client_listener(ctx_clone, &handler_map).await;
                            debug!("[CONNECTION] Client listener task ended");
                        });
                        debug!("[CONNECTION] ===== CONNECTION SETUP COMPLETE FOR {}:{} =====", remote_addr_for_log, remote_port_for_log);
                    }
                        Err(e) => {
                        if shutting_down.load(std::sync::atomic::Ordering::Acquire) {
                            info!("stopping listening due to server shutdown");
                            break;
                        }
                        error!("[CONNECTION] ===== FAILED TO ACCEPT INCOMING CONNECTION =====");
                        error!("[CONNECTION] Error: {}", e);
                        error!("[CONNECTION] Error kind: {:?}", e.kind());
                        error!("[CONNECTION] Failed to accept connection: {} (kind: {:?})", e, e.kind());
                        }
                    }
                }
            }
        } else {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let remote_addr = addr.ip().to_string();
                    let remote_port = addr.port();

                    debug!("[CONNECTION] new client connecting - {}:{}", remote_addr, remote_port);
                    debug!("[CONNECTION] ===== TCP CONNECTION ESTABLISHED =====");
                    debug!("[CONNECTION] Remote address: {}:{}", remote_addr, remote_port);
                    debug!("[CONNECTION] Local address: {:?}", stream.local_addr());
                    debug!("[CONNECTION] Connection accepted successfully");

                    use crate::mining_state::MiningState;
                    let state = Arc::new(MiningState::new());

                    let remote_addr_for_log = remote_addr.clone();
                    let remote_port_for_log = remote_port;

                    debug!("[CONNECTION] Creating StratumContext for {}:{}", remote_addr_for_log, remote_port_for_log);
                    let ctx = StratumContext::new(remote_addr, remote_port, stream, state, disconnect_tx_clone.clone());
                    debug!("[CONNECTION] StratumContext created successfully");

                    debug!("[CONNECTION] Calling on_connect handler");
                    (config.on_connect)(ctx.clone());
                    debug!("[CONNECTION] on_connect handler completed");

                    debug!("[CONNECTION] Spawning client listener task for {}:{}", remote_addr_for_log, remote_port_for_log);
                    let ctx_clone = ctx.clone();
                    let handler_map = config.handler_map.clone();
                    tokio::spawn(async move {
                        debug!("[CONNECTION] Client listener task started for {}:{}", ctx_clone.remote_addr, ctx_clone.remote_port);
                        spawn_client_listener(ctx_clone, &handler_map).await;
                        debug!("[CONNECTION] Client listener task ended");
                    });
                    debug!("[CONNECTION] ===== CONNECTION SETUP COMPLETE FOR {}:{} =====", remote_addr_for_log, remote_port_for_log);
                }
                Err(e) => {
                    if shutting_down.load(std::sync::atomic::Ordering::Acquire) {
                        info!("stopping listening due to server shutdown");
                        break;
                    }
                    error!("[CONNECTION] ===== FAILED TO ACCEPT INCOMING CONNECTION =====");
                    error!("[CONNECTION] Error: {}", e);
                    error!("[CONNECTION] Error kind: {:?}", e.kind());
                    error!("[CONNECTION] Failed to accept connection: {} (kind: {:?})", e, e.kind());
                }
            }
        }
    }

    Ok(())
}
