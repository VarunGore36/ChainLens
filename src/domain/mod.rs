pub mod block;
pub mod log;
pub mod token_transfer;
pub mod transaction;

pub use block::Block;
pub use log::Log;
pub use token_transfer::{TokenStandard, TokenTransfer};
pub use transaction::Transaction;

use alloy_primitives::{Address, B256};

#[derive(Debug, Clone)]
pub struct IndexedBlock {
    pub block: Block,
    pub transactions: Vec<Transaction>,
    pub logs: Vec<Log>,
    pub token_transfers: Vec<TokenTransfer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValidatedAddress(pub Address);

impl std::ops::Deref for ValidatedAddress {
    type Target = Address;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Address> for ValidatedAddress {
    fn as_ref(&self) -> &Address {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValidatedHash(pub B256);

impl std::ops::Deref for ValidatedHash {
    type Target = B256;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<B256> for ValidatedHash {
    fn as_ref(&self) -> &B256 {
        &self.0
    }
}
