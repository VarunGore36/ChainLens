# ChainLens — Engineering Specification

A high-performance Ethereum indexer in Rust. This document is the contract we work
against: it defines scope, architecture, the invariants that make the system correct,
and a phase-by-phase roadmap with acceptance criteria.

**Status:** design agreed, implementation not started.

## 0. Decided parameters

| Parameter | Decision | Consequence |
|---|---|---|
| Chain | Ethereum mainnet | Recognizable, real data volume, post-Merge finality semantics |
| RPC access | Hosted provider (Alchemy/Infura/QuickNode) | Rate-limited — benchmarks require a local replay harness |
| Decode depth | All logs stored raw; ERC-20/721 `Transfer`/`Approval` decoded into typed tables | No ABI registry to build |
| Language / runtime | Rust + Tokio | Async model is new to us, so async concepts get explained as they arrive |
| Storage | PostgreSQL (RocksDB write-path comparison = stretch) | One storage engine to get right |
| Time budget | 4–6 weeks, evenings and weekends | Forces a hard cut list (§12) |

## 1. Scope

**In scope.** Continuous ingestion of Ethereum mainnet blocks, transactions, receipts,
and logs into PostgreSQL; canonical-chain tracking with reorg detection and rollback;
crash-safe resumable indexing; a bounded-concurrency ingestion pipeline; a read API over
the indexed data; Prometheus metrics; and a reproducible benchmark suite.

**Out of scope, deliberately.** Multi-chain support. GraphQL. A frontend dashboard.
Kubernetes, Kafka, Redis, or any message broker. Internal EVM call tracing
(`debug_traceTransaction`). A dynamic ABI registry. Full-archive backfill from genesis.

**Deferred (stretch, only if time remains).** RocksDB write-path comparison. WebSocket
subscription endpoint. Table partitioning by block range.

Every item in the "out of scope" list is there because it adds operational surface
without adding a systems-engineering story we can defend. If asked in an interview why
there is no Kafka, the answer is: a bounded in-process channel provides the same
backpressure semantics for a single-writer pipeline, and a broker would add a durability
layer we already get from PostgreSQL.

## 2. Why performance matters here (the framing that justifies the whole project)

Ethereum mainnet produces one block every ~12 seconds — **0.083 blocks/sec**. Following
the chain head needs essentially no throughput. So the performance story is *not* about
keeping up with the head; it is entirely about **historical backfill**.

At roughly 26M blocks on mainnet as of late 2026 (verify the current head before planning a
backfill run):

| Ingestion rate | Time to backfill 1M blocks | Time to backfill full history |
|---|---|---|
| 10 blocks/sec (naive sequential) | ~28 hours | ~30 days |
| 100 blocks/sec | ~2.8 hours | ~3 days |
| 500 blocks/sec | ~33 minutes | ~14 hours |

This is the number that justifies concurrency, batching, and every RPC optimization in
this document. Be ready to say it out loud: *"Concurrency isn't for the head — the head
is 0.08 blocks/sec. It's because a serial backfill of mainnet takes a month."*

**Scope boundary that follows from this:** we do not backfill from genesis. Disk cost is
prohibitive on a laptop. We define a configurable `BACKFILL_FROM` and target a window of
recent history — start with 10k blocks in development, scale to 100k–1M for the
benchmark run, and measure actual bytes-per-block on disk in Phase 4 rather than guessing.

## 3. Architecture

```
                    ┌──────────────────┐
                    │  Head Watcher    │  polls latest / safe / finalized
                    │  (1 task)        │  every ~4s
                    └────────┬─────────┘
                             │ target height, finalized height
                             ▼
                    ┌──────────────────┐
   cursor height ──▶│   Scheduler      │  decides what to fetch next;
   (from DB)        │   (1 task)       │  owns rewind-on-reorg
                    └────────┬─────────┘
                             │ bounded channel <block numbers>   ◀── BACKPRESSURE
                             ▼
        ┌────────────────────────────────────────────┐
        │        Fetch + Decode Worker Pool          │  N tasks, order-independent
        │  eth_getBlockByNumber + eth_getBlockReceipts│  RPC I/O + CPU decode
        │  → raw JSON → IndexedBlock (typed)          │
        └────────────────────┬───────────────────────┘
                             │ bounded channel <IndexedBlock>    ◀── BACKPRESSURE
                             ▼
                    ┌──────────────────┐
                    │   Sequencer      │  reorder buffer: releases blocks
                    │   (1 task)       │  in strictly ascending contiguous order
                    └────────┬─────────┘
                             ▼
                    ┌──────────────────┐         ┌──────────────────┐
                    │  Committer       │────────▶│  Reorg Handler   │
                    │  (1 task, SINGLE │ parent  │  common-ancestor │
                    │   WRITER)        │ mismatch│  walk + rollback │
                    └────────┬─────────┘         └────────┬─────────┘
                             │                            │
                             ▼      one PG transaction     ▼
                    ┌────────────────────────────────────────────┐
                    │              PostgreSQL                    │
                    │  blocks · transactions · logs ·            │
                    │  token_transfers · address_transactions ·  │
                    │  indexer_state (cursor) · reorgs (audit)   │
                    └───────────────────┬────────────────────────┘
                                        │
                            ┌───────────┴───────────┐
                            ▼                       ▼
                   ┌─────────────────┐    ┌──────────────────┐
                   │  Query API      │    │  /metrics        │
                   │  (axum, separate│    │  (Prometheus)    │
                   │   binary)       │    │                  │
                   └─────────────────┘    └──────────────────┘
```

The shape to notice: **the pipeline fans out for the expensive, order-independent work
(network I/O and decoding) and funnels back to a single writer for the work that must be
ordered (committing to the canonical chain).** That asymmetry is the central design
decision of the project, and §5 explains why it costs nothing.

## 4. Core invariants

These five statements are the spine of the system. Every design choice below exists to
protect one of them, and in an interview these are what you actually defend.

**I1 — Contiguity and completeness.** For every block number `n ≤ cursor.height`, block
`n` and *all* of its transactions, logs, and decoded transfers are durably stored, and
`blocks[n].parent_hash == blocks[n-1].hash`. There are no gaps and no partial blocks
below the cursor.

