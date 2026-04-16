//! Block template RPC, balances, color queries, and block submission.

use super::KaspaApi;
use super::block_submit_guard::{remove_block_submit, try_mark_block_submit};
use anyhow::{Context, Result};
use kaspa_addresses::Address;
use kaspa_consensus_core::block::Block;
use kaspa_rpc_core::{
    GetBlockDagInfoRequest, GetBlockTemplateRequest, GetCurrentBlockColorRequest, RpcHash,
    RpcRawBlock, SubmitBlockRequest, SubmitBlockResponse, api::rpc::RpcApi,
};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::log_colors::LogColors;

impl KaspaApi {
    /// Submit a block
    pub async fn submit_block(&self, block: Block) -> Result<SubmitBlockResponse> {
        if !self.is_node_synced_for_mining().await {
            return Err(anyhow::anyhow!(
                "refusing block submit: node not mining-ready (sync, P2P IBD, or DAG block/header count mismatch)"
            ));
        }

        // Use kaspa_consensus_core::hashing::header::hash() for block hash calculation
        // In Kaspa, the block hash is the header hash (transactions are represented by hash_merkle_root in header)
        use kaspa_consensus_core::hashing::header;
        let block_hash = header::hash(&block.header).to_string();
        let blue_score = block.header.blue_score;
        let timestamp = block.header.timestamp;
        let nonce = block.header.nonce;

        {
            let now = Instant::now();
            if !try_mark_block_submit(&block_hash, now) {
                return Err(anyhow::anyhow!(
                    "ErrDuplicateBlock: block already submitted"
                ));
            }
        }

        debug!(
            "{} {}",
            LogColors::api("[API]"),
            LogColors::api(&format!(
                "===== ATTEMPTING BLOCK SUBMISSION TO KASPA NODE ===== Hash: {}",
                block_hash
            ))
        );
        debug!(
            "{} {}",
            LogColors::api("[API]"),
            LogColors::label("Block Details:")
        );
        debug!(
            "{} {} {}",
            LogColors::api("[API]"),
            LogColors::label("  - Hash:"),
            block_hash
        );
        debug!(
            "{} {} {}",
            LogColors::api("[API]"),
            LogColors::label("  - Blue Score:"),
            blue_score
        );
        debug!(
            "{} {} {}",
            LogColors::api("[API]"),
            LogColors::label("  - Timestamp:"),
            timestamp
        );
        debug!(
            "{} {} {}",
            LogColors::api("[API]"),
            LogColors::label("  - Nonce:"),
            format!("{:x} ({})", nonce, nonce)
        );
        debug!(
            "{} {}",
            LogColors::api("[API]"),
            "Converting block to RPC format and sending to node..."
        );

        // Convert Block to RpcRawBlock (use reference)
        let rpc_block: RpcRawBlock = (&block).into();

        // Submit block (don't allow non-DAA blocks)
        debug!(
            "{} {}",
            LogColors::api("[API]"),
            "Calling submit_block via RPC client..."
        );
        let result = self
            .client
            .submit_block_call(None, SubmitBlockRequest::new(rpc_block, false))
            .await
            .context("Failed to submit block");

        if let Err(e) = &result {
            let error_str = e.to_string();
            let is_duplicate =
                error_str.contains("ErrDuplicateBlock") || error_str.contains("duplicate");
            if !is_duplicate {
                let now = Instant::now();
                remove_block_submit(&block_hash, now);
            }
        }

        match &result {
            Ok(response) => {
                // IMPORTANT: The RPC call can succeed while the node still rejects the block.
                // Only treat SubmitBlockReport::Success as accepted.
                if !response.report.is_success() {
                    let now = Instant::now();
                    remove_block_submit(&block_hash, now);

                    warn!(
                        "{} {}",
                        LogColors::api("[API]"),
                        LogColors::validation(&format!(
                            "===== BLOCK REJECTED BY KASPA NODE ===== Hash: {}",
                            block_hash
                        ))
                    );
                    warn!(
                        "{} {} {}",
                        LogColors::api("[API]"),
                        LogColors::label("REJECTION REASON:"),
                        format!("{:?}", response.report)
                    );
                    warn!(
                        "{} {} {}",
                        LogColors::api("[API]"),
                        LogColors::label("  - Blue Score:"),
                        format!(
                            "{}, Timestamp: {}, Nonce: {:x}",
                            blue_score, timestamp, nonce
                        )
                    );
                    return Err(anyhow::anyhow!(
                        "Block rejected by node: {:?}",
                        response.report
                    ));
                }

                // Keep block accepted message at info (important operational event)
                info!(
                    "{} {}",
                    LogColors::api("[API]"),
                    LogColors::block(&format!(
                        "===== BLOCK ACCEPTED BY KASPA NODE ===== Hash: {}",
                        block_hash
                    ))
                );
                // Detailed acceptance logs moved to debug
                debug!(
                    "{} {} {}",
                    LogColors::api("[API]"),
                    LogColors::label("ACCEPTANCE REASON:"),
                    "Block passed all node validation checks"
                );
                debug!(
                    "{} {} {}",
                    LogColors::api("[API]"),
                    LogColors::label("  - Block structure:"),
                    "VALID"
                );
                debug!(
                    "{} {} {}",
                    LogColors::api("[API]"),
                    LogColors::label("  - Block header:"),
                    "VALID"
                );
                debug!(
                    "{} {} {}",
                    LogColors::api("[API]"),
                    LogColors::label("  - Transactions:"),
                    "VALID"
                );
                debug!(
                    "{} {} {}",
                    LogColors::api("[API]"),
                    LogColors::label("  - DAA validation:"),
                    "PASSED"
                );
                debug!(
                    "{} {} {}",
                    LogColors::api("[API]"),
                    LogColors::label("  - Node Response:"),
                    format!("{:?}", response)
                );
                debug!(
                    "{} {} {}",
                    LogColors::api("[API]"),
                    LogColors::label("  - Blue Score:"),
                    format!(
                        "{}, Timestamp: {}, Nonce: {:x}",
                        blue_score, timestamp, nonce
                    )
                );

                // Optional: Check if block appears in tip hashes (verifies propagation)
                // This is informational only - block may still propagate even if not immediately in tips
                let client_clone = Arc::clone(&self.client);
                let block_hash_clone = block_hash.clone();
                let block_hash_for_check = header::hash(&block.header); // Use the actual Hash type
                tokio::spawn(async move {
                    // Wait a bit for block to be processed and potentially added to DAG
                    tokio::time::sleep(Duration::from_secs(2)).await;

                    // Check if block appears in tip hashes
                    if let Ok(dag_response) = client_clone
                        .get_block_dag_info_call(None, GetBlockDagInfoRequest {})
                        .await
                    {
                        // Check if our block hash is in tip hashes
                        let in_tips = dag_response.tip_hashes.contains(&block_hash_for_check);

                        if in_tips {
                            info!(
                                "{} {} {}",
                                LogColors::api("[API]"),
                                LogColors::block(
                                    "Block appears in tip hashes (good sign for propagation)"
                                ),
                                format!("Hash: {}", block_hash_clone)
                            );
                        } else {
                            // This is not necessarily bad - block may still propagate or be in a side chain
                            info!(
                                "{} {} {}",
                                LogColors::api("[API]"),
                                LogColors::label(
                                    "Block not yet in tip hashes (may still propagate)"
                                ),
                                format!("Hash: {}", block_hash_clone)
                            );
                            info!(
                                "{} {} {}",
                                LogColors::api("[API]"),
                                LogColors::label("  - Note:"),
                                "Block may be in a side chain or still propagating"
                            );
                            info!(
                                "{} {} {}",
                                LogColors::api("[API]"),
                                LogColors::label("  - Tip hashes count:"),
                                dag_response.tip_hashes.len()
                            );
                        }
                    }
                });
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("ErrDuplicateBlock") || error_str.contains("duplicate") {
                    warn!(
                        "{} {}",
                        LogColors::api("[API]"),
                        LogColors::validation(&format!(
                            "===== BLOCK REJECTED BY KASPA NODE: STALE ===== Hash: {}",
                            block_hash
                        ))
                    );
                    warn!(
                        "{} {} {}",
                        LogColors::api("[API]"),
                        LogColors::label("REJECTION REASON:"),
                        "Block already exists in the network"
                    );
                    warn!(
                        "{} {}",
                        LogColors::api("[API]"),
                        LogColors::label("  - Block was previously submitted and accepted")
                    );
                    warn!(
                        "{} {}",
                        LogColors::api("[API]"),
                        LogColors::label("  - This is a duplicate/stale block submission")
                    );
                    warn!(
                        "{} {} {}",
                        LogColors::api("[API]"),
                        LogColors::error("  - Error:"),
                        error_str
                    );
                    warn!(
                        "{} {} {}",
                        LogColors::api("[API]"),
                        LogColors::label("  - Blue Score:"),
                        format!(
                            "{}, Timestamp: {}, Nonce: {:x}",
                            blue_score, timestamp, nonce
                        )
                    );
                } else {
                    error!(
                        "{} {}",
                        LogColors::api("[API]"),
                        LogColors::error(&format!(
                            "===== BLOCK REJECTED BY KASPA NODE: INVALID ===== Hash: {}",
                            block_hash
                        ))
                    );
                    error!(
                        "{} {} {}",
                        LogColors::api("[API]"),
                        LogColors::label("REJECTION REASON:"),
                        "Block failed node validation"
                    );
                    error!(
                        "{} {}",
                        LogColors::api("[API]"),
                        LogColors::label("  - Possible validation failures:")
                    );
                    error!(
                        "{} {}",
                        LogColors::api("[API]"),
                        "    * Invalid block structure or format"
                    );
                    error!(
                        "{} {}",
                        LogColors::api("[API]"),
                        "    * Block header validation failed"
                    );
                    error!(
                        "{} {}",
                        LogColors::api("[API]"),
                        "    * Transaction validation failed"
                    );
                    error!(
                        "{} {}",
                        LogColors::api("[API]"),
                        "    * DAA (Difficulty Adjustment Algorithm) validation failed"
                    );
                    error!(
                        "{} {}",
                        LogColors::api("[API]"),
                        "    * Block does not meet network consensus rules"
                    );
                    error!(
                        "{} {} {}",
                        LogColors::api("[API]"),
                        LogColors::error("  - Error from node:"),
                        error_str
                    );
                    error!(
                        "{} {} {}",
                        LogColors::api("[API]"),
                        LogColors::label("  - Blue Score:"),
                        format!(
                            "{}, Timestamp: {}, Nonce: {:x}",
                            blue_score, timestamp, nonce
                        )
                    );
                }
            }
        }

        result
    }
    /// Get block template for a client
    pub async fn get_block_template(
        &self,
        wallet_addr: &str,
        _remote_app: &str,
        _canxium_addr: &str,
    ) -> Result<Block> {
        if !self.is_node_synced_for_mining().await {
            return Err(anyhow::anyhow!(
                "refusing block template: node not mining-ready (sync, P2P IBD, or DAG block/header count mismatch)"
            ));
        }

        // Retry up to 3 times if we get "Odd number of digits" error
        // This error can occur if the block template has malformed hash fields
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 0..max_retries {
            // Parse wallet address each time (in case Address doesn't implement Clone)
            let address = Address::try_from(wallet_addr)
                .map_err(|e| anyhow::anyhow!("Could not decode address {}: {}", wallet_addr, e))?;

            // Request block template using RPC client wrapper
            let response = match self
                .client
                .get_block_template_call(
                    None,
                    GetBlockTemplateRequest::new(address, self.coinbase_tag.clone()),
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    if attempt < max_retries - 1 {
                        warn!(
                            "Failed to get block template (attempt {}/{}): {}, retrying...",
                            attempt + 1,
                            max_retries,
                            e
                        );
                        sleep(Duration::from_millis(100 * (attempt + 1) as u64)).await;
                        continue;
                    }
                    return Err(anyhow::anyhow!(
                        "Failed to get block template after {} attempts: {}",
                        max_retries,
                        e
                    ));
                }
            };

            // Get RPC block from response
            let rpc_block = response.block;

            // Convert RpcRawBlock to Block
            // The RpcRawBlock contains the block data that we need to convert
            // The "Odd number of digits" error can occur here if hash fields have malformed hex strings
            match Block::try_from(rpc_block) {
                Ok(block) => {
                    // Validate that we can serialize the block header
                    // This catches "Odd number of digits" errors early
                    // Convert error to String immediately to avoid Send issues
                    let serialize_result =
                        crate::hasher::serialize_block_header(&block).map_err(|e| e.to_string());

                    match serialize_result {
                        Ok(_) => {
                            return Ok(block);
                        }
                        Err(error_str) => {
                            if error_str.contains("Odd number of digits") {
                                last_error =
                                    Some(format!("Block has malformed hash field: {}", error_str));
                                if attempt < max_retries - 1 {
                                    warn!(
                                        "Block template has malformed hash field (attempt {}/{}), retrying...",
                                        attempt + 1,
                                        max_retries
                                    );
                                    sleep(Duration::from_millis(100 * (attempt + 1) as u64)).await;
                                    continue;
                                }
                            }
                            // If it's a different error, return it
                            return Err(anyhow::anyhow!(
                                "Failed to serialize block header: {}",
                                error_str
                            ));
                        }
                    }
                }
                Err(e) => {
                    let error_str = format!("{:?}", e);
                    last_error = Some(error_str.clone());
                    if error_str.contains("Odd number of digits") && attempt < max_retries - 1 {
                        warn!(
                            "Block conversion failed with 'Odd number of digits' error (attempt {}/{}), retrying...",
                            attempt + 1,
                            max_retries
                        );
                        sleep(Duration::from_millis(100 * (attempt + 1) as u64)).await;
                        continue;
                    }
                    // If the error contains "Odd number of digits", provide more context
                    if error_str.contains("Odd number of digits") {
                        return Err(anyhow::anyhow!(
                            "Failed to convert RPC block to Block after {} attempts: {} - This usually indicates a malformed hash field in the block template from the Kaspa node. The block may have a hash with an odd-length hex string.",
                            max_retries,
                            error_str
                        ));
                    } else {
                        return Err(anyhow::anyhow!(
                            "Failed to convert RPC block to Block: {}",
                            error_str
                        ));
                    }
                }
            }
        }

