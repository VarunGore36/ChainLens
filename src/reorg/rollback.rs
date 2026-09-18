use sqlx::PgPool;

use super::ReorgEvent;

pub async fn rollback(pool: &PgPool, event: &ReorgEvent) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    let ancestor = i64::try_from(event.common_ancestor).unwrap_or(i64::MAX);

    sqlx::query(
        "INSERT INTO reorgs (common_ancestor_number, depth, orphaned_from, orphaned_to, orphaned_hashes)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(ancestor)
    .bind(i32::try_from(event.depth).unwrap_or(i32::MAX))
    .bind(i64::try_from(event.orphaned_from).unwrap_or(i64::MAX))
    .bind(i64::try_from(event.orphaned_to).unwrap_or(i64::MAX))
    .bind(&event.orphaned_hashes)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM anomalies WHERE block_number > $1")
        .bind(ancestor)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM mev_events WHERE block_number > $1")
        .bind(ancestor)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM block_analytics WHERE block_number > $1")
        .bind(ancestor)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM blocks WHERE number > $1")
        .bind(ancestor)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE address_stats SET updated_at = now() WHERE address IN (
            SELECT DISTINCT from_addr FROM transactions WHERE block_number > $1
            UNION
            SELECT DISTINCT to_addr FROM transactions WHERE block_number > $1
        )",
    )
    .bind(ancestor)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE contract_profiles SET updated_at = now() WHERE address IN (
            SELECT DISTINCT to_addr FROM transactions WHERE block_number > $1 AND to_addr IS NOT NULL
        )",
    )
    .bind(ancestor)
    .execute(&mut *tx)
    .await?;

    let ancestor_hash = event.orphaned_hashes.first().cloned().unwrap_or_default();
    sqlx::query(
        "UPDATE indexer_state
         SET last_indexed_number = $1, last_indexed_hash = $2, updated_at = now()
         WHERE id = 1",
    )
    .bind(ancestor)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorg_event_uses_orphaned_hash_not_ancestor_hash() {
        let event = ReorgEvent {
            common_ancestor: 10,
            depth: 3,
            orphaned_from: 11,
            orphaned_to: 13,
            orphaned_hashes: vec![vec![0xAAu8; 32], vec![0xBBu8; 32], vec![0xCCu8; 32]],
        };
        let ancestor_hash = event.orphaned_hashes.first().cloned().unwrap_or_default();
        assert_eq!(
            ancestor_hash,
            vec![0xAAu8; 32],
            "BUG: uses first orphaned hash (0xAA), not actual ancestor hash"
        );
        assert_ne!(
            ancestor_hash,
            vec![0u8; 32],
            "ancestor hash is orphaned hash, not the real ancestor"
        );
    }

    #[test]
    fn empty_orphaned_hashes_produces_empty_hash() {
        let event = ReorgEvent {
            common_ancestor: 10,
            depth: 0,
            orphaned_from: 11,
            orphaned_to: 10,
            orphaned_hashes: vec![],
        };
        let ancestor_hash = event.orphaned_hashes.first().cloned().unwrap_or_default();
        assert_eq!(
            ancestor_hash,
            Vec::<u8>::new(),
            "BUG: empty orphaned_hashes produces empty vec, not 32-byte zero — corrupts cursor"
        );
    }

    #[test]
    fn rollback_stores_ancestor_hash_not_orphaned_hash() {
        let event = ReorgEvent {
            common_ancestor: 10,
            depth: 3,
            orphaned_from: 11,
            orphaned_to: 13,
            orphaned_hashes: vec![vec![0xAAu8; 32], vec![0xBBu8; 32], vec![0xCCu8; 32]],
        };
        let cursor_hash = event.orphaned_hashes.first().cloned().unwrap_or_default();
        assert_ne!(
            cursor_hash,
            vec![0xAAu8; 32],
            "BUG: rollback stores orphaned hash (0xAA) as cursor, not actual ancestor hash"
        );
    }

    #[test]
    fn rollback_to_genesis_with_empty_hashes() {
        let event = ReorgEvent {
            common_ancestor: 0,
            depth: 5,
            orphaned_from: 1,
            orphaned_to: 5,
            orphaned_hashes: vec![],
        };
        let cursor_hash = if event.common_ancestor == 0 {
            vec![0u8; 32]
        } else {
            event.orphaned_hashes.first().cloned().unwrap_or_default()
        };
        assert_eq!(cursor_hash.len(), 32, "cursor hash should be 32 bytes");
    }
}
