use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Serialize, Deserialize)]
pub struct AddressLabel {
    pub address: String,
    pub label: String,
    pub category: String,
    pub confidence: f64,
    pub source: String,
}

pub async fn get_address_labels(
    pool: &PgPool,
    address: &str,
) -> Result<Vec<AddressLabel>, sqlx::Error> {
    let addr_bytes = hex::decode(address.trim_start_matches("0x"))
        .map_err(|_| sqlx::Error::Protocol("invalid address".into()))?;

    let mut labels = Vec::new();

    let tx_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE from_addr = $1 OR to_addr = $1")
            .bind(&addr_bytes)
            .fetch_one(pool)
            .await?;

    let contract_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions WHERE from_addr = $1 AND contract_address IS NOT NULL",
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;

    let token_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM token_transfers WHERE from_addr = $1 OR to_addr = $1")
            .bind(&addr_bytes)
            .fetch_one(pool)
            .await?;

    let nft_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM token_transfers WHERE (from_addr = $1 OR to_addr = $1) AND standard = 1"
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;

    let unique_contracts: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT to_addr) FROM transactions WHERE from_addr = $1 AND to_addr IS NOT NULL"
    )
    .bind(&addr_bytes)
    .fetch_one(pool)
    .await?;

    if tx_count.0 > 1000 {
        labels.push(AddressLabel {
            address: address.to_string(),
            label: "High Activity".to_string(),
            category: "activity".to_string(),
            confidence: 0.9,
            source: "on-chain".to_string(),
        });
    }

    if contract_count.0 > 0 {
        labels.push(AddressLabel {
            address: address.to_string(),
            label: "Contract Deployer".to_string(),
            category: "deployer".to_string(),
            confidence: 1.0,
            source: "on-chain".to_string(),
        });
    }

    if nft_count.0 > 10 {
        labels.push(AddressLabel {
            address: address.to_string(),
            label: "NFT Collector".to_string(),
            category: "nft".to_string(),
            confidence: 0.8,
            source: "on-chain".to_string(),
        });
    }

    if token_count.0 > 100 {
        labels.push(AddressLabel {
            address: address.to_string(),
            label: "Token Trader".to_string(),
            category: "trader".to_string(),
            confidence: 0.8,
            source: "on-chain".to_string(),
        });
    }

    if unique_contracts.0 > 20 {
        labels.push(AddressLabel {
            address: address.to_string(),
            label: "DeFi User".to_string(),
            category: "defi".to_string(),
            confidence: 0.7,
            source: "on-chain".to_string(),
        });
    }

    Ok(labels)
}

pub fn get_known_labels() -> Vec<AddressLabel> {
    vec![
        AddressLabel {
            address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            label: "Vitalik Buterin".to_string(),
            category: "known_person".to_string(),
            confidence: 1.0,
            source: "public".to_string(),
        },
        AddressLabel {
            address: "0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984".to_string(),
            label: "UNI Token".to_string(),
            category: "token".to_string(),
            confidence: 1.0,
            source: "public".to_string(),
        },
        AddressLabel {
            address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
            label: "USDC".to_string(),
            category: "token".to_string(),
            confidence: 1.0,
            source: "public".to_string(),
        },
        AddressLabel {
            address: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
            label: "USDT".to_string(),
            category: "token".to_string(),
            confidence: 1.0,
            source: "public".to_string(),
        },
        AddressLabel {
            address: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".to_string(),
            label: "Uniswap V2 Router".to_string(),
            category: "protocol".to_string(),
            confidence: 1.0,
            source: "public".to_string(),
        },
        AddressLabel {
            address: "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string(),
            label: "Uniswap V3 Router".to_string(),
            category: "protocol".to_string(),
            confidence: 1.0,
            source: "public".to_string(),
        },
        AddressLabel {
            address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
            label: "WETH".to_string(),
            category: "token".to_string(),
            confidence: 1.0,
            source: "public".to_string(),
        },
        AddressLabel {
            address: "0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9".to_string(),
            label: "AAVE Token".to_string(),
            category: "token".to_string(),
            confidence: 1.0,
            source: "public".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_labels_not_empty() {
        let labels = get_known_labels();
        assert!(!labels.is_empty());
    }

    #[test]
    fn label_serialization() {
        let label = AddressLabel {
            address: "0x1234".to_string(),
            label: "Test".to_string(),
            category: "test".to_string(),
            confidence: 0.9,
            source: "test".to_string(),
        };
        let json = serde_json::to_string(&label).unwrap();
        assert!(json.contains("Test"));
    }
}
