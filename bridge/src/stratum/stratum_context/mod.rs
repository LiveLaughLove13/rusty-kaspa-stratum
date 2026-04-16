//! Stratum per-connection state: identity, TCP halves, and JSON-RPC outbound I/O.
//!
//! Connection lifecycle and accessors live here; verbose send/reply logging is in [`outbound`].

mod outbound;
mod types;

pub use types::{ClientIdentity, ContextSummary, ErrorDisconnected};

use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

/// Stratum client context
pub struct StratumContext {
    pub remote_addr: String,
    pub remote_port: u16,
    pub identity: Arc<Mutex<ClientIdentity>>,
    pub id: Arc<Mutex<i32>>,
    pub extranonce: Arc<Mutex<String>>,
    pub state: Arc<crate::mining_state::MiningState>,
    disconnecting: Arc<AtomicBool>,
    write_lock: Arc<AtomicBool>,
    read_half: Arc<Mutex<Option<tokio::io::ReadHalf<TcpStream>>>>,
    write_half: Arc<Mutex<Option<tokio::io::WriteHalf<TcpStream>>>>,
    on_disconnect: mpsc::UnboundedSender<Arc<StratumContext>>,
}

impl StratumContext {
    pub fn new(
        remote_addr: String,
        remote_port: u16,
        stream: TcpStream,
        state: Arc<crate::mining_state::MiningState>,
        on_disconnect: mpsc::UnboundedSender<Arc<StratumContext>>,
    ) -> Arc<Self> {
        let (read_half, write_half) = tokio::io::split(stream);
        Arc::new(Self {
            remote_addr,
            remote_port,
            identity: Arc::new(Mutex::new(ClientIdentity::default())),
            id: Arc::new(Mutex::new(0)),
            extranonce: Arc::new(Mutex::new(String::new())),
            state,
            disconnecting: Arc::new(AtomicBool::new(false)),
            write_lock: Arc::new(AtomicBool::new(false)),
            read_half: Arc::new(Mutex::new(Some(read_half))),
            write_half: Arc::new(Mutex::new(Some(write_half))),
            on_disconnect,
        })
    }

    /// Check if client is connected
    pub fn connected(&self) -> bool {
        !self.disconnecting.load(Ordering::Acquire)
    }

    /// Get client ID
    pub fn id(&self) -> Option<i32> {
        let id = *self.id.lock();
        if id > 0 { Some(id) } else { None }
    }

    /// Set client ID
    pub fn set_id(&self, id: i32) {
        *self.id.lock() = id;
    }

    /// Get context summary
    pub fn summary(&self) -> ContextSummary {
        let id = self.identity.lock();
        ContextSummary {
            remote_addr: self.remote_addr.clone(),
            remote_port: self.remote_port,
            wallet_addr: id.wallet_addr.clone(),
            worker_name: id.worker_name.clone(),
            remote_app: id.remote_app.clone(),
        }
    }

    /// Get remote address string
    pub fn remote_addr(&self) -> &str {
        &self.remote_addr
    }

    /// Get remote port
    pub fn remote_port(&self) -> u16 {
        self.remote_port
    }

    /// Disconnect the client
    pub fn disconnect(&self) {
        if !self.disconnecting.swap(true, Ordering::Release) {
            let (worker_name, remote_app, wallet_addr) = {
                let id = self.identity.lock();
                (id.worker_name.clone(), id.remote_app.clone(), id.wallet_addr.clone())
            };
            let is_pre_handshake = worker_name.is_empty() && remote_app.is_empty() && wallet_addr.is_empty();
            if is_pre_handshake {
                tracing::debug!(
                    "disconnecting client {}:{} worker='{}' app='{}'",
                    self.remote_addr,
                    self.remote_port,
                    worker_name,
                    remote_app
                );
            } else {
                tracing::info!(
                    "disconnecting client {}:{} worker='{}' app='{}'",
                    self.remote_addr,
                    self.remote_port,
                    worker_name,
                    remote_app
                );
            }

            // Close the write half
            let write_half_opt = {
                let mut write_guard = self.write_half.lock();
                write_guard.take()
            };

            if let Some(mut write_half) = write_half_opt {
                // Try to shutdown gracefully in async context
                tokio::spawn(async move {
                    let _ = write_half.shutdown().await;
                });
            }

            // Close the read half
            let _ = {
                let mut read_guard = self.read_half.lock();
                read_guard.take()
            };
        }
    }

    fn check_disconnect(&self) {
        if !self.disconnecting.load(Ordering::Acquire) {
            // Spawn async disconnect
            let ctx = self.clone();
            tokio::spawn(async move {
                ctx.disconnect();
            });
        }
    }

    /// Get a reference to the read half (for reading)
    pub fn get_read_half(&self) -> parking_lot::MutexGuard<'_, Option<tokio::io::ReadHalf<TcpStream>>> {
        self.read_half.lock()
    }
}

impl Clone for StratumContext {
    fn clone(&self) -> Self {
        Self {
            remote_addr: self.remote_addr.clone(),
            remote_port: self.remote_port,
            identity: self.identity.clone(),
            id: self.id.clone(),
            extranonce: self.extranonce.clone(),
            state: self.state.clone(),
            disconnecting: self.disconnecting.clone(),
            write_lock: self.write_lock.clone(),
            read_half: self.read_half.clone(),
            write_half: self.write_half.clone(),
            on_disconnect: self.on_disconnect.clone(),
        }
    }
}
