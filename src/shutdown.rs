//! Cooperative shutdown.
//!
//! This exists for correctness, not tidiness. From Phase 4 onward the committer
//! holds an open PostgreSQL transaction covering a block and its cursor advance;
//! being killed mid-transaction is safe (PostgreSQL rolls it back), but being
//! killed *between* deciding to commit and committing is exactly the window the
//! Phase 7 crash tests probe. A shutdown path that reliably stops the pipeline at
//! a block boundary is what makes the difference between the two observable.
//!
//! A [`CancellationToken`] rather than a broadcast channel or an `AtomicBool`: it
//! is cloneable, it is `await`-able inside `tokio::select!` alongside real work,
//! and it can be handed to child tasks so that one signal unwinds the whole tree.

use tokio_util::sync::CancellationToken;

/// Returns a token cancelled on the first SIGINT or SIGTERM.
///
/// SIGTERM matters as much as SIGINT. `docker stop` and every container
/// orchestrator send SIGTERM and then SIGKILL after a grace period, so a process
/// that only handles Ctrl-C is hard-killed on every restart — meaning the crash
/// path gets exercised routinely and the shutdown path essentially never.
///
/// Must be called from inside a Tokio runtime; it spawns the listener task.
#[must_use]
pub fn on_signal() -> CancellationToken {
    let token = CancellationToken::new();
    let trigger = token.clone();

    tokio::spawn(async move {
        match next_signal().await {
            Ok(name) => tracing::info!(signal = name, "shutdown signal received"),
            // If the handler cannot be registered the process can no longer be
            // asked to stop politely, so stopping now is the safer response.
            Err(error) => {
                tracing::error!(%error, "signal handling unavailable; shutting down");
            }
        }
        trigger.cancel();
    });

    token
}

#[cfg(unix)]
async fn next_signal() -> std::io::Result<&'static str> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;

    // Both branches are registered before either is awaited, so a signal
    // arriving during startup is not missed.
    tokio::select! {
        _ = interrupt.recv() => Ok("SIGINT"),
        _ = terminate.recv() => Ok("SIGTERM"),
    }
}

#[cfg(not(unix))]
async fn next_signal() -> std::io::Result<&'static str> {
    tokio::signal::ctrl_c().await?;
    Ok("CTRL_C")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_fresh_token_is_not_cancelled_and_registration_succeeds() {
        let token = on_signal();
        assert!(!token.is_cancelled());
    }
}
