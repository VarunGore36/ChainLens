//! Log and trace initialisation.
//!
//! `tracing` rather than a log crate, and not only for structured fields. From
//! Phase 5 onward the interesting unit of work is a block moving through several
//! `await` points across several tasks, and a span attaches `block_number` to
//! every event emitted underneath it without threading it through call
//! signatures. A log line records that something happened; a span records what
//! it happened *inside*, which is the question worth asking of a pipeline.
//!
//! Installation happens exactly once, at startup, and is fallible rather than
//! panicking.

use tracing_subscriber::EnvFilter;

use crate::config::LogFormat;

/// Used when `RUST_LOG` is unset.
///
/// `info` for dependencies and `debug` for this crate: sqlx and hyper at debug
/// are unreadable, and this crate at info hides the per-block detail that is the
/// reason to run it locally.
const DEFAULT_FILTER: &str = "info,chainlens=debug";

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("RUST_LOG=\"{directives}\" is not a valid tracing filter")]
    Directives {
        directives: String,
        #[source]
        source: tracing_subscriber::filter::ParseError,
    },

    #[error("the built-in default log filter \"{DEFAULT_FILTER}\" is not valid")]
    DefaultFilter(#[source] tracing_subscriber::filter::ParseError),

    #[error("a global tracing subscriber is already installed")]
    AlreadyInstalled,
}

/// Install the global subscriber. Call once, as early as possible.
pub fn init(format: LogFormat) -> Result<(), TelemetryError> {
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter()?)
        .with_target(true);

    let installed = match format {
        LogFormat::Text => builder.try_init(),
        LogFormat::Json => builder
            .json()
            // Event fields at the top level rather than nested under "fields",
            // which is what every log aggregator expects to index.
            .flatten_event(true)
            // Carries the enclosing span's fields onto each event, so
            // block_number survives into the JSON.
            .with_current_span(true)
            .with_span_list(false)
            .with_ansi(false)
            .try_init(),
    };

    installed.map_err(|_| TelemetryError::AlreadyInstalled)
}

/// `RUST_LOG` wins if set; otherwise [`DEFAULT_FILTER`].
///
/// A malformed `RUST_LOG` is a hard startup failure rather than a silent
/// fallback. It is a configuration error, and this process fails fast on those:
/// discovering hours into a backfill that a typo turned logging off is worse than
/// not starting.
fn filter() -> Result<EnvFilter, TelemetryError> {
    match std::env::var(EnvFilter::DEFAULT_ENV) {
        Ok(directives) if !directives.trim().is_empty() => EnvFilter::try_new(&directives)
            .map_err(|source| TelemetryError::Directives { directives, source }),
        // Unset, empty, or not valid UTF-8.
        _ => EnvFilter::try_new(DEFAULT_FILTER).map_err(TelemetryError::DefaultFilter),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A typo in a string constant would otherwise only show up at runtime, and
    /// only as "no logs appeared".
    #[test]
    fn the_default_filter_is_a_valid_directive() {
        assert!(EnvFilter::try_new(DEFAULT_FILTER).is_ok());
    }

    #[test]
    fn a_malformed_directive_is_rejected() {
        assert!(EnvFilter::try_new("chainlens=nonsense").is_err());
    }
}
