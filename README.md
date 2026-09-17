<![CDATA[<div align="center">

# ⟠ ChainLens

### Ethereum Blockchain Indexer

**Reorg-safe • Crash-resumable • 392 blocks/sec**

[![Rust](https://img.shields.io/badge/Rust-2024-edition?logo=rust&color=dea584)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/Tests-93%20passed-22c55e)]()
[![Clippy](https://img.shields.io/badge/Clippy-0%20warnings-22c55e)]()
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-17-4169e1)](https://www.postgresql.org/)

[Quick Start](#quick-start) • [Benchmarks](#benchmarks) • [Architecture](#architecture) • [API](#api) • [Tests](#tests)

</div>

---

## What it does

ChainLens continuously follows the Ethereum mainnet chain head while backfilling historical ranges. For every block it stores the header, all transactions with receipt data, and every event log — with ERC-20 and ERC-721 `Transfer` events decoded into typed tables.

It tracks the canonical chain by verifying parent-hash linkage on every commit. When the chain reorganizes, it identifies the common ancestor, rolls back the orphaned range atomically, and re-indexes the new canonical branch. **Kill the process at any point and it resumes without gaps, duplicates, or manual repair.**

<div align="center">

| Metric | Value |
|:-------|:-----:|
| **Pipeline throughput** | 392 blocks/sec |
| **Decode throughput** | 1,700,000 blocks/sec |
| **Decode latency** | 150 ns/block (ERC-20) |
| **Tests** | 93 passing, 0 failures |
| **Clippy warnings** | 0 |

</div>

---

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

---

## Benchmarks

### Pipeline throughput (PostgreSQL 17, mock RPC)

```
Workers    blocks/sec    Speedup
─────────────────────────────────
   1          392         1.0x
   2          382         1.0x
   4          376         1.0x
   8          208         0.55x
  16           60         0.16x
```

> Bottleneck is the DB commit path (~2.5ms/block), not fetch workers.

### Decode latency (criterion)

```
Benchmark                    Time (ns)
──────────────────────────────────────
decode_empty_block              28
decode_legacy_tx                67
decode_contract_creation        65
decode_erc721                  126
decode_eip1559_with_erc20      151
decode_multi_tx_with_logs      242
```

### Decode-only throughput

```
1,700,000 blocks/sec — Mock RPC, zero latency, no DB writes
```

---

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

### Key invariants

| Invariant | Why |
|:----------|:----|
| Cursor in same transaction as data | Crash recovery = single SELECT |
| Natural primary keys | Idempotent writes, no sequences |
| Hard-delete on reorg | Clean read paths, audit table preserves history |
| Bounded channels | Structural backpressure, no OOM |

---

## API

| Endpoint | Description |
|:---------|:------------|
| `GET /block/{number_or_hash}` | Block by number or hash |
| `GET /transaction/{hash}` | Transaction by hash |
| `GET /address/{address}/transactions` | Address history (keyset pagination) |
| `GET /contract/{address}/events` | Contract events by topic |
| `GET /health` | Health check |
| `GET /status` | Cursor, finalized, lag |

---

## Configuration

| Setting | Env var | Default | Purpose |
|:--------|:--------|:--------|:--------|
| `rpc_url` | `CHAINLENS_RPC_URL` | — | Ethereum JSON-RPC endpoint |
| `database_url` | `DATABASE_URL` | — | PostgreSQL connection string |
| `worker_count` | `CHAINLENS_WORKER_COUNT` | 4 | Concurrent fetch workers |
| `fetch_queue_depth` | `CHAINLENS_FETCH_QUEUE_DEPTH` | 32 | Bounded channel depth |
| `max_reorg_depth` | `CHAINLENS_MAX_REORG_DEPTH` | 128 | Max reorg depth before halt |
| `rpc_rate_limit` | `CHAINLENS_RPC_RATE_LIMIT` | 10 | Max RPC requests/sec |
| `head_poll_secs` | `CHAINLENS_HEAD_POLL_SECS` | 4 | Chain head poll interval |

---

## Tests

```bash
# Unit + integration tests (93 tests)
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

### Test coverage

| Module | Tests | What's tested |
|:-------|:-----:|:--------------|
| `config` | 18 | URL redaction, validation, boundary conditions |
| `decode/block` | 11 | Empty block, EIP-1559, receipt mismatch, status overflow |
| `decode/erc20` | 14 | ERC-20, ERC-721, edge cases, max U256, zero values |
| `rpc` | 23 | Retry, rate limiting, serialization, error classification |
| `store` | 7 | U256 conversion, boundary values |
| `domain` | 2 | Topic constants match keccak256 |
| `telemetry` | 2 | Filter parsing |
| `shutdown` | 1 | Token registration |
| `db` | 1 | Error formatting |
| `integration` | 7 | All fixture files decode correctly |
| `crash recovery` | 5 | PostgreSQL required, marked `ignore` |

---

## Observability

Prometheus metrics at `/metrics`:

| Metric | Type | What it shows |
|:-------|:-----|:--------------|
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

---

## Security posture

- Query API is read-only, exposes no mutating routes, binds to localhost
- Credentials, RPC endpoints, and database URLs via environment variables only
- `RedactedUrl` prevents credentials from reaching log output (compiler-enforced)
- No private keys, no transactions signed

---

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

---

## License

[Apache License 2.0](LICENSE)

---

<div align="center">

**Built with Rust, PostgreSQL, and Ethereum.**

</div>
]]>