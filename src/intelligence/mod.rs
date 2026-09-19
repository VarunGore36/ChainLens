pub mod actions;
pub mod address;
pub mod anomaly;
pub mod background;
pub mod cache;
pub mod contract;
pub mod events;
pub mod export;
pub mod graph;
pub mod interfaces;
pub mod mev;
pub mod persist;
pub mod trends;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    EthTransfer,
    Erc20Transfer,
    Erc20Approval,
    Erc721Transfer,
    Erc1155Transfer,
    ContractCreation,
    ContractCall,
    Swap,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionAction {
    pub action_type: ActionType,
    pub from: String,
    pub to: Option<String>,
    pub value: Option<String>,
    pub token_address: Option<String>,
    pub token_symbol: Option<String>,
    pub amount: Option<String>,
    pub spender: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionExplanation {
    pub tx_hash: String,
    pub block_number: i64,
    pub timestamp: String,
    pub from: String,
    pub to: Option<String>,
    pub value_wei: String,
    pub value_eth: String,
    pub gas_used: i64,
    pub gas_price: Option<String>,
    pub effective_gas_price: Option<String>,
    pub tx_fee: String,
    pub status: String,
    pub actions: Vec<TransactionAction>,
    pub contracts_involved: Vec<String>,
    pub summary: String,
}
