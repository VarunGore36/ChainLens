use std::collections::HashMap;
use std::sync::Mutex;

use alloy_primitives::{Address, B256, Bytes, U256};

use crate::rpc::{
    BlockTag, EthClient, RpcError,
    types::{BlockResponse, LogResponse, ReceiptResponse, TransactionResponse},
};

#[derive(Clone, Debug)]
pub struct MockBlock {
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    pub timestamp: u64,
    pub transactions: Vec<TransactionResponse>,
    pub receipts: Vec<ReceiptResponse>,
}

#[derive(Debug)]
pub struct MockRpcClient {
    blocks: Mutex<HashMap<u64, MockBlock>>,
    head: Mutex<u64>,
    finalized: Mutex<u64>,
    supports_block_receipts: bool,
}

impl MockRpcClient {
    pub fn new(supports_block_receipts: bool) -> Self {
        Self {
            blocks: Mutex::new(HashMap::new()),
            head: Mutex::new(0),
            finalized: Mutex::new(0),
            supports_block_receipts,
        }
    }

    pub fn add_block(&self, block: MockBlock) {
        let number = block.number;
        self.blocks.lock().unwrap().insert(number, block);
        let mut head = self.head.lock().unwrap();
        if number > *head {
            *head = number;
        }
    }

    pub fn set_head(&self, head: u64) {
        *self.head.lock().unwrap() = head;
    }

    pub fn set_finalized(&self, finalized: u64) {
        *self.finalized.lock().unwrap() = finalized;
    }

    pub fn replace_block(&self, block: MockBlock) {
        self.blocks.lock().unwrap().insert(block.number, block);
    }
}

fn make_genesis() -> MockBlock {
    MockBlock {
        number: 0,
        hash: B256::with_last_byte(0),
        parent_hash: B256::ZERO,
        timestamp: 1000,
        transactions: vec![],
        receipts: vec![],
    }
}

pub fn make_chain(length: u64) -> MockRpcClient {
    let client = MockRpcClient::new(true);
    let genesis = make_genesis();
    client.add_block(genesis);

    for i in 1..=length {
        #[allow(clippy::cast_possible_truncation)]
        let block = MockBlock {
            number: i,
            hash: B256::with_last_byte(i as u8),
            parent_hash: B256::with_last_byte((i - 1) as u8),
            timestamp: 1000 + i * 12,
            transactions: vec![make_tx(i, 0)],
            receipts: vec![make_receipt(i, 0)],
        };
        client.add_block(block);
    }

    client.set_head(length);
    client.set_finalized(length.saturating_sub(32));
    client
}

fn make_tx(block_number: u64, tx_index: u64) -> TransactionResponse {
    let hash_bytes = {
        let mut b = [0u8; 32];
        let bn = block_number.to_be_bytes();
        let ti = tx_index.to_be_bytes();
        b[24..32].copy_from_slice(&bn);
        b[16..24].copy_from_slice(&ti);
        B256::new(b)
    };

    TransactionResponse {
        hash: hash_bytes,
        transaction_index: tx_index,
        from: Address::with_last_byte(1),
        to: Some(Address::with_last_byte(2)),
        value: U256::from(1000u64),
        nonce: 0,
        gas: 21000,
        gas_price: Some(U256::from(7u64)),
        input: Bytes::new(),
        transaction_type: Some(0),
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        chain_id: Some(U256::from(1u64)),
        v: Some(U256::from(27u64)),
        r: Some(U256::ZERO),
        s: Some(U256::ZERO),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn make_receipt(block_number: u64, tx_index: u64) -> ReceiptResponse {
    let tx_hash = make_tx(block_number, tx_index).hash;
    ReceiptResponse {
        transaction_hash: tx_hash,
        transaction_index: tx_index,
        block_hash: B256::with_last_byte(block_number as u8),
        block_number,
        from: Address::with_last_byte(1),
        to: Some(Address::with_last_byte(2)),
        cumulative_gas_used: 21000,
        gas_used: 21000,
        contract_address: None,
        status: Some(1),
        logs: vec![],
        transaction_type: Some(0),
        effective_gas_price: Some(U256::from(7u64)),
    }
}

#[async_trait::async_trait]
impl EthClient for MockRpcClient {
    async fn get_block_by_number(
        &self,
        number: u64,
        _full_transactions: bool,
    ) -> Result<Option<BlockResponse>, RpcError> {
        let blocks = self.blocks.lock().unwrap();
        Ok(blocks.get(&number).map(|b| BlockResponse {
            number: b.number,
            hash: b.hash,
            parent_hash: b.parent_hash,
            timestamp: b.timestamp,
            miner: Address::ZERO,
            gas_used: 21000,
            gas_limit: 30000000,
            base_fee_per_gas: Some(7),
            transactions: b.transactions.clone(),
            size: 256,
        }))
    }

    async fn get_block_by_tag(
        &self,
        tag: BlockTag,
        full_transactions: bool,
    ) -> Result<Option<BlockResponse>, RpcError> {
        let number = match tag {
            BlockTag::Latest => *self.head.lock().unwrap(),
            BlockTag::Finalized => *self.finalized.lock().unwrap(),
            BlockTag::Safe => *self.finalized.lock().unwrap(),
            BlockTag::Earliest => 0,
            BlockTag::Pending => *self.head.lock().unwrap(),
        };
        self.get_block_by_number(number, full_transactions).await
    }

    async fn get_block_number(&self, tag: BlockTag) -> Result<u64, RpcError> {
        match tag {
            BlockTag::Latest => Ok(*self.head.lock().unwrap()),
            BlockTag::Finalized | BlockTag::Safe => Ok(*self.finalized.lock().unwrap()),
            BlockTag::Earliest => Ok(0),
            BlockTag::Pending => Ok(*self.head.lock().unwrap()),
        }
    }

    async fn get_block_receipts(&self, number: u64) -> Result<Vec<ReceiptResponse>, RpcError> {
        let blocks = self.blocks.lock().unwrap();
        blocks
            .get(&number)
            .map(|b| b.receipts.clone())
            .ok_or_else(|| RpcError::Deserialize(format!("block {number} not found")))
    }

    async fn get_transaction_receipt(
        &self,
        hash: B256,
    ) -> Result<Option<ReceiptResponse>, RpcError> {
        let blocks = self.blocks.lock().unwrap();
        for block in blocks.values() {
            for receipt in &block.receipts {
                if receipt.transaction_hash == hash {
                    return Ok(Some(receipt.clone()));
                }
            }
        }
        Ok(None)
    }

    fn supports_block_receipts(&self) -> bool {
        self.supports_block_receipts
    }

    async fn get_transaction_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, RpcError> {
        let blocks = self.blocks.lock().unwrap();
        for block in blocks.values() {
            for tx in &block.transactions {
                if tx.hash == hash {
                    return Ok(Some(tx.clone()));
                }
            }
        }
        Ok(None)
    }

    async fn get_logs_for_transaction(&self, tx_hash: B256) -> Result<Vec<LogResponse>, RpcError> {
        let receipt = self.get_transaction_receipt(tx_hash).await?;
        Ok(receipt.map(|r| r.logs).unwrap_or_default())
    }
}
