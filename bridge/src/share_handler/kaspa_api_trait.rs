use anyhow::Result;
use kaspa_consensus_core::block::Block;

/// Kaspa node / RPC surface used by the share handler and stratum server (`KaspaApi` is the main impl).
#[async_trait::async_trait]
pub trait KaspaApiTrait: Send + Sync {
    async fn get_block_template(&self, wallet_addr: &str, remote_app: &str, canxium_addr: &str) -> Result<Block>;

    async fn submit_block(&self, block: Block) -> Result<kaspa_rpc_core::SubmitBlockResponse>;

    async fn get_balances_by_addresses(&self, addresses: &[String]) -> Result<Vec<(String, u64)>>;

    async fn get_current_block_color(&self, block_hash: &str) -> Result<bool>;

    /// `true` only when the node reports fully synced for mining (`getSyncStatus`: sink recent + not in transitional IBD).
    async fn is_node_synced_for_mining(&self) -> bool;
}
