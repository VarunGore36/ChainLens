use std::collections::BTreeMap;

use tokio::sync::mpsc;

use super::worker::FetchedBlock;

pub async fn run(
    mut rx: mpsc::Receiver<FetchedBlock>,
    tx: mpsc::Sender<FetchedBlock>,
    mut expected: u64,
) {
    let mut buffer: BTreeMap<u64, FetchedBlock> = BTreeMap::new();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some(block) => {
                        buffer.insert(block.number, block);
                        drain_buffer(&mut buffer, &mut expected, &tx).await;
                    }
                    None => break,
                }
            }
        }
    }

    drain_buffer(&mut buffer, &mut expected, &tx).await;
}

async fn drain_buffer(
    buffer: &mut BTreeMap<u64, FetchedBlock>,
    expected: &mut u64,
    tx: &mpsc::Sender<FetchedBlock>,
) {
    while let Some(block) = buffer.remove(expected) {
        if tx.send(block).await.is_err() {
            return;
        }
        *expected += 1;
    }
}
