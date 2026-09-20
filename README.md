# ChainLens

**Ethereum Intelligence & Forensics Engine — indexes, reconstructs, maps relationships, and detects unusual behavior.**

[![Rust](https://img.shields.io/badge/Rust-1.85+-dea584?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/Tests-218_passing-22c55e)]()
[![Clippy](https://img.shields.io/badge/Clippy-clean-22c55e)]()
[![Website](https://img.shields.io/badge/Website-live-627eea)](https://chain-lens-chi.vercel.app/)

**Live site:** [chain-lens-chi.vercel.app](https://chain-lens-chi.vercel.app/)

## What it does

ChainLens indexes Ethereum and turns raw blockchain data into explainable intelligence:

- **Transaction Explanation** — decodes ETH transfers, ERC-20/721/1155 transfers, approvals, DEX swaps (Uniswap V2/V3, SushiSwap, 1inch, Curve, Balancer), WETH wrap/unwrap, Aave/Compound/Lido interactions
- **Address Intelligence** — activity stats, ETH/token volumes, behavioral classification, address labeling
- **Relationship Graph** — bounded BFS traversal mapping address interactions (TRANSFERRED_TO, RECEIVED_FROM, CALLED, APPROVED)
- **Address Clustering** — groups related addresses by interaction patterns, finds deployers and whales
- **Anomaly Detection** — large transfers, activity spikes, new wallet high-value, contract interaction spikes, coordinated activity
- **MEV Detection** — sandwich patterns, arbitrage, priority fee anomalies, repeated protocol interactions
- **Contract Intelligence** — deployment info, unique callers, function selectors, interface detection
- **Event Signature Decoding** — 25+ known events (Transfer, Swap, Mint, Borrow, Liquidation, VoteCast, etc.)
- **Interface Detection** — ERC-20/721/1155, Uniswap V2/V3, WETH, Aave, Compound, Curve, Balancer, Lido, Ownable, Proxy
- **Background Processing** — intelligence computed incrementally as blocks are indexed, not on API calls
- **Real-time Updates** — WebSocket for live block commits, anomalies, MEV, and reorgs
- **Export** — CSV/JSON export for transactions, anomalies, MEV events
- **Historical Trends** — address activity, contract interactions, anomaly trends over time
- **Alerting** — automatic alert creation for large transfers and anomalies

## Performance

| Metric | Value |
|--------|-------|
| Pipeline throughput | **392 blocks/sec** (PostgreSQL 17) |
| Decode throughput | **1,700,000 blocks/sec** (pure CPU) |
| Decode latency | **150 ns/block** (ERC-20 transfers) |
| Tests | **218 passing**, 0 clippy warnings |

## Quick start

```bash
git clone https://github.com/VarunGore36/ChainLens.git && cd ChainLens
cp .env.example .env                    # add your RPC endpoint
docker compose up -d                    # PostgreSQL
cargo run --bin chainlens               # indexer
cargo run --bin api                     # API at http://127.0.0.1:8080
```

## Production deployment

```bash
cp .env.prod.example .env.prod          # fill in production values
docker compose -f docker-compose.prod.yml up -d
```

## API

### Core endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /block/{number_or_hash}` | Block by number or hash |
| `GET /transaction/{hash}` | Transaction by hash |
| `GET /address/{address}/transactions` | Address history (keyset pagination) |
| `GET /contract/{address}/events` | Contract events by topic |
| `GET /health` | Health check |
| `GET /status` | Cursor, finalized, lag |

### Intelligence endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /api/v1/transactions/{hash}/explain` | Decoded actions, summary, participants |
| `GET /api/v1/addresses/{address}/intelligence` | Activity stats, behavior classification |
| `GET /api/v1/addresses/{address}/graph?depth=2&limit=50` | Relationship graph |
| `GET /api/v1/contracts/{address}/intelligence` | Deployment, callers, selectors |
| `GET /api/v1/anomalies` | Detected anomalies with severity/evidence |
| `GET /api/v1/blocks/{number}/analytics` | Gas, priority fees, MEV candidates |
| `GET /api/v1/addresses/{address}/export?format=csv&limit=1000` | Export transactions (JSON/CSV) |
| `GET /api/v1/addresses/{address}/trends?days=30` | Address activity trends |
| `GET /api/v1/contracts/{address}/trends?days=30` | Contract interaction trends |
| `GET /api/v1/anomalies/trends?days=30` | Anomaly trends over time |
| `GET /api/v1/mev/trends?days=30` | MEV activity trends |
| `GET /ws` | WebSocket for real-time updates |
| `GET /api/v1/addresses/{address}/cluster?depth=2` | Address clustering |
| `GET /api/v1/contracts/deployers` | Top contract deployers |
| `GET /api/v1/tokens/{address}/whales` | Token whale addresses |

## Architecture

```
Ethereum RPC → Head Watcher → Scheduler → Workers (N) → Sequencer → Committer → PostgreSQL
                                                      ↑                     │
                                                      └── Reorg Handler ←──┘
                                                                  │
                                                    Background Intelligence Processor
                                                                  │
                                          ┌───────────────────────┼───────────────────────┐
                                          │                       │                       │
                                    Transaction              Address                  Anomaly
                                     Actions                 Graph                   Detection
                                          │                       │                       │
                                    Contract                   MEV                    Events
                                    Intelligence              Analysis               Signatures
```

**Key design choices:**
- **Single-writer committer** — bottleneck is RPC round-trips, not DB
- **Cursor in same transaction as data** — crash recovery = single SELECT
- **Natural primary keys** — idempotent writes, no sequences
- **Hard-delete on reorg** — clean read paths, intelligence data also cleaned
- **Bounded channels** — structural backpressure, no OOM
- **Background processing** — intelligence computed incrementally, not on API calls
- **Deterministic intelligence** — no LLM/API dependency, evidence-backed results

## Decoding coverage

| Standard | Events | Functions |
|----------|--------|-----------|
| ERC-20 | Transfer, Approval | transfer, approve, transferFrom, balanceOf, allowance, totalSupply |
| ERC-721 | Transfer, Approval, ApprovalForAll | safeTransferFrom, ownerOf, getApproved, setApprovalForAll |
| ERC-1155 | TransferSingle, TransferBatch, ApprovalForAll | safeTransferFrom, safeBatchTransferFrom, balanceOf, balanceOfBatch |
| Uniswap V2 | Swap, Sync, PairCreated | swapExactTokensForTokens, swapExactETHForTokens, addLiquidity, removeLiquidity |
| Uniswap V3 | Swap, PoolCreated | exactInputSingle, exactInput, exactOutputSingle, exactOutput |
| WETH | Deposit, Withdrawal | deposit, withdraw |
| Aave | Deposit, Withdraw, Borrow, Repay, Liquidation | deposit, withdraw, borrow, repay |
| Compound | — | mint, redeem, redeemUnderlying |
| Curve | — | exchange, exchange_underlying |
| Balancer | — | swap |
| Lido | — | submit, requestWithdrawal |
| 1inch | — | swap |
| ENS | — | register |
| Governance | DelegateChanged, VoteCast | — |
| Proxy | Upgraded, Initialized | implementation, upgradeTo |

## Tests

```bash
cargo test --locked                              # 218 tests
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
cargo bench --bench decode -- --quick
```

**Coverage:** config (26) · decode (25) · rpc (27) · store (13) · intelligence (64) · integration (7) · crash recovery (5) · other (4)

## Observability

Prometheus metrics at `/metrics`:

| Metric | Type |
|--------|------|
| `chainlens_blocks_indexed_total` | counter |
| `chainlens_transactions_indexed_total` | counter |
| `chainlens_transactions_decoded_total` | counter |
| `chainlens_addresses_analyzed_total` | counter |
| `chainlens_anomalies_detected_total` | counter |
| `chainlens_mev_candidates_detected_total` | counter |
| `chainlens_graphs_generated_total` | counter |
| `chainlens_rpc_request_duration_seconds` | histogram |
| `chainlens_db_commit_duration_seconds` | histogram |
| `chainlens_intelligence_processing_duration_seconds` | histogram |
| `chainlens_indexing_lag_blocks` | gauge |
| `chainlens_reorgs_total` | counter |

Grafana dashboard included: `docker compose up -d` → http://localhost:3001

## Project structure

```
src/
├── api/              Query API (axum)
│   ├── auth.rs       API key authentication
│   ├── cache.rs      Response caching
│   ├── cors.rs       CORS configuration
│   ├── docs.rs       API documentation endpoint
│   ├── logging.rs    Request logging middleware
│   ├── ratelimit.rs  Rate limiting middleware
│   └── websocket.rs  WebSocket for real-time updates
├── decode/           Block, transaction, log, ERC-20/721/1155 decoding
├── domain/           Domain types (Block, Transaction, Log, TokenTransfer)
├── intelligence/     Intelligence engine
│   ├── actions.rs    Transaction action decoding (Uniswap, Curve, Balancer, Lido, etc.)
│   ├── address.rs    Address intelligence
│   ├── alerting.rs   Alert system for anomalies and large transfers
│   ├── anomaly.rs    Anomaly detection
│   ├── background.rs Background processor
│   ├── cache.rs      LRU cache with TTL
│   ├── clustering.rs Address clustering & whale detection
│   ├── contract.rs   Contract intelligence
│   ├── events.rs     Event signature decoding
│   ├── export.rs     CSV/JSON export
│   ├── graph.rs      Relationship graph
│   ├── interfaces.rs Interface detection
│   ├── labels.rs     Address labeling
│   ├── mev.rs        MEV detection
│   ├── persist.rs    DB persistence
│   └── trends.rs     Historical trend queries
├── pipeline/         Head watcher, scheduler, workers, sequencer, committer
├── reorg/            Reorg detection and rollback (intelligence-safe)
├── rpc/              JSON-RPC client with retry, rate limiting
├── store/            PostgreSQL storage
└── bin/              Binaries: chainlens, api, benchmark_harness
```

## Database schema

```
blocks, transactions, logs, token_transfers, address_transactions  ← indexed data
transaction_actions, address_stats, address_relationships           ← intelligence
contract_profiles, anomalies, mev_events, block_analytics           ← intelligence
indexer_state, reorgs                                               ← system
```

All intelligence tables have `ON DELETE CASCADE` to blocks — reorgs clean up automatically.

## Docs

- [Architecture](docs/ARCHITECTURE.md) — data flow, invariants, schema rationale
- [Design Decisions](docs/DESIGN_DECISIONS.md) — each choice with rejected alternative
- [Benchmarks](docs/BENCHMARKS.md) — methodology, results, analysis
- [Results](docs/RESULTS.md) — test results and project status

## License

[Apache License 2.0](LICENSE)
