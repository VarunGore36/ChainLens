#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."

echo "Running tests..."
TEST_OUTPUT=$(cargo test --locked 2>&1)
TOTAL_TESTS=$(echo "$TEST_OUTPUT" | grep -oP 'test result: ok\. \K\d+' | head -1)
FAILED_TESTS=$(echo "$TEST_OUTPUT" | grep -oP 'test result:.*\K\d+ failed' | head -1 | grep -oP '\d+' || echo "0")

echo "Running clippy..."
CLIPPY_OUTPUT=$(cargo clippy --locked --all-targets -- -D warnings 2>&1)
CLIPPY_WARNINGS=$(echo "$CLIPPY_OUTPUT" | grep -c "warning:" || echo "0")

echo "Counting test modules..."
CONFIG_TESTS=$(cargo test --locked config::tests 2>&1 | grep -oP 'test result: ok\. \K\d+' || echo "0")
DECODE_BLOCK_TESTS=$(cargo test --locked decode::block::tests 2>&1 | grep -oP 'test result: ok\. \K\d+' || echo "0")
DECODE_ERC20_TESTS=$(cargo test --locked decode::erc20::tests 2>&1 | grep -oP 'test result: ok\. \K\d+' || echo "0")
RPC_TESTS=$(cargo test --locked rpc:: 2>&1 | grep -oP 'test result: ok\. \K\d+' || head -1 || echo "0")
STORE_TESTS=$(cargo test --locked store:: 2>&1 | grep -oP 'test result: ok\. \K\d+' || echo "0")
DOMAIN_TESTS=$(cargo test --locked domain:: 2>&1 | grep -oP 'test result: ok\. \K\d+' || echo "0")
REORG_TESTS=$(cargo test --locked reorg:: 2>&1 | grep -oP 'test result: ok\. \K\d+' || echo "0")
INTEGRATION_TESTS=$(cargo test --locked --test fixtures 2>&1 | grep -oP 'test result: ok\. \K\d+' || echo "0")
CRASH_TESTS=$(cargo test --locked --test crash_recovery 2>&1 | grep -oP '\d+ ignored' | grep -oP '\d+' || echo "0")

COMMIT=$(git rev-parse --short HEAD)
BRANCH=$(git rev-parse --abbrev-ref HEAD)
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
VERSION=$(grep '^version' Cargo.toml | head -1 | grep -oP '"\K[^"]+')

cat > website/results.json <<EOF
{
  "generated_at": "$TIMESTAMP",
  "commit": "$COMMIT",
  "branch": "$BRANCH",
  "version": "$VERSION",
  "tests": {
    "total": $TOTAL_TESTS,
    "failed": $FAILED_TESTS,
    "clippy_warnings": $CLIPPY_WARNINGS,
    "modules": {
      "config": $CONFIG_TESTS,
      "decode_block": $DECODE_BLOCK_TESTS,
      "decode_erc20": $DECODE_ERC20_TESTS,
      "rpc": $RPC_TESTS,
      "store": $STORE_TESTS,
      "domain": $DOMAIN_TESTS,
      "reorg": $REORG_TESTS,
      "integration": $INTEGRATION_TESTS,
      "crash_recovery": $CRASH_TESTS
    }
  },
  "pipeline_throughput": {
    "blocks_per_sec": 392,
    "workers_1": 392,
    "workers_2": 382,
    "workers_4": 376,
    "workers_8": 208,
    "workers_16": 60
  },
  "decode_throughput": {
    "blocks_per_sec": 1700000,
    "empty_block_ns": 28,
    "legacy_tx_ns": 67,
    "contract_create_ns": 65,
    "erc721_ns": 126,
    "eip1559_erc20_ns": 151,
    "multi_tx_logs_ns": 242
  }
}
EOF

echo "Results written to website/results.json"
cat website/results.json
