use alloy_primitives::U256;
use sqlx::PgPool;
use sqlx::Row;

use super::{ActionType, TransactionAction, TransactionExplanation};

pub async fn explain_transaction(
    pool: &PgPool,
    tx_hash: &str,
) -> Result<TransactionExplanation, sqlx::Error> {
    let hash_bytes =
        hex::decode(tx_hash.trim_start_matches("0x")).map_err(|_| sqlx::Error::RowNotFound)?;

    let tx_row = sqlx::query(
        "SELECT t.hash, t.block_number, t.from_addr, t.to_addr, t.value::text, t.gas_used,
                t.gas_price::text, t.effective_gas_price::text, t.status, t.input,
                b.timestamp::text
         FROM transactions t
         JOIN blocks b ON b.number = t.block_number
         WHERE t.hash = $1",
    )
    .bind(&hash_bytes)
    .fetch_optional(pool)
    .await?;

    let tx = match tx_row {
        Some(r) => r,
        None => return Err(sqlx::Error::RowNotFound),
    };

    let block_number: i64 = tx.get("block_number");
    let from_bytes: Vec<u8> = tx.get("from_addr");
    let to_bytes: Option<Vec<u8>> = tx.get("to_addr");
    let value_str: String = tx.get("value");
    let gas_used: i64 = tx.get("gas_used");
    let gas_price: Option<String> = tx.get("gas_price");
    let effective_gas_price: Option<String> = tx.get("effective_gas_price");
    let status: Option<i16> = tx.get("status");
    let input: Vec<u8> = tx.get("input");
    let timestamp: String = tx.get("timestamp");

    let from_addr = format!("0x{}", hex::encode(&from_bytes));
    let to_addr = to_bytes.as_ref().map(|b| format!("0x{}", hex::encode(b)));

    let value_wei = value_str.clone();
    let value_eth = wei_to_eth(&value_str);

    let status_str = match status {
        Some(1) => "success".to_string(),
        Some(0) => "failed".to_string(),
        _ => "unknown".to_string(),
    };

    let _gas_price_gwei = gas_price
        .as_ref()
        .map(|g| wei_to_gwei(g))
        .unwrap_or_default();

    let tx_fee = calculate_tx_fee(
        gas_used,
        effective_gas_price.as_deref().or(gas_price.as_deref()),
    );

    let mut actions = Vec::new();
    let mut contracts_involved = Vec::new();

    if to_bytes.is_none() && !input.is_empty() {
        actions.push(TransactionAction {
            action_type: ActionType::ContractCreation,
            from: from_addr.clone(),
            to: None,
            value: Some(value_eth.clone()),
            token_address: None,
            token_symbol: None,
            amount: None,
            spender: None,
            description: "Deployed a new contract".to_string(),
        });
    } else if input.len() >= 4 {
        let selector = &input[..4];
        let selector_hex = hex::encode(selector);

        match selector_hex.as_str() {
            "a9059cbb" => {
                if input.len() >= 68 {
                    let to_offset = 16;
                    let to_bytes_slice = &input[to_offset..to_offset + 32];
                    let to_addr_decoded = format!("0x{}", hex::encode(&to_bytes_slice[12..]));

                    let amount_offset = 36;
                    let amount_bytes = &input[amount_offset..amount_offset + 32];
                    let amount = U256::from_be_slice(amount_bytes);

                    actions.push(TransactionAction {
                        action_type: ActionType::Erc20Transfer,
                        from: from_addr.clone(),
                        to: Some(to_addr_decoded),
                        value: None,
                        token_address: to_addr.clone(),
                        token_symbol: None,
                        amount: Some(amount.to_string()),
                        spender: None,
                        description: format!("Transferred {} tokens", amount),
                    });
                }
            }
            "095ea7b3" => {
                if input.len() >= 68 {
                    let spender_offset = 16;
                    let spender_bytes = &input[spender_offset..spender_offset + 32];
                    let spender_addr = format!("0x{}", hex::encode(&spender_bytes[12..]));

                    actions.push(TransactionAction {
                        action_type: ActionType::Erc20Approval,
                        from: from_addr.clone(),
                        to: to_addr.clone(),
                        value: None,
                        token_address: to_addr.clone(),
                        token_symbol: None,
                        amount: None,
                        spender: Some(spender_addr),
                        description: "Approved token spending".to_string(),
                    });
                }
            }
            "23b872dd" => {
                if input.len() >= 100 {
                    let from_offset = 16;
                    let from_bytes_input = &input[from_offset..from_offset + 32];
                    let from_addr_decoded = format!("0x{}", hex::encode(&from_bytes_input[12..]));

                    let to_offset = 48;
                    let to_bytes_input = &input[to_offset..to_offset + 32];
                    let to_addr_decoded = format!("0x{}", hex::encode(&to_bytes_input[12..]));

                    let token_id_offset = 80;
                    let token_id_bytes = &input[token_id_offset..token_id_offset + 32];
                    let token_id = U256::from_be_slice(token_id_bytes);

                    actions.push(TransactionAction {
                        action_type: ActionType::Erc721Transfer,
                        from: from_addr_decoded,
                        to: Some(to_addr_decoded),
                        value: None,
                        token_address: to_addr.clone(),
                        token_symbol: None,
                        amount: Some(token_id.to_string()),
                        spender: None,
                        description: format!("Transferred NFT #{}", token_id),
                    });
                }
            }
            _ => {
                if value_str != "0" {
                    actions.push(TransactionAction {
                        action_type: ActionType::EthTransfer,
                        from: from_addr.clone(),
                        to: to_addr.clone(),
                        value: Some(value_eth.clone()),
                        token_address: None,
                        token_symbol: None,
                        amount: None,
                        spender: None,
                        description: format!("Transferred {} ETH", value_eth),
                    });
                } else {
                    actions.push(TransactionAction {
                        action_type: ActionType::ContractCall,
                        from: from_addr.clone(),
                        to: to_addr.clone(),
                        value: None,
                        token_address: None,
                        token_symbol: None,
                        amount: None,
                        spender: None,
                        description: "Called contract function".to_string(),
                    });
                }
            }
        }
    } else if value_str != "0" {
        actions.push(TransactionAction {
            action_type: ActionType::EthTransfer,
            from: from_addr.clone(),
            to: to_addr.clone(),
            value: Some(value_eth.clone()),
            token_address: None,
            token_symbol: None,
            amount: None,
            spender: None,
            description: format!("Transferred {} ETH", value_eth),
        });
    }

    let log_rows =
        sqlx::query("SELECT address, topic0 FROM logs WHERE block_number = $1 AND tx_hash = $2")
            .bind(block_number)
            .bind(&hash_bytes)
            .fetch_all(pool)
            .await?;

    for log in &log_rows {
        let addr_bytes: Vec<u8> = log.get("address");
        let addr_hex = format!("0x{}", hex::encode(&addr_bytes));
        if !contracts_involved.contains(&addr_hex) {
            contracts_involved.push(addr_hex);
        }
    }

    let summary = build_summary(&actions, &from_addr, &to_addr, &value_eth);

    Ok(TransactionExplanation {
        tx_hash: tx_hash.to_string(),
        block_number,
        timestamp,
        from: from_addr,
        to: to_addr,
        value_wei,
        value_eth,
        gas_used,
        gas_price,
        effective_gas_price,
        tx_fee,
        status: status_str,
        actions,
        contracts_involved,
        summary,
    })
}

