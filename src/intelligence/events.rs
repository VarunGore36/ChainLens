use std::collections::HashMap;
use std::sync::LazyLock;

use alloy_primitives::B256;

#[derive(Debug)]
pub struct EventSignature {
    pub name: String,
    pub description: String,
}

static KNOWN_EVENTS: LazyLock<HashMap<[u8; 32], EventSignature>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    m.insert(
        keccak256("Transfer(address,address,uint256)"),
        EventSignature {
            name: "Transfer".to_string(),
            description: "ERC-20/721 token transfer".to_string(),
        },
    );

    m.insert(
        keccak256("Approval(address,address,uint256)"),
        EventSignature {
            name: "Approval".to_string(),
            description: "ERC-20/721 token approval".to_string(),
        },
    );

    m.insert(
        keccak256("TransferSingle(address,address,address,uint256,uint256)"),
        EventSignature {
            name: "TransferSingle".to_string(),
            description: "ERC-1155 single token transfer".to_string(),
        },
    );

    m.insert(
        keccak256("TransferBatch(address,address,address,uint256[],uint256[])"),
        EventSignature {
            name: "TransferBatch".to_string(),
            description: "ERC-1155 batch token transfer".to_string(),
        },
    );

    m.insert(
        keccak256("ApprovalForAll(address,address,bool)"),
        EventSignature {
            name: "ApprovalForAll".to_string(),
            description: "ERC-721/1155 operator approval".to_string(),
        },
    );

    m.insert(
        keccak256("Swap(address,uint256,uint256,uint256,uint256,address)"),
        EventSignature {
            name: "Swap".to_string(),
            description: "Uniswap V2 swap".to_string(),
        },
    );

    m.insert(
        keccak256("Swap(address,address,int256,int256,uint160,uint128,int24,address)"),
        EventSignature {
            name: "Swap".to_string(),
            description: "Uniswap V3 swap".to_string(),
        },
    );

    m.insert(
        keccak256("Mint(address,uint256)"),
        EventSignature {
            name: "Mint".to_string(),
            description: "Token mint".to_string(),
        },
    );

    m.insert(
        keccak256("Burn(address,uint256)"),
        EventSignature {
            name: "Burn".to_string(),
            description: "Token burn".to_string(),
        },
    );

    m.insert(
        keccak256("Deposit(address,uint256)"),
        EventSignature {
            name: "Deposit".to_string(),
            description: "WETH deposit".to_string(),
        },
    );

    m.insert(
        keccak256("Withdrawal(address,uint256)"),
        EventSignature {
            name: "Withdrawal".to_string(),
            description: "WETH withdrawal".to_string(),
        },
    );

    m.insert(
        keccak256("Sync(uint112,uint112)"),
        EventSignature {
            name: "Sync".to_string(),
            description: "Uniswap V2 reserve sync".to_string(),
        },
    );

    m.insert(
        keccak256("PairCreated(address,address,address,uint256)"),
        EventSignature {
            name: "PairCreated".to_string(),
            description: "Uniswap V2 pair created".to_string(),
        },
    );

    m.insert(
        keccak256("PoolCreated(address,address,uint24,int24,address)"),
        EventSignature {
            name: "PoolCreated".to_string(),
            description: "Uniswap V3 pool created".to_string(),
        },
    );

    m.insert(
        keccak256("Borrow(address,address,uint256,uint256,uint256,address)"),
        EventSignature {
            name: "Borrow".to_string(),
            description: "Aave/Compound borrow".to_string(),
        },
    );

    m.insert(
        keccak256("Repay(address,address,uint256,address)"),
        EventSignature {
            name: "Repay".to_string(),
            description: "Aave/Compound repay".to_string(),
        },
    );

    m.insert(
        keccak256("Liquidation(address,address,address,uint256,uint256,address)"),
        EventSignature {
            name: "Liquidation".to_string(),
            description: "Liquidation event".to_string(),
        },
    );

    m.insert(
        keccak256("Deposit(address,address,uint256,uint256)"),
        EventSignature {
            name: "Deposit".to_string(),
            description: "Aave deposit".to_string(),
        },
    );

    m.insert(
        keccak256("Withdraw(address,address,uint256,uint256)"),
        EventSignature {
            name: "Withdraw".to_string(),
            description: "Aave withdraw".to_string(),
        },
    );

    m.insert(
        keccak256("DelegateChanged(address,address,address)"),
        EventSignature {
            name: "DelegateChanged".to_string(),
            description: "Governance delegation change".to_string(),
        },
    );

    m.insert(
        keccak256("VoteCast(address,uint256,uint8,uint256,string)"),
        EventSignature {
            name: "VoteCast".to_string(),
            description: "Governance vote".to_string(),
        },
    );

    m.insert(
        keccak256("OwnershipTransferred(address,address)"),
        EventSignature {
            name: "OwnershipTransferred".to_string(),
            description: "Ownership transfer".to_string(),
        },
    );

    m.insert(
        keccak256("Paused(address)"),
        EventSignature {
            name: "Paused".to_string(),
            description: "Contract paused".to_string(),
        },
    );

    m.insert(
        keccak256("Unpaused(address)"),
        EventSignature {
            name: "Unpaused".to_string(),
            description: "Contract unpaused".to_string(),
        },
    );

    m.insert(
        keccak256("Upgraded(address)"),
        EventSignature {
            name: "Upgraded".to_string(),
            description: "Proxy upgraded".to_string(),
        },
    );

    m.insert(
        keccak256("Initialized(uint8)"),
        EventSignature {
            name: "Initialized".to_string(),
            description: "Contract initialized".to_string(),
        },
    );

    m
});

