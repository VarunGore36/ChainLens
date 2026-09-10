use alloy_primitives::U256;
use sqlx::{PgPool, Postgres, Transaction};

use crate::domain::{IndexedBlock, TokenStandard};

use super::{BlockStore, StoreError};

#[derive(Debug, Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn begin(&self) -> Result<Transaction<'_, Postgres>, StoreError> {
        self.pool.begin().await.map_err(StoreError::Database)
    }
}

#[async_trait::async_trait]
impl BlockStore for PostgresStore {
    async fn commit_block(&self, block: &IndexedBlock) -> Result<(), StoreError> {
        let mut tx = self.begin().await?;

        insert_block(&mut tx, block).await?;
        insert_transactions(&mut tx, block).await?;
        insert_logs(&mut tx, block).await?;
        insert_token_transfers(&mut tx, block).await?;
        insert_address_transactions(&mut tx, block).await?;
        update_cursor(&mut tx, block).await?;

        tx.commit().await.map_err(StoreError::Database)
    }

    async fn last_indexed_block(&self) -> Result<Option<(u64, Vec<u8>)>, StoreError> {
        let row = sqlx::query_as::<_, (Option<i64>, Option<Vec<u8>>)>(
            "SELECT last_indexed_number, last_indexed_hash FROM indexer_state WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(StoreError::Database)?;

        Ok(row.and_then(|(num, hash)| match (num, hash) {
            (Some(n), Some(h)) => {
                #[allow(clippy::cast_sign_loss)]
                let n = n as u64;
                Some((n, h))
            }
            _ => None,
        }))
    }

    async fn run_migrations(&self) -> Result<(), StoreError> {
        let migrator = sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
            .await
            .map_err(StoreError::Migration)?;
        migrator
            .run(&self.pool)
            .await
            .map_err(StoreError::Migration)
    }

    fn get_pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}

async fn insert_block(
    tx: &mut Transaction<'_, Postgres>,
    block: &IndexedBlock,
) -> Result<(), StoreError> {
    let b = &block.block;
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, miner, gas_used, gas_limit, base_fee, tx_count)
         VALUES ($1, $2, $3, to_timestamp($4), $5, $6, $7, $8::NUMERIC, $9)
         ON CONFLICT (number) DO NOTHING",
    )
    .bind(i64::try_from(b.number).unwrap_or(i64::MAX))
    .bind(b.hash.as_slice())
    .bind(b.parent_hash.as_slice())
    .bind(i64::try_from(b.timestamp).unwrap_or(i64::MAX))
    .bind(b.miner.as_slice())
    .bind(i64::try_from(b.gas_used).unwrap_or(i64::MAX))
    .bind(i64::try_from(b.gas_limit).unwrap_or(i64::MAX))
    .bind(b.base_fee_per_gas.map(num_bigint))
    .bind(i32::try_from(b.tx_count).unwrap_or(i32::MAX))
    .execute(&mut **tx)
    .await
    .map_err(StoreError::Database)?;

    Ok(())
}

async fn insert_transactions(
    tx: &mut Transaction<'_, Postgres>,
    block: &IndexedBlock,
) -> Result<(), StoreError> {
    for t in &block.transactions {
        sqlx::query(
            "INSERT INTO transactions (hash, block_number, tx_index, from_addr, to_addr, value, nonce, tx_type, gas_limit, gas_price, max_fee_per_gas, max_priority_fee_per_gas, input, status, gas_used, effective_gas_price, contract_address)
             VALUES ($1, $2, $3, $4, $5, $6::NUMERIC, $7, $8, $9, $10::NUMERIC, $11::NUMERIC, $12::NUMERIC, $13, $14, $15, $16::NUMERIC, $17)
             ON CONFLICT (hash) DO NOTHING",
        )
        .bind(t.hash.as_slice())
        .bind(i64::try_from(t.block_number).unwrap_or(i64::MAX))
        .bind(i32::try_from(t.tx_index).unwrap_or(i32::MAX))
        .bind(t.from.as_slice())
        .bind(t.to.map(|a| a.as_slice().to_vec()))
        .bind(u256_to_numeric(t.value))
        .bind(i64::try_from(t.nonce).unwrap_or(i64::MAX))
        .bind(i16::from(t.tx_type))
        .bind(i64::try_from(t.gas_limit).unwrap_or(i64::MAX))
        .bind(t.gas_price.map(u256_to_numeric))
        .bind(t.max_fee_per_gas.map(u256_to_numeric))
        .bind(t.max_priority_fee_per_gas.map(u256_to_numeric))
        .bind(t.input.as_ref())
        .bind(t.status.map(i16::from))
        .bind(i64::try_from(t.gas_used).unwrap_or(i64::MAX))
        .bind(t.effective_gas_price.map(u256_to_numeric))
        .bind(t.contract_address.map(|a| a.as_slice().to_vec()))
        .execute(&mut **tx)
        .await
        .map_err(StoreError::Database)?;
    }

    Ok(())
}

