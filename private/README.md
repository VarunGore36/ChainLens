# ChainLens

An Ethereum blockchain indexer with reorg-safe, crash-resumable ingestion.

> **This is a private repository.** The core implementation, internal tooling, and experimental code live here.
>
> **Public results and dashboard:** [ChainLens-Results](https://github.com/VarunGore36/ChainLens-Results)

## What it does

ChainLens continuously follows the Ethereum mainnet chain head while also backfilling historical ranges. For every block it stores the header, all transactions with their receipt data, and every event log — with ERC-20 and ERC-721 `Transfer` events additionally decoded into typed tables.

It tracks the canonical chain by verifying parent-hash linkage on every commit. When the chain reorganizes, it identifies the common ancestor, rolls back the orphaned range atomically, and re-indexes the new canonical branch. Kill the process at any point and it resumes without gaps, duplicates, or manual repair.

## Quick start

```bash
# 1. Clone
git clone https://github.com/VarunGore36/ChainLens.git
cd ChainLens/private

# 2. Configure
cp .env.example .env
# Edit .env — add your RPC endpoint (Alchemy/Infura/QuickNode)

# 3. Start PostgreSQL
docker compose up -d

# 4. Run the indexer
cargo run --bin chainlens

# 5. Run the API (separate terminal)
cargo run --bin api
# API at http://127.0.0.1:8080
```

## API endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /block/{number_or_hash}` | Block by number or hash |
| `GET /transaction/{hash}` | Transaction by hash |
| `GET /address/{address}/transactions?limit&before_block` | Address history |
| `GET /contract/{address}/events?topic0&from_block&to_block&limit` | Contract events |
| `GET /health` | Health check |
| `GET /status` | Cursor, finalized, lag |

## Configuration

All configuration via environment variables or CLI flags:

| Setting | Env var | Default | Purpose |
|---------|---------|---------|---------|
| `rpc_url` | `CHAINLENS_RPC_URL` | — | Ethereum JSON-RPC endpoint |
| `database_url` | `DATABASE_URL` | — | PostgreSQL connection string |
| `backfill_from` | `CHAINLENS_BACKFILL_FROM` | 0 | Starting block for backfill |
| `worker_count` | `CHAINLENS_WORKER_COUNT` | 4 | Concurrent fetch workers |
| `fetch_queue_depth` | `CHAINLENS_FETCH_QUEUE_DEPTH` | 32 | Bounded channel depth |
| `max_reorg_depth` | `CHAINLENS_MAX_REORG_DEPTH` | 128 | Max reorg depth before halt |
| `rpc_rate_limit` | `CHAINLENS_RPC_RATE_LIMIT` | 10 | Max RPC requests/sec |
| `head_poll_secs` | `CHAINLENS_HEAD_POLL_SECS` | 4 | Chain head poll interval |

## Tests

```bash
# Unit + integration tests (63 tests)
cargo test --locked

# Clippy
cargo clippy --locked --all-targets -- -D warnings

# Format check
cargo fmt --all -- --check

# Decode benchmarks
cargo bench --bench decode -- --quick

# Pipeline benchmarks (requires PostgreSQL)
cargo run --release --bin benchmark_harness -- all
```

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for data flow, invariants, and schema rationale.

## Design decisions

See [docs/DESIGN_DECISIONS.md](docs/DESIGN_DECISIONS.md) for each significant choice with its rejected alternative.

## Benchmarks

See [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for methodology, results, and analysis.

## Security posture

- The query API is read-only, exposes no mutating routes, and binds to localhost
- Credentials, RPC endpoints, and database URLs are supplied through environment variables only
- `RedactedUrl` prevents credentials from reaching log output (compiler-enforced)
- The project holds no private keys and signs no transactions

## Known limitations

- No backfill from genesis (a configurable recent window is indexed)
- Single chain (Ethereum mainnet)
- No internal EVM call tracing
- The API is not production-hardened (no auth, no rate limiting)

## Repository structure

```
private/
├── src/                    Rust source code
│   ├── api/                Query API (axum)
│   ├── decode/             Block/transaction/log decoding
│   ├── domain/             Domain types
│   ├── pipeline/           Head watcher, scheduler, workers, sequencer, committer
│   ├── reorg/              Reorg detection and rollback
│   ├── rpc/                JSON-RPC client
│   ├── store/              PostgreSQL storage
│   └── test_helpers/       Mock RPC for testing
├── migrations/             Database schema
├── tests/                  Integration tests
├── benches/                Benchmarks and fixtures
├── docs/                   Design decisions, architecture, benchmarks
├── scripts/                Invariant checker, benchmark runner
├── docker-compose.yml      PostgreSQL + Prometheus + Grafana
└── .github/                CI workflows
```

## License

Apache License 2.0
