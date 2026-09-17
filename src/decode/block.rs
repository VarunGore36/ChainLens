use crate::domain::{Block, IndexedBlock, Log, Transaction};
use crate::rpc::types::{BlockResponse, LogResponse, ReceiptResponse, TransactionResponse};

use super::DecodeError;
use super::erc20::decode_token_transfers;

pub fn decode_block(
    block: &BlockResponse,
    receipts: &[ReceiptResponse],
) -> Result<IndexedBlock, DecodeError> {
    let header = decode_header(block)?;
    let transactions = decode_transactions(block, receipts)?;
    let logs = decode_logs(
        block,
        &receipts
            .iter()
            .flat_map(|r| r.logs.clone())
            .collect::<Vec<_>>(),
    )?;
    let token_transfers = decode_token_transfers(&logs)?;

    Ok(IndexedBlock {
        block: header,
        transactions,
        logs,
        token_transfers,
    })
}

fn decode_header(block: &BlockResponse) -> Result<Block, DecodeError> {
    Ok(Block {
        number: block.number,
        hash: block.hash,
        parent_hash: block.parent_hash,
        timestamp: block.timestamp,
        miner: block.miner,
        gas_used: block.gas_used,
        gas_limit: block.gas_limit,
        base_fee_per_gas: block.base_fee_per_gas,
        tx_count: u32::try_from(block.transactions.len()).unwrap_or(u32::MAX),
    })
}

fn decode_transactions(
    block: &BlockResponse,
    receipts: &[ReceiptResponse],
) -> Result<Vec<Transaction>, DecodeError> {
    let block_number = block.number;

    if block.transactions.len() != receipts.len() {
        return Err(DecodeError::ReceiptCountMismatch {
            block_number,
            transactions: block.transactions.len(),
            receipts: receipts.len(),
        });
    }

    let mut transactions = Vec::with_capacity(block.transactions.len());

    for (index, (tx, receipt)) in block.transactions.iter().zip(receipts.iter()).enumerate() {
        if tx.hash != receipt.transaction_hash {
            return Err(DecodeError::ReceiptHashMismatch {
                block_number,
                index,
                receipt_hash: receipt.transaction_hash,
                tx_hash: tx.hash,
            });
        }

        transactions.push(decode_single_transaction(block_number, index, tx, receipt)?);
    }

    Ok(transactions)
}

fn decode_single_transaction(
    block_number: u64,
    index: usize,
    tx: &TransactionResponse,
    receipt: &ReceiptResponse,
) -> Result<Transaction, DecodeError> {
    let _ = tx.hash;
    let from = tx.from;

    Ok(Transaction {
        hash: tx.hash,
        block_number,
        tx_index: u32::try_from(index).unwrap_or(u32::MAX),
        from,
        to: tx.to,
        value: tx.value,
        nonce: tx.nonce,
        tx_type: u8::try_from(tx.transaction_type.unwrap_or(0)).unwrap_or(0),
        gas_limit: tx.gas,
        gas_price: tx.gas_price,
        max_fee_per_gas: tx.max_fee_per_gas,
        max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
        input: tx.input.clone(),
        status: receipt.status.map(|s| u8::try_from(s).unwrap_or(0)),
        gas_used: receipt.gas_used,
        effective_gas_price: receipt.effective_gas_price,
        contract_address: receipt.contract_address,
    })
}

