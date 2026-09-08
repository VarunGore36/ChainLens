use chainlens::decode::block::decode_block;
use chainlens::domain::TokenStandard;
use chainlens::rpc::types::{BlockResponse, ReceiptResponse};

use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    block: BlockResponse,
    receipts: Vec<ReceiptResponse>,
}

fn load_fixture(name: &str) -> Fixture {
    let path = format!("benches/fixtures/{name}.json");
    let data =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    serde_json::from_str(&data).unwrap_or_else(|e| panic!("failed to parse {path}: {e}"))
}

#[test]
fn empty_block_decodes_cleanly() {
    let f = load_fixture("empty_block");
    let result = decode_block(&f.block, &f.receipts).unwrap();
    assert_eq!(result.block.number, 1);
    assert!(result.transactions.is_empty());
    assert!(result.logs.is_empty());
    assert!(result.token_transfers.is_empty());
}

#[test]
fn legacy_transaction_decodes() {
    let f = load_fixture("legacy_tx");
    let result = decode_block(&f.block, &f.receipts).unwrap();
    assert_eq!(result.block.number, 0x10);
    assert_eq!(result.transactions.len(), 1);

    let tx = &result.transactions[0];
    assert_eq!(tx.tx_type, 0);
    assert!(tx.gas_price.is_some());
    assert!(tx.max_fee_per_gas.is_none());
    assert_eq!(tx.status, Some(1));
}

#[test]
fn eip1559_with_erc20_transfer_decodes() {
    let f = load_fixture("eip1559_erc20_transfer");
    let result = decode_block(&f.block, &f.receipts).unwrap();
    assert_eq!(result.block.number, 0x20);
    assert_eq!(result.transactions.len(), 1);

    let tx = &result.transactions[0];
    assert_eq!(tx.tx_type, 2);
    assert!(tx.gas_price.is_none());
    assert!(tx.max_fee_per_gas.is_some());

    assert_eq!(result.logs.len(), 1);
    assert_eq!(result.token_transfers.len(), 1);
    let transfer = &result.token_transfers[0];
    assert_eq!(transfer.standard, TokenStandard::Erc20);
    assert_eq!(transfer.value.to_string(), "1000");
}

#[test]
fn erc721_transfer_decodes() {
    let f = load_fixture("erc721_transfer");
    let result = decode_block(&f.block, &f.receipts).unwrap();
    assert_eq!(result.block.number, 0x30);
    assert_eq!(result.token_transfers.len(), 1);

    let transfer = &result.token_transfers[0];
    assert_eq!(transfer.standard, TokenStandard::Erc721);
    assert_eq!(transfer.token_id.to_string(), "12345");
    assert_eq!(transfer.value.to_string(), "0");
}

#[test]
fn contract_creation_decodes() {
    let f = load_fixture("contract_creation");
    let result = decode_block(&f.block, &f.receipts).unwrap();
    assert_eq!(result.block.number, 0x40);
    assert_eq!(result.transactions.len(), 1);

    let tx = &result.transactions[0];
    assert!(tx.to.is_none());
    assert!(tx.contract_address.is_some());
}

#[test]
fn multi_transaction_block_decodes() {
    let f = load_fixture("multi_tx_with_logs");
    let result = decode_block(&f.block, &f.receipts).unwrap();
    assert_eq!(result.block.number, 0x50);
    assert_eq!(result.transactions.len(), 2);
    assert_eq!(result.logs.len(), 2);

    assert_eq!(result.token_transfers.len(), 1);
    assert_eq!(result.token_transfers[0].standard, TokenStandard::Erc20);
}

#[test]
fn all_fixtures_decode_without_panic() {
    let fixtures = [
        "empty_block",
        "legacy_tx",
        "eip1559_erc20_transfer",
        "erc721_transfer",
        "contract_creation",
        "multi_tx_with_logs",
    ];

    for name in &fixtures {
        let f = load_fixture(name);
        let _ = decode_block(&f.block, &f.receipts);
    }
}
