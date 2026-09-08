use super::ChainHead;

pub fn next_block(cursor: u64, head: &ChainHead, backfill_from: u64) -> Option<u64> {
    if cursor < backfill_from {
        return Some(backfill_from);
    }

    let target = head.finalized;

    if cursor < target {
        return Some(cursor + 1);
    }

    if cursor < head.latest {
        return Some(cursor + 1);
    }

    None
}
