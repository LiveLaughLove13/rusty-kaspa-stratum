use crate::log_colors::LogColors;
use crate::share_handler::KaspaApiTrait;
use anyhow::Result;
use kaspa_consensus_core::block::Block;
use kaspa_grpc_client::GrpcClient;
use kaspa_notify::{listener::ListenerId, scope::NewBlockTemplateScope};
use kaspa_rpc_core::notify::mode::NotificationMode;
use kaspa_rpc_core::{
    GetBlockDagInfoRequest, GetConnectedPeerInfoRequest, GetInfoRequest, GetServerInfoRequest, GetSinkBlueScoreRequest, Notification,
    api::rpc::RpcApi,
};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use super::coinbase_tag::build_coinbase_tag_bytes;
use super::node_status::NODE_STATUS;

mod block_submit_guard;
mod streams;
mod template_submit;

const MIN_MINING_READY_STABLE: Duration = Duration::from_secs(2);
const MINING_READY_STABLE_POLL: Duration = Duration::from_millis(400);

/// Kaspa API client wrapper using RPC client
/// Both use gRPC under the hood, but through an RPC client wrapper abstraction
pub struct KaspaApi {
    pub(crate) client: Arc<GrpcClient>,
    pub(crate) notification_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<Notification>>>>,
    pub(crate) connected: Arc<Mutex<bool>>,
    pub(crate) coinbase_tag: Vec<u8>,
}

impl KaspaApi {
    /// Create a new Kaspa API client
    pub async fn new(
        address: String,
        coinbase_tag_suffix: Option<String>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<Arc<Self>> {
        info!("Connecting to Kaspa node at {}", address);

        // GrpcClient requires explicit "grpc://" prefix for connection
        // Always add it if not present (avoids unnecessary connection failure)
        let grpc_address = if address.starts_with("grpc://") { address.clone() } else { format!("grpc://{}", address) };

        // Log connection attempt (detailed logs moved to debug)
        debug!("{} {}", LogColors::api("[API]"), LogColors::label("Establishing RPC connection to Kaspa node:"));
        debug!("{} {} {}", LogColors::api("[API]"), LogColors::label("  - Address:"), &grpc_address);
        debug!("{} {} {}", LogColors::api("[API]"), LogColors::label("  - Protocol:"), "gRPC (via RPC client wrapper)");

        let mut attempt: u64 = 0;
        let mut backoff_ms: u64 = 250;

        let client = loop {
            attempt += 1;
            let connect_fut = GrpcClient::connect_with_args(
                NotificationMode::Direct,
                grpc_address.clone(),
                None,
                true,
                None,
                false,
                Some(500_000),
                Default::default(),
            );

            let res = tokio::select! {
                _ = shutdown_rx.wait_for(|v| *v) => {
                    return Err(anyhow::anyhow!("shutdown requested"));
                }
                res = connect_fut => res,
            };

            match res {
                Ok(client) => break Arc::new(client),
                Err(e) => {
                    let backoff = Duration::from_millis(backoff_ms);
                    warn!(
                        "failed to connect to kaspa node at {} (attempt {}): {}, retrying in {:.2}s",
                        grpc_address,
                        attempt,
                        e,
                        backoff.as_secs_f64()
                    );

                    tokio::select! {
                        _ = shutdown_rx.wait_for(|v| *v) => {
                            return Err(anyhow::anyhow!("shutdown requested"));
                        }
                        _ = sleep(backoff) => {}
                    }

                    backoff_ms = (backoff_ms.saturating_mul(2)).min(5_000);
                }
            }
        };

        // Log successful connection (detailed logs moved to debug)
        debug!("{} {}", LogColors::api("[API]"), LogColors::block("RPC Connection Established Successfully"));
        debug!("{} {} {}", LogColors::api("[API]"), LogColors::label("  - Connected to:"), &grpc_address);
        debug!("{} {} {}", LogColors::api("[API]"), LogColors::label("  - Connection Type:"), "gRPC (via RPC client wrapper)");

        // Start the client (no notify needed for Direct mode)
        client.start(None).await;

        // Subscribe to block template notifications
        // Some nodes may take time to accept notification subscriptions; retry until it succeeds.
        // This retry logic with exponential backoff handles transient failures where nodes are not
        // immediately ready to accept subscriptions after connection, preventing tight-looping and log spam.
        let mut attempt: u64 = 0;
        let mut backoff_ms: u64 = 250;
        loop {
            attempt += 1;
            let notify_fut = client.start_notify(ListenerId::default(), NewBlockTemplateScope {}.into());

            let res = tokio::select! {
                _ = shutdown_rx.wait_for(|v| *v) => {
                    return Err(anyhow::anyhow!("shutdown requested"));
                }
                res = notify_fut => res,
            };

            match res {
                Ok(_) => break,
                Err(e) => {
                    let backoff = Duration::from_millis(backoff_ms);
                    warn!(
                        "failed to subscribe to block template notifications (attempt {}): {}, retrying in {:.2}s",
                        attempt,
                        e,
                        backoff.as_secs_f64()
                    );

                    tokio::select! {
                        _ = shutdown_rx.wait_for(|v| *v) => {
                            return Err(anyhow::anyhow!("shutdown requested"));
                        }
                        _ = sleep(backoff) => {}
                    }
                    backoff_ms = (backoff_ms.saturating_mul(2)).min(5_000);
                }
            }
        }

        // Start receiving notifications
        let notification_rx = {
            let receiver = client.notification_channel_receiver();
            // Convert async_channel::Receiver to tokio::sync::mpsc::UnboundedReceiver
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let receiver_clone = receiver.clone();
            tokio::spawn(async move {
                while let Ok(notification) = receiver_clone.recv().await {
                    let _ = tx.send(notification);
                }
            });
            Arc::new(Mutex::new(Some(rx)))
        };

        let coinbase_tag = build_coinbase_tag_bytes(coinbase_tag_suffix.as_deref());
        let api = Arc::new(Self { client, notification_rx, connected: Arc::new(Mutex::new(true)), coinbase_tag });

        // Start network stats thread
        let api_clone = Arc::clone(&api);
        tokio::spawn(async move {
            api_clone.start_stats_thread().await;
        });

        // Start node status polling thread (for console status display)
        let api_clone = Arc::clone(&api);
        tokio::spawn(async move {
            api_clone.start_node_status_thread().await;
        });

        Ok(api)
    }

    /// Start network stats thread
    /// Fetches network stats every 30 seconds and records them in Prometheus
    async fn start_stats_thread(self: Arc<Self>) {
        use crate::prom::record_network_stats;
        use kaspa_rpc_core::{EstimateNetworkHashesPerSecondRequest, GetBlockDagInfoRequest};

        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;

            // Get block DAG info
            // GetBlockDagInfoRequest is a unit struct, construct directly
            let dag_response = match self.client.get_block_dag_info_call(None, GetBlockDagInfoRequest {}).await {
                Ok(r) => r,
                Err(e) => {
                    warn!("failed to get network hashrate from kaspa, prom stats will be out of date: {}", e);
                    continue;
                }
            };

            // Get tip hash (first one)
            // tip_hashes is Vec<Hash> in the response (already parsed)
            let tip_hash = match dag_response.tip_hashes.first() {
                Some(hash) => Some(*hash), // Clone the Hash
                None => {
                    warn!("no tip hashes available for network hashrate estimation");
                    continue;
                }
            };

            // Estimate network hashes per second
            // new(window_size: u32, start_hash: Option<RpcHash>)
            // RpcHash is the same as Hash, so we can use tip_hash directly
            let hashrate_response = match self
                .client
                .estimate_network_hashes_per_second_call(None, EstimateNetworkHashesPerSecondRequest::new(1000, tip_hash))
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!("failed to get network hashrate from kaspa, prom stats will be out of date: {}", e);
                    continue;
                }
            };

