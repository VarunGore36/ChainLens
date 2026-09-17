pub mod detect;
pub mod rollback;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReorgError {
    #[error("reorg depth {depth} exceeds maximum {max_depth}")]
    DepthExceeded { depth: u64, max_depth: u64 },

    #[error("RPC error during ancestor search: {0}")]
    Rpc(#[from] crate::rpc::RpcError),

    #[error("store error during rollback: {0}")]
    Store(#[from] crate::store::StoreError),
}

#[derive(Debug, Clone)]
pub struct ReorgEvent {
    pub common_ancestor: u64,
    pub depth: u64,
    pub orphaned_from: u64,
    pub orphaned_to: u64,
    pub orphaned_hashes: Vec<Vec<u8>>,
}
