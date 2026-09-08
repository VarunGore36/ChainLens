use alloy_primitives::{Address, B256, U256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenStandard {
    Erc20,
    Erc721,
}

#[derive(Debug, Clone)]
pub struct TokenTransfer {
    pub block_number: u64,
    pub log_index: u32,
    pub token_address: Address,
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub token_id: U256,
    pub standard: TokenStandard,
}

pub const TRANSFER_TOPIC: B256 = B256::new([
    0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
    0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0xef,
]);

pub const APPROVAL_TOPIC: B256 = B256::new([
    0x8c, 0x5b, 0xe1, 0xe5, 0xeb, 0xec, 0x7d, 0x5b, 0xd1, 0x4f, 0x71, 0x42, 0x7d, 0x1e, 0x84, 0xf3,
    0xdd, 0x03, 0x14, 0xc0, 0xf7, 0xb2, 0x29, 0x1e, 0x5b, 0x20, 0x0a, 0xc8, 0xc7, 0xc3, 0xb9, 0x25,
]);

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::keccak256;

    #[test]
    fn transfer_topic_matches_keccak() {
        let expected = keccak256("Transfer(address,address,uint256)");
        assert_eq!(TRANSFER_TOPIC, expected);
    }

    #[test]
    fn approval_topic_matches_keccak() {
        let expected = keccak256("Approval(address,address,uint256)");
        assert_eq!(APPROVAL_TOPIC, expected);
    }
}
