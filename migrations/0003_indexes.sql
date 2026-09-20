-- Performance optimization indexes for ChainLens Intelligence Engine

-- Transaction actions lookup by tx_hash (already exists)
-- CREATE INDEX IF NOT EXISTS idx_tx_actions_hash ON transaction_actions(tx_hash);

-- Address stats lookup (primary key already exists)

-- Address relationships - composite index for graph traversal
CREATE INDEX IF NOT EXISTS idx_rel_from_type ON address_relationships(from_addr, relationship_type);
CREATE INDEX IF NOT EXISTS idx_rel_to_type ON address_relationships(to_addr, relationship_type);

-- Anomalies - index for time-range queries
CREATE INDEX IF NOT EXISTS idx_anomalies_type_time ON anomalies(anomaly_type, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_anomalies_severity_time ON anomalies(severity, detected_at DESC);

-- MEV events - index for block queries
CREATE INDEX IF NOT EXISTS idx_mev_type_block ON mev_events(mev_type, block_number DESC);

-- Contract profiles - index for creator lookup
CREATE INDEX IF NOT EXISTS idx_contract_creator ON contract_profiles(creator);

-- Transactions - composite indexes for common queries
CREATE INDEX IF NOT EXISTS idx_tx_from_block ON transactions(from_addr, block_number DESC);
CREATE INDEX IF NOT EXISTS idx_tx_to_block ON transactions(to_addr, block_number DESC);
CREATE INDEX IF NOT EXISTS idx_tx_block_index ON transactions(block_number, tx_index);

-- Token transfers - composite indexes for whale queries
CREATE INDEX IF NOT EXISTS idx_token_transfer_token_to ON token_transfers(token_address, to_addr);
CREATE INDEX IF NOT EXISTS idx_token_transfer_token_from ON token_transfers(token_address, from_addr);
CREATE INDEX IF NOT EXISTS idx_token_transfer_value ON token_transfers(token_address, value DESC);

-- Address transactions - already has primary key, add direction index
CREATE INDEX IF NOT EXISTS idx_addr_tx_direction ON address_transactions(address, direction, block_number DESC);

-- Blocks - timestamp index for time-range queries
CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blocks(timestamp DESC);

-- Logs - composite index for contract event queries
CREATE INDEX IF NOT EXISTS idx_logs_address_topic ON logs(address, topic0, block_number DESC);
