# ChainLens

**Ethereum blockchain indexer with reorg-safe, crash-resumable ingestion.**

[![Rust](https://img.shields.io/badge/Rust-1.85+-dea584?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/Tests-93_passing-22c55e)]()
[![Clippy](https://img.shields.io/badge/Clippy-clean-22c55e)]()
[![Website](https://img.shields.io/badge/Website-live-627eea)](https://chain-lens-chi.vercel.app/)

**Live site:** [chain-lens-chi.vercel.app](https://chain-lens-chi.vercel.app/)

ChainLens follows the Ethereum chain head while backfilling historical blocks. It stores headers, transactions, receipts, and event logs — with ERC-20/721 `Transfer` events decoded into typed tables. Reorganizations are handled atomically: identify common ancestor, rollback orphaned blocks, re-index canonical branch. Crash at any point and it resumes without gaps or duplicates.

## Performance

| Metric | Value |
|--------|-------|
| Pipeline throughput | **392 blocks/sec** (PostgreSQL 17) |
| Decode throughput | **1,700,000 blocks/sec** (pure CPU) |
| Decode latency | **150 ns/block** (ERC-20 transfers) |
| Tests | **93 passing**, 0 failures, 0 clippy warnings |

## Quick start

```bash
git clone https://github.com/VarunGore36/ChainLens.git && cd ChainLens
cp .env.example .env                    # add your RPC endpoint
docker compose up -d                    # PostgreSQL
cargo run --bin chainlens               # indexer
cargo run --bin api                     # API at http://127.0.0.1:8080
```

## Architecture

```
Ethereum RPC → Head Watcher → Scheduler → Workers (N) → Sequencer → Committer → PostgreSQL
                                                      ↑                     │
                                                      └── Reorg Handler ←──┘
```

**Key design choices:**
- **Single-writer committer** — bottleneck is RPC round-trips, not DB
- **Cursor in same transaction as data** — crash recovery = single SELECT
- **Natural primary keys** — idempotent writes, no sequences
- **Hard-delete on reorg** — clean read paths, audit table preserves history
- **Bounded channels** — structural backpressure, no OOM

## API

| Endpoint | Description |
|----------|-------------|
| `GET /block/{number_or_hash}` | Block by number or hash |
| `GET /transaction/{hash}` | Transaction by hash |
| `GET /address/{address}/transactions` | Address history (keyset pagination) |
| `GET /contract/{address}/events` | Contract events by topic |
| `GET /health` | Health check |
| `GET /status` | Cursor, finalized, lag |

## Configuration

All configuration via environment variables:

| Env var | Default | Description |
|---------|---------|-------------|
| `CHAINLENS_RPC_URL` | — | Ethereum JSON-RPC endpoint |
| `DATABASE_URL` | — | PostgreSQL connection string |
| `CHAINLENS_WORKER_COUNT` | 4 | Concurrent fetch workers |
| `CHAINLENS_FETCH_QUEUE_DEPTH` | 32 | Bounded channel depth |
| `CHAINLENS_MAX_REORG_DEPTH` | 128 | Max reorg depth before halt |
| `CHAINLENS_RPC_RATE_LIMIT` | 10 | Max RPC requests/sec |
| `CHAINLENS_HEAD_POLL_SECS` | 4 | Chain head poll interval |

## Tests

```bash
cargo test --locked                              # 93 tests
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
cargo bench --bench decode -- --quick            # decode benchmarks
```

**Coverage:** config (18) · decode (25) · rpc (23) · store (7) · integration (7) · crash recovery (5) · other (8)

## Observability

Prometheus metrics at `/metrics`:

| Metric | Type |
|--------|------|
| `chainlens_blocks_indexed_total` | counter |
| `chainlens_transactions_indexed_total` | counter |
| `chainlens_rpc_request_duration_seconds` | histogram |
| `chainlens_db_commit_duration_seconds` | histogram |
| `chainlens_indexing_lag_blocks` | gauge |
| `chainlens_reorgs_total` | counter |

Grafana dashboard included: `docker compose up -d` → http://localhost:3001

## Project structure

```
src/
├── api/           Query API (axum)
├── decode/        Block, transaction, log, ERC-20/721 decoding
├── domain/        Domain types (Block, Transaction, Log, TokenTransfer)
├── pipeline/      Head watcher, scheduler, workers, sequencer, committer
├── reorg/         Reorg detection and rollback
├── rpc/           JSON-RPC client with retry, rate limiting
├── store/         PostgreSQL storage
└── bin/           Binaries: chainlens, api, benchmark_harness
```

## Docs

- [Architecture](docs/ARCHITECTURE.md) — data flow, invariants, schema rationale
- [Design Decisions](docs/DESIGN_DECISIONS.md) — each choice with rejected alternative
- [Benchmarks](docs/BENCHMARKS.md) — methodology, results, analysis
- [Results](docs/RESULTS.md) — test results and project status

## License

[Apache License 2.0](LICENSE)