            // Record network stats
            record_network_stats(hashrate_response.network_hashes_per_second, dag_response.block_count, dag_response.difficulty);
        }
    }

    /// One RPC round-trip to refresh [`NODE_STATUS`] (console `[NODE]` line and `/api/status`).
    /// The background poller runs every 10s; call this when mining-ready flips so the snapshot
    /// matches [`is_node_synced_for_mining`] instead of lagging by up to one interval.
    async fn refresh_node_status_snapshot(&self) {
        let connected = self.client.is_connected();

        let server_info_fut = self.client.get_server_info_call(None, GetServerInfoRequest {});
        let dag_info_fut = self.client.get_block_dag_info_call(None, GetBlockDagInfoRequest {});
        let peers_fut = self.client.get_connected_peer_info_call(None, GetConnectedPeerInfoRequest {});
        let info_fut = self.client.get_info_call(None, GetInfoRequest {});
        let sink_bs_fut = self.client.get_sink_blue_score_call(None, GetSinkBlueScoreRequest {});
        let sync_fut = self.client.get_sync_status();

        let (server_info, dag_info, peers_info, info_resp, sink_bs_resp, sync_res) =
            tokio::join!(server_info_fut, dag_info_fut, peers_fut, info_fut, sink_bs_fut, sync_fut);

        let mut snapshot = NODE_STATUS.lock();
        snapshot.last_updated = Some(Instant::now());
        snapshot.last_updated_unix_ms = SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_millis() as u64);
        snapshot.is_connected = connected;

        // Prefer `getSyncStatus` over `getServerInfo.is_synced`; clear "synced" while any peer is
        // the P2P IBD peer, or while DAG bodies lag headers (`block_count != header_count`).
        let mut synced = match sync_res {
            Ok(v) => Some(v),
            Err(_) => server_info.as_ref().ok().map(|s| s.is_synced),
        };
        if let Ok(peers) = &peers_info
            && synced == Some(true)
            && peers.peer_info.iter().any(|p| p.is_ibd_peer)
        {
            synced = Some(false);
        }
        if let Ok(dag) = &dag_info
            && synced == Some(true)
            && dag.block_count != dag.header_count
        {
            synced = Some(false);
        }
        snapshot.is_synced = synced;

        if let Ok(server_info) = server_info {
            snapshot.network_id = Some(format!("{:?}", server_info.network_id));
            snapshot.server_version = Some(server_info.server_version);
            snapshot.virtual_daa_score = Some(server_info.virtual_daa_score);
        }

        if let Ok(dag) = dag_info {
            snapshot.block_count = Some(dag.block_count);
            snapshot.header_count = Some(dag.header_count);
            snapshot.difficulty = Some(dag.difficulty);
            snapshot.tip_hash = dag.tip_hashes.first().map(|h| format!("{}", h));
            if snapshot.virtual_daa_score.is_none() {
                snapshot.virtual_daa_score = Some(dag.virtual_daa_score);
            }
            if snapshot.network_id.is_none() {
                snapshot.network_id = Some(format!("{:?}", dag.network));
            }
        }

        if let Ok(peers) = peers_info {
            snapshot.peers = Some(peers.peer_info.len());
        }

        if let Ok(info) = info_resp {
            snapshot.mempool_size = Some(info.mempool_size);
            if snapshot.server_version.is_none() {
                snapshot.server_version = Some(info.server_version);
            }
        }

        snapshot.sink_blue_score = sink_bs_resp.ok().map(|r| r.blue_score);
    }

    async fn start_node_status_thread(self: Arc<Self>) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            self.refresh_node_status_snapshot().await;
        }
    }

    /// Mining-safe sync: node's `getSyncStatus` (sink recent + not in transitional IBD), no active
    /// P2P IBD peer (`getConnectedPeerInfo`: `is_ibd_peer`), and `getBlockDagInfo` **block/header
    /// parity** (`block_count == header_count`). Headers can run ahead of bodies during catch-up; the
    /// dashboard `blk=a/b` line reflects the same counts.
    pub async fn is_node_synced_for_mining(&self) -> bool {
        if !self.client.get_sync_status().await.unwrap_or(false) {
            return false;
        }

        let peers_fut = self.client.get_connected_peer_info_call(None, GetConnectedPeerInfoRequest {});
        let dag_fut = self.client.get_block_dag_info_call(None, GetBlockDagInfoRequest {});
        let (peers_res, dag_res) = tokio::join!(peers_fut, dag_fut);

        let ibd_peer_active = match &peers_res {
            Ok(resp) => resp.peer_info.iter().any(|p| p.is_ibd_peer),
            Err(e) => {
                debug!("getConnectedPeerInfo failed while checking P2P IBD; ignoring IBD-peer gate: {}", e);
                false
            }
        };
        if ibd_peer_active {
            return false;
        }

        match &dag_res {
            Ok(dag) => dag.block_count == dag.header_count,
            Err(e) => {
                debug!("getBlockDagInfo failed while checking block/header parity; not mining-ready: {}", e);
                false
            }
        }
    }

    /// Wait until [`is_node_synced_for_mining`] stays true for [`MIN_MINING_READY_STABLE`]. If
    /// `shutdown_rx` is set, returns `false` when shutdown is requested; otherwise only returns `true`.
    async fn wait_until_mining_ready_stable(&self, mut shutdown_rx: Option<&mut watch::Receiver<bool>>) -> bool {
        let mut stable_since: Option<Instant> = None;
        // So the first "not synced" path can warn without waiting 10s from process start.
        let mut last_slow_warn = Instant::now() - Duration::from_secs(30);

        loop {
            if let Some(rx) = shutdown_rx.as_mut()
                && *rx.borrow()
            {
                return false;
            }

            let ready_fut = self.is_node_synced_for_mining();
            let ready = match shutdown_rx.as_mut() {
                Some(rx) => {
                    tokio::select! {
                        _ = rx.wait_for(|v| *v) => return false,
                        r = ready_fut => r,
                    }
                }
                None => ready_fut.await,
            };

            let now = Instant::now();
            if ready {
                match stable_since {
                    None => stable_since = Some(now),
                    Some(t0) if now.duration_since(t0) >= MIN_MINING_READY_STABLE => {
                        self.refresh_node_status_snapshot().await;
                        return true;
                    }
                    Some(_) => {}
                }
            } else {
                if stable_since.take().is_some() {
                    warn!(
                        "{} {}",
                        LogColors::api("[API]"),
                        LogColors::label(
                            "Mining-ready dropped before stability window elapsed; continuing to wait (avoids opening stratum right before P2P IBD)"
                        )
                    );
                }
                if now.duration_since(last_slow_warn) >= Duration::from_secs(10) {
                    warn!("Kaspa is not synced (or P2P IBD still active), waiting before starting bridge");
                    last_slow_warn = now;
                }
            }

            match shutdown_rx.as_mut() {
                Some(rx) => {
                    tokio::select! {
                        _ = rx.wait_for(|v| *v) => return false,
                        _ = sleep(MINING_READY_STABLE_POLL) => {}
                    }
                }
                None => sleep(MINING_READY_STABLE_POLL).await,
            }
        }
    }

    /// Block until the node reports fully synced. Logs at WARN on each wait cycle (same message as startup).
    async fn wait_for_sync(&self) -> Result<()> {
        self.wait_until_mining_ready_stable(None).await;
        Ok(())
    }

    pub async fn wait_for_sync_with_shutdown(&self, mut shutdown_rx: watch::Receiver<bool>) -> Result<()> {
        debug!("checking kaspad sync state");
        if !self.wait_until_mining_ready_stable(Some(&mut shutdown_rx)).await {
            return Err(anyhow::anyhow!("shutdown requested"));
        }
        debug!("kaspad mining-ready (stable window passed), starting stratum");
        Ok(())
    }

    /// Block template notifications plus ticker fallback (implementation in `streams` submodule).
    pub async fn start_block_template_listener<F>(self: Arc<Self>, block_wait_time: Duration, block_cb: F) -> Result<()>
    where
        F: FnMut() + Send + 'static,
    {
        streams::start_block_template_listener(self, block_wait_time, block_cb).await
    }

    /// Like [`Self::start_block_template_listener`] but respects shutdown on the given watch channel.
    pub async fn start_block_template_listener_with_shutdown<F>(
        self: Arc<Self>,
        block_wait_time: Duration,
        shutdown_rx: watch::Receiver<bool>,
        block_cb: F,
    ) -> Result<()>
    where
        F: FnMut() + Send + 'static,
    {
        streams::start_block_template_listener_with_shutdown(self, block_wait_time, shutdown_rx, block_cb).await
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        *self.connected.lock()
    }
}

#[async_trait::async_trait]
impl KaspaApiTrait for KaspaApi {
    async fn get_block_template(&self, wallet_addr: &str, _remote_app: &str, _canxium_addr: &str) -> Result<Block> {
        KaspaApi::get_block_template(self, wallet_addr, "", "").await
    }

    async fn submit_block(&self, block: Block) -> Result<kaspa_rpc_core::SubmitBlockResponse> {
        KaspaApi::submit_block(self, block).await
    }

    async fn get_balances_by_addresses(&self, addresses: &[String]) -> Result<Vec<(String, u64)>> {
        KaspaApi::get_balances_by_addresses(self, addresses).await
    }

    async fn get_current_block_color(&self, block_hash: &str) -> Result<bool> {
        KaspaApi::get_current_block_color(self, block_hash).await
    }

    async fn is_node_synced_for_mining(&self) -> bool {
        KaspaApi::is_node_synced_for_mining(self).await
    }
}
