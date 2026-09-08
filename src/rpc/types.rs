//! Wire-format types for Ethereum JSON-RPC 2.0.
//!
//! These are the shapes that travel over HTTP — not the domain model. Phase 3
//! introduces typed domain structs; this module is concerned solely with getting
//! bytes off the wire without losing information.
//!
//! # Hex encoding
//!
//! Ethereum JSON-RPC uses two incompatible hex conventions:
//!
//! - **Hex quantity** — `0x1a` for 26, `0x0` for zero. No leading zeros.
//!   Used for numbers: block numbers, gas, nonces, indices.
//! - **Hex data** — `0x0000001a` for 26, padded to even length. Used for
//!   fixed-size byte arrays: addresses (20 bytes), hashes (32 bytes).
//!
//! alloy-primitives handles hex data correctly for `Address`, `B256`, `U256`.
//! The [`hex_quantity`] module handles hex quantities for `u64`.

use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 framing
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub method: String,
    pub params: serde_json::Value,
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcResponse<T> {
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Hex-quantity helpers (numbers, not byte arrays)
// ---------------------------------------------------------------------------

pub mod hex_quantity {
    //! Deserializers for Ethereum hex-quantity encoding.
    //!
    //! `0x0` → 0, `0x1a` → 26, `0xff` → 255. Leading zeros are invalid per
    //! the spec except for `0x0` itself. An empty string after the prefix is
    //! treated as zero (some providers do this).

    use serde::{Deserialize, Deserializer};

    /// Deserialize a hex-quantity string into `u64`.
    pub fn u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let s: String = Deserialize::deserialize(deserializer)?;
        parse(&s).map_err(serde::de::Error::custom)
    }

    /// Deserialize an optional hex-quantity string into `Option<u64>`.
    ///
    /// A `null` JSON value becomes `None`. A present string is parsed as hex.
    pub fn optional_u64<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u64>, D::Error> {
        let s: Option<String> = Deserialize::deserialize(deserializer)?;
        s.map(|s| parse(&s).map_err(serde::de::Error::custom))
            .transpose()
    }

    fn parse(s: &str) -> Result<u64, String> {
        let digits = s
            .strip_prefix("0x")
            .ok_or_else(|| format!("missing 0x prefix: {s}"))?;
        if digits.is_empty() {
            return Ok(0);
        }
        u64::from_str_radix(digits, 16).map_err(|e| format!("invalid hex quantity {s}: {e}"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn zero() {
            assert_eq!(parse("0x0").unwrap(), 0);
        }

        #[test]
        fn typical() {
            assert_eq!(parse("0x1a").unwrap(), 26);
            assert_eq!(parse("0xff").unwrap(), 255);
            assert_eq!(parse("0x1234abcd").unwrap(), 0x1234_abcd);
        }

        #[test]
        fn empty_after_prefix_is_zero() {
            assert_eq!(parse("0x").unwrap(), 0);
        }

        #[test]
        fn missing_prefix_is_an_error() {
            assert!(parse("1a").is_err());
        }

        #[test]
        fn empty_string_is_an_error() {
            assert!(parse("").is_err());
        }

        #[test]
        fn large_block_number() {
            // Mainnet block 20_000_000 = 0x131_2D00
            assert_eq!(parse("0x1312D00").unwrap(), 20_000_000);
        }
    }
}

// ---------------------------------------------------------------------------
// Ethereum RPC response types
// ---------------------------------------------------------------------------

/// A block as returned by `eth_getBlockByNumber` / `eth_getBlockByHash`.
///
/// Fields that Phase 2 does not use are omitted; they will be added in Phase 3
/// when the domain model is finalized. Every field that *is* present is one the
/// pipeline needs: block linkage, timing, and the transaction list.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockResponse {
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub timestamp: u64,
    pub miner: Address,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub gas_used: u64,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub gas_limit: u64,
    #[serde(default, deserialize_with = "hex_quantity::optional_u64")]
    pub base_fee_per_gas: Option<u64>,
    /// `true` → full transaction objects; `false` → just hashes.
    /// When `true`, each element is a [`TransactionResponse`].
    /// When `false`, each element is a `B256` hash (not yet handled — Phase 3).
    pub transactions: Vec<TransactionResponse>,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub size: u64,
}

/// A transaction as returned inside a block with `full_transactions: true`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    pub hash: B256,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub transaction_index: u64,
    pub from: Address,
    #[serde(default)]
    pub to: Option<Address>,
    pub value: U256,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub nonce: u64,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub gas: u64,
    #[serde(default)]
    pub gas_price: Option<U256>,
    pub input: alloy_primitives::Bytes,
    #[serde(default, deserialize_with = "hex_quantity::optional_u64")]
    pub transaction_type: Option<u64>,
    #[serde(default)]
    pub max_fee_per_gas: Option<U256>,
    #[serde(default)]
    pub max_priority_fee_per_gas: Option<U256>,
    #[serde(default)]
    pub chain_id: Option<U256>,
    #[serde(default)]
    pub v: Option<U256>,
    #[serde(default)]
    pub r: Option<U256>,
    #[serde(default)]
    pub s: Option<U256>,
}

