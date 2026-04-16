use crate::{
    prom::*,
    share_handler::{KaspaApiTrait, ShareHandler},
    stratum_context::StratumContext,
};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

mod handshake;
mod job_dispatch;

pub struct ClientHandler {
    clients: Arc<Mutex<HashMap<i32, Arc<StratumContext>>>>,
    client_counter: AtomicI32,
    min_share_diff: f64,
    _extranonce_size: i8, // Kept for backward compatibility, but now auto-detected per client
    _max_extranonce: i32, // Kept for backward compatibility
    last_template_time: Arc<Mutex<Instant>>,
    last_balance_check: Arc<Mutex<Instant>>,
    share_handler: Arc<ShareHandler>,
    instance_id: String, // Instance identifier for logging
}

impl ClientHandler {
    pub fn new(
        share_handler: Arc<ShareHandler>,
        min_share_diff: f64,
        extranonce_size: i8,
        instance_id: String,
    ) -> Self {
        let max_extranonce = if extranonce_size > 0 {
            (2_f64.powi(8 * extranonce_size.min(3) as i32) - 1.0) as i32
        } else {
            0
        };

        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            client_counter: AtomicI32::new(0),
            min_share_diff,
            _extranonce_size: extranonce_size,
            _max_extranonce: max_extranonce,
            last_template_time: Arc::new(Mutex::new(Instant::now())),
            last_balance_check: Arc::new(Mutex::new(Instant::now())),
            share_handler,
            instance_id,
        }
    }

    pub fn on_connect(&self, ctx: Arc<StratumContext>) {
        let idx = self.client_counter.fetch_add(1, Ordering::Relaxed);

        // Don't assign extranonce here - will be assigned in handle_subscribe based on detected miner type
        // Leave extranonce empty initially
        *ctx.extranonce.lock() = String::new();

        ctx.set_id(idx);
        self.clients.lock().insert(idx, Arc::clone(&ctx));

        debug!(
            "{} [CONNECTION] Client {} connected (ID: {}), extranonce will be assigned after miner type detection",
            self.instance_id, ctx.remote_addr, idx
        );

        // Create stats after 5 seconds (give time for authorize)
        let share_handler = Arc::clone(&self.share_handler);
        let ctx_clone = Arc::clone(&ctx);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if !ctx_clone.identity.lock().worker_name.is_empty() {
                share_handler.get_create_stats(&ctx_clone);
            }
        });
    }

    /// Assign extranonce to a client based on detected miner type
    /// Called from handle_subscribe after miner type is detected
    pub fn assign_extranonce_for_miner(&self, ctx: &StratumContext, remote_app: &str) {
        handshake::assign_extranonce_for_miner(ctx, remote_app);
    }

    pub fn on_disconnect(&self, ctx: &StratumContext) {
        ctx.disconnect();
        let mut clients = self.clients.lock();
        if let Some(id) = ctx.id() {
            debug!("removing client {}", id);
            clients.remove(&id);
            debug!("removed client {}", id);
        }
        let (wallet_addr, worker_name, remote_app) = {
            let id = ctx.identity.lock();
            (
                id.wallet_addr.clone(),
                id.worker_name.clone(),
                id.remote_app.clone(),
            )
        };

        let is_unauthed = wallet_addr.is_empty() && worker_name.is_empty();
        if !is_unauthed {
            record_disconnect(&crate::prom::WorkerContext {
                instance_id: self.instance_id.clone(),
                worker_name: worker_name.clone(),
                miner: remote_app,
                wallet: wallet_addr.clone(),
                ip: format!("{}:{}", ctx.remote_addr(), ctx.remote_port()),
            });
        }
    }

    pub fn disconnect_all(&self) {
        let clients = {
            let guard = self.clients.lock();
            guard.values().cloned().collect::<Vec<_>>()
        };

        for client in clients {
            client.disconnect();
        }

        self.clients.lock().clear();
    }

    /// Send an immediate job to a specific client (for use after authorization)
    /// This ensures IceRiver and other ASICs get a job immediately, not waiting for polling
    pub async fn send_immediate_job_to_client<T: KaspaApiTrait + Send + Sync + ?Sized + 'static>(
        &self,
        client: Arc<StratumContext>,
        kaspa_api: Arc<T>,
    ) {
        // Check if client has wallet address
        let _wallet_addr_str = {
            let id = client.identity.lock();
            if id.wallet_addr.is_empty() {
                debug!(
                    "send_immediate_job: client {} has no wallet address yet, skipping",
                    client.remote_addr
                );
                return;
            }
            id.wallet_addr.clone()
        };

        if !client.connected() {
            debug!(
                "send_immediate_job: client {} not connected, skipping",
                client.remote_addr
            );
            return;
        }

        let client_clone = Arc::clone(&client);
        let kaspa_api_clone = Arc::clone(&kaspa_api);
        let share_handler = Arc::clone(&self.share_handler);
        let min_diff = self.min_share_diff;
        let instance_id = self.instance_id.clone();

        tokio::spawn(async move {
            job_dispatch::send_immediate_job_task(
                client_clone,
                kaspa_api_clone,
                share_handler,
                min_diff,
                instance_id,
            )
            .await;
        });
    }

    pub async fn new_block_available<T: KaspaApiTrait + Send + Sync + 'static>(
        &self,
        kaspa_api: Arc<T>,
    ) {
        // Rate limit templates (250ms minimum between sends)
        {
            let mut last_time = self.last_template_time.lock();
            if last_time.elapsed() < Duration::from_millis(250) {
                return;
            }
            *last_time = Instant::now();
        }

        let clients = {
            let clients_guard = self.clients.lock();
            clients_guard.values().cloned().collect::<Vec<_>>()
        };

        // Collect addresses for balance checking
        let mut addresses: Vec<String> = Vec::new();
        let mut client_count = 0;

        for client in clients {
            if !client.connected() {
                continue;
            }

            if client_count > 0 {
                tokio::time::sleep(Duration::from_micros(500)).await;
            }
            client_count += 1;

            // Collect wallet address for balance checking
            {
                let w = client.identity.lock().wallet_addr.clone();
                if !w.is_empty() {
                    addresses.push(w);
                }
            }

            let client_clone = Arc::clone(&client);
            let kaspa_api_clone = Arc::clone(&kaspa_api);
            let share_handler = Arc::clone(&self.share_handler);
            let min_diff = self.min_share_diff;
            let instance_id = self.instance_id.clone();

            tokio::spawn(async move {
                job_dispatch::new_block_job_task(
                    client_clone,
                    kaspa_api_clone,
                    share_handler,
                    min_diff,
                    instance_id,
                )
                .await;
            });
        }

        // Check balances periodically
        {
            let mut last_check = self.last_balance_check.lock();
            if last_check.elapsed() > job_dispatch::BALANCE_DELAY && !addresses.is_empty() {
                *last_check = Instant::now();
                drop(last_check);

                // Fetch balances via kaspa_api
                let addresses_clone = addresses.clone();
                let kaspa_api_clone = Arc::clone(&kaspa_api);
                let instance_id = self.instance_id.clone();
                tokio::spawn(async move {
                    match kaspa_api_clone
                        .get_balances_by_addresses(&addresses_clone)
                        .await
                    {
                        Ok(balances) => {
                            // Record balances
                            crate::prom::record_balances(&instance_id, &balances);
                        }
                        Err(e) => {
                            warn!(
                                "failed to get balances from kaspa, prom stats will be out of date: {}",
                                e
                            );
                        }
                    }
                });
            }
        }
    }
}
