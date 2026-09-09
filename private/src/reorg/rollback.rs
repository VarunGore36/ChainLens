use sqlx::PgPool;

use super::ReorgEvent;

pub async fn rollback(pool: &PgPool, event: &ReorgEvent) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO reorgs (common_ancestor_number, depth, orphaned_from, orphaned_to, orphaned_hashes)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(i64::try_from(event.common_ancestor).unwrap_or(i64::MAX))
    .bind(i32::try_from(event.depth).unwrap_or(i32::MAX))
    .bind(i64::try_from(event.orphaned_from).unwrap_or(i64::MAX))
    .bind(i64::try_from(event.orphaned_to).unwrap_or(i64::MAX))
    .bind(&event.orphaned_hashes)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM blocks WHERE number > $1")
        .bind(i64::try_from(event.common_ancestor).unwrap_or(i64::MAX))
        .execute(&mut *tx)
        .await?;

    let ancestor_hash = event.orphaned_hashes.first().cloned().unwrap_or_default();
    sqlx::query(
        "UPDATE indexer_state
         SET last_indexed_number = $1, last_indexed_hash = $2, updated_at = now()
         WHERE id = 1",
    )
    .bind(i64::try_from(event.common_ancestor).unwrap_or(i64::MAX))
    .bind(if event.common_ancestor == 0 {
        vec![0u8; 32]
    } else {
        ancestor_hash
    })
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}