fn decode_logs(block: &BlockResponse, raw_logs: &[LogResponse]) -> Result<Vec<Log>, DecodeError> {
    let block_number = block.number;
    let mut logs = Vec::with_capacity(raw_logs.len());

    for (expected_index, (position, raw_log)) in (0u32..).zip(raw_logs.iter().enumerate()) {
        let log_index = u32::try_from(raw_log.log_index).unwrap_or(u32::MAX);

        if log_index != expected_index {
            return Err(DecodeError::LogIndexGap {
                block_number,
                expected: expected_index,
                actual: log_index,
                position,
            });
        }

        let topic0 = raw_log.topics.first().copied();
        let topic1 = raw_log.topics.get(1).copied();
        let topic2 = raw_log.topics.get(2).copied();
        let topic3 = raw_log.topics.get(3).copied();

        logs.push(Log {
            block_number,
            log_index,
            tx_hash: raw_log.transaction_hash,
            address: raw_log.address,
            topic0,
            topic1,
            topic2,
            topic3,
            data: raw_log.data.clone(),
        });
    }

    Ok(logs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::types::LogResponse;
    use alloy_primitives::{Address, B256, Bytes, U256};

    fn minimal_block() -> BlockResponse {
        BlockResponse {
            number: 1,
            hash: B256::with_last_byte(1),
            parent_hash: B256::ZERO,
            timestamp: 1000,
            miner: Address::ZERO,
            gas_used: 21000,
            gas_limit: 30000000,
            base_fee_per_gas: Some(7),
            transactions: vec![],
            size: 0,
        }
    }

    fn minimal_receipt(tx_hash: B256) -> ReceiptResponse {
        ReceiptResponse {
            transaction_hash: tx_hash,
            transaction_index: 0,
            block_hash: B256::with_last_byte(1),
            block_number: 1,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            cumulative_gas_used: 21000,
            gas_used: 21000,
            contract_address: None,
            status: Some(1),
            logs: vec![],
            transaction_type: Some(0),
            effective_gas_price: Some(U256::from(7u64)),
        }
    }

    #[test]
    fn decodes_an_empty_block() {
        let block = minimal_block();
        let result = decode_block(&block, &[]).unwrap();
        assert_eq!(result.block.number, 1);
        assert!(result.transactions.is_empty());
        assert!(result.logs.is_empty());
        assert!(result.token_transfers.is_empty());
    }

    #[test]
    fn rejects_mismatched_receipt_count() {
        let mut block = minimal_block();
        let tx_hash = B256::with_last_byte(0xaa);
        block.transactions.push(TransactionResponse {
            hash: tx_hash,
            transaction_index: 0,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            value: U256::ZERO,
            nonce: 0,
            gas: 21000,
            gas_price: Some(U256::from(1u64)),
            input: Bytes::new(),
            transaction_type: Some(0),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: Some(U256::from(1u64)),
            v: Some(U256::from(27u64)),
            r: Some(U256::ZERO),
            s: Some(U256::ZERO),
        });
        let result = decode_block(&block, &[]);
        assert!(matches!(
            result,
            Err(DecodeError::ReceiptCountMismatch { .. })
        ));
    }

    #[test]
    fn rejects_hash_mismatch() {
        let mut block = minimal_block();
        let tx_hash = B256::with_last_byte(0xaa);
        block.transactions.push(TransactionResponse {
            hash: tx_hash,
            transaction_index: 0,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            value: U256::ZERO,
            nonce: 0,
            gas: 21000,
            gas_price: Some(U256::from(1u64)),
            input: Bytes::new(),
            transaction_type: Some(0),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: Some(U256::from(1u64)),
            v: Some(U256::from(27u64)),
            r: Some(U256::ZERO),
            s: Some(U256::ZERO),
        });
        let wrong_hash = B256::with_last_byte(0xbb);
        let receipt = minimal_receipt(wrong_hash);
        let result = decode_block(&block, &[receipt]);
        assert!(matches!(
            result,
            Err(DecodeError::ReceiptHashMismatch { .. })
        ));
    }

    #[test]
    fn rejects_non_contiguous_log_indices() {
        let block_number = 1u64;
        let raw_logs = vec![
            LogResponse {
                address: Address::ZERO,
                topics: vec![],
                data: Bytes::new(),
                block_number,
                transaction_hash: B256::ZERO,
                transaction_index: 0,
                block_hash: B256::ZERO,
                log_index: 0,
                removed: false,
            },
            LogResponse {
                address: Address::ZERO,
                topics: vec![],
                data: Bytes::new(),
                block_number,
                transaction_hash: B256::ZERO,
                transaction_index: 0,
                block_hash: B256::ZERO,
                log_index: 5,
                removed: false,
            },
        ];
        let block = minimal_block();
        let result = decode_logs(&block, &raw_logs);
        assert!(matches!(result, Err(DecodeError::LogIndexGap { .. })));
    }

    #[test]
    fn status_value_2_is_preserved() {
        let mut block = minimal_block();
        let tx_hash = B256::with_last_byte(0xaa);
        block.transactions.push(TransactionResponse {
            hash: tx_hash,
            transaction_index: 0,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            value: U256::ZERO,
            nonce: 0,
            gas: 21000,
            gas_price: Some(U256::from(1u64)),
            input: Bytes::new(),
            transaction_type: Some(0),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: Some(U256::from(1u64)),
            v: Some(U256::from(27u64)),
            r: Some(U256::ZERO),
            s: Some(U256::ZERO),
        });

        let mut receipt = minimal_receipt(tx_hash);
        receipt.status = Some(2);
        let result = decode_block(&block, &[receipt]).unwrap();
        assert_eq!(
            result.transactions[0].status,
            Some(2),
            "status=2 is preserved (only values > 255 would be clamped to 0)"
        );
    }

    #[test]
    fn status_value_256_clamped_to_0() {
        let mut block = minimal_block();
        let tx_hash = B256::with_last_byte(0xaa);
        block.transactions.push(TransactionResponse {
            hash: tx_hash,
            transaction_index: 0,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            value: U256::ZERO,
            nonce: 0,
            gas: 21000,
            gas_price: Some(U256::from(1u64)),
            input: Bytes::new(),
            transaction_type: Some(0),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: Some(U256::from(1u64)),
            v: Some(U256::from(27u64)),
            r: Some(U256::ZERO),
            s: Some(U256::ZERO),
        });

        let mut receipt = minimal_receipt(tx_hash);
        receipt.status = Some(256);
        let result = decode_block(&block, &[receipt]).unwrap();
        assert_eq!(
            result.transactions[0].status,
            Some(0),
            "status=256 overflows u8, silently clamped to 0"
        );
    }

    #[test]
    fn transaction_type_256_silently_becomes_0() {
        let mut block = minimal_block();
        let tx_hash = B256::with_last_byte(0xaa);
        block.transactions.push(TransactionResponse {
            hash: tx_hash,
            transaction_index: 0,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            value: U256::ZERO,
            nonce: 0,
            gas: 21000,
            gas_price: Some(U256::from(1u64)),
            input: Bytes::new(),
            transaction_type: Some(256),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: Some(U256::from(1u64)),
            v: Some(U256::from(27u64)),
            r: Some(U256::ZERO),
            s: Some(U256::ZERO),
        });

        let receipt = minimal_receipt(tx_hash);
        let result = decode_block(&block, &[receipt]).unwrap();
        assert_eq!(
            result.transactions[0].tx_type, 0,
            "type=256 silently mapped to 0 (Legacy)"
        );
    }

    #[test]
    fn pre_eip1559_block_with_no_base_fee() {
        let mut block = minimal_block();
        block.base_fee_per_gas = None;
        let result = decode_block(&block, &[]).unwrap();
        assert!(result.block.base_fee_per_gas.is_none());
    }

    #[test]
    fn more_receipts_than_transactions_rejected() {
        let mut block = minimal_block();
        let tx_hash = B256::with_last_byte(0xaa);
        block.transactions.push(TransactionResponse {
            hash: tx_hash,
            transaction_index: 0,
            from: Address::ZERO,
            to: Some(Address::ZERO),
            value: U256::ZERO,
            nonce: 0,
            gas: 21000,
            gas_price: Some(U256::from(1u64)),
            input: Bytes::new(),
            transaction_type: Some(0),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            chain_id: Some(U256::from(1u64)),
            v: Some(U256::from(27u64)),
            r: Some(U256::ZERO),
            s: Some(U256::ZERO),
        });

        let receipt1 = minimal_receipt(tx_hash);
        let receipt2 = minimal_receipt(B256::with_last_byte(0xbb));
        let result = decode_block(&block, &[receipt1, receipt2]);
        assert!(matches!(
            result,
            Err(DecodeError::ReceiptCountMismatch { .. })
        ));
    }

    #[test]
    fn empty_logs_array() {
        let block = minimal_block();
        let result = decode_block(&block, &[]).unwrap();
        assert!(result.logs.is_empty());
    }

    #[test]
    fn log_index_starts_at_nonzero() {
        let block = minimal_block();
        let raw_logs = vec![LogResponse {
            address: Address::ZERO,
            topics: vec![],
            data: Bytes::new(),
            block_number: 1,
            transaction_hash: B256::ZERO,
            transaction_index: 0,
            block_hash: B256::ZERO,
            log_index: 5,
            removed: false,
        }];
        let result = decode_logs(&block, &raw_logs);
        assert!(
            matches!(
                result,
                Err(DecodeError::LogIndexGap {
                    actual: 5,
                    expected: 0,
                    ..
                })
            ),
            "should reject log_index=5 when expecting 0"
        );
    }

    #[test]
    fn decodes_a_transaction_with_eip1559_fields() {
        let mut block = minimal_block();
        let tx_hash = B256::with_last_byte(0xaa);
        block.transactions.push(TransactionResponse {
            hash: tx_hash,
            transaction_index: 0,
            from: Address::with_last_byte(0x01),
            to: Some(Address::with_last_byte(0x02)),
            value: U256::from(1_000_000_000_000_000_000u64),
            nonce: 42,
            gas: 21000,
            gas_price: None,
            input: Bytes::new(),
            transaction_type: Some(2),
            max_fee_per_gas: Some(U256::from(20_000_000_000u64)),
            max_priority_fee_per_gas: Some(U256::from(2_000_000_000u64)),
            chain_id: Some(U256::from(1u64)),
            v: None,
            r: Some(U256::ZERO),
            s: Some(U256::ZERO),
        });

        let receipt = minimal_receipt(tx_hash);
        let result = decode_block(&block, &[receipt]).unwrap();
        let tx = &result.transactions[0];

        assert_eq!(tx.tx_type, 2);
        assert_eq!(tx.nonce, 42);
        assert!(tx.max_fee_per_gas.is_some());
        assert!(tx.gas_price.is_none());
        assert_eq!(tx.status, Some(1));
    }
}