**I2 — Atomic cursor.** The cursor advances *only* inside the same PostgreSQL transaction
that writes the block data it refers to. There is no separate checkpoint file, no
separate write, no fsync ordering to reason about. This is what reduces crash recovery
from a hard distributed-systems problem to `SELECT last_indexed_number FROM
indexer_state` — resume at `n+1`.

**I3 — Single writer.** Exactly one task mutates canonical chain state. Fetching and
decoding are parallel and order-free; committing is serial and strictly ordered.

**I4 — Idempotent writes.** Every row's primary key is derived from chain data, never
from a database sequence. Reprocessing block `n` yields identical state. Concretely: no
`SERIAL`/`BIGSERIAL` primary keys on any chain-derived table.

**I5 — Fail-stop on ambiguity.** If the indexer cannot establish chain linkage within
`MAX_REORG_DEPTH` (default 128 blocks), it halts loudly rather than guessing. Downtime is
recoverable; silent corruption is not.

## 5. The two ingestion modes

A single pipeline, two operating regimes. Understanding the difference is what makes the
concurrency design defensible.

| | **Backfill mode** | **Follow mode** |
|---|---|---|
| Range | `[from, finalized]` | `(finalized, latest]` |
| Can history change? | No — blocks are finalized | Yes — reorgs possible |
| Parent-hash validation | Still done (cheap correctness check) | Load-bearing (reorg detection) |
| Commit ordering | Not strictly required | Required |
| Purpose | Where throughput is demonstrated | Where correctness is demonstrated |

**Why serializing the committer costs nothing.** A mainnet block carries roughly 150–250
transactions and several hundred logs. Even at an aggressive 100 blocks/sec, the committer
handles ~20k transaction inserts/sec — well within a single PostgreSQL connection doing
batched `COPY`. Meanwhile a single fetch worker at 50 ms round-trip latency and 2 RPC
calls per block manages only ~10 blocks/sec. **The bottleneck is RPC round-trips, by an
order of magnitude.** Serializing the commit stage therefore buys the entire correctness
story for approximately zero throughput cost.

That claim is a hypothesis, not a fact, until Phase 11 measures it. If the benchmark shows
the committer *is* the constraint, the escape hatch is already implied by this table:
backfill blocks are immutable, so commits below `finalized` can safely run in parallel
with a gap-set cursor instead of a high-water mark. We add that **only if the data says
we need it** — that sequencing (measure, then optimize) is itself part of the story.

## 6. The RPC round-trip problem

This is where a naive indexer dies, and being able to explain it is high-signal.

Fetching one block naively costs `1 × eth_getBlockByNumber(n, full_txs=true)` plus
`T × eth_getTransactionReceipt` — at 200 transactions per block, **201 round trips per
block**. At 50 ms latency that is over 10 seconds per block sequentially. Backfilling
1M blocks would take four months.

Fixes, in descending order of impact:

1. **`eth_getBlockReceipts(block)`** returns every receipt in one call, collapsing 201
   requests into 2. Supported by Geth ≥1.13, Erigon, Reth, and Alchemy — *verify against
   your provider in Phase 2 and probe for it at startup.* This single change is a ~100×
   reduction in request count.
2. **JSON-RPC batching.** The spec allows an array of requests in one HTTP POST. Use it as
   the fallback when `eth_getBlockReceipts` is unavailable, and to fetch several blocks
   per round trip.
3. **Connection reuse.** One `reqwest::Client` for the process lifetime, keep-alive on.
   Constructing a client per request re-does TLS handshakes and will silently dominate
   your latency profile.
4. **Concurrency.** With RPC latency fixed, throughput is governed by Little's Law:
   `throughput ≈ in-flight requests / latency`. At 50 ms and 2 calls/block, 1 worker gives
   ~10 blocks/sec and 32 workers give ~320 blocks/sec — until the provider's rate limit
   binds. Concurrency exists to fill the bandwidth-delay product, nothing more.

**Rate limits are the real ceiling.** Hosted providers meter compute units per second, not
requests, and `eth_getBlockByNumber` with full transactions is expensive. So we need a
token-bucket limiter plus retry with exponential backoff and jitter on HTTP 429 — and
crucially, the honest benchmark environment described in §11 that removes the provider
from the measurement entirely.

## 7. Storage design (sketch — finalized in Phase 4)

```
blocks
  number         BIGINT PRIMARY KEY
  hash           BYTEA NOT NULL UNIQUE
  parent_hash    BYTEA NOT NULL
  timestamp      TIMESTAMPTZ NOT NULL
  miner          BYTEA
  gas_used       BIGINT
  gas_limit      BIGINT
  base_fee       NUMERIC(78,0)
  tx_count       INT

transactions
  hash                BYTEA PRIMARY KEY
  block_number        BIGINT NOT NULL REFERENCES blocks(number) ON DELETE CASCADE
  tx_index            INT NOT NULL
  from_addr           BYTEA NOT NULL
  to_addr             BYTEA                    -- NULL for contract creation
  value               NUMERIC(78,0) NOT NULL
  nonce               BIGINT, tx_type SMALLINT
  gas_limit           BIGINT, gas_price NUMERIC(78,0), max_fee_per_gas NUMERIC(78,0)
  input               BYTEA
  status              SMALLINT                 -- from receipt
  gas_used            BIGINT
  effective_gas_price NUMERIC(78,0)
  contract_address    BYTEA

logs
  block_number  BIGINT NOT NULL REFERENCES blocks(number) ON DELETE CASCADE
  log_index     INT NOT NULL
  tx_hash       BYTEA NOT NULL
  address       BYTEA NOT NULL
  topic0..topic3 BYTEA                         -- separate columns, not an array
  data          BYTEA
  PRIMARY KEY (block_number, log_index)

token_transfers        -- decoded ERC-20 / ERC-721 Transfer
  block_number BIGINT, log_index INT, PRIMARY KEY (block_number, log_index)
  token_address BYTEA, from_addr BYTEA, to_addr BYTEA
  value NUMERIC(78,0)  -- ERC-20 amount
  token_id NUMERIC(78,0) -- ERC-721 id
  standard SMALLINT

address_transactions   -- denormalized index for address history
  address BYTEA, block_number BIGINT, tx_index INT, tx_hash BYTEA, direction SMALLINT
  PRIMARY KEY (address, block_number, tx_index, direction)

indexer_state          -- the cursor; exactly one row
  id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1)
  last_indexed_number BIGINT, last_indexed_hash BYTEA
  finalized_number BIGINT, updated_at TIMESTAMPTZ

reorgs                 -- audit trail, survives rollback
  id BIGSERIAL, detected_at TIMESTAMPTZ
  common_ancestor_number BIGINT, depth INT
  orphaned_from BIGINT, orphaned_to BIGINT, orphaned_hashes BYTEA[]
```

