#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== ChainLens Benchmark Suite ==="
echo ""

if [ -z "${DATABASE_URL:-}" ]; then
    export DATABASE_URL="postgres://chainlens:chainlens@localhost:5432/chainlens"
fi

echo "database: $DATABASE_URL"
echo ""

echo "--- Decode benchmarks (criterion) ---"
cargo bench --bench decode -- --quick
echo ""

echo "--- Pipeline benchmarks ---"
cargo run --release --bin benchmark_harness -- all
echo ""

echo "=== Done ==="
echo "Results in benches/results/"
