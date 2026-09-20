#!/bin/bash
set -euo pipefail

API_BASE="${1:-http://127.0.0.1:8080}"
CONCURRENT="${2:-10}"
TOTAL="${3:-100}"

echo "=== ChainLens Load Test ==="
echo "Target: $API_BASE"
echo "Concurrent requests: $CONCURRENT"
echo "Total requests: $TOTAL"
echo ""

check_endpoint() {
    local endpoint=$1
    local method=${2:-GET}
    local start=$(date +%s%N)
    
    if [ "$method" = "GET" ]; then
        response=$(curl -s -o /dev/null -w "%{http_code}" "$API_BASE$endpoint" 2>/dev/null || echo "000")
    else
        response=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" "$API_BASE$endpoint" 2>/dev/null || echo "000")
    fi
    
    local end=$(date +%s%N)
    local duration=$(( (end - start) / 1000000 ))
    
    echo "$response $duration"
}

run_load_test() {
    local endpoint=$1
    local name=$2
    local success=0
    local fail=0
    local total_time=0
    
    echo "Testing: $name ($endpoint)"
    
    for i in $(seq 1 $TOTAL); do
        result=$(check_endpoint "$endpoint")
        code=$(echo $result | cut -d' ' -f1)
        time=$(echo $result | cut -d' ' -f2)
        
        if [ "$code" = "200" ]; then
            ((success++))
        else
            ((fail++))
        fi
        ((total_time += time))
    done
    
    local avg_time=$((total_time / TOTAL))
    echo "  Results: $success success, $fail failed"
    echo "  Avg response time: ${avg_time}ms"
    echo ""
}

echo "=== Health Check ==="
run_load_test "/health" "Health"

echo "=== Status ==="
run_load_test "/status" "Status"

echo "=== Block Lookup ==="
run_load_test "/block/18000000" "Block by number"

echo "=== Transaction Lookup ==="
run_load_test "/transaction/0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" "Transaction (likely 404)"

echo "=== Address Intelligence ==="
run_load_test "/api/v1/addresses/0x0000000000000000000000000000000000000000/intelligence" "Address intelligence"

echo "=== Anomalies ==="
run_load_test "/api/v1/anomalies" "Anomalies"

echo "=== Block Analytics ==="
run_load_test "/api/v1/blocks/18000000/analytics" "Block analytics"

echo "=== API Docs ==="
run_load_test "/docs" "API docs"

echo "=== Load Test Complete ==="
