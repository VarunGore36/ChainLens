pub mod committer;
pub mod head_watcher;
pub mod scheduler;
pub mod sequencer;
pub mod worker;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("RPC error: {0}")]
    Rpc(#[from] crate::rpc::RpcError),

    #[error("store error: {0}")]
    Store(#[from] crate::store::StoreError),

    #[error("decode error: {0}")]
    Decode(#[from] crate::decode::DecodeError),

    #[error("reorg error: {0}")]
    Reorg(#[from] crate::reorg::ReorgError),

    #[error("pipeline cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy)]
pub struct ChainHead {
    pub latest: u64,
    pub safe: u64,
    pub finalized: u64,
}