/// A transaction receipt.
///
/// Returned by `eth_getTransactionReceipt` and inside `eth_getBlockReceipts`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptResponse {
    pub transaction_hash: B256,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub transaction_index: u64,
    pub block_hash: B256,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub block_number: u64,
    pub from: Address,
    #[serde(default)]
    pub to: Option<Address>,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub cumulative_gas_used: u64,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub gas_used: u64,
    #[serde(default)]
    pub contract_address: Option<Address>,
    /// 1 = success, 0 = failure. Post-Byzantium only; `None` for pre-Byzantium.
    #[serde(default, deserialize_with = "hex_quantity::optional_u64")]
    pub status: Option<u64>,
    pub logs: Vec<LogResponse>,
    #[serde(default, deserialize_with = "hex_quantity::optional_u64")]
    pub transaction_type: Option<u64>,
    #[serde(default)]
    pub effective_gas_price: Option<U256>,
}

/// An event log inside a receipt.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogResponse {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: alloy_primitives::Bytes,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub block_number: u64,
    pub transaction_hash: B256,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub transaction_index: u64,
    pub block_hash: B256,
    #[serde(deserialize_with = "hex_quantity::u64")]
    pub log_index: u64,
    #[serde(default)]
    pub removed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_a_minimal_block() {
        let json = serde_json::json!({
            "number": "0x1",
            "hash": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "timestamp": "0x64a0c8a0",
            "miner": "0x0000000000000000000000000000000000000000",
            "gasUsed": "0x5208",
            "gasLimit": "0x1c9c380",
            "baseFeePerGas": "0x7",
            "transactions": [],
            "size": "0x200"
        });
        let block: BlockResponse = serde_json::from_value(json).unwrap();
        assert_eq!(block.number, 1);
        assert_eq!(block.gas_used, 0x5208);
        assert_eq!(block.base_fee_per_gas, Some(7));
        assert!(block.transactions.is_empty());
    }

    #[test]
    fn deserialize_a_receipt_with_status() {
        let json = serde_json::json!({
            "transactionHash": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "transactionIndex": "0x0",
            "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000002",
            "blockNumber": "0x1",
            "from": "0x0000000000000000000000000000000000000001",
            "to": "0x0000000000000000000000000000000000000002",
            "cumulativeGasUsed": "0x5208",
            "gasUsed": "0x5208",
            "contractAddress": null,
            "status": "0x1",
            "logs": []
        });
        let receipt: ReceiptResponse = serde_json::from_value(json).unwrap();
        assert_eq!(receipt.status, Some(1));
        assert_eq!(receipt.gas_used, 0x5208);
    }

    #[test]
    fn deserialize_a_pre_byzantium_receipt_without_status() {
        let json = serde_json::json!({
            "transactionHash": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "transactionIndex": "0x0",
            "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000002",
            "blockNumber": "0x1",
            "from": "0x0000000000000000000000000000000000000001",
            "to": "0x0000000000000000000000000000000000000002",
            "cumulativeGasUsed": "0x5208",
            "gasUsed": "0x5208",
            "contractAddress": null,
            "logs": []
        });
        let receipt: ReceiptResponse = serde_json::from_value(json).unwrap();
        assert_eq!(receipt.status, None);
    }

    #[test]
    fn deserialize_a_transaction_with_eip1559_fields() {
        let json = serde_json::json!({
            "hash": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "transactionIndex": "0x0",
            "from": "0x0000000000000000000000000000000000000001",
            "to": "0x0000000000000000000000000000000000000002",
            "value": "0xde0b6b3a7640000",
            "nonce": "0x1",
            "gas": "0x5208",
            "input": "0x",
            "transactionType": "0x2",
            "maxFeePerGas": "0x4a817c800",
            "maxPriorityFeePerGas": "0x3b9aca00",
            "chainId": "0x1",
            "v": "0x1",
            "r": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "s": "0x0000000000000000000000000000000000000000000000000000000000000001"
        });
        let tx: TransactionResponse = serde_json::from_value(json).unwrap();
        assert_eq!(tx.transaction_type, Some(2));
        assert!(tx.max_fee_per_gas.is_some());
        assert!(tx.max_priority_fee_per_gas.is_some());
    }

    #[test]
    fn deserialize_a_log() {
        let json = serde_json::json!({
            "address": "0x0000000000000000000000000000000000000001",
            "topics": [
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                "0x0000000000000000000000000000000000000000000000000000000000000000",
                "0x0000000000000000000000000000000000000000000000000000000000000001"
            ],
            "data": "0x0000000000000000000000000000000000000000000000000de0b6b3a7640000",
            "blockNumber": "0x1",
            "transactionHash": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "transactionIndex": "0x0",
            "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000002",
            "logIndex": "0x0",
            "removed": false
        });
        let log: LogResponse = serde_json::from_value(json).unwrap();
        assert_eq!(log.topics.len(), 3);
        assert_eq!(log.log_index, 0);
        assert!(!log.removed);
    }

    #[test]
    fn deserialize_a_contract_creation_receipt() {
        let json = serde_json::json!({
            "transactionHash": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "transactionIndex": "0x0",
            "blockHash": "0x0000000000000000000000000000000000000000000000000000000000000002",
            "blockNumber": "0x1",
            "from": "0x0000000000000000000000000000000000000001",
            "to": null,
            "cumulativeGasUsed": "0x5208",
            "gasUsed": "0x5208",
            "contractAddress": "0x0000000000000000000000000000000000000003",
            "status": "0x1",
            "logs": []
        });
        let receipt: ReceiptResponse = serde_json::from_value(json).unwrap();
        assert!(receipt.to.is_none());
        assert!(receipt.contract_address.is_some());
    }
}
