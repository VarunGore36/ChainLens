# ChainLens — Phase 1 & 2 Results

## Phase 1 — Foundation and scaffolding

**Status:** Complete.

### What was built

- Cargo project with library + binary targets
- Configuration via `clap` derive with environment variable fallback
- `RedactedUrl` type that prevents credentials from reaching log output
- Structured logging via `tracing` (text and JSON formats)
- PostgreSQL connection pool with timeout handling
- Cooperative shutdown via `CancellationToken` (SIGINT/SIGTERM)
- `docker-compose.yml` with PostgreSQL 17, tuned for development
- GitHub Actions CI (fmt, clippy, test, doc, audit)
- `.env.example` with all configuration documented

### Test results

```
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Verification

- `cargo clippy --locked --all-targets -- -D warnings` — clean
- `cargo fmt --all -- --check` — clean
- `cargo build --locked --release` — clean

---

## Phase 2 — RPC client layer

**Status:** Complete.

### What was built

A typed, resilient Ethereum JSON-RPC client behind an `EthClient` trait:

| Component | File | Responsibility |
|-----------|------|----------------|
| Trait + errors | `src/rpc/mod.rs` | `EthClient` trait, `RpcError` with retry classification, `BlockId`/`BlockTag` |
| Wire types | `src/rpc/types.rs` | JSON-RPC 2.0 framing, Ethereum response types with hex-quantity decoding |
| Retry | `src/rpc/retry.rs` | Exponential backoff with full jitter (AWS-recommended strategy) |
| Rate limiter | `src/rpc/ratelimit.rs` | Token-bucket via `governor` crate |
| HTTP transport | `src/rpc/http.rs` | Hand-rolled JSON-RPC 2.0 over `reqwest`, capability probe |

### Key design decisions

1. **Hand-rolled transport** over using a provider library — demonstrates understanding of the
   wire protocol and gives direct control over request batching (Phase 9).

2. **`EthClient` trait** — the seam that makes deterministic reorg testing (Phase 6) and
   honest benchmarking (Phase 11) possible. Two implementations: `HttpRpcClient` for
   production, mock RPC for testing.

3. **Full jitter** on retry — avoids thundering herds when N workers retry simultaneously.
   The expected retry time is half the ceiling, so it's both faster on average and kinder
   to the server than fixed delays.

4. **Capability probe at startup** — `eth_getBlockReceipts` support is tested once against
   the latest block. If the node supports it, all subsequent receipt fetches use that method
   (one round trip per block). Otherwise, falls back to per-transaction receipts. The result
   is logged at startup.

5. **Hex quantity vs hex data** — Ethereum JSON-RPC uses two incompatible hex conventions.
   `alloy-primitives` handles hex data (addresses, hashes) correctly. Custom deserializers
   handle hex quantities (block numbers, gas values) per the spec.

### Dependencies added

| Crate | Purpose |
|-------|---------|
| `async-trait` | Async methods in traits (needed for object safety) |
| `reqwest` | HTTP client (rustls-tls, matching sqlx's TLS choice) |
| `serde` / `serde_json` | Serialization for JSON-RPC framing |
| `alloy-primitives` | `Address`, `B256`, `U256` — the Ethereum building blocks |
| `governor` | Token-bucket rate limiter |
| `rand` | Jitter for retry backoff |

### Test results

```
running 40 tests
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

New tests added:
- 6 hex-quantity decoding edge cases (`0x0`, `0x`, empty string, missing prefix, typical values, large block numbers)
- 5 JSON deserialization tests (minimal block, receipt with status, pre-Byzantium receipt, EIP-1559 transaction, contract creation)
- 5 retry policy tests (zero delay, doubling, cap, floor, jitter variation)
- 2 rate limiter tests (delays excess requests, passes within-limit quickly)
- 4 RPC error classification tests (429, 5xx, timeout, method-not-found)
- 3 BlockId serialization tests (tag, number, all tags)

### Verification

- `cargo clippy --locked --all-targets -- -D warnings` — clean
- `cargo fmt --all -- --check` — clean
- `cargo build --locked --release` — clean

### Config additions

| Setting | Env var | Default | Purpose |
|---------|---------|---------|---------|
| `rpc_rate_limit` | `CHAINLENS_RPC_RATE_LIMIT` | 10 | Max RPC requests/sec |
| `rpc_timeout_secs` | `CHAINLENS_RPC_TIMEOUT_SECS` | 30 | Request timeout |
| `rpc_max_retries` | `CHAINLENS_RPC_MAX_RETRIES` | 3 | Retries for transient failures |

---

## What is next

Phase 3 — Domain model, validation, and ERC-20/721 decoding:
- Typed domain structs (`Block`, `Transaction`, `Log`, `TokenTransfer`)
- Pure function `raw_json → IndexedBlock` that validates as it decodes
- ERC-20/721 `Transfer` and `Approval` event decoding from log topics
- `criterion` micro-benchmarks for the decode path

---

## Phase 3 — Domain model, validation, and decoding

