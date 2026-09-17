CREATE TABLE IF NOT EXISTS transaction_actions (
    id BIGSERIAL PRIMARY KEY,
    tx_hash BYTEA NOT NULL REFERENCES transactions(hash) ON DELETE CASCADE,
    action_type VARCHAR(50) NOT NULL,
    from_addr BYTEA NOT NULL,
    to_addr BYTEA,
    value NUMERIC(78,0),
    token_address BYTEA,
    token_symbol VARCHAR(20),
    amount NUMERIC(78,0),
    spender BYTEA,
    description TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_tx_actions_hash ON transaction_actions(tx_hash);
CREATE INDEX IF NOT EXISTS idx_tx_actions_type ON transaction_actions(action_type);
CREATE INDEX IF NOT EXISTS idx_tx_actions_from ON transaction_actions(from_addr);
CREATE INDEX IF NOT EXISTS idx_tx_actions_to ON transaction_actions(to_addr);

CREATE TABLE IF NOT EXISTS address_stats (
    address BYTEA PRIMARY KEY,
    first_seen TIMESTAMPTZ,
    last_active TIMESTAMPTZ,
    total_transactions BIGINT NOT NULL DEFAULT 0,
    total_eth_sent NUMERIC(78,0) NOT NULL DEFAULT 0,
    total_eth_received NUMERIC(78,0) NOT NULL DEFAULT 0,
    unique_contracts_called BIGINT NOT NULL DEFAULT 0,
    unique_callers BIGINT NOT NULL DEFAULT 0,
    token_transfers_sent BIGINT NOT NULL DEFAULT 0,
    token_transfers_received BIGINT NOT NULL DEFAULT 0,
    behavior_classifications TEXT[] NOT NULL DEFAULT '{}',
    activity_level VARCHAR(20) NOT NULL DEFAULT 'none',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS address_relationships (
    from_addr BYTEA NOT NULL,
    to_addr BYTEA NOT NULL,
    relationship_type VARCHAR(50) NOT NULL,
    interaction_count BIGINT NOT NULL DEFAULT 0,
    total_value NUMERIC(78,0),
    first_seen TIMESTAMPTZ,
    last_seen TIMESTAMPTZ,
    PRIMARY KEY (from_addr, to_addr, relationship_type)
);

CREATE INDEX IF NOT EXISTS idx_rel_from ON address_relationships(from_addr);
CREATE INDEX IF NOT EXISTS idx_rel_to ON address_relationships(to_addr);
CREATE INDEX IF NOT EXISTS idx_rel_type ON address_relationships(relationship_type);

CREATE TABLE IF NOT EXISTS anomalies (
    id BIGSERIAL PRIMARY KEY,
    anomaly_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL,
    detected_at TIMESTAMPTZ NOT NULL,
    entity BYTEA NOT NULL,
    entity_type VARCHAR(20) NOT NULL,
    description TEXT NOT NULL,
    observed_value DOUBLE PRECISION NOT NULL,
    baseline_value DOUBLE PRECISION NOT NULL,
    threshold DOUBLE PRECISION NOT NULL,
    evidence JSONB NOT NULL DEFAULT '[]',
    block_number BIGINT REFERENCES blocks(number) ON DELETE CASCADE,
    tx_hash BYTEA REFERENCES transactions(hash) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_anomalies_type ON anomalies(anomaly_type);
CREATE INDEX IF NOT EXISTS idx_anomalies_severity ON anomalies(severity);
CREATE INDEX IF NOT EXISTS idx_anomalies_entity ON anomalies(entity);
CREATE INDEX IF NOT EXISTS idx_anomalies_detected ON anomalies(detected_at);

CREATE TABLE IF NOT EXISTS contract_profiles (
    address BYTEA PRIMARY KEY,
    first_seen TIMESTAMPTZ,
    deployment_block BIGINT REFERENCES blocks(number) ON DELETE SET NULL,
    creator BYTEA,
    deployment_tx BYTEA,
    total_transactions BIGINT NOT NULL DEFAULT 0,
    unique_callers BIGINT NOT NULL DEFAULT 0,
    unique_contracts_called BIGINT NOT NULL DEFAULT 0,
    token_transfers BIGINT NOT NULL DEFAULT 0,
    event_count BIGINT NOT NULL DEFAULT 0,
    function_selectors TEXT[] NOT NULL DEFAULT '{}',
    is_token BOOLEAN NOT NULL DEFAULT false,
    is_nft BOOLEAN NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS block_analytics (
    block_number BIGINT PRIMARY KEY REFERENCES blocks(number) ON DELETE CASCADE,
    total_priority_fees NUMERIC(78,0) NOT NULL DEFAULT 0,
    avg_priority_fee NUMERIC(78,0) NOT NULL DEFAULT 0,
    max_priority_fee NUMERIC(78,0) NOT NULL DEFAULT 0,
    mev_event_count INT NOT NULL DEFAULT 0,
    unusual_tx_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS mev_events (
    id BIGSERIAL PRIMARY KEY,
    mev_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL,
    block_number BIGINT NOT NULL REFERENCES blocks(number) ON DELETE CASCADE,
    detected_at TIMESTAMPTZ NOT NULL,
    description TEXT NOT NULL,
    involved_addresses BYTEA[] NOT NULL DEFAULT '{}',
    involved_transactions BYTEA[] NOT NULL DEFAULT '{}',
    estimated_value NUMERIC(78,0),
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_mev_block ON mev_events(block_number);
CREATE INDEX IF NOT EXISTS idx_mev_type ON mev_events(mev_type);
CREATE INDEX IF NOT EXISTS idx_mev_severity ON mev_events(severity);
