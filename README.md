# ChainLens

An Ethereum blockchain indexer with reorg-safe, crash-resumable ingestion.

## What it does

ChainLens continuously follows the Ethereum mainnet chain head while also backfilling historical ranges. For every block it stores the header, all transactions with their receipt data, and every event log — with ERC-20 and ERC-721 `Transfer` events additionally decoded into typed tables.

It tracks the canonical chain by verifying parent-hash linkage on every commit. When the chain reorganizes, it identifies the common ancestor, rolls back the orphaned range atomically, and re-indexes the new canonical branch. Kill the process at any point and it resumes without gaps, duplicates, or manual repair.

## Project status

**All 12 phases complete.** See [docs/RESULTS.md](docs/RESULTS.md) for detailed results.

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Foundation and scaffolding | Complete |
| 2 | RPC client (retry, rate limiting, capability probe) | Complete |
| 3 | Domain model, validation, ERC-20/721 decoding | Complete |
| 4 | Schema and single-transaction commit | Complete |
| 5 | Sequential pipeline end to end | Complete |
| 6 | Mock RPC harness and reorg handling | Complete |
| 7 | Crash recovery hardening | Complete |
| 8 | Observability (Prometheus metrics) | Complete |
| 9 | Concurrency (worker pool, reorder buffer) | Complete |
| 10 | Query API (axum) | Complete |
| 11 | Benchmarking | Complete |
| 12 | Documentation and polish | Complete |

## Benchmark results

### Pipeline throughput (PostgreSQL 17, mock RPC)

| Experiment | blocks/sec |
|------------|------------|
| Sequential (1 worker) | **392** |
| Concurrent (2 workers) | 382 |
| Concurrent (4 workers) | 376 |
| Decode only (no DB) | **1,700,000** |

### Decode benchmarks (criterion)

| Benchmark | Time (ns) |
|-----------|-----------|
| `decode_empty_block` | 28.3 |
| `decode_legacy_tx` | 67.2 |
| `decode_eip1559_with_erc20` | 150.6 |
| `decode_erc721` | 125.6 |
| `decode_contract_creation` | 65.1 |
| `decode_multi_tx_with_logs` | 241.5 |

### Test results

| Metric | Value |
|--------|-------|
| Total tests | 63 |
| Unit tests | 56 |
| Integration tests | 7 |
| Failures | 0 |
| Clippy warnings | 0 |

## Quick start

```bash
# 1. Clone
git clone https://github.com/VarunGore36/ChainLens.git
cd ChainLens

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
| `GET /address/{address}/transactions?limit&before_block` | Address history (keyset pagination) |
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
| `rpc_timeout_secs` | `CHAINLENS_RPC_TIMEOUT_SECS` | 30 | Request timeout |
| `rpc_max_retries` | `CHAINLENS_RPC_MAX_RETRIES` | 3 | Retries for transient failures |
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

```
  Ethereum JSON-RPC
         │
         ▼
  ┌─────────────┐   ┌─────────────┐
  │ Head Watcher│──▶│  Scheduler  │
  └─────────────┘   └──────┬──────┘
                           │
                    bounded channel ◀── backpressure
                           │
         ┌─────────────────┴─────────────────┐
         │      Fetch + Decode Workers       │
         └─────────────────┬─────────────────┘
                           │
                    bounded channel ◀── backpressure
                           │
                    ┌──────▼──────┐
                    │  Sequencer  │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐      ┌───────────────┐
                    │  Committer  │─────▶│ Reorg Handler │
                    └──────┬──────┘      └──────┬──────┘
                           │                     │
                           ▼   one transaction   ▼
                    ┌──────────────────────────────────┐
                    │           PostgreSQL             │
                    └──────────────────────────────────┘
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for data flow, invariants, and schema rationale.

## Design decisions

See [docs/DESIGN_DECISIONS.md](docs/DESIGN_DECISIONS.md) for each significant choice with its rejected alternative. Key decisions:

- **Single-writer committer** — bottleneck is RPC round-trips, not DB
- **Cursor in same transaction as data** — crash recovery is a single SELECT
- **Hard-delete on reorg** — clean read paths, audit table preserves history
- **Natural primary keys** — idempotent writes, no sequences
- **Bounded channels** — structural backpressure
- **No message broker** — in-process channels + PostgreSQL durability

## Benchmarks

See [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for methodology, results, and analysis.

```bash
# Run all benchmarks
cargo run --release --bin benchmark_harness -- all

# Run specific experiment
cargo run --release --bin benchmark_harness -- sequential
cargo run --release --bin benchmark_harness -- concurrent_scaling
cargo run --release --bin benchmark_harness -- decode_only
```

## Observability

Prometheus metrics at `/metrics`:

| Metric | Type | What it shows |
|--------|------|---------------|
| `chainlens_blocks_indexed_total` | counter | blocks/sec |
| `chainlens_transactions_indexed_total` | counter | tx/sec |
| `chainlens_rpc_request_duration_seconds` | histogram | RPC latency |
| `chainlens_db_commit_duration_seconds` | histogram | DB commit latency |
| `chainlens_indexing_lag_blocks` | gauge | lag behind head |
| `chainlens_reorgs_total` | counter | reorg count |

```bash
# Start Prometheus + Grafana
docker compose up -d
# Grafana: http://localhost:3001 (admin/chainlens)
# Prometheus: http://localhost:9090
```

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
ChainLens/
├── src/                    Rust source code
│   ├── api/                Query API (axum)
│   ├── bin/                Binaries (chainlens, api, benchmark_harness)
│   ├── decode/             Block/transaction/log decoding
│   ├── domain/             Domain types
│   ├── pipeline/           Head watcher, scheduler, workers, sequencer, committer
│   ├── reorg/              Reorg detection and rollback
│   ├── rpc/                JSON-RPC client
│   ├── store/              PostgreSQL storage
│   └── test_helpers/       Mock RPC for testing
├── migrations/             Database schema
├── tests/                  Integration tests
├── benches/                Criterion benchmarks + fixtures
├── docs/                   Design decisions, architecture, benchmarks
├── scripts/                Invariant checker, benchmark runner
├── docker/                 Prometheus config, Grafana dashboard
├── docker-compose.yml      PostgreSQL + Prometheus + Grafana
└── .github/                CI workflows
```

## License

Apache License 2.0