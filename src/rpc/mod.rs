//! Ethereum JSON-RPC client.
//!
//! The trait boundary ([`EthClient`]) is what makes the rest of the system
//! testable without a live Ethereum node. Phase 6's mock RPC implements this
//! trait; Phase 11's benchmark harness uses it to replay fixtures with
//! synthetic latency.
//!
//! The production implementation is [`http::HttpRpcClient`] — a hand-rolled
//! JSON-RPC 2.0 transport over `reqwest`. The transport is explicit rather
//! than delegated to a provider library because this is a systems-programming
//! portfolio piece and the wire protocol is part of the story.
//!
//! # Module layout
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | `types` | Wire-format structs, hex decoding |
//! | `retry` | Exponential backoff with full jitter |
//! | `ratelimit` | Token-bucket rate limiter (governor) |
//! | `http` | Production `EthClient` implementation |

pub mod http;
pub mod ratelimit;
pub mod retry;
pub mod types;

use alloy_primitives::B256;
use types::{BlockResponse, LogResponse, ReceiptResponse, TransactionResponse};

// ---------------------------------------------------------------------------
// Block identification
// ---------------------------------------------------------------------------

/// How to name a block in an RPC call.
///
/// Tags are the string constants Ethereum nodes understand; numbers are
/// hex-encoded. Serializing this enum produces the correct JSON-RPC
/// parameter — no call site needs to know the encoding rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockId {
    Number(u64),
    Tag(BlockTag),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTag {
    Latest,
    Safe,
    Finalized,
    Earliest,
    Pending,
}

impl BlockTag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Latest => "latest",
            Self::Safe => "safe",
            Self::Finalized => "finalized",
            Self::Earliest => "earliest",
            Self::Pending => "pending",
        }
    }
}

impl serde::Serialize for BlockId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Number(n) => serializer.serialize_str(&format!("0x{n:x}")),
            Self::Tag(t) => serializer.serialize_str(t.as_str()),
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Everything that can go wrong on the wire.
///
/// The `is_retryable` classification is the single source of truth for the
/// retry policy. HTTP 429 and 5xx, timeouts, and connection errors get
/// retried. Everything else — 4xx, JSON-RPC protocol errors, deserialization
/// failures — is deterministic and retrying it would just waste time.
#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },

    #[error("JSON-RPC error {code}: {message}")]
    JsonRpc { code: i64, message: String },

    #[error("request timed out")]
    Timeout,

    #[error("connection error: {0}")]
    Connection(String),

    #[error("deserialization error: {0}")]
    Deserialize(String),
}

impl RpcError {
    /// Whether the retry layer should try again.
    pub fn is_retryable(&self) -> bool {
        match self {
            // 429 = rate limited, 5xx = server is having a bad day.
            Self::Http { status, .. } => *status == 429 || *status >= 500,
            Self::Timeout | Self::Connection(_) => true,
            // JSON-RPC errors (method not found, invalid params, etc.) and
            // deserialization failures are deterministic.
            Self::JsonRpc { .. } | Self::Deserialize(_) => false,
        }
    }
}

// ---------------------------------------------------------------------------
// The trait
// ---------------------------------------------------------------------------

/// The Ethereum JSON-RPC surface that the pipeline uses.
///
/// Every method returns a concrete type rather than `serde_json::Value`,
/// because the point of the trait boundary is to make the pipeline testable
/// against a mock that returns real typed data, not loosely-typed JSON.
///
/// Implementors must be `Send + Sync` because the pipeline shares a single
/// client across multiple Tokio tasks.
#[async_trait::async_trait]
pub trait EthClient: Send + Sync {
    /// Fetch a block by number. Returns `None` if the block does not exist
    /// (number above the chain head).
    async fn get_block_by_number(
        &self,
        number: u64,
        full_transactions: bool,
    ) -> Result<Option<BlockResponse>, RpcError>;

    /// Fetch a block by tag (`latest`, `safe`, `finalized`, etc.).
    async fn get_block_by_tag(
        &self,
        tag: BlockTag,
        full_transactions: bool,
    ) -> Result<Option<BlockResponse>, RpcError>;

    /// Resolve a tag to a block number.
    async fn get_block_number(&self, tag: BlockTag) -> Result<u64, RpcError>;

    /// Fetch all receipts for a block in a single RPC call.
    ///
    /// Returns an empty vec if the block has no transactions, or an error if
    /// the node does not support `eth_getBlockReceipts`. The caller should
    /// check [`EthClient::supports_block_receipts`] before using this method.
    async fn get_block_receipts(&self, number: u64) -> Result<Vec<ReceiptResponse>, RpcError>;

    /// Fetch a single transaction receipt.
    ///
    /// This is the fallback when `eth_getBlockReceipts` is unavailable: the
    /// caller fetches each receipt individually.
    async fn get_transaction_receipt(
        &self,
        hash: B256,
    ) -> Result<Option<ReceiptResponse>, RpcError>;

    /// Whether the node supports `eth_getBlockReceipts`.
    ///
    /// Probed at startup. If `false`, the pipeline falls back to one
    /// `eth_getTransactionReceipt` call per transaction.
    fn supports_block_receipts(&self) -> bool;

    /// Fetch a single transaction by hash.
    async fn get_transaction_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, RpcError>;

    /// Fetch a single event log by transaction hash and log index.
    /// Returns all logs for the given transaction.
    async fn get_logs_for_transaction(&self, tx_hash: B256) -> Result<Vec<LogResponse>, RpcError>;
}
