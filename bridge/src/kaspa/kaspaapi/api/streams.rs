//! gRPC new-block-template notification fan-out and polling ticker.

use super::KaspaApi;
use anyhow::Result;
use kaspa_rpc_core::Notification;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::log_colors::LogColors;

async fn block_until_synced_or_shutdown(
    api: Arc<KaspaApi>,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> bool {
    loop {
        if *shutdown_rx.borrow() {
            return false;
        }

        let ready_fut = api.is_node_synced_for_mining();
        let ready = tokio::select! {
            _ = shutdown_rx.wait_for(|v| *v) => {
                return false;
            }
            r = ready_fut => r,
        };

        if ready {
            return true;
        }
        warn!(
            "Kaspa is not synced (or P2P IBD still active), waiting for sync before starting bridge"
        );

        tokio::select! {
            _ = shutdown_rx.wait_for(|v| *v) => {
                return false;
            }
            _ = sleep(Duration::from_secs(10)) => {}
        }
    }
}

/// Start listening for block template notifications
/// Uses RegisterForNewBlockTemplateNotifications with ticker fallback
/// This provides immediate notifications when new blocks are available, with polling as fallback
///
/// **Sync safety:** templates are only dispatched while the node is mining-ready (same as
/// [`KaspaApi::is_node_synced_for_mining`](crate::kaspaapi::KaspaApi::is_node_synced_for_mining)). If sync is lost or P2P IBD resumes, we stop calling the callback.
pub(super) async fn start_block_template_listener<F>(
    api: Arc<KaspaApi>,
    block_wait_time: Duration,
    mut block_cb: F,
) -> Result<()>
where
    F: FnMut() + Send + 'static,
{
    let mut rx = api
        .notification_rx
        .lock()
        .take()
        .ok_or_else(|| anyhow::anyhow!("Notification receiver already taken"))?;

    let api_clone = Arc::clone(&api);
    tokio::spawn(async move {
        let mut log_sync_resume = true;

        'outer: loop {
            let _ = api_clone.wait_for_sync().await;

            if std::mem::take(&mut log_sync_resume) {
                info!(
                    "{} {}",
                    LogColors::api("[API]"),
                    LogColors::label(
                        "Node fully synced — distributing block templates to stratum miners"
                    )
                );
            }

            let mut ticker = tokio::time::interval(block_wait_time);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            'inner: loop {
                tokio::select! {
                    notification_result = rx.recv() => {
                        match notification_result {
                            Some(Notification::NewBlockTemplate(_)) => {
                                while rx.try_recv().is_ok() {}
                            }
                            Some(_) => continue,
                            None => {
                                warn!("Block template notification channel closed");
                                break 'outer;
                            }
                        }

                        if !api_clone.is_node_synced_for_mining().await {
                            warn!(
                                "{} {}",
                                LogColors::api("[API]"),
                                LogColors::label(
                                    "Node left fully-synced state; pausing stratum jobs until sync completes (IBD / catch-up)"
                                )
                            );
                            log_sync_resume = true;
                            break 'inner;
                        }

                        block_cb();
                        ticker = tokio::time::interval(block_wait_time);
                        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    }
                    _ = ticker.tick() => {
                        if !api_clone.is_node_synced_for_mining().await {
                            warn!(
                                "{} {}",
                                LogColors::api("[API]"),
                                LogColors::label(
                                    "Node left fully-synced state; pausing stratum jobs until sync completes (IBD / catch-up)"
                                )
                            );
                            log_sync_resume = true;
                            break 'inner;
                        }

                        block_cb();
                    }
                }
            }
        }
    });

    Ok(())
}

pub(super) async fn start_block_template_listener_with_shutdown<F>(
    api: Arc<KaspaApi>,
    block_wait_time: Duration,
    mut shutdown_rx: watch::Receiver<bool>,
    mut block_cb: F,
) -> Result<()>
where
    F: FnMut() + Send + 'static,
{
    let mut rx = api
        .notification_rx
        .lock()
        .take()
        .ok_or_else(|| anyhow::anyhow!("Notification receiver already taken"))?;

    let api_clone = Arc::clone(&api);
    tokio::spawn(async move {
        let mut log_sync_resume = true;

        'outer: loop {
            if !block_until_synced_or_shutdown(Arc::clone(&api_clone), &mut shutdown_rx).await {
                break;
            }

            if std::mem::take(&mut log_sync_resume) {
                info!(
                    "{} {}",
                    LogColors::api("[API]"),
                    LogColors::label(
                        "Node fully synced — distributing block templates to stratum miners"
                    )
                );
            }

            let mut ticker = tokio::time::interval(block_wait_time);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            'inner: loop {
                if *shutdown_rx.borrow() {
                    break 'outer;
                }

                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break 'outer;
                        }
                    }
                    notification_result = rx.recv() => {
                        match notification_result {
                            Some(Notification::NewBlockTemplate(_)) => {
                                while rx.try_recv().is_ok() {}
                            }
                            Some(_) => continue,
                            None => {
                                warn!("Block template notification channel closed");
                                break 'outer;
                            }
                        }

                        if *shutdown_rx.borrow() {
                            break 'outer;
                        }

                        if !api_clone.is_node_synced_for_mining().await {
                            warn!(
                                "{} {}",
                                LogColors::api("[API]"),
                                LogColors::label(
                                    "Node left fully-synced state; pausing stratum jobs until sync completes (IBD / catch-up)"
                                )
                            );
                            log_sync_resume = true;
                            break 'inner;
                        }

                        block_cb();
                        ticker = tokio::time::interval(block_wait_time);
                        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    }
                    _ = ticker.tick() => {
                        if *shutdown_rx.borrow() {
                            break 'outer;
                        }

                        if !api_clone.is_node_synced_for_mining().await {
                            warn!(
                                "{} {}",
                                LogColors::api("[API]"),
                                LogColors::label(
                                    "Node left fully-synced state; pausing stratum jobs until sync completes (IBD / catch-up)"
                                )
                            );
                            log_sync_resume = true;
                            break 'inner;
                        }

                        block_cb();
                    }
                }
            }
        }
    });

    Ok(())
}
