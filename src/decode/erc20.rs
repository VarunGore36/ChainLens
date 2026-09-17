use alloy_primitives::{Address, B256, U256};

use super::DecodeError;
use crate::domain::token_transfer::TRANSFER_TOPIC;
use crate::domain::{Log, TokenStandard, TokenTransfer};

pub fn decode_token_transfers(logs: &[Log]) -> Result<Vec<TokenTransfer>, DecodeError> {
    let mut transfers = Vec::new();

    for log in logs {
        if log.topic0 != Some(TRANSFER_TOPIC) {
            continue;
        }

        match log.topics().len() {
            3 => transfers.push(decode_erc20_transfer(log)?),
            4 => transfers.push(decode_erc721_transfer(log)?),
            _ => continue,
        }
    }

    Ok(transfers)
}

fn decode_erc20_transfer(log: &Log) -> Result<TokenTransfer, DecodeError> {
    let from = extract_address_from_topic(log.topic1, log.log_index, 1)?;
    let to = extract_address_from_topic(log.topic2, log.log_index, 2)?;

    let data = &log.data;
    if data.len() != 32 {
        return Err(DecodeError::InvalidErc20DataLength {
            log_index: log.log_index,
            length: data.len(),
        });
    }

    let value = U256::from_be_slice(data);

    Ok(TokenTransfer {
        block_number: log.block_number,
        log_index: log.log_index,
        token_address: log.address,
        from,
        to,
        value,
        token_id: U256::ZERO,
        standard: TokenStandard::Erc20,
    })
}

fn decode_erc721_transfer(log: &Log) -> Result<TokenTransfer, DecodeError> {
    let from = extract_address_from_topic(log.topic1, log.log_index, 1)?;
    let to = extract_address_from_topic(log.topic2, log.log_index, 2)?;

    let token_id_bytes = log.topic3.ok_or(DecodeError::InvalidTransferAddress {
        log_index: log.log_index,
        topic_index: 3,
    })?;
    let token_id = U256::from_be_slice(token_id_bytes.as_slice());

    Ok(TokenTransfer {
        block_number: log.block_number,
        log_index: log.log_index,
        token_address: log.address,
        from,
        to,
        value: U256::ZERO,
        token_id,
        standard: TokenStandard::Erc721,
    })
}

fn extract_address_from_topic(
    topic: Option<B256>,
    log_index: u32,
    topic_index: u8,
) -> Result<Address, DecodeError> {
    let bytes = topic.ok_or(DecodeError::InvalidTransferAddress {
        log_index,
        topic_index,
    })?;

    Ok(Address::from_slice(&bytes[12..32]))
}