fn keccak256(input: &str) -> [u8; 32] {
    use alloy_primitives::keccak256;
    keccak256(input).0
}

pub fn decode_event_signature(topic0: &B256) -> Option<&'static EventSignature> {
    let bytes: [u8; 32] = *topic0.as_ref();
    KNOWN_EVENTS.get(&bytes)
}

pub fn decode_event_name(topic0: &B256) -> String {
    decode_event_signature(topic0)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("0x{}", hex::encode(&topic0[..4])))
}

pub fn decode_event_description(topic0: &B256) -> String {
    decode_event_signature(topic0)
        .map(|s| s.description.clone())
        .unwrap_or_else(|| "Unknown event".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_event_decoded() {
        let topic = B256::new(keccak256("Transfer(address,address,uint256)"));
        let sig = decode_event_signature(&topic).unwrap();
        assert_eq!(sig.name, "Transfer");
    }

    #[test]
    fn approval_event_decoded() {
        let topic = B256::new(keccak256("Approval(address,address,uint256)"));
        let sig = decode_event_signature(&topic).unwrap();
        assert_eq!(sig.name, "Approval");
    }

    #[test]
    fn unknown_event_returns_none() {
        let topic = B256::with_last_byte(0x99);
        assert!(decode_event_signature(&topic).is_none());
    }

    #[test]
    fn unknown_event_name_is_hex() {
        let topic = B256::with_last_byte(0x99);
        let name = decode_event_name(&topic);
        assert!(name.starts_with("0x"));
    }

    #[test]
    fn all_known_events_decode() {
        let events = [
            "Transfer(address,address,uint256)",
            "Approval(address,address,uint256)",
            "TransferSingle(address,address,address,uint256,uint256)",
            "TransferBatch(address,address,address,uint256[],uint256[])",
            "ApprovalForAll(address,address,bool)",
            "Swap(address,uint256,uint256,uint256,uint256,address)",
            "Mint(address,uint256)",
            "Burn(address,uint256)",
            "Deposit(address,uint256)",
            "Withdrawal(address,uint256)",
        ];
        for event in events {
            let topic = B256::new(keccak256(event));
            let sig = decode_event_signature(&topic);
            assert!(sig.is_some(), "Failed to decode: {}", event);
        }
    }
}
