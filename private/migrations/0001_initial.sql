CREATE TABLE IF NOT EXISTS blocks (
    number         BIGINT PRIMARY KEY,
    hash           BYTEA NOT NULL UNIQUE,
    parent_hash    BYTEA NOT NULL,
    timestamp      TIMESTAMPTZ NOT NULL,
    miner          BYTEA NOT NULL,
    gas_used       BIGINT NOT NULL,
    gas_limit      BIGINT NOT NULL,
    base_fee       NUMERIC(78,0),
    tx_count       INT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS transactions (
    hash                BYTEA PRIMARY KEY,
    block_number        BIGINT NOT NULL REFERENCES blocks(number) ON DELETE CASCADE,
    tx_index            INT NOT NULL,
    from_addr           BYTEA NOT NULL,
    to_addr             BYTEA,
    value               NUMERIC(78,0) NOT NULL,
    nonce               BIGINT NOT NULL,
    tx_type             SMALLINT NOT NULL DEFAULT 0,
    gas_limit           BIGINT NOT NULL,
    gas_price           NUMERIC(78,0),
    max_fee_per_gas     NUMERIC(78,0),
    max_priority_fee_per_gas NUMERIC(78,0),
    input               BYTEA NOT NULL DEFAULT '\\x',
    status              SMALLINT,
    gas_used            BIGINT NOT NULL,
    effective_gas_price NUMERIC(78,0),
    contract_address    BYTEA
);

CREATE INDEX IF NOT EXISTS idx_transactions_block ON transactions(block_number);

CREATE TABLE IF NOT EXISTS logs (
    block_number  BIGINT NOT NULL REFERENCES blocks(number) ON DELETE CASCADE,
    log_index     INT NOT NULL,
    tx_hash       BYTEA NOT NULL,
    address       BYTEA NOT NULL,
    topic0        BYTEA,
    topic1        BYTEA,
    topic2        BYTEA,
    topic3        BYTEA,
    data          BYTEA NOT NULL DEFAULT '\\x',
    PRIMARY KEY (block_number, log_index)
);

CREATE INDEX IF NOT EXISTS idx_logs_tx_hash ON logs(tx_hash);
CREATE INDEX IF NOT EXISTS idx_logs_address ON logs(address);

CREATE TABLE IF NOT EXISTS token_transfers (
    block_number BIGINT NOT NULL,
    log_index    INT NOT NULL,
    token_address BYTEA NOT NULL,
    from_addr    BYTEA NOT NULL,
    to_addr      BYTEA NOT NULL,
    value        NUMERIC(78,0) NOT NULL,
    token_id     NUMERIC(78,0) NOT NULL,
    standard     SMALLINT NOT NULL,
    PRIMARY KEY (block_number, log_index),
    FOREIGN KEY (block_number, log_index) REFERENCES logs(block_number, log_index) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_token_transfers_token ON token_transfers(token_address);
CREATE INDEX IF NOT EXISTS idx_token_transfers_from ON token_transfers(from_addr);
CREATE INDEX IF NOT EXISTS idx_token_transfers_to ON token_transfers(to_addr);

CREATE TABLE IF NOT EXISTS address_transactions (
    address      BYTEA NOT NULL,
    block_number BIGINT NOT NULL,
    tx_index     INT NOT NULL,
    tx_hash      BYTEA NOT NULL,
    direction    SMALLINT NOT NULL,
    PRIMARY KEY (address, block_number, tx_index, direction)
);

CREATE INDEX IF NOT EXISTS idx_address_tx_hash ON address_transactions(tx_hash);

CREATE TABLE IF NOT EXISTS indexer_state (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    last_indexed_number BIGINT,
    last_indexed_hash   BYTEA,
    finalized_number    BIGINT,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO indexer_state (id, last_indexed_number, last_indexed_hash, finalized_number)
VALUES (1, NULL, NULL, NULL)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS reorgs (
    id                    BIGSERIAL PRIMARY KEY,
    detected_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    common_ancestor_number BIGINT NOT NULL,
    depth                 INT NOT NULL,
    orphaned_from         BIGINT NOT NULL,
    orphaned_to           BIGINT NOT NULL,
    orphaned_hashes       BYTEA[] NOT NULL
);
