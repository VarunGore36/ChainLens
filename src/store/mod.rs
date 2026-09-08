pub mod postgres;

use thiserror::Error;

use crate::domain::IndexedBlock;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migration(#[source] sqlx::migrate::MigrateError),
}

#[async_trait::async_trait]
pub trait BlockStore: Send + Sync {
    async fn commit_block(&self, block: &IndexedBlock) -> Result<(), StoreError>;

    async fn last_indexed_block(&self) -> Result<Option<(u64, Vec<u8>)>, StoreError>;

    async fn run_migrations(&self) -> Result<(), StoreError>;
}
