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
