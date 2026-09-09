use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BlockResponse {
    pub number: i64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: String,
    pub miner: String,
    pub gas_used: i64,
    pub gas_limit: i64,
    pub base_fee: Option<String>,
    pub tx_count: i32,
}

#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    pub hash: String,
    pub block_number: i64,
    pub tx_index: i32,
    pub from_addr: String,
    pub to_addr: Option<String>,
    pub value: String,
    pub nonce: i64,
    pub tx_type: i16,
    pub gas_limit: i64,
    pub gas_price: Option<String>,
    pub status: Option<i16>,
    pub gas_used: i64,
}

#[derive(Debug, Serialize)]
pub struct LogResponse {
    pub block_number: i64,
    pub log_index: i32,
    pub tx_hash: String,
    pub address: String,
    pub topic0: Option<String>,
    pub topic1: Option<String>,
    pub topic2: Option<String>,
    pub topic3: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AddressTransactionResponse {
    pub tx_hash: String,
    pub block_number: i64,
    pub tx_index: i32,
    pub direction: i16,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub last_indexed_number: Option<i64>,
    pub finalized_number: Option<i64>,
    pub lag: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
