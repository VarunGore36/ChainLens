use tracing_subscriber::EnvFilter;

use crate::config::LogFormat;

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

pub fn init(format: LogFormat) -> Result<(), TelemetryError> {
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter()?)
        .with_target(true);

    let installed = match format {
        LogFormat::Text => builder.try_init(),
        LogFormat::Json => builder
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(false)
            .with_ansi(false)
            .try_init(),
    };

    installed.map_err(|_| TelemetryError::AlreadyInstalled)
}

fn filter() -> Result<EnvFilter, TelemetryError> {
    match std::env::var(EnvFilter::DEFAULT_ENV) {
        Ok(directives) if !directives.trim().is_empty() => EnvFilter::try_new(&directives)
            .map_err(|source| TelemetryError::Directives { directives, source }),
        _ => EnvFilter::try_new(DEFAULT_FILTER).map_err(TelemetryError::DefaultFilter),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_filter_is_a_valid_directive() {
        assert!(EnvFilter::try_new(DEFAULT_FILTER).is_ok());
    }

    #[test]
    fn a_malformed_directive_is_rejected() {
        assert!(EnvFilter::try_new("chainlens=nonsense").is_err());
    }
}
