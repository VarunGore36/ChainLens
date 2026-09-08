# ChainLens

> **Status: Phase 4 complete.** Schema, migrations, and single-transaction commit are
> implemented. Phase 5 (sequential pipeline) is next.

An Ethereum blockchain indexer written in Rust. It ingests blocks, transactions, receipts,
and event logs from a JSON-RPC endpoint, decodes them, persists them to PostgreSQL, and
serves them over a read API.

It is built around three properties that indexer implementations commonly skip: **correct
handling of chain reorganizations**, **crash safety that is tested rather than assumed**,
and **performance that is measured rather than claimed**.

---

## What it does

ChainLens continuously follows the Ethereum mainnet chain head while also backfilling
historical ranges. For every block it stores the header, all transactions with their
receipt data, and every event log — with ERC-20 and ERC-721 `Transfer` and `Approval`
events additionally decoded into typed tables.

It tracks the canonical chain by verifying parent-hash linkage on every commit. When the
chain reorganizes, it identifies the common ancestor, rolls back the orphaned range
atomically, and re-indexes the new canonical branch. It records where it stopped in a way
that makes restarts safe: kill the process at any point and it resumes without gaps,
duplicates, or manual repair.

## Why it's built this way

The design decisions below are the substance of the project. Each one has a rejected
alternative, and those will be recorded in `docs/DESIGN_DECISIONS.md` as each is implemented.

### The performance problem is backfill, not the chain head

Ethereum mainnet produces one block every ~12 seconds — **0.083 blocks/sec**. Keeping pace
with the head is trivial and needs no concurrency at all. Concurrency exists in this
project for one reason: historical backfill.

| Ingestion rate | 1M blocks | Full history (~26M blocks) |
| --- | --- | --- |
| 10 blocks/sec (naive sequential) | ~28 hours | ~30 days |
| 100 blocks/sec | ~2.8 hours | ~3 days |
| 500 blocks/sec | ~33 minutes | ~14 hours |

A related consequence shapes the RPC layer. Fetching a block naively costs one
`eth_getBlockByNumber` plus one `eth_getTransactionReceipt` per transaction — roughly **201
round trips per block** at mainnet transaction counts. Using `eth_getBlockReceipts` reduces
that to two. That single change matters more to end-to-end throughput than any amount of
optimization inside the process.

### Fan out for I/O, funnel to a single writer

Fetching and decoding are order-independent and dominated by network latency, so they run
across a pool of workers. Committing is order-*dependent* — the canonical chain is a linked
list, and validating it requires knowing the previous block — so all writes funnel through
a single committer task.

This sounds like it discards the benefit of concurrency. It does not, and the reason is
quantitative: one fetch worker at 50 ms round-trip latency manages roughly 10 blocks/sec,
while a single PostgreSQL connection doing batched writes handles thousands of rows per
second. The bottleneck is RPC round-trips by roughly an order of magnitude, so serializing
the commit stage costs very little and buys the entire correctness story.

That is a hypothesis until it is measured, which is what the benchmark phase is for. If the
committer turns out to be the binding constraint, the escape hatch is already implied by the
architecture: blocks below the finalized checkpoint are immutable, so historical commits can
safely be parallelized. That change will be made if and only if measurement calls for it.

### The cursor lives in the same transaction as the data

A block's header, transactions, logs, decoded transfers, address index, **and the indexing
cursor** are written in a single PostgreSQL transaction. There is no checkpoint file and no
second write to coordinate.

This is the decision that makes crash recovery uninteresting, which is the goal. Recovery is
a single `SELECT` against the cursor followed by resuming at the next block. Every
chain-derived table uses a primary key derived from chain data rather than a database
sequence, so reprocessing a block is idempotent. The alternative — tracking progress
separately from the data — turns restart safety into a two-phase commit problem for no
benefit.

### Reorgs are tested against a scriptable mock, not against mainnet

Post-Merge Ethereum reorganizations are rare and usually one block deep. Waiting for one to
occur is not a test strategy, and an indexer whose reorg path has never executed is an
indexer whose reorg path does not work.