Four design decisions worth being able to defend:

**`uint256` → `NUMERIC(78,0)`.** 2^256 ≈ 1.16 × 10^77, so 78 digits covers it. `NUMERIC`
is sortable and arithmetic-capable in SQL, which `BYTEA(32)` is not — you can write
`ORDER BY value DESC` and `SUM(value)`. The cost is slower writes and more storage than
fixed-width bytes. That tradeoff is measurable in Phase 11.

**Hard-delete orphaned data, keep a `reorgs` audit row.** The alternative is a `canonical
BOOLEAN` flag with soft deletes. Soft deletes preserve forensic history but poison *every*
query with `WHERE canonical = true` and let dead rows accumulate forever. Hard deletion
via `ON DELETE CASCADE` from `blocks` keeps read paths clean, and the `reorgs` table
preserves the demo story ("here are the reorgs my indexer survived") without the query tax.

**`blocks.number` as primary key** is only sound *because* we hard-delete on reorg — there
is never more than one block per height in the table. If we kept orphaned blocks, the key
would have to be `hash`. Note the related subtlety: the same transaction hash can appear in
both an orphaned and a canonical block, so `transactions.hash` as PK requires that the
rollback delete and the re-insert happen in one transaction. They do (§4, I2).

**`address_transactions` is denormalized on purpose.** "All transactions for an address"
means `from_addr = X OR to_addr = X`, which PostgreSQL cannot serve from a single index —
you get either a bitmap OR of two index scans or a `UNION ALL`, both of which sort poorly
under pagination. One row per (address, direction) turns the hot API endpoint into a single
ordered index scan. The cost is ~2× row amplification on writes, which is exactly the kind
of thing a benchmark should quantify.

## 8. Repository layout

One crate, two binaries. Not a Cargo workspace — a workspace buys compile-time
parallelism and independent versioning that a 4–6 week single-author project does not need,
and it adds friction to sharing domain types. Split later if the API and indexer need
independent deployment, or if clean-build time exceeds a minute.

```
chainlens/
  Cargo.toml
  docker-compose.yml            # postgres, later prometheus + grafana
  migrations/                   # sqlx migrations, checked in, forward-only
  src/
    main.rs                     # indexer binary
    bin/api.rs                  # API binary
    config.rs                   # clap derive + env vars
    telemetry.rs                # tracing subscriber setup
    metrics.rs                  # metric handles, one place
    domain/                     # Block, Transaction, Log, TokenTransfer, newtypes
    rpc/
      mod.rs                    # trait EthClient  <-- the seam that makes mocking possible
      http.rs                   # reqwest transport, batching
      retry.rs                  # backoff + jitter
      ratelimit.rs              # token bucket
    decode/
      block.rs  receipts.rs  erc20.rs
    store/
      mod.rs                    # trait BlockStore  <-- the seam for the RocksDB stretch goal
      postgres.rs
    pipeline/
      head_watcher.rs  scheduler.rs  worker.rs  sequencer.rs  committer.rs
    reorg/
      detect.rs  rollback.rs
    api/
      routes.rs  handlers.rs  dto.rs
  tests/                        # integration tests against real postgres
  benches/
    harness/                    # mock RPC: fixture replay + latency injection
    fixtures/                   # captured mainnet JSON
    results/                    # committed CSV + config + git SHA per run
  docs/
```

The two `trait` seams (`EthClient`, `BlockStore`) are the only abstractions we introduce
up front, and each earns its place: `EthClient` is what makes deterministic reorg testing
and honest benchmarking possible, and `BlockStore` is what makes the RocksDB comparison a
day of work instead of a rewrite. Resist adding a third.

## 9. Dependencies

Versions get pinned in Phase 1. `ethers-rs` is deprecated and `alloy` is the officially
recommended successor (verified against the alloy-rs docs and the ethers-rs deprecation
notice), but the ecosystem moves fast — confirm current minor versions and API shapes when
we start Phase 1 rather than trusting exact signatures written here.

| Concern | Choice | Note |
|---|---|---|
| Ethereum types + ABI | `alloy` (`alloy-primitives`, `alloy-sol-types`) | `Address`, `U256`, keccak, typed event decoding |
| RPC transport | `reqwest` + `serde_json`, or `alloy`'s provider | See below |
| Runtime | `tokio` (full features) | |
| Postgres | `sqlx` (postgres, tokio, macros, migrate) | Compile-time-checked SQL; `copy_in_raw` for the fast path |
| HTTP server | `axum` + `tower-http` | |
| Logging | `tracing` + `tracing-subscriber` | Structured, span-based |
| Metrics | `metrics` + `metrics-exporter-prometheus` | |
| Errors | `thiserror` in modules, `anyhow` in binaries | |
| Config | `clap` derive with `env` | No config file to document |
| Rate limiting | `governor` | Token bucket |
| Micro-benchmarks | `criterion` | Decode path only; throughput uses our own harness |

**On hand-rolling the JSON-RPC transport:** for a portfolio piece that is specifically
about systems programming, writing the transport yourself over `reqwest` is a net positive
— it demonstrates you understand the wire protocol, and it gives you direct control over
request batching, which a high-level provider abstraction tends to hide. Recommendation:
use `alloy` for primitives and ABI decoding (do not reimplement keccak or `U256`), and
hand-roll the transport. We decide this together in Phase 2.

## 10. Phased roadmap

Two deliberate changes from the originally proposed ordering, both worth understanding:

**Correctness before concurrency (reorgs and recovery move ahead of the worker pool).**
The shape of the commit stage is *determined* by the reorg and cursor requirements. Build a
concurrent pipeline first and you will discover the committer has to be serialized anyway,
then refactor the concurrency you just wrote. Build the sequential-but-correct version
first and adding concurrency becomes a localized change: a worker pool and a reorder buffer
inserted *in front of an unchanged committer*. That the concurrency phase requires no
changes to the correctness code is itself the evidence that the design was right — and it
is a strong thing to say in an interview.

**Observability before concurrency.** You cannot tune what you cannot measure. Having
blocks/sec, queue depth, and RPC latency histograms *before* adding workers means the
concurrency phase produces a scaling graph rather than a feeling.

