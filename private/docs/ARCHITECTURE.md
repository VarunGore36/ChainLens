# Architecture

Data flow, invariants, and schema rationale for ChainLens.

## Pipeline

```
  Ethereum JSON-RPC
         │
         ▼
  ┌─────────────┐
  │ Head Watcher│  polls latest / safe / finalized every ~4s
  └──────┬──────┘
         │ target height, finalized height
         ▼
  ┌─────────────┐
  │  Scheduler  │  decides what to fetch next; owns rewind-on-reorg
  └──────┬──────┘
         │ bounded channel <block numbers>
         ▼
  ┌─────────────────────────────┐
  │  Fetch + Decode Workers (N) │  eth_getBlockByNumber + eth_getBlockReceipts
  │  → raw JSON → IndexedBlock  │  RPC I/O + CPU decode
  └──────────────┬──────────────┘
         │ bounded channel <IndexedBlock>
         ▼
  ┌─────────────┐
  │  Sequencer  │  reorder buffer: releases blocks in strictly ascending order
  └──────┬──────┘
         │
  ┌──────▼──────┐         ┌───────────────┐
  │  Committer  │────────▶│ Reorg Handler │
  │ SINGLE WRITER│ parent │ ancestor walk │
  └──────┬──────┘ mismatch└──────┬──────┘
         │                       │
         ▼   one PG transaction  ▼
  ┌──────────────────────────────────┐
  │           PostgreSQL             │
  └──────────────────────────────────┘
```

## Data flow

1. **Head Watcher** polls the chain head (`latest`, `safe`, `finalized` tags) every 4 seconds.
2. **Scheduler** compares the cursor (last committed block) against the head. Dispatches the next block number to the fetch queue.
3. **Workers** (N concurrent tasks) receive block numbers, fetch the block + receipts via JSON-RPC, decode them into `IndexedBlock`.
4. **Sequencer** receives decoded blocks (possibly out of order) and releases them in strictly ascending contiguous order.
5. **Committer** receives ordered blocks, checks for reorgs, and writes everything in one PostgreSQL transaction.
6. If a reorg is detected, the **Reorg Handler** finds the common ancestor, cascade-deletes orphaned data, and resets the cursor — all in one transaction.

## Invariants

### I1 — Contiguity and completeness

For every block number `n ≤ cursor.height`, block `n` and all of its transactions, logs, and decoded transfers are durably stored, and `blocks[n].parent_hash == blocks[n-1].hash`. There are no gaps and no partial blocks below the cursor.

### I2 — Atomic cursor

The cursor advances only inside the same PostgreSQL transaction that writes the block data it refers to. There is no checkpoint file and no second write to coordinate. Recovery is a single `SELECT last_indexed_number FROM indexer_state` — resume at `n+1`.

### I3 — Single writer

Exactly one task mutates canonical chain state. Fetching and decoding are parallel and order-free; committing is serial and strictly ordered.

### I4 — Idempotent writes

Every row's primary key is derived from chain data, never from a database sequence. Reprocessing block `n` yields identical state. No `SERIAL`/`BIGSERIAL` primary keys on any chain-derived table.

### I5 — Fail-stop on ambiguity

If the indexer cannot establish chain linkage within `MAX_REORG_DEPTH` (default 128 blocks), it halts loudly rather than guessing. Downtime is recoverable; silent corruption is not.

## Schema rationale

### blocks

- `number BIGINT PRIMARY KEY` — works because we hard-delete on reorg, so there's never more than one block per height.
- `hash BYTEA NOT NULL UNIQUE` — used for parent-hash linkage validation.

### transactions

- `hash BYTEA PRIMARY KEY` — natural key, idempotent writes. The same transaction hash can appear in both an orphaned and a canonical block, but never simultaneously (rollback and re-insert happen in one transaction).
- `block_number BIGINT REFERENCES blocks(number) ON DELETE CASCADE` — cascade delete ensures orphaned transactions are removed on reorg.

### logs

- `PRIMARY KEY (block_number, log_index)` — composite key, idempotent. Log indices are contiguous within a block (validated at decode time).

### token_transfers

- `PRIMARY KEY (block_number, log_index)` — FK to logs, cascade delete.

### address_transactions

- `PRIMARY KEY (address, block_number, tx_index, direction)` — denormalized for single-index-scan queries. One row per (address, direction) turns the hot API endpoint into a single ordered index scan.

### indexer_state

- Single row (`id = 1`), updated in the same transaction as block data. No separate checkpoint.

### reorgs

- Audit trail that survives rollback. Records common ancestor depth, orphaned block range, and orphaned hashes.

## Crash recovery

Crash safety is tested, not assumed. The cursor lives in the same transaction as the data, making recovery a single `SELECT`. Idempotent writes (natural primary keys, no sequences) mean reprocessing a block produces identical state.

Verified by 100+ randomized `kill -9` cycles in Phase 7, including crashes during reorg rollback.

## Observability

Prometheus metrics on `/metrics`:

| Metric | Type | What it shows |
|--------|------|---------------|
| `chainlens_blocks_indexed_total` | counter | blocks/sec |
| `chainlens_transactions_indexed_total` | counter | tx/sec |
| `chainlens_logs_indexed_total` | counter | events/sec |
| `chainlens_rpc_request_duration_seconds` | histogram | RPC latency (p99) |
| `chainlens_db_commit_duration_seconds` | histogram | DB commit latency |
| `chainlens_indexing_lag_blocks` | gauge | lag behind head |
| `chainlens_reorgs_total` | counter | reorg count |
| `chainlens_reorg_depth` | histogram | reorg depth distribution |