impl Log {
    pub fn topics(&self) -> Vec<B256> {
        let mut topics = Vec::with_capacity(4);
        if let Some(t) = self.topic0 {
            topics.push(t);
        }
        if let Some(t) = self.topic1 {
            topics.push(t);
        }
        if let Some(t) = self.topic2 {
            topics.push(t);
        }
        if let Some(t) = self.topic3 {
            topics.push(t);
        }
        topics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{B256, Bytes};

    fn address_to_topic(addr: Address) -> B256 {
        let mut bytes = [0u8; 32];
        bytes[12..32].copy_from_slice(addr.as_slice());
        B256::new(bytes)
    }

    fn make_erc20_transfer_log(log_index: u32, from: Address, to: Address, amount: U256) -> Log {
        let data_bytes = amount.to_be_bytes_vec();
        let mut data = [0u8; 32];
        let start = 32usize.saturating_sub(data_bytes.len());
        data[start..].copy_from_slice(&data_bytes);
        Log {
            block_number: 1,
            log_index,
            tx_hash: B256::ZERO,
            address: Address::with_last_byte(0x42),
            topic0: Some(TRANSFER_TOPIC),
            topic1: Some(address_to_topic(from)),
            topic2: Some(address_to_topic(to)),
            topic3: None,
            data: Bytes::from(data.to_vec()),
        }
    }

    fn make_erc721_transfer_log(log_index: u32, from: Address, to: Address, token_id: U256) -> Log {
        Log {
            block_number: 1,
            log_index,
            tx_hash: B256::ZERO,
            address: Address::with_last_byte(0x43),
            topic0: Some(TRANSFER_TOPIC),
            topic1: Some(address_to_topic(from)),
            topic2: Some(address_to_topic(to)),
            topic3: Some(B256::from(token_id)),
            data: Bytes::new(),
        }
    }

    #[test]
    fn decodes_erc20_transfer() {
        let from = Address::with_last_byte(0x01);
        let to = Address::with_last_byte(0x02);
        let amount = U256::from(1_000_000_000_000_000_000u64);
        let log = make_erc20_transfer_log(0, from, to, amount);

        let transfers = decode_token_transfers(&[log]).unwrap();
        assert_eq!(transfers.len(), 1);

        let t = &transfers[0];
        assert_eq!(t.standard, TokenStandard::Erc20);
        assert_eq!(t.from, from);
        assert_eq!(t.to, to);
        assert_eq!(t.value, amount);
        assert_eq!(t.token_id, U256::ZERO);
    }

    #[test]
    fn decodes_erc721_transfer() {
        let from = Address::with_last_byte(0x01);
        let to = Address::with_last_byte(0x02);
        let token_id = U256::from(12345u64);
        let log = make_erc721_transfer_log(0, from, to, token_id);

        let transfers = decode_token_transfers(&[log]).unwrap();
        assert_eq!(transfers.len(), 1);

        let t = &transfers[0];
        assert_eq!(t.standard, TokenStandard::Erc721);
        assert_eq!(t.from, from);
        assert_eq!(t.to, to);
        assert_eq!(t.value, U256::ZERO);
        assert_eq!(t.token_id, token_id);
    }

    #[test]
    fn skips_non_transfer_events() {
        let log = Log {
            block_number: 1,
            log_index: 0,
            tx_hash: B256::ZERO,
            address: Address::ZERO,
            topic0: Some(B256::with_last_byte(0x99)),
            topic1: None,
            topic2: None,
            topic3: None,
            data: Bytes::new(),
        };
        let transfers = decode_token_transfers(&[log]).unwrap();
        assert!(transfers.is_empty());
    }

    #[test]
    fn skips_transfer_with_wrong_topic_count() {
        let log = Log {
            block_number: 1,
            log_index: 0,
            tx_hash: B256::ZERO,
            address: Address::ZERO,
            topic0: Some(TRANSFER_TOPIC),
            topic1: Some(B256::ZERO),
            topic2: None,
            topic3: None,
            data: Bytes::new(),
        };
        let transfers = decode_token_transfers(&[log]).unwrap();
        assert!(transfers.is_empty());
    }

    #[test]
    fn rejects_erc20_with_wrong_data_length() {
        let log = Log {
            block_number: 1,
            log_index: 0,
            tx_hash: B256::ZERO,
            address: Address::ZERO,
            topic0: Some(TRANSFER_TOPIC),
            topic1: Some(B256::ZERO),
            topic2: Some(B256::ZERO),
            topic3: None,
            data: Bytes::from(vec![0u8; 16]),
        };
        let result = decode_token_transfers(&[log]);
        assert!(matches!(
            result,
            Err(DecodeError::InvalidErc20DataLength { .. })
        ));
    }

    #[test]
    fn decodes_multiple_transfers_in_order() {
        let logs = vec![
            make_erc20_transfer_log(
                0,
                Address::with_last_byte(0x01),
                Address::with_last_byte(0x02),
                U256::from(100u64),
            ),
            make_erc721_transfer_log(
                1,
                Address::with_last_byte(0x03),
                Address::with_last_byte(0x04),
                U256::from(1u64),
            ),
            make_erc20_transfer_log(
                2,
                Address::with_last_byte(0x05),
                Address::with_last_byte(0x06),
                U256::from(200u64),
            ),
        ];

        let transfers = decode_token_transfers(&logs).unwrap();
        assert_eq!(transfers.len(), 3);
        assert_eq!(transfers[0].standard, TokenStandard::Erc20);
        assert_eq!(transfers[1].standard, TokenStandard::Erc721);
        assert_eq!(transfers[2].standard, TokenStandard::Erc20);
        assert_eq!(transfers[0].log_index, 0);
        assert_eq!(transfers[1].log_index, 1);
        assert_eq!(transfers[2].log_index, 2);
    }

    #[test]
    fn erc20_empty_data_rejected() {
        let log = Log {
            block_number: 1,
            log_index: 0,
            tx_hash: B256::ZERO,
            address: Address::ZERO,
            topic0: Some(TRANSFER_TOPIC),
            topic1: Some(B256::ZERO),
            topic2: Some(B256::ZERO),
            topic3: None,
            data: Bytes::new(),
        };
        let result = decode_token_transfers(&[log]);
        assert!(matches!(
            result,
            Err(DecodeError::InvalidErc20DataLength { length: 0, .. })
        ));
    }

    #[test]
    fn erc20_31_bytes_data_rejected() {
        let log = Log {
            block_number: 1,
            log_index: 0,
            tx_hash: B256::ZERO,
            address: Address::ZERO,
            topic0: Some(TRANSFER_TOPIC),
            topic1: Some(B256::ZERO),
            topic2: Some(B256::ZERO),
            topic3: None,
            data: Bytes::from(vec![0u8; 31]),
        };
        let result = decode_token_transfers(&[log]);
        assert!(matches!(
            result,
            Err(DecodeError::InvalidErc20DataLength { length: 31, .. })
        ));
    }

    #[test]
    fn erc20_33_bytes_data_rejected() {
        let log = Log {
            block_number: 1,
            log_index: 0,
            tx_hash: B256::ZERO,
            address: Address::ZERO,
            topic0: Some(TRANSFER_TOPIC),
            topic1: Some(B256::ZERO),
            topic2: Some(B256::ZERO),
            topic3: None,
            data: Bytes::from(vec![0u8; 33]),
        };
        let result = decode_token_transfers(&[log]);
        assert!(matches!(
            result,
            Err(DecodeError::InvalidErc20DataLength { length: 33, .. })
        ));
    }

    #[test]
    fn three_topics_with_empty_data_fails_as_erc20() {
        let log = Log {
            block_number: 1,
            log_index: 0,
            tx_hash: B256::ZERO,
            address: Address::ZERO,
            topic0: Some(TRANSFER_TOPIC),
            topic1: Some(B256::ZERO),
            topic2: Some(B256::ZERO),
            topic3: None,
            data: Bytes::new(),
        };
        let result = decode_token_transfers(&[log]);
        assert!(
            matches!(result, Err(DecodeError::InvalidErc20DataLength { length: 0, .. })),
            "3 topics + empty data falls into ERC-20 path, rejects empty data"
        );
    }

    #[test]
    fn zero_address_from_and_to_is_valid() {
        let log = make_erc20_transfer_log(0, Address::ZERO, Address::ZERO, U256::from(100u64));
        let transfers = decode_token_transfers(&[log]).unwrap();
        assert_eq!(transfers[0].from, Address::ZERO);
        assert_eq!(transfers[0].to, Address::ZERO);
    }

    #[test]
    fn zero_value_transfer_is_valid() {
        let log = make_erc20_transfer_log(
            0,
            Address::with_last_byte(0x01),
            Address::with_last_byte(0x02),
            U256::ZERO,
        );
        let transfers = decode_token_transfers(&[log]).unwrap();
        assert_eq!(transfers[0].value, U256::ZERO);
    }

    #[test]
    fn self_transfer_is_valid() {
        let addr = Address::with_last_byte(0x01);
        let log = make_erc20_transfer_log(0, addr, addr, U256::from(100u64));
        let transfers = decode_token_transfers(&[log]).unwrap();
        assert_eq!(transfers[0].from, transfers[0].to);
    }

    #[test]
    fn max_u256_value_decodes() {
        let max_val = U256::MAX;
        let data_bytes = max_val.to_be_bytes_vec();
        let mut data = [0u8; 32];
        let start = 32usize.saturating_sub(data_bytes.len());
        data[start..].copy_from_slice(&data_bytes);
        let log = Log {
            block_number: 1,
            log_index: 0,
            tx_hash: B256::ZERO,
            address: Address::with_last_byte(0x42),
            topic0: Some(TRANSFER_TOPIC),
            topic1: Some(address_to_topic(Address::with_last_byte(0x01))),
            topic2: Some(address_to_topic(Address::with_last_byte(0x02))),
            topic3: None,
            data: Bytes::from(data.to_vec()),
        };
        let transfers = decode_token_transfers(&[log]).unwrap();
        assert_eq!(transfers[0].value, U256::MAX);
    }

    #[test]
    fn erc20_address_extraction_round_trips() {
        let original = Address::with_last_byte(0xab);
        let as_topic = address_to_topic(original);
        let extracted = extract_address_from_topic(Some(as_topic), 0, 1).unwrap();
        assert_eq!(original, extracted);
    }
}
