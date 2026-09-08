pub mod http;
pub mod ratelimit;
pub mod retry;
pub mod types;

use alloy_primitives::B256;
use types::{BlockResponse, LogResponse, ReceiptResponse, TransactionResponse};

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
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http { status, .. } => *status == 429 || *status >= 500,
            Self::Timeout | Self::Connection(_) => true,
            Self::JsonRpc { .. } | Self::Deserialize(_) => false,
        }
    }
}

#[async_trait::async_trait]
pub trait EthClient: Send + Sync {
    async fn get_block_by_number(
        &self,
        number: u64,
        full_transactions: bool,
    ) -> Result<Option<BlockResponse>, RpcError>;

    async fn get_block_by_tag(
        &self,
        tag: BlockTag,
        full_transactions: bool,
    ) -> Result<Option<BlockResponse>, RpcError>;

    async fn get_block_number(&self, tag: BlockTag) -> Result<u64, RpcError>;

    async fn get_block_receipts(&self, number: u64) -> Result<Vec<ReceiptResponse>, RpcError>;

    async fn get_transaction_receipt(
        &self,
        hash: B256,
    ) -> Result<Option<ReceiptResponse>, RpcError>;

    fn supports_block_receipts(&self) -> bool;

    async fn get_transaction_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, RpcError>;

    async fn get_logs_for_transaction(&self, tx_hash: B256) -> Result<Vec<LogResponse>, RpcError>;
}
