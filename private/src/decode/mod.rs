pub mod block;
pub mod erc20;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("block number missing from RPC response")]
    MissingBlockNumber,

    #[error("block hash missing from RPC response")]
    MissingBlockHash,

    #[error("parent hash missing from RPC response")]
    MissingParentHash,

    #[error(
        "receipt count ({receipts}) does not match transaction count ({transactions}) in block {block_number}"
    )]
    ReceiptCountMismatch {
        block_number: u64,
        transactions: usize,
        receipts: usize,
    },

    #[error(
        "receipt {index} hash ({receipt_hash}) does not match transaction {index} hash ({tx_hash}) in block {block_number}"
    )]
    ReceiptHashMismatch {
        block_number: u64,
        index: usize,
        receipt_hash: alloy_primitives::B256,
        tx_hash: alloy_primitives::B256,
    },

    #[error(
        "log indices not contiguous in block {block_number}: expected {expected}, got {actual} at position {position}"
    )]
    LogIndexGap {
        block_number: u64,
        expected: u32,
        actual: u32,
        position: usize,
    },

    #[error("transaction at index {index} in block {block_number} has no hash")]
    MissingTxHash { block_number: u64, index: usize },

    #[error("transaction at index {index} in block {block_number} has no sender")]
    MissingTxFrom { block_number: u64, index: usize },

    #[error("receipt at index {index} in block {block_number} has no transaction hash")]
    MissingReceiptTxHash { block_number: u64, index: usize },

    #[error("ERC-20 Transfer event at log index {log_index} has invalid data length: {length}")]
    InvalidErc20DataLength { log_index: u32, length: usize },

    #[error(
        "ERC-20 Transfer event at log index {log_index} has malformed address in topic {topic_index}"
    )]
    InvalidTransferAddress { log_index: u32, topic_index: u8 },
}
