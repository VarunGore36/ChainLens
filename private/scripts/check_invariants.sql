-- ChainLens Invariant Checker
--
-- Run after crash recovery to verify all invariants hold.
-- Returns rows only if violations are found; empty result = clean.
--
-- Usage:
--   psql -f scripts/check_invariants.sql chainlens

-- I1: Contiguity — no gaps in blocks.number up to the cursor
WITH expected AS (
    SELECT generate_series(
        (SELECT COALESCE(MIN(number), 0) FROM blocks),
        (SELECT COALESCE(last_indexed_number, 0) FROM indexer_state WHERE id = 1)
    ) AS number
)
SELECT 'gap' AS violation, e.number
FROM expected e
LEFT JOIN blocks b ON b.number = e.number
WHERE b.number IS NULL AND e.number > 0;

-- I1: Contiguity — parent_hash linkage
SELECT 'parent_hash_mismatch' AS violation,
       b.number,
       b.hash,
       b.parent_hash,
       prev.hash AS expected_parent_hash
FROM blocks b
JOIN blocks prev ON prev.number = b.number - 1
WHERE b.parent_hash != prev.hash
  AND b.number > 0;

-- I2: Atomic cursor — cursor must point to a block that exists
SELECT 'cursor_orphan' AS violation,
       last_indexed_number,
       last_indexed_hash
FROM indexer_state
WHERE id = 1
  AND last_indexed_number IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM blocks WHERE number = last_indexed_number
  );

-- I4: Idempotent writes — no duplicate block numbers
SELECT 'duplicate_block' AS violation, number, COUNT(*)
FROM blocks
GROUP BY number
HAVING COUNT(*) > 1;

-- I4: Idempotent writes — no duplicate transaction hashes
SELECT 'duplicate_tx' AS violation, hash, COUNT(*)
FROM transactions
GROUP BY hash
HAVING COUNT(*) > 1;

-- Log index contiguity within each block
SELECT 'log_index_gap' AS violation,
       block_number,
       log_index,
       LAG(log_index) OVER (PARTITION BY block_number ORDER BY log_index) AS prev_index
FROM logs
WHERE log_index != 0
  AND log_index != LAG(log_index) OVER (PARTITION BY block_number ORDER BY log_index) + 1;

-- Cursor consistency — last_indexed_hash must match stored block hash
SELECT 'cursor_hash_mismatch' AS violation,
       s.last_indexed_number,
       s.last_indexed_hash,
       b.hash AS stored_hash
FROM indexer_state s
JOIN blocks b ON b.number = s.last_indexed_number
WHERE s.id = 1
  AND s.last_indexed_hash != b.hash;
