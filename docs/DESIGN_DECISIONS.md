# Design Decisions

Each significant choice in ChainLens with the rejected alternative and why it lost.

## 1. Single-writer committer

**Decision:** All writes to the canonical chain state funnel through a single task.

**Rejected alternative:** Parallel writes with a gap-set cursor.

**Why:** The bottleneck is RPC round-trips, not the database. A single PostgreSQL connection doing batched inserts handles thousands of rows per second. Meanwhile, a single fetch worker at 50ms latency manages ~10 blocks/sec. Serializing the commit stage buys the entire correctness story for approximately zero throughput cost. If measurement in Phase 11 proves the committer is the binding constraint, backfill blocks (which are immutable below `finalized`) can safely be parallelized — but only if the data says we need it.

## 2. Cursor in the same transaction as the data

**Decision:** The `indexer_state` cursor advances inside the same PostgreSQL transaction that writes the block's header, transactions, logs, and address index.

**Rejected alternative:** A separate checkpoint file or a second database write.

**Why:** A separate checkpoint turns restart safety into a two-phase commit problem. If the process crashes after writing block data but before updating the checkpoint, the block is stored but the cursor doesn't advance — the indexer reprocesses it. If the checkpoint advances but the data write fails, there's a gap. Putting both in one transaction makes crash recovery a single `SELECT`: resume at `cursor + 1`.

## 3. Hard-delete on reorg, not soft-delete

**Decision:** Orphaned data is deleted via `ON DELETE CASCADE` from `blocks`. The `reorgs` audit table preserves the history.

**Rejected alternative:** A `canonical BOOLEAN` flag on every row, with `WHERE canonical = true` on every read path.

**Why:** Soft deletion poisons every query with a filter, lets dead rows accumulate indefinitely, and adds complexity to every index. Hard deletion keeps read paths clean. The `reorgs` table records what was orphaned and why, which is the forensic history that actually matters.

## 4. NUMERIC(78,0) for uint256

**Decision:** Ethereum's 256-bit unsigned integers are stored as PostgreSQL `NUMERIC(78,0)`.

**Rejected alternative:** `BYTEA(32)` — fixed-width 32-byte columns.

**Why:** 2^256 ≈ 1.16 × 10^77, so 78 digits covers it. `NUMERIC` is sortable and arithmetic-capable in SQL (`ORDER BY value DESC`, `SUM(value)`), which `BYTEA` is not. The cost is slower writes and more storage than fixed-width bytes — a tradeoff that Phase 11 quantifies.

## 5. Denormalized address_transactions index

**Decision:** A separate `address_transactions` table with one row per (address, direction).

**Rejected alternative:** Querying `transactions` with `WHERE from_addr = X OR to_addr = X`.

**Why:** PostgreSQL cannot serve `OR` across two columns from a single index. You get either a bitmap OR of two index scans or a `UNION ALL`, both of which sort poorly under pagination. The denormalized table turns "all transactions for an address" into a single ordered index scan. The cost is ~2× row amplification on writes — exactly the kind of thing a benchmark should quantify.

## 6. Natural primary keys, no sequences

**Decision:** Every chain-derived table uses a primary key derived from chain data: `blocks.number`, `transactions.hash`, `logs.(block_number, log_index)`.

**Rejected alternative:** `BIGSERIAL` auto-incrementing IDs.

**Why:** Natural keys make writes idempotent. Reprocessing block N produces identical rows. With `BIGSERIAL`, reprocessing creates duplicates. Combined with `ON CONFLICT DO NOTHING`, this means the decode→commit path can be retried safely at any point.

## 7. Bounded channels throughout

**Decision:** Every channel between pipeline stages is bounded.

**Rejected alternative:** Unbounded channels for simplicity.

**Why:** An unbounded queue does not remove backpressure — it converts it into unbounded memory growth and an eventual out-of-memory kill. Bounding them means a slow database propagates backwards as workers blocking on a full channel, which is observable in queue-depth metrics rather than as a crash.

## 8. One crate, not a workspace

**Decision:** A single Cargo crate with a library target and two binaries (`chainlens`, `api`).

**Rejected alternative:** A Cargo workspace with separate crates for domain, rpc, store, etc.

**Why:** A workspace buys compile-time parallelism and independent versioning that a single-author 4-6 week project does not need. It adds friction to sharing domain types. Split later if the API and indexer need independent deployment, or if clean-build time exceeds a minute.

## 9. No message broker

**Decision:** Bounded in-process channels, not Kafka or Redis.

**Rejected alternative:** Kafka for the fetch→commit pipeline.

**Why:** A bounded in-process channel provides the same backpressure semantics for a single-writer pipeline. PostgreSQL already provides the durability a broker would be added for. Introducing Kafka would add an operational component without adding a capability.

## 10. Hand-rolled JSON-RPC transport

**Decision:** The RPC client is implemented directly over `reqwest`, not using a provider library.

**Rejected alternative:** Using `alloy`'s provider abstraction or `ethers-rs`.

**Why:** For a portfolio piece that is specifically about systems programming, writing the transport yourself demonstrates understanding of the wire protocol and gives direct control over request batching, which a high-level provider abstraction tends to hide. `alloy-primitives` is used for the primitive types (Address, B256, U256) — reimplementing keccak or U256 would not be defensible.

## 11. eth_getBlockReceipts over per-transaction receipts

**Decision:** Use `eth_getBlockReceipts` when available, falling back to per-transaction `eth_getTransactionReceipt`.

**Rejected alternative:** Always use per-transaction receipts.

**Why:** At 200 transactions per block, per-transaction receipts cost 201 round trips per block. `eth_getBlockReceipts` reduces that to 2. This single change is a ~100× reduction in request count and matters more to end-to-end throughput than any amount of optimization inside the process.

## 12. Reorgs tested against a mock, not mainnet

**Decision:** A scriptable mock RPC that serves a synthetic chain and can be told to switch to a competing fork at a chosen height.

**Rejected alternative:** Wait for a real mainnet reorg to test against.

**Why:** Post-Merge Ethereum reorganizations are rare and usually one block deep. Waiting for one is not a test strategy, and an indexer whose reorg path has never executed is an indexer whose reorg path does not work.
