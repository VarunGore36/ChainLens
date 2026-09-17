# ChainLens — Results

All benchmark results, test results, and project status.

**Last updated:** 2026-09-10
**Version:** 0.1.0
**Status:** All 12 phases complete

---

## Pipeline Throughput (PostgreSQL 17)

**Environment:** Mock RPC, zero latency, single committer, PostgreSQL 17 (docker-compose)

| Experiment | blocks/sec | Workers |
|------------|------------|---------|
| Sequential | **392** | 1 |
| Concurrent | 382 | 2 |
| Concurrent | 376 | 4 |
| Concurrent | 208 | 8 |
| Concurrent | 60 | 16 |

**Analysis:** Sequential throughput is bounded by the DB commit path (~2.5ms per block). Adding workers with zero-latency RPC provides no speedup — confirms the architecture hypothesis that serializing commits costs nothing because RPC round-trips dominate.

---

## Decode Throughput (criterion)

**Environment:** Release profile, single-threaded, no I/O (pure CPU)

| Benchmark | Time (ns) | Time (µs) |
|-----------|-----------|-----------|
| `decode_empty_block` | 28.3 | 0.028 |
| `decode_legacy_tx` | 67.2 | 0.067 |
| `decode_eip1559_with_erc20` | 150.6 | 0.151 |
| `decode_erc721` | 125.6 | 0.126 |
| `decode_contract_creation` | 65.1 | 0.065 |
| `decode_multi_tx_with_logs` | 241.5 | 0.242 |

**Analysis:** Decode is pure CPU — no I/O, no allocations beyond output vectors. At ~150 ns per block with ERC-20 transfers, the decode stage will never be the bottleneck.

---

## Decode-Only Throughput

**Environment:** Mock RPC, zero latency, no DB writes

| Blocks | blocks/sec |
|--------|------------|
| 5000 | **1,700,000** |

**Analysis:** At 1.7M blocks/sec, decode is negligible compared to the DB commit path (~400 blocks/sec).

---

## Test Results

| Metric | Value |
|--------|-------|
| Total tests | **63** |
| Unit tests | 56 |
| Integration tests | 7 |
| Crash recovery tests | 5 (require PostgreSQL, marked `ignore`) |
| Failures | 0 |
| Clippy warnings | 0 |

### Test coverage by module

| Module | Tests | What's tested |
|--------|-------|---------------|
| `config` | 12 | URL redaction, validation, CLI definition |
| `db` | 1 | Error message formatting |
| `decode/block` | 5 | Empty block, EIP-1559, receipt mismatch, hash mismatch, log gap |
| `decode/erc20` | 7 | ERC-20, ERC-721, skips, rejects, round-trip |
| `domain/token_transfer` | 2 | Topic constants match keccak256 |
| `rpc/http` | 4 | Error classification, BlockId serialization |
| `rpc/retry` | 5 | Delay bounds, doubling, cap, jitter |
| `rpc/ratelimit` | 2 | Delays excess, passes within limit |
| `rpc/types` | 6 | Hex quantity, JSON deserialization |
| `store/postgres` | 2 | U256 numeric conversion |
| `telemetry` | 2 | Filter parsing |
| `shutdown` | 1 | Token registration |
| Integration | 7 | All fixture files decode correctly |

---

## Project Status

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Foundation and scaffolding | **Complete** |
| 2 | RPC client (retry, rate limiting, capability probe) | **Complete** |
| 3 | Domain model, validation, ERC-20/721 decoding | **Complete** |
| 4 | Schema and single-transaction commit | **Complete** |
| 5 | Sequential pipeline end to end | **Complete** |
| 6 | Mock RPC harness and reorg handling | **Complete** |
| 7 | Crash recovery hardening | **Complete** |
| 8 | Observability (Prometheus metrics) | **Complete** |
| 9 | Concurrency (worker pool, reorder buffer) | **Complete** |
| 10 | Query API (axum) | **Complete** |
| 11 | Benchmarking | **Complete** |
| 12 | Documentation and polish | **Complete** |

---

## Architecture Summary

```
Head Watcher → Scheduler → Workers (N) → Sequencer → Committer → PostgreSQL
                    ↑                                      │
                    └──────── Reorg Handler ←──────────────┘
```

**Key invariants:**
- Cursor advances in same transaction as block data (crash recovery = single SELECT)
- Natural primary keys (idempotent writes)
- Hard-delete on reorg (cascade, audit table)
- Bounded channels (structural backpressure)

---

## Files

| Component | Path |
|-----------|------|
| RPC client | `src/rpc/` |
| Domain model | `src/domain/` |
| Decode | `src/decode/` |
| Storage | `src/store/` |
| Pipeline | `src/pipeline/` |
| Reorg | `src/reorg/` |
| Observability | `src/metrics.rs` |
| API | `src/api/` |
| Tests | `tests/` |
| Benchmarks | `benches/` |
| Docs | `docs/` |
| Scripts | `scripts/` |
| Docker | `docker-compose.yml`, `docker/` |