ChainLens therefore includes a mock JSON-RPC implementation that serves a synthetic chain
and can be instructed to switch to a competing fork at a chosen height. Reorg handling is
verified by a deterministic, table-driven test suite covering shallow reorgs, deep reorgs,
reorgs arriving while the indexer is behind the head, and crashes occurring mid-rollback.
If a reorg deeper than the configured maximum is detected, the indexer halts rather than
guessing — downtime is recoverable, silent corruption is not.

The same mock harness doubles as the benchmark engine, because a rate-limited hosted RPC
provider cannot be used to measure pipeline throughput: it only measures the provider's
throttle.

## Architecture

```
  Ethereum JSON-RPC
         │
         ▼
  ┌─────────────┐   ┌─────────────┐
  │ Head Watcher│──▶│  Scheduler  │   tracks latest / safe / finalized;
  └─────────────┘   └──────┬──────┘   decides what to fetch next
                           │
                    bounded channel  ◀── backpressure
                           │
         ┌─────────────────┴─────────────────┐
         │      Fetch + Decode Workers       │   N tasks, order-independent
         │  getBlockByNumber + getBlockReceipts│  RPC I/O and CPU decode
         └─────────────────┬─────────────────┘
                           │
                    bounded channel  ◀── backpressure
                           │
                    ┌──────▼──────┐
                    │  Sequencer  │   reorder buffer: restores strict
                    └──────┬──────┘   ascending contiguous order
                           │
                    ┌──────▼──────┐      ┌───────────────┐
                    │  Committer  │─────▶│ Reorg Handler │
                    │ SINGLE WRITER│ parent│ ancestor walk │
                    └──────┬──────┘ mismatch└──────┬──────┘
                           │                       │
                           ▼   one transaction     ▼
                    ┌──────────────────────────────────┐
                    │           PostgreSQL             │
                    └──────┬─────────────────┬─────────┘
                           ▼                 ▼
                    ┌─────────────┐   ┌─────────────┐
                    │  Query API  │   │  /metrics   │
                    └─────────────┘   └─────────────┘
```

Every channel is bounded. An unbounded queue does not remove backpressure — it converts it
into unbounded memory growth and an eventual out-of-memory kill. Bounding them means a slow
database propagates backwards as workers blocking on a full channel, which is observable in
the queue-depth metrics rather than as a crash.

## Storage

PostgreSQL, with `blocks`, `transactions`, `logs`, `token_transfers`, a denormalized
`address_transactions` index for address history queries, a single-row `indexer_state`
cursor, and a `reorgs` audit table that survives rollback.

Orphaned data is hard-deleted on reorg via cascade rather than flagged as non-canonical. Soft
deletion would preserve forensic history at the cost of adding a `WHERE canonical = true`
predicate to every read path and accumulating dead rows indefinitely; the `reorgs` audit
table preserves the history that actually matters without that tax.

`uint256` values are stored as `NUMERIC(78,0)`. 2^256 is approximately 1.16 × 10^77, so 78
digits is sufficient, and unlike a 32-byte column it remains sortable and summable in SQL.

## API

```
GET /block/{number_or_hash}
GET /transaction/{hash}
GET /address/{address}/transactions?limit&before_block
GET /contract/{address}/events?topic0&from_block&to_block&limit
GET /health
GET /status                      indexing cursor, chain head, lag, finalized height
```

Pagination is keyset-based rather than `OFFSET`-based, so deep pages stay constant-time and
remain stable while new blocks are being indexed concurrently.

## Observability

Prometheus metrics on `/metrics`, covering indexing throughput (blocks, transactions, and
logs per second), RPC and database latency as histograms, queue depth per pipeline stage,
sequencer buffer occupancy, indexing lag in blocks, reorg count and depth, worker
utilization, and resident memory. Latency is recorded as histograms rather than averages,
because a pipeline with a serialized stage is characterized by its tail, not its mean.

## Project status

