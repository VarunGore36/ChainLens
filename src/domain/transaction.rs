use alloy_primitives::{Address, B256, Bytes, U256};

#[derive(Debug, Clone)]
pub struct Transaction {
    pub hash: B256,
    pub block_number: u64,
    pub tx_index: u32,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub nonce: u64,
    pub tx_type: u8,
    pub gas_limit: u64,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub input: Bytes,
    pub status: Option<u8>,
    pub gas_used: u64,
    pub effective_gas_price: Option<U256>,
    pub contract_address: Option<Address>,
}