        // Should never reach here, but handle it just in case
        Err(anyhow::anyhow!(
            "Failed to get valid block template after {} attempts: {:?}",
            max_retries,
            last_error
        ))
    }

    /// Get balances by addresses (for Prometheus metrics)
    pub async fn get_balances_by_addresses(
        &self,
        addresses: &[String],
    ) -> Result<Vec<(String, u64)>> {
        let parsed_addresses: Result<Vec<Address>, _> = addresses
            .iter()
            .map(|addr| Address::try_from(addr.as_str()))
            .collect();

        let addresses =
            parsed_addresses.map_err(|e| anyhow::anyhow!("Failed to parse addresses: {:?}", e))?;

        let utxos = self
            .client
            .get_utxos_by_addresses_call(
                None,
                kaspa_rpc_core::GetUtxosByAddressesRequest::new(addresses),
            )
            .await
            .context("Failed to get UTXOs by addresses")?;

        // Calculate balances from UTXOs
        // Group entries by address
        let mut balance_map: HashMap<String, u64> = HashMap::new();
        for entry in utxos.entries {
            if let Some(address) = entry.address {
                let addr_str = address.to_string();
                let amount = entry.utxo_entry.amount;
                *balance_map.entry(addr_str).or_insert(0) += amount;
            }
        }
        let balances: Vec<(String, u64)> = balance_map.into_iter().collect();

        Ok(balances)
    }

    pub async fn get_current_block_color(&self, block_hash: &str) -> Result<bool> {
        let hash = RpcHash::from_str(block_hash).context("Failed to parse block hash")?;
        let resp = self
            .client
            .get_current_block_color_call(None, GetCurrentBlockColorRequest { hash })
            .await
            .context("Failed to query current block color")?;
        Ok(resp.blue)
    }
}