### Phase 1 — Foundation and scaffolding

**What.** A compiling, running, testable skeleton: Cargo project, layered config from env,
structured logging, error types, `docker-compose.yml` with PostgreSQL, CI on push.

**Why.** Every later phase pays interest on decisions made here. Specifically: if
structured logging and config aren't in place before the pipeline exists, you will debug
concurrency with `println!`, which does not work in async code with interleaved tasks.

**Concepts.** Cargo features and profiles; `thiserror` vs `anyhow` (libraries return typed
errors so callers can match on them; binaries collapse to `anyhow` because nobody matches
on `main`'s error); `tracing` spans vs log lines; why `RUST_LOG`-style filtering matters.

**Components.** `Cargo.toml`, `src/main.rs`, `src/config.rs`, `src/telemetry.rs`,
`src/error.rs`, `docker-compose.yml`, `.env.example`, `.github/workflows/ci.yml`,
`README.md` stub, `rustfmt.toml`, `clippy.toml`.

**Acceptance criteria.**
- `cargo run` starts, emits one structured startup log line including resolved config, exits cleanly on Ctrl-C.
- Config resolves from environment with documented defaults; missing required values fail at startup with a clear message, not a panic deep in a task.
- `docker compose up -d` yields a reachable PostgreSQL; connection string is config-driven.
- `cargo clippy --all-targets -- -D warnings` is clean. `cargo fmt --check` is clean.
- CI runs fmt, clippy, and test on push.
- No secrets in the repo; `.env` is gitignored and `.env.example` is committed.

**How to test.** Run with a deliberately broken config and confirm the failure message is
actionable. Run with `RUST_LOG=debug` and confirm log level changes. Confirm CI fails when
you introduce a clippy warning on purpose.

### Phase 2 — RPC client layer

**What.** A typed, resilient Ethereum JSON-RPC client behind an `EthClient` trait:
`get_block_by_number`, `get_block_receipts`, `get_block_number`, `get_block_by_tag`.
Includes retry with exponential backoff and jitter, a token-bucket rate limiter, connection
reuse, and a startup capability probe for `eth_getBlockReceipts`.

**Why.** This is where 90% of wall-clock time will be spent, and where the difference
between a naive and a serious indexer is decided (§6). The trait boundary is also what makes
Phases 6, 7, and 11 possible at all.

**Concepts.** `async fn` and futures as state machines that do nothing until polled;
`async_trait` / return-position `impl Future` in traits; why a shared `reqwest::Client` is
cheap to clone and expensive to recreate; JSON-RPC 2.0 request/response and batch framing;
hex-quantity vs hex-data encoding in Ethereum JSON-RPC (`0x1a` vs `0x0000…1a` — a classic
source of decode bugs); exponential backoff with jitter and why jitter matters (synchronized
retries from N workers create a thundering herd).

**Components.** `src/rpc/mod.rs` (trait), `http.rs`, `retry.rs`, `ratelimit.rs`.

**Acceptance criteria.**
- Fetches a known mainnet block by number and by tag (`latest`, `safe`, `finalized`) against the real provider.
- Retries transient failures (429, 5xx, timeouts) with backoff+jitter; does *not* retry deterministic errors (malformed request, method not found).
- Rate limiter is configurable and demonstrably caps request rate.
- Probes `eth_getBlockReceipts` at startup and logs which receipt strategy is active, falling back to batched `eth_getTransactionReceipt`.
- One HTTP client for process lifetime — assert this in code review, and confirm via connection count or provider metrics.
- Unit tests cover retry classification and hex decoding edge cases (`0x0`, empty data, odd-length hex, `null` for pending fields).

**How to test.** Point at the real provider for a handful of known blocks and assert against
values from Etherscan. Then unit-test retry and decode logic with hand-written fixtures — no
network. Deliberately request a nonexistent block (height above head) and confirm you get a
clean `None`, not an error or a hang.

### Phase 3 — Domain model, validation, and decoding

**What.** Typed domain structs (`Block`, `Transaction`, `Log`, `TokenTransfer`) and a pure
function `raw_json → IndexedBlock` that validates as it decodes. Plus ERC-20/721 `Transfer`
and `Approval` decoding from log topics.

**Why.** Isolating decode as a pure, synchronous, side-effect-free function is what makes
it unit-testable against fixtures and micro-benchmarkable with `criterion`. It also means
the worker pool in Phase 9 has no shared state to contend over.

**Concepts.** Block header fields and what each is for; the receipt/transaction split and
why `status` and `gas_used` live only in the receipt; event log topic layout (`topic0` =
`keccak256("Transfer(address,address,uint256)")`, indexed parameters in `topic1..3`,
non-indexed in `data`); **how to distinguish ERC-20 from ERC-721 `Transfer`** — identical
`topic0`, but ERC-721 indexes `tokenId` so it has 4 topics while ERC-20 has 3 and carries
the amount in `data`; newtype wrappers to prevent mixing up a 20-byte address and a 32-byte
hash at the type level.

**Components.** `src/domain/*`, `src/decode/{block,receipts,erc20}.rs`,
`benches/fixtures/*.json`.

**Acceptance criteria.**
- Decodes a captured mainnet block with all transaction types (legacy, EIP-2930, EIP-1559, EIP-4844 blob) without loss.
- Validates internal consistency: receipt count equals transaction count, receipt tx hashes match transaction hashes in order, log indices are contiguous within the block.
- Correctly classifies ERC-20 vs ERC-721 transfers; a test fixture contains at least one of each plus one non-standard `Transfer`-like event that must *not* be misclassified.
- Decoding never panics on malformed input — returns a typed error. Prove it with a fuzz-ish test that truncates and mutates fixture bytes.
- `criterion` benchmark records baseline decode time per block.

**How to test.** Capture ~20 diverse mainnet blocks as JSON fixtures once (a busy block, a
near-empty block, one with a contract creation, one with a blob transaction, one with
thousands of logs). Golden-file tests: decode and assert on a serialized snapshot. These
fixtures get reused as the benchmark corpus in Phase 11, so capture them properly.

### Phase 4 — Storage layer and schema

**What.** Migrations, the `BlockStore` trait, and a PostgreSQL implementation whose central
operation is `commit_block(IndexedBlock) -> Result<()>` writing block, transactions, logs,
token transfers, address index, *and the cursor* in one transaction.

**Why.** This is where invariants I1, I2, and I4 become real. Get this right and crash
recovery in Phase 7 is nearly free; get it wrong and no amount of later work fixes it.

**Concepts.** ACID and what "one transaction" actually guarantees; `ON CONFLICT DO NOTHING`
vs `DO UPDATE` and why natural keys make the choice safe; multi-row `INSERT` vs `UNNEST`
arrays vs `COPY` (three very different performance classes); why `BIGSERIAL` on chain data
breaks idempotency; index write amplification; connection pooling and why the committer
should hold exactly one connection.

**Components.** `migrations/0001_initial.sql`, `src/store/mod.rs`, `src/store/postgres.rs`,
`tests/store.rs`.

**Acceptance criteria.**
- Migrations apply to an empty database and are forward-only; `sqlx migrate run` is idempotent.
- `commit_block` is atomic: injecting a failure after inserting transactions but before the cursor update leaves **zero** trace of that block.
- `commit_block` is idempotent: committing the same block twice leaves identical state and does not error.
- Cursor and block data are updated in the same transaction — verifiable by reading the code and by the failure-injection test above.
- **Report measured bytes-on-disk per block** (table size + index size after committing 1,000 blocks) and extrapolate the disk budget for the target backfill window. This is an acceptance criterion because the number determines Phase 11's scale.
- Integration tests run against real PostgreSQL, not a mock.

**How to test.** `tests/store.rs` against the docker-compose database, truncating between
tests. Use a store wrapper that can fail on command to test atomicity. Query
`pg_total_relation_size` for the disk report.

### Phase 5 — Sequential pipeline (first end-to-end system)

**What.** Head watcher, scheduler, and committer wired into a single-threaded loop that
indexes from `BACKFILL_FROM` to the chain head and then follows it. Graceful shutdown.
Cursor-driven startup.

**Why.** First moment the project is a *system* rather than components. Deliberately
sequential: it establishes a correctness baseline and a performance baseline, and Phase 11's
headline result is measured against this number.

**Concepts.** `tokio::main` and the multi-threaded scheduler; `tokio::select!` for
concurrent waiting; cooperative cancellation and why `CancellationToken` beats aborting
tasks mid-transaction; the difference between "caught up" and "behind" and how the loop
switches between polling and racing ahead.

**Components.** `src/pipeline/{head_watcher,scheduler,committer}.rs`, wiring in `main.rs`.

**Acceptance criteria.**
- Starts from an empty database at `BACKFILL_FROM`, indexes forward, catches up to head, then keeps pace with new blocks.
- Restarting resumes at `cursor + 1` with no gaps and no duplicates.
- Ctrl-C completes the in-flight block commit and exits within a bounded time — never mid-transaction.
- Logs blocks/sec and current lag periodically.
- **Record the sequential baseline throughput** (blocks/sec, tx/sec) against the real provider. Write it down; this is the "before" number.
- An invariant-checker query returns clean: no gaps in `blocks.number` up to the cursor, and every `blocks[n].parent_hash = blocks[n-1].hash`.

**How to test.** Run for 30+ minutes past the chain head and confirm it tracks. Kill and
restart repeatedly, running the invariant-checker query each time. Keep that query — it
becomes the oracle for Phases 6, 7, and 11.

### Phase 6 — Test harness and reorg handling

**What.** Two deliverables. First, a **scriptable mock RPC** implementing `EthClient` that
serves a synthetic chain from fixtures and can be told to switch to a competing fork at a
chosen height. Second, reorg detection and rollback in the committer.

**Why.** Post-Merge mainnet reorgs are rare and usually one block deep, so you *cannot*
reliably test reorg handling against mainnet — waiting for one is not a test strategy. The
mock harness makes reorgs deterministic and table-driven, and it is reused as the benchmark
engine in Phase 11. Build it once, use it twice.

**Concepts.** What a reorg physically is (a competing chain with more accumulated
attestation weight replaces the current tip); why `parent_hash` linkage is the detection
mechanism and block hash alone is not; common-ancestor search; post-Merge `safe` and
`finalized` tags and what they actually promise (finality after ~2 epochs ≈ 64 slots ≈ 12.8
minutes — data below `finalized` cannot be reorganized without a slashing-level consensus
failure); why finality means the reorg-capable window is bounded and small.

**Detection and rollback algorithm.**
1. At commit time for block `n`, compare `incoming.parent_hash` against the stored hash of `n-1`. Match → normal commit.
2. Mismatch → reorg. Walk backwards from `n-1`: fetch canonical block `k` from RPC, compare its hash to stored `blocks[k].hash`. First `k` where they agree is the common ancestor.
3. If no ancestor found within `MAX_REORG_DEPTH`, **halt** (invariant I5).
4. In one PostgreSQL transaction: insert a `reorgs` audit row, `DELETE FROM blocks WHERE number > ancestor` (cascades to transactions, logs, transfers, address index), and reset the cursor to the ancestor.
5. Tell the scheduler to rewind; the pipeline naturally re-fetches from `ancestor + 1`.

**Components.** `benches/harness/mock_rpc.rs`, `src/reorg/{detect,rollback}.rs`,
`migrations/0002_reorgs.sql`, `tests/reorg.rs`.

**Acceptance criteria.**
- Table-driven tests pass for: 1-block reorg, 5-block reorg, `MAX_REORG_DEPTH`-exceeding reorg (must halt, not guess), reorg arriving while the pipeline is behind head, and two reorgs in quick succession.
- After every reorg test, the invariant-checker query from Phase 5 is clean.
- Rollback is atomic — a failure mid-rollback leaves the database at the pre-rollback state, never partially rolled back.
- Orphaned transaction and log rows are fully gone; no orphaned `address_transactions` rows remain (verify the cascade actually reaches every dependent table).
- A `reorgs` row is written for each event, with correct depth and orphaned hashes.
- Indexer tracks and persists `finalized_number`, and logs a warning if a reorg is ever detected below it (that would indicate a bug or a genuinely extraordinary chain event).

**How to test.** Entirely against the mock — deterministic and fast enough for CI. Then, as a
smoke test only, run against mainnet and confirm the reorg counter stays at or near zero
without false positives. False positives here usually mean a caching or read-your-writes bug.

### Phase 7 — Crash recovery hardening

**What.** Not new features — *evidence*. A fuzz harness that kills the process at random
points and verifies the invariants hold every time, plus measurement of recovery time.

**Why.** By this phase the design should already make crashes safe (I2). This phase proves
it. "I designed for crash safety" is a claim; "I killed it 200 times at random offsets and
the invariant checker never failed" is evidence. That distinction is most of the interview
value.

**Concepts.** `SIGKILL` vs `SIGTERM` and why only `SIGKILL` tests durability (`SIGTERM`
exercises your graceful shutdown path, which is a *different* test); PostgreSQL WAL and
`fsync` guarantees; why an in-process checkpoint would have needed a two-phase commit;
at-least-once delivery plus idempotency as an alternative to exactly-once.

**Components.** `tests/crash_recovery.rs` or a shell/Python driver script,
`scripts/check_invariants.sql`, `src/pipeline/` hardening as bugs surface.

**Acceptance criteria.**
- 100+ `kill -9` cycles at randomized wall-clock offsets. After each: invariant checker clean, zero duplicate rows, zero gaps, cursor consistent with stored data.
- Recovery time from process start to first newly committed block is measured and reported (expected: dominated by connection setup and one RPC round trip, i.e. well under a second — if it is seconds, find out why).
- A crash *during* reorg rollback recovers correctly. This is the nastiest case and deserves its own test.
- No `unwrap()` or `expect()` on any error path reachable from the pipeline. Audit and document any that remain as provably infallible.
- Recovery requires no manual intervention and no repair tooling.

**How to test.** Driver script: start indexer against the mock RPC (fast, deterministic),
sleep a random 0.5–10s, `kill -9`, run the invariant SQL, restart, repeat. Fail the run on
the first violation and preserve the database for inspection.

### Phase 8 — Observability

**What.** Prometheus metrics endpoint, the full metric set, `tracing` spans on the hot path,
and a Grafana dashboard committed as JSON.

**Why.** Moved ahead of concurrency on purpose: the concurrency phase needs instrumentation
to produce a scaling curve instead of an anecdote. Queue-depth metrics in particular are how
you *see* backpressure working rather than assert that it does.

**Concepts.** Counter vs gauge vs histogram, and why latency must be a histogram (averages
hide the tail that actually hurts); why p99 matters more than mean for a pipeline with a
serialized stage; cardinality explosions (never label a metric with block number or
address); the difference between indexing *lag* (blocks behind head) and indexing *rate*.

**Metric set.**

| Metric | Type | Why it matters |
|---|---|---|
| `blocks_indexed_total`, `transactions_indexed_total`, `logs_indexed_total` | counter | rate() gives blocks/sec, tx/sec, events/sec |
| `rpc_request_duration_seconds{method}` | histogram | the dominant cost; watch p99 |
| `rpc_requests_total{method,outcome}` | counter | retry and 429 rate |
| `db_commit_duration_seconds` | histogram | is the single writer actually a bottleneck? |
| `db_rows_written_total{table}` | counter | write amplification, incl. `address_transactions` |
| `pipeline_queue_depth{stage}` | gauge | **the backpressure story** — shows where the constraint is |
| `sequencer_buffer_size` | gauge | head-of-line blocking detector |
| `indexing_lag_blocks` | gauge | head height minus cursor; the headline SLO |
| `reorgs_total`, `reorg_depth` | counter, histogram | correctness in production |
| `worker_busy_seconds_total` | counter | utilization; derive idle fraction |
| `process_resident_memory_bytes` | gauge | bounded-memory claim |

**Components.** `src/metrics.rs`, `/metrics` route, `docker/prometheus.yml`,
`docker/grafana/dashboard.json`, docker-compose additions.

**Acceptance criteria.**
- `/metrics` serves valid Prometheus text format; `promtool check metrics` passes.
- Every metric in the table above is populated and moves as expected under load.
- Grafana dashboard is committed and reproduces from `docker compose up` with no manual clicking.
- `tracing` spans wrap block fetch, decode, and commit, carrying block number as a span field (not a metric label).
- Metric cardinality is bounded — no unbounded label values. State the total series count.

**How to test.** Run the indexer, scrape `/metrics`, and confirm `rate(blocks_indexed_total)`
matches externally observed throughput. Then artificially slow the database (reduce pool size
to 1, or inject `pg_sleep`) and confirm `pipeline_queue_depth` rises — proving the metric
actually observes the bottleneck rather than just existing.

### Phase 9 — Concurrency

**What.** Insert a bounded worker pool and a reorder buffer in front of the *unchanged*
committer. Bounded channels for backpressure. Configurable worker count. Graceful shutdown
that drains rather than drops.

**Why.** The performance payoff, and — because the committer needs no changes — the proof
that the Phase 5 design was correct. If you find yourself modifying reorg or cursor logic
in this phase, stop: that means the architecture is wrong and we should discuss it.

**Concepts.** `tokio::spawn` and the `Send + 'static` requirement, and why the pure decode
function from Phase 3 makes this trivial; bounded vs unbounded channels (an unbounded queue
does not remove backpressure, it converts it into unbounded memory growth and eventual OOM
— this is the single most common async mistake); `JoinSet` for supervising a task group;
head-of-line blocking in the reorder buffer and why the buffer must be bounded too;
Little's Law as the model for choosing worker count; `spawn_blocking` and whether decode is
CPU-heavy enough to warrant it (measure — probably not).

**Components.** `src/pipeline/{worker,sequencer}.rs`, scheduler changes for dispatch,
config for `WORKER_COUNT`, `FETCH_QUEUE_DEPTH`, `SEQUENCER_CAPACITY`.

**Acceptance criteria.**
- Committer, reorg, and storage code are **unmodified** from Phases 4–7 except for the input source. Verify with `git diff --stat`; this is the headline claim of the phase.
- Blocks commit in strictly ascending contiguous order despite out-of-order completion — assert this in the committer and make the assertion a hard error, not a log line.
- Every channel is bounded. Resident memory stays flat over a multi-hour run regardless of worker count. Graph it.
- Under a slow database, workers block on the full channel rather than accumulating results — visible in `pipeline_queue_depth`.
- Ctrl-C drains in-flight work and commits what is complete; no partial blocks.
- Worker count is runtime-configurable and throughput improves measurably from 1 → 8 workers.
- All Phase 6 reorg tests and Phase 7 crash tests still pass unchanged, now with concurrency enabled.
- One worker failing permanently does not wedge the pipeline; it is either retried or the process fails loudly. No silent stalls.

**How to test.** Re-run the entire Phase 6 and 7 suites with `WORKER_COUNT` set to 1, 4, and
16 — concurrency bugs are often count-sensitive. Add a mock RPC mode that returns blocks in
deliberately shuffled order with randomized delays, to force the reorder buffer to work hard.

### Phase 10 — Query API

**What.** A separate `axum` binary serving four endpoints over the indexed data, with
pagination and proper error handling.

```
GET /block/{number_or_hash}
GET /transaction/{hash}
GET /address/{address}/transactions?limit&before_block
GET /contract/{address}/events?topic0&from_block&to_block&limit
GET /health   GET /status     -- cursor, head, lag, finalized
```

**Why.** Makes the indexed data usable and demonstrates that the schema was designed for
reads, not just writes. `/status` is also the endpoint that makes the indexing lag visible
to a human, which matters more than it sounds.

**Concepts.** Keyset (cursor) pagination vs `OFFSET`, and why `OFFSET` degrades linearly on
deep pages while keyset stays constant; `EXPLAIN ANALYZE` and reading a query plan; why the
`address_transactions` table turns the hot endpoint into a single index scan; separating the
read binary from the write binary so a query storm cannot slow indexing.

**Components.** `src/bin/api.rs`, `src/api/{routes,handlers,dto}.rs`, `tests/api.rs`.

**Acceptance criteria.**
- All endpoints return correct data, spot-verified against Etherscan for at least three known addresses and contracts.
- Pagination is keyset-based and stable under concurrent indexing — no duplicated or skipped rows when new blocks arrive mid-pagination.
- `EXPLAIN ANALYZE` output for each endpoint is committed to `docs/query-plans.md`, showing index scans and no sequential scans on large tables.
- Every endpoint responds in <50 ms p99 at the target dataset size — measured, not assumed.
- Invalid input (bad hex, unknown hash, out-of-range block) returns a structured 4xx, never a 500 or a panic.
- `limit` is capped server-side so a client cannot request a million rows.
- **The API is read-only and unauthenticated.** This is acceptable for a portfolio project but must be stated explicitly in the README: it binds to localhost by default and exposes no mutating routes. Do not deploy it publicly without adding auth and rate limiting.

**How to test.** Integration tests against a database seeded with fixture blocks. Load-test
the address endpoint with a deep-pagination loop to confirm constant-time behavior.

**Optional (only if ahead of schedule).** WebSocket `/ws/blocks` streaming newly committed
blocks. Note the design constraint: the stream must emit reorg notifications too, or
downstream consumers silently diverge. That subtlety makes it a good stretch goal and a bad
rushed one.

### Phase 11 — Benchmarking

**What.** A reproducible benchmark suite, results committed to the repo, and a written
analysis explaining *why* performance scales and where it stops.

**Why.** This phase is what converts "I built an indexer" into "I built a high-performance
indexer and here is the evidence." An unsubstantiated performance claim in an interview is
worse than no claim, because the follow-up question exposes it.

**Three environments, so cost is attributable.** The single most important idea in this
phase: you cannot benchmark your pipeline through a rate-limited third-party provider,
because you will only ever measure *their* throttle.

| Env | Setup | Isolates | Answers |
|---|---|---|---|
| **E1 Replay** | Mock RPC serving fixtures from local disk, zero added latency | decode + DB write path | What is my ceiling? Where does CPU go? Is the committer the bottleneck? |
| **E2 Synthetic latency** | Same fixtures, mock injects configurable latency and rate limit | concurrency behavior | Does throughput scale with workers per Little's Law? Where and why does it flatten? |
| **E3 Real provider** | Actual hosted RPC | reality | What do I actually get, and what is the binding constraint? |

**Experiments.**

1. **Sequential vs concurrent.** Worker count 1, 2, 4, 8, 16, 32, 64 in E2. Plot throughput and p50/p95/p99 commit latency. Expect near-linear then flat. *Be able to explain the flattening in each environment:* in E1 it is the serialized committer or CPU; in E2 it is the injected rate limit or reorder-buffer head-of-line blocking; in E3 it is the provider's compute-unit budget. Different causes, same-shaped curve — knowing which is which is the whole point.
2. **DB batch size.** 1, 10, 50, 100, 500 blocks per transaction. Throughput vs. recovery granularity. Note the real tradeoff: larger transactions are faster but a crash redoes up to N blocks and locks are held longer. Correctness is unaffected (I2 still holds) — only recovery cost changes.
3. **Insert strategy.** Single-row `INSERT` vs multi-row `INSERT` vs `UNNEST` arrays vs `COPY BINARY`. Expect a large win for `COPY`. "I got Nx on the write path by moving from row-at-a-time inserts to batched `COPY`" is a more impressive sentence than any framework choice.
4. **Index cost.** Backfill with indexes present vs. created afterwards. Quantify write amplification, including the cost of `address_transactions`.
5. **Crash recovery.** Distribution of recovery time over 100 `kill -9` cycles, and the invariant-violation count (target: zero).
6. **Reorg cost.** Injected reorgs at depth 1, 5, 20, 50. Rollback latency vs. depth, and rows deleted vs. depth.
7. **Bottleneck injection.** Independently throttle RPC and database. Confirm queue depths shift to identify the constraint, memory stays flat, and the system degrades gracefully instead of OOMing. This is the experiment that *proves* backpressure works.
8. **Stretch: PostgreSQL vs RocksDB** on the write path only. Frame it honestly — RocksDB offers no query flexibility, so this is not "which database is better," it is "what does the ingest path cost when you remove SQL, indexes, and transactions." Expect RocksDB to win on raw writes and to be unusable for the Phase 10 endpoints. Saying that plainly is stronger than pretending it is a fair fight.

**Reproducibility requirements (non-negotiable).** Every result file records: git SHA, full
config, hardware, PostgreSQL version and relevant settings, dataset block range, wall-clock
duration, and the raw metric series. A benchmark that cannot be re-run is an anecdote.

**Components.** `benches/harness/*`, `benches/results/*.csv`, `scripts/plot.py`,
`docs/BENCHMARKS.md`.

**Acceptance criteria.**
- All eight experiments (seven if RocksDB is dropped) executed with results committed.
- Each result reproducible from a single documented command.
- `docs/BENCHMARKS.md` contains graphs *and prose explaining causation*, not just numbers.
- The sequential baseline from Phase 5 and the concurrent result are compared directly, with the speedup stated and explained.
- For every curve that flattens, the document names the specific binding constraint and the evidence for it.
- At least one result is genuinely surprising or negative, and is reported anyway. Honest negative results are a credibility signal.

### Phase 12 — Documentation and polish

**What.** `README.md` with a working quickstart, `docs/ARCHITECTURE.md` with the diagram and
data flow, `docs/DESIGN_DECISIONS.md` recording each significant choice with its rejected
alternatives, `docs/BENCHMARKS.md`, and an architecture diagram rendered as an image.

**Why.** A reviewer spends 90 seconds on your repo before deciding whether to read the code.
The README decides that. And `DESIGN_DECISIONS.md` is the artifact that makes the interview
conversation easy, because you will have already written down the answer to every "why did
you do it that way" question.

**Acceptance criteria.**
- A stranger can go from `git clone` to a running indexer using only the README, on a clean machine. Test this literally — fresh clone, fresh container, follow your own instructions verbatim.
- `DESIGN_DECISIONS.md` covers at minimum: single-writer committer, cursor-in-transaction, hard-delete-on-reorg, `NUMERIC(78,0)` for uint256, denormalized address index, bounded channels, one crate not a workspace, and no message broker. Each with the alternative considered and why it lost.
- The security posture is stated: unauthenticated read-only API bound to localhost, secrets via env only, no private keys anywhere in the project.
- Known limitations are listed honestly, including: no genesis backfill, single chain, no internal call traces, API not production-hardened.
- No dead code, no commented-out blocks, no TODOs left in `main`.

## 11. MVP definition

**MVP = Phases 1–7, plus a minimal Phase 10.**

Concretely, the MVP is a system that: continuously indexes Ethereum mainnet blocks,
transactions, receipts, and logs into PostgreSQL; decodes ERC-20/721 transfers; detects and
correctly rolls back chain reorganizations, proven by a deterministic test suite; survives
`kill -9` at any point with zero gaps, zero duplicates, and no manual repair; and serves the
four read endpoints.

Note what is *in* the MVP that a typical version of this project would defer: reorg handling
and proven crash recovery. Those are in the MVP because they are the interview story. A
concurrent indexer that corrupts on reorg is a worse project than a sequential one that does
not, and the sequential-first ordering means you always have something defensible.

Note what is *not* in the MVP: concurrency. This is intentional. The MVP is the correctness
baseline; performance is measured against it.

**What makes it resume-strong:** Phases 8, 9, and 11. Without benchmarks you cannot use the
words "high performance," so if time runs short, cut scope elsewhere — cut the API to two
endpoints, cut the Grafana dashboard, cut RocksDB — but do not cut Phase 11.

**Priority order if time runs out**, most to least important: correctness (1–7) → benchmarks
(11) → documentation (12) → concurrency (9) → observability (8) → API polish (10) → stretch
goals. Concurrency ranks below benchmarks because a measured sequential system with an honest
analysis of where its bottleneck lies is more impressive than an unmeasured concurrent one.

**Rough schedule** for 4–6 weeks of evenings and weekends:

| Week | Phases |
|---|---|
| 1 | 1, 2, 3 |
| 2 | 4, 5 |
| 3 | 6, 7 |
| 4 | 8, 9 |
| 5 | 10, 11 |
| 6 | 11 (finish), 12, buffer |

Week 3 is the hardest and most valuable week. Do not compress it.

## 12. The cut list

If the schedule slips, these go first, in this order: RocksDB comparison → WebSocket
streaming → Grafana dashboard (keep raw `/metrics`) → table partitioning →
`/contract/{address}/events` filtering options → ERC-721 decoding (keep ERC-20).

These never get cut: single-transaction commits, parent-hash validation, the reorg test
suite, the crash-recovery fuzz harness, benchmark reproducibility metadata.

## 13. What you will be able to claim at the end

The point of the project is this list of sentences, each backed by something in the repo:

**Architecture.** "The pipeline fans out for network I/O and decoding, which are
order-independent, and funnels to a single writer for commits, which must be ordered. That
asymmetry is deliberate: the bottleneck is RPC round-trips by roughly an order of magnitude,
so serializing commits costs almost nothing and buys the entire correctness story. I measured
this rather than assuming it — see experiment 1."

**Reorgs.** "Detection is a parent-hash linkage check at commit time. On mismatch I walk back
to the common ancestor, then in one PostgreSQL transaction I write an audit row, cascade-delete
everything above the ancestor, and reset the cursor. If I can't find an ancestor within 128
blocks I halt rather than guess, because silent corruption is worse than downtime. I test it
against a scriptable mock RPC because waiting for a real mainnet reorg is not a test strategy."

**Concurrency.** "Bounded channels throughout, so backpressure is structural rather than
something I hope for — an unbounded queue doesn't remove backpressure, it converts it into
unbounded memory growth. Worker count follows Little's Law against RPC latency. Adding
concurrency required zero changes to the commit, reorg, or storage code, which is the evidence
that the sequential design was right."

**Crash recovery.** "The cursor advances only inside the same database transaction that writes
the block's data. That one decision makes recovery a single `SELECT` instead of a two-phase
commit problem. Combined with natural primary keys on every chain-derived table, reprocessing
is idempotent. I verified it with 100+ randomized `kill -9` cycles including one during reorg
rollback, checked against an invariant query each time."

**Performance.** "Sequential baseline was X blocks/sec; concurrent is Y. It scales linearly to
N workers and then flattens — in the replay environment because of the write path, and against
the real provider because of the compute-unit rate limit. I know which is which because I
benchmarked in three separate environments to make the cost attributable. The largest single
win was Z, from moving the write path to batched `COPY`."

**Judgment.** "I left out Kafka, Redis, and Kubernetes because a bounded in-process channel
gives the same backpressure semantics for a single-writer pipeline and PostgreSQL already
provides durability. Every abstraction in the codebase is there for a reason I can name —
there are exactly two traits, and each exists to make testing or benchmarking possible."

---

## Working agreement

One phase at a time. At the end of each phase, I review what you built against that phase's
acceptance criteria before we move on. If there is an architectural mistake, I explain the
reasoning and the tradeoff rather than quietly fixing it — the goal is that you can defend
every decision, which requires having made it yourself.