**Status:** Complete.

### What was built

| Component | File | Responsibility |
|-----------|------|----------------|
| Domain types | `src/domain/mod.rs` | `IndexedBlock`, newtype wrappers (`ValidatedAddress`, `ValidatedHash`) |
| Block | `src/domain/block.rs` | `Block` struct — header fields, primary key is `number` |
| Transaction | `src/domain/transaction.rs` | `Transaction` struct — merged request + receipt data |
| Log | `src/domain/log.rs` | `Log` struct — event data with separate topic columns |
| Token transfer | `src/domain/token_transfer.rs` | `TokenTransfer`, `TokenStandard` enum, `TRANSFER_TOPIC` / `APPROVAL_TOPIC` constants |
| Decode entry point | `src/decode/mod.rs` | `DecodeError` enum with typed validation errors |
| Block decode | `src/decode/block.rs` | `decode_block()` — the pure function: RPC response → `IndexedBlock` |
| ERC-20/721 decode | `src/decode/erc20.rs` | `decode_token_transfers()` — Transfer event classification |

### Key design decisions

1. **Domain types are separate from RPC types.** The RPC types (`src/rpc/types/`) are what
   arrives over HTTP. The domain types (`src/domain/`) are what gets stored and reasoned about.
   The boundary is the `decode` module, which converts while validating.

2. **Merged transaction + receipt.** The RPC response splits transactions and receipts into
   separate objects. The domain model merges them because every consumer needs both. Splitting
   them would force every reader to do the join.

3. **Pure decode function.** `decode_block()` is synchronous, side-effect-free, and takes
   only its inputs. This makes it unit-testable against fixtures with no network or database,
   micro-benchmarkable with `criterion`, and safe to run in parallel (Phase 9 worker pool).

4. **ERC-20 vs ERC-721 classification by topic count.** Both standards share the same
   `Transfer(address,address,uint256)` event signature. They are distinguished by topic count:
   - 3 topics (topic0, from, to) → ERC-20: `value` in `data`
   - 4 topics (topic0, from, to, tokenId) → ERC-721: `data` is empty

5. **Newtype wrappers.** `ValidatedAddress` and `ValidatedHash` prevent accidental mixing of
   addresses and hashes at the type level. They deref to the underlying alloy-primitives types,
   so they are zero-cost.

6. **Hardened input handling.** The decode functions never panic on malformed input. Missing
   fields, wrong lengths, and inconsistencies all return typed `DecodeError` variants. This
   is verified by a fuzz-ish test that decodes every fixture.

### Validation checks

- Receipt count matches transaction count
- Receipt tx hashes match transaction hashes in order
- Log indices are contiguous within the block
- ERC-20 Transfer data is exactly 32 bytes
- Transfer topics contain valid addresses

### Test results

```
running 61 tests (54 unit + 7 integration)
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

New tests added:
- 5 block decode tests (empty block, EIP-1559 fields, receipt count mismatch, hash mismatch, log index gap)
- 7 ERC-20/721 decode tests (ERC-20 transfer, ERC-721 transfer, skips non-Transfer, skips wrong topic count, rejects bad data length, multiple transfers, address round-trip)
- 2 domain constant tests (Transfer topic matches keccak256, Approval topic matches keccak256)
- 7 integration tests against fixture files (empty block, legacy tx, EIP-1559 + ERC-20, ERC-721, contract creation, multi-tx, all-fixtures-no-panic)

### Benchmark results

```
decode_empty_block           time:   [28.3 ns]
decode_legacy_tx             time:   [67.2 ns]
decode_eip1559_with_erc20   time:   [150.6 ns]
decode_erc721                time:   [125.6 ns]
decode_contract_creation     time:   [65.1 ns]
decode_multi_tx_with_logs   time:   [241.5 ns]
```

The decode path is pure CPU — no I/O, no allocations beyond the output vectors. At ~150 ns
per block with ERC-20 transfers, the decode stage will not be the bottleneck even at 1000
blocks/sec (which would use ~150 µs of CPU per second).

### Test fixtures

Six JSON fixtures in `benches/fixtures/`:

| Fixture | Content |
|---------|---------|
| `empty_block` | Block with no transactions |
| `legacy_tx` | Legacy (type 0) transaction |
| `eip1559_erc20_transfer` | EIP-1559 transaction with ERC-20 Transfer event |
| `erc721_transfer` | ERC-721 NFT Transfer event (4 topics) |
| `contract_creation` | Contract creation (to = null) |
| `multi_tx_with_logs` | 2 transactions, 2 logs (Transfer + Approval) |

These fixtures are reused as the benchmark corpus in Phase 11.

### Dependencies added

| Crate | Purpose |
|-------|---------|
| `criterion` (dev) | Micro-benchmarks for the decode path |

### Verification

- `cargo clippy --locked --all-targets -- -D warnings` — clean
- `cargo fmt --all -- --check` — clean
- `cargo build --locked --release` — clean
- `cargo bench --bench decode -- --quick` — all benchmarks pass
