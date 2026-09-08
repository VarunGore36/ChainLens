# ChainLens

An Ethereum blockchain indexer built in Rust, designed around three properties that indexer implementations commonly skip: **correct handling of chain reorganizations**, **crash safety that is tested rather than assumed**, and **performance that is measured rather than claimed**.

## What it does

ChainLens continuously follows the Ethereum mainnet chain head while also backfilling historical ranges. For every block it stores the header, all transactions with their receipt data, and every event log — with ERC-20 and ERC-721 `Transfer` events additionally decoded into typed tables.

It tracks the canonical chain by verifying parent-hash linkage on every commit. When the chain reorganizes, it identifies the common ancestor, rolls back the orphaned range atomically, and re-indexes the new canonical branch. Kill the process at any point and it resumes without gaps, duplicates, or manual repair.

## Why it matters

Ethereum mainnet produces one block every ~12 seconds. Keeping pace with the head is trivial. The real challenge is **historical backfill**:

| Ingestion rate | 1M blocks | Full history (~26M blocks) |
| --- | --- | --- |
| 10 blocks/sec (naive sequential) | ~28 hours | ~30 days |
| 100 blocks/sec | ~2.8 hours | ~3 days |
| 500 blocks/sec | ~33 minutes | ~14 hours |

ChainLens is built to measure and demonstrate where the bottlenecks actually are, rather than claiming performance without evidence.

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

**Key design decisions:**
- Fan out for I/O, funnel to a single writer — the bottleneck is RPC round-trips by an order of magnitude
- The cursor lives in the same transaction as the data — crash recovery is a single `SELECT`
- Reorgs are tested against a scriptable mock, not against mainnet
- Every channel is bounded — backpressure is structural, not hoped for

## Results

### Decode benchmarks (Phase 3)

| Benchmark | Time |
|-----------|------|
| `decode_empty_block` | 28.3 ns |
| `decode_legacy_tx` | 67.2 ns |
| `decode_eip1559_with_erc20` | 150.6 ns |
| `decode_erc721` | 125.6 ns |
| `decode_contract_creation` | 65.1 ns |
| `decode_multi_tx_with_logs` | 241.5 ns |

The decode path is pure CPU — no I/O, no allocations beyond the output vectors. At ~150 ns per block with ERC-20 transfers, the decode stage will not be the bottleneck even at 1000 blocks/sec.

### Test results

63 tests passing (56 unit + 7 integration). Zero failures.

## Live dashboard

[ChainLens Results Dashboard](website/index.html) — performance metrics, benchmark results, and project status.

## How results are generated

1. **Decode benchmarks** run via `criterion` against JSON fixture files (empty block, legacy tx, EIP-1559 + ERC-20, ERC-721, contract creation, multi-tx with logs)
2. **Integration tests** verify decode correctness against the same fixtures
3. **Throughput benchmarks** (Phase 11) will run in three environments to make costs attributable: fixture replay with zero latency, fixture replay with synthetic latency, and real RPC provider
4. All results record git revision, configuration, and hardware

## Project status

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Foundation and scaffolding | Complete |
| 2 | RPC client | Complete |
| 3 | Domain model and decoding | Complete |
| 4 | Schema and commit | Complete |
| 5 | Sequential pipeline | Next |
| 6–12 | Reorgs, crash recovery, observability, concurrency, API, benchmarks, docs | Planned |

## Roadmap

**MVP (Phases 1–7 + minimal API):** A system that continuously indexes Ethereum mainnet, decodes ERC-20/721 transfers, detects and correctly rolls back chain reorganizations (proven by deterministic test suite), survives `kill -9` at any point, and serves read endpoints.

**Performance (Phase 11):** Measured throughput across three environments, with the sequential baseline compared to concurrent results. The largest single win expected: moving the write path to batched `COPY`.

## Scope

**In scope:** Ethereum mainnet; block, transaction, receipt, and log indexing; ERC-20 and ERC-721 transfer decoding; canonical chain tracking with reorg rollback; crash-safe resumable indexing; bounded-concurrency ingestion; a read API; Prometheus metrics; a reproducible benchmark suite.

**Deliberately excluded:** Multi-chain support, GraphQL, a frontend dashboard, Kubernetes, Kafka.

## License

Licensed under the Apache License, Version 2.0.