async fn insert_logs(
    tx: &mut Transaction<'_, Postgres>,
    block: &IndexedBlock,
) -> Result<(), StoreError> {
    for l in &block.logs {
        sqlx::query(
            "INSERT INTO logs (block_number, log_index, tx_hash, address, topic0, topic1, topic2, topic3, data)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (block_number, log_index) DO NOTHING",
        )
        .bind(i64::try_from(l.block_number).unwrap_or(i64::MAX))
        .bind(i32::try_from(l.log_index).unwrap_or(i32::MAX))
        .bind(l.tx_hash.as_slice())
        .bind(l.address.as_slice())
        .bind(l.topic0.map(|t| t.as_slice().to_vec()))
        .bind(l.topic1.map(|t| t.as_slice().to_vec()))
        .bind(l.topic2.map(|t| t.as_slice().to_vec()))
        .bind(l.topic3.map(|t| t.as_slice().to_vec()))
        .bind(l.data.as_ref())
        .execute(&mut **tx)
        .await
        .map_err(StoreError::Database)?;
    }

    Ok(())
}

async fn insert_token_transfers(
    tx: &mut Transaction<'_, Postgres>,
    block: &IndexedBlock,
) -> Result<(), StoreError> {
    for t in &block.token_transfers {
        sqlx::query(
            "INSERT INTO token_transfers (block_number, log_index, token_address, from_addr, to_addr, value, token_id, standard)
             VALUES ($1, $2, $3, $4, $5, $6::NUMERIC, $7::NUMERIC, $8)
             ON CONFLICT (block_number, log_index) DO NOTHING",
        )
        .bind(i64::try_from(t.block_number).unwrap_or(i64::MAX))
        .bind(i32::try_from(t.log_index).unwrap_or(i32::MAX))
        .bind(t.token_address.as_slice())
        .bind(t.from.as_slice())
        .bind(t.to.as_slice())
        .bind(u256_to_numeric(t.value))
        .bind(u256_to_numeric(t.token_id))
        .bind(match t.standard {
            TokenStandard::Erc20 => 0i16,
            TokenStandard::Erc721 => 1i16,
        })
        .execute(&mut **tx)
        .await
        .map_err(StoreError::Database)?;
    }

    Ok(())
}

async fn insert_address_transactions(
    tx: &mut Transaction<'_, Postgres>,
    block: &IndexedBlock,
) -> Result<(), StoreError> {
    for t in &block.transactions {
        let from_direction: i16 = 0;
        sqlx::query(
            "INSERT INTO address_transactions (address, block_number, tx_index, tx_hash, direction)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (address, block_number, tx_index, direction) DO NOTHING",
        )
        .bind(t.from.as_slice())
        .bind(i64::try_from(t.block_number).unwrap_or(i64::MAX))
        .bind(i32::try_from(t.tx_index).unwrap_or(i32::MAX))
        .bind(t.hash.as_slice())
        .bind(from_direction)
        .execute(&mut **tx)
        .await
        .map_err(StoreError::Database)?;

        if let Some(to) = t.to {
            let to_direction: i16 = 1;
            sqlx::query(
                "INSERT INTO address_transactions (address, block_number, tx_index, tx_hash, direction)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (address, block_number, tx_index, direction) DO NOTHING",
            )
            .bind(to.as_slice())
            .bind(i64::try_from(t.block_number).unwrap_or(i64::MAX))
            .bind(i32::try_from(t.tx_index).unwrap_or(i32::MAX))
            .bind(t.hash.as_slice())
            .bind(to_direction)
            .execute(&mut **tx)
            .await
            .map_err(StoreError::Database)?;
        }
    }

    Ok(())
}

async fn update_cursor(
    tx: &mut Transaction<'_, Postgres>,
    block: &IndexedBlock,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE indexer_state
         SET last_indexed_number = $1, last_indexed_hash = $2, updated_at = now()
         WHERE id = 1",
    )
    .bind(i64::try_from(block.block.number).unwrap_or(i64::MAX))
    .bind(block.block.hash.as_slice())
    .execute(&mut **tx)
    .await
    .map_err(StoreError::Database)?;

    Ok(())
}

fn u256_to_numeric(value: U256) -> String {
    value.to_string()
}

fn num_bigint(value: u64) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u256_to_numeric_handles_zero() {
        assert_eq!(u256_to_numeric(U256::ZERO), "0");
    }

    #[test]
    fn u256_to_numeric_handles_large_values() {
        let val = U256::from(1_000_000_000_000_000_000u64);
        assert_eq!(u256_to_numeric(val), "1000000000000000000");
    }
}