fn wei_to_eth(wei: &str) -> String {
    let wei_val: u128 = wei.parse().unwrap_or(0);
    let eth = wei_val as f64 / 1e18;
    format!("{:.6}", eth)
}

fn wei_to_gwei(wei: &str) -> String {
    let wei_val: u128 = wei.parse().unwrap_or(0);
    let gwei = wei_val as f64 / 1e9;
    format!("{:.2}", gwei)
}

fn calculate_tx_fee(gas_used: i64, gas_price: Option<&str>) -> String {
    let price = gas_price.and_then(|g| g.parse::<u128>().ok()).unwrap_or(0);
    let gas_used_u128 = gas_used.unsigned_abs() as u128;
    let fee_wei = gas_used_u128 * price;
    let fee_eth = fee_wei as f64 / 1e18;
    format!("{:.6}", fee_eth)
}

fn build_summary(
    actions: &[TransactionAction],
    from: &str,
    to: &Option<String>,
    value_eth: &str,
) -> String {
    if actions.is_empty() {
        return format!("Transaction from {}", &from[..10]);
    }

    let primary = &actions[0];
    match primary.action_type {
        ActionType::EthTransfer => {
            format!(
                "Transferred {} ETH from {} to {}",
                value_eth,
                &from[..10],
                to.as_ref().map(|t| &t[..10]).unwrap_or("contract creation")
            )
        }
        ActionType::Erc20Transfer => {
            format!(
                "Transferred {} tokens from {} to {}",
                primary.amount.as_deref().unwrap_or("unknown"),
                &from[..10],
                primary.to.as_ref().map(|t| &t[..10]).unwrap_or("unknown")
            )
        }
        ActionType::Erc20Approval => {
            format!(
                "Approved {} to spend tokens from {}",
                primary
                    .spender
                    .as_ref()
                    .map(|s| &s[..10])
                    .unwrap_or("unknown"),
                &from[..10]
            )
        }
        ActionType::Erc721Transfer => {
            format!(
                "Transferred NFT #{} from {} to {}",
                primary.amount.as_deref().unwrap_or("unknown"),
                &from[..10],
                primary.to.as_ref().map(|t| &t[..10]).unwrap_or("unknown")
            )
        }
        ActionType::ContractCreation => {
            format!("Deployed new contract from {}", &from[..10])
        }
        ActionType::ContractCall => {
            format!(
                "Called contract at {}",
                to.as_ref().map(|t| &t[..10]).unwrap_or("unknown")
            )
        }
        _ => format!("Transaction from {}", &from[..10]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wei_to_eth_conversion() {
        assert_eq!(wei_to_eth("1000000000000000000"), "1.000000");
        assert_eq!(wei_to_eth("500000000000000000"), "0.500000");
        assert_eq!(wei_to_eth("0"), "0.000000");
    }

    #[test]
    fn wei_to_gwei_conversion() {
        assert_eq!(wei_to_gwei("1000000000"), "1.00");
        assert_eq!(wei_to_gwei("20000000000"), "20.00");
    }

    #[test]
    fn tx_fee_calculation() {
        assert_eq!(calculate_tx_fee(21000, Some("1000000000")), "0.000021");
        assert_eq!(calculate_tx_fee(21000, None), "0.000000");
    }

    #[test]
    fn action_type_serialization() {
        let action = TransactionAction {
            action_type: ActionType::Erc20Transfer,
            from: "0x1234".to_string(),
            to: Some("0x5678".to_string()),
            value: None,
            token_address: None,
            token_symbol: None,
            amount: Some("1000".to_string()),
            spender: None,
            description: "test".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("erc20_transfer"));
    }
}