Correctness is built before performance, and instrumentation before concurrency. The
ordering is deliberate: the shape of the commit stage is determined by the reorg and cursor
requirements, so building concurrency first would mean rewriting it. That adding concurrency
requires no changes to the commit, reorg, or storage code is the intended evidence that the
sequential design was correct.

| | Phase | State |
| --- | --- | --- |
| 1 | Foundation and scaffolding | **complete** |
| 2 | RPC client: retry, rate limiting, capability probe | **complete** |
| 3 | Domain model, validation, ERC-20/721 decoding | **complete** |
| 4 | Schema and single-transaction commit | **complete** |
| 5 | Sequential pipeline end to end (baseline) | next |
| 6 | Mock RPC harness and reorg handling | planned |
| 7 | Crash recovery hardening | planned |
| 8 | Observability | planned |
| 9 | Concurrency | planned |
| 10 | Query API | planned |
| 11 | Benchmarking | planned |
| 12 | Documentation and polish | planned |

Phases 1 through 7 plus a minimal API constitute the MVP. Note that reorg handling and
proven crash recovery are inside that boundary and concurrency is outside it: a concurrent
indexer that corrupts on reorg is a worse system than a sequential one that does not.

## Getting started

Phases 1 and 2 are complete. The process starts, connects to PostgreSQL, constructs the
RPC client, probes for `eth_getBlockReceipts` support, and exits cleanly on Ctrl-C. There
is no pipeline yet — the process idles after startup.

```bash
cp .env.example .env          # add your RPC endpoint
docker compose up -d          # PostgreSQL
cargo run --bin chainlens     # starts, probes RPC, idles
```

To verify everything works:

```bash
cargo test --locked                              # 40 unit tests
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Documentation

Each document is written as the thing it describes is built, rather than retroactively.
Until then this README is the source of truth for architecture.

- `docs/DESIGN_DECISIONS.md` — each significant choice with its rejected alternatives *(from Phase 4)*
- `docs/ARCHITECTURE.md` — data flow, invariants, and schema rationale *(from Phase 5)*
- `docs/query-plans.md` — `EXPLAIN ANALYZE` output for each API endpoint *(from Phase 10)*
- `docs/BENCHMARKS.md` — methodology, results, and analysis *(from Phase 11)*

## Benchmarks

Not yet measured. No performance claim appears in this README until it does.

When they exist, results will be reported across three environments so that cost is
attributable rather than aggregated: fixture replay with zero added latency (isolating the
decode and write path), fixture replay with synthetic injected latency (isolating
concurrency scaling), and a real provider (establishing what is actually achievable). Every
result will record its git revision, configuration, hardware, and PostgreSQL version, since
a benchmark that cannot be re-run is an anecdote.

## Scope and non-goals

**In scope:** Ethereum mainnet; block, transaction, receipt, and log indexing; ERC-20 and
ERC-721 transfer decoding; canonical chain tracking with reorg rollback; crash-safe resumable
indexing; bounded-concurrency ingestion; a read API; Prometheus metrics; a reproducible
benchmark suite.

**Deliberately excluded:** multi-chain support, GraphQL, a frontend dashboard, Kubernetes,
and any message broker. On the last point specifically — a bounded in-process channel
provides the same backpressure semantics for a single-writer pipeline, and PostgreSQL already
provides the durability a broker would be added for. Introducing Kafka here would add an
operational component without adding a capability.

**Known limitations:** no backfill from genesis (a configurable recent window is indexed
instead, as full history is not practical on commodity hardware); a single chain; no internal
EVM call tracing; and the API is not production-hardened.

## Security posture

The query API is read-only, exposes no mutating routes, and is **unauthenticated** — it binds
to localhost and is not intended to be exposed publicly without adding authentication and
rate limiting first. Credentials, RPC endpoints, and database URLs are supplied through
environment variables only, are redacted before reaching any log line, and are never
committed. The project holds no private keys and signs no transactions; it is a read-only
consumer of chain data.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the full text.
