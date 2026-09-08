use alloy_primitives::{Address, B256, Bytes};

#[derive(Debug, Clone)]
pub struct Log {
    pub block_number: u64,
    pub log_index: u32,
    pub tx_hash: B256,
    pub address: Address,
    pub topic0: Option<B256>,
    pub topic1: Option<B256>,
    pub topic2: Option<B256>,
    pub topic3: Option<B256>,
    pub data: Bytes,
}
