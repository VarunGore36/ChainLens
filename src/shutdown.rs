use tokio_util::sync::CancellationToken;

#[must_use]
pub fn on_signal() -> CancellationToken {
    let token = CancellationToken::new();
    let trigger = token.clone();

    tokio::spawn(async move {
        match next_signal().await {
            Ok(name) => tracing::info!(signal = name, "shutdown signal received"),
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
