use chainlens::decode::block::decode_block;
use chainlens::rpc::types::{BlockResponse, ReceiptResponse};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
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

fn bench_decode_empty_block(c: &mut Criterion) {
    let f = load_fixture("empty_block");
    c.bench_function("decode_empty_block", |b| {
        b.iter(|| decode_block(black_box(&f.block), black_box(&f.receipts)))
    });
}

fn bench_decode_legacy_tx(c: &mut Criterion) {
    let f = load_fixture("legacy_tx");
    c.bench_function("decode_legacy_tx", |b| {
        b.iter(|| decode_block(black_box(&f.block), black_box(&f.receipts)))
    });
}

fn bench_decode_eip1559_with_erc20(c: &mut Criterion) {
    let f = load_fixture("eip1559_erc20_transfer");
    c.bench_function("decode_eip1559_with_erc20", |b| {
        b.iter(|| decode_block(black_box(&f.block), black_box(&f.receipts)))
    });
}

fn bench_decode_erc721(c: &mut Criterion) {
    let f = load_fixture("erc721_transfer");
    c.bench_function("decode_erc721", |b| {
        b.iter(|| decode_block(black_box(&f.block), black_box(&f.receipts)))
    });
}

fn bench_decode_contract_creation(c: &mut Criterion) {
    let f = load_fixture("contract_creation");
    c.bench_function("decode_contract_creation", |b| {
        b.iter(|| decode_block(black_box(&f.block), black_box(&f.receipts)))
    });
}

fn bench_decode_multi_tx(c: &mut Criterion) {
    let f = load_fixture("multi_tx_with_logs");
    c.bench_function("decode_multi_tx_with_logs", |b| {
        b.iter(|| decode_block(black_box(&f.block), black_box(&f.receipts)))
    });
}

criterion_group!(
    benches,
    bench_decode_empty_block,
    bench_decode_legacy_tx,
    bench_decode_eip1559_with_erc20,
    bench_decode_erc721,
    bench_decode_contract_creation,
    bench_decode_multi_tx,
);
criterion_main!(benches);
