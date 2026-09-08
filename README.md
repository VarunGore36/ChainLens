# ChainLens

An Ethereum blockchain indexer with reorg-safe, crash-resumable ingestion.

> **This is a private repository.** The core implementation, internal tooling, and experimental code live here.
>
> **Public results and dashboard:** [ChainLens-Results](https://github.com/VarunGore36/ChainLens-Results)

## Repository structure

```
ChainLens/                  (private)
├── src/                    Rust source code
├── migrations/             Database schema
├── tests/                  Integration tests
├── benches/                Benchmarks and fixtures
├── notes/                  Internal engineering spec
├── .github/                CI workflows
└── docker-compose.yml      Local PostgreSQL
```

## What it does

ChainLens continuously follows the Ethereum mainnet chain head while also backfilling historical ranges. For every block it stores the header, all transactions with their receipt data, and every event log — with ERC-20 and ERC-721 `Transfer` events additionally decoded into typed tables.

It tracks the canonical chain by verifying parent-hash linkage on every commit. When the chain reorganizes, it identifies the common ancestor, rolls back the orphaned range atomically, and re-indexes the new canonical branch.

## Quick start

```bash
cp .env.example .env          # add your RPC endpoint
docker compose up -d          # PostgreSQL
cargo run --bin chainlens     # indexer
```

## Tests

```bash
cargo test --locked                              # 63 tests
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
cargo bench --bench decode -- --quick            # decode benchmarks
```

## License

Apache License 2.0
