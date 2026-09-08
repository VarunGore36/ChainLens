//! Configuration, resolved once at startup from CLI flags and the environment.
//!
//! Two properties matter here beyond reading values.
//!
//! **Fail fast.** Everything wrong with the configuration is reported before any
//! socket is opened, naming the setting that is wrong. A process that starts and
//! then dies twenty seconds later inside a connection pool is much harder to
//! diagnose than one that refuses to start.
//!
//! **Credentials cannot reach a log line.** [`RedactedUrl`] has no `Debug` or
//! `Display` implementation capable of printing a secret, so the compiler rather
//! than reviewer discipline is what prevents `%config.rpc_url` from publishing
//! an API key. The unit tests at the bottom of this file assert it.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use clap::{Parser, ValueEnum};
use url::Url;

/// A URL whose rendered form is limited to scheme, host and port.
///
/// Which component of a URL holds the secret depends on the provider: hosted RPC
/// endpoints put the API key in the path (`/v2/<key>`), in the query string
/// (`?apikey=`), or in userinfo, and a PostgreSQL URL carries the password in
/// userinfo. Rather than guess, everything after the authority is elided.
///
/// What survives is what is actually useful when reading a log — is this mainnet
/// or Sepolia, localhost or the shared database — and none of it can contain a
/// credential.
#[derive(Clone, PartialEq, Eq)]
pub struct RedactedUrl(Url);

impl RedactedUrl {
    /// The complete URL, credentials included.
    ///
    /// Every call site is a place a secret could escape, so there are
    /// deliberately very few of them.
    #[must_use]
    pub fn expose(&self) -> &Url {
        &self.0
    }

    #[must_use]
    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }

    /// True only for a non-empty host. `file:///tmp/x` parses successfully and
    /// reports an *empty* host, which is not a thing you can connect to.
    #[must_use]
    pub fn has_host(&self) -> bool {
        self.0.host_str().is_some_and(|host| !host.is_empty())
    }
}

impl fmt::Display for RedactedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://", self.0.scheme())?;
        // Not `unwrap`: a URL with no authority is rejected by
        // `Config::validate`, but rendering must not panic even if that check is
        // ever bypassed.
        let host = self.0.host_str().unwrap_or_default();
        f.write_str(if host.is_empty() { "<no-host>" } else { host })?;
        if let Some(port) = self.0.port() {
            write!(f, ":{port}")?;
        }
        let elided = self.0.path().len() > 1
            || self.0.query().is_some()
            || !self.0.username().is_empty()
            || self.0.password().is_some();
        if elided {
            f.write_str("/...")?;
        }
        Ok(())
    }
}

impl fmt::Debug for RedactedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Delegating to Display is the entire point: `#[derive(Debug)]` on
        // `Config` must not be able to print a credential.
        write!(f, "{self}")
    }
}

impl FromStr for RedactedUrl {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Url::parse(s)?))
    }
}

/// How log records are rendered.
///
/// An enum rather than a `--log-json` boolean on purpose. clap's `SetTrue` action
/// treats the mere presence of an environment variable as true, so
/// `CHAINLENS_LOG_JSON=false` would switch JSON logging *on* — a footgun that is
/// invisible until someone reads their logs. An enum has no such failure mode and
/// leaves room for a third format later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum LogFormat {
    /// One line per event, coloured when the terminal supports it. Single-line
    /// on purpose: during a backfill a multi-line formatter is unreadable.
    Text,
    /// One JSON object per line, for anything that ships logs elsewhere.
    Json,
}

/// Everything the process needs to know, resolved before it does anything.
#[derive(Debug, Parser)]
#[command(
    name = "chainlens",
    version,
    about = "Ethereum indexer: reorg-safe, crash-resumable block ingestion"
)]
pub struct Config {
    /// Ethereum JSON-RPC endpoint. Contains an API key; only scheme and host are
    /// ever logged.
    #[arg(long, env = "CHAINLENS_RPC_URL")]
    pub rpc_url: RedactedUrl,

    /// PostgreSQL connection string. Contains a password; only scheme, host and
    /// port are ever logged.
    ///
    /// The environment variable is unprefixed because sqlx's own tooling —
    /// `sqlx migrate`, and the compile-time-checked `query!` macros from Phase 4
    /// onward — reads `DATABASE_URL` by that exact name. Two names for one value
    /// is a worse problem than one name that breaks the prefix convention.
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: RedactedUrl,

    /// Maximum PostgreSQL connections in the pool.
    ///
    /// The committer is a single writer and needs exactly one. The remainder are
    /// for the query API and for ad-hoc inspection.
    #[arg(long, env = "CHAINLENS_DB_MAX_CONNECTIONS", default_value_t = 5)]
    pub db_max_connections: u32,

    /// Seconds to wait for PostgreSQL at startup before giving up.
    #[arg(long, env = "CHAINLENS_DB_CONNECT_TIMEOUT_SECS", default_value_t = 5)]
    pub db_connect_timeout_secs: u64,

    /// Log output format.
    #[arg(
        long,
        env = "CHAINLENS_LOG_FORMAT",
        value_enum,
        default_value_t = LogFormat::Text
    )]
    pub log_format: LogFormat,

    /// Maximum RPC requests per second. Hosted providers meter compute units,
    /// not raw requests, so this is a baseline — the provider's actual budget
    /// is the ceiling that Phase 11 measures.
    #[arg(long, env = "CHAINLENS_RPC_RATE_LIMIT", default_value_t = 10)]
    pub rpc_rate_limit: u32,

    /// Seconds to wait for an RPC response before timing out.
    #[arg(long, env = "CHAINLENS_RPC_TIMEOUT_SECS", default_value_t = 30)]
    pub rpc_timeout_secs: u64,

    /// Maximum retries for transient RPC failures (429, 5xx, timeout).
    /// Deterministic errors (malformed request, method not found) are never
    /// retried.
    #[arg(long, env = "CHAINLENS_RPC_MAX_RETRIES", default_value_t = 3)]
    pub rpc_max_retries: u32,
}

/// Everything that can be wrong with a configuration that nonetheless parsed.
///
/// No variant carries any part of a URL except its scheme. Error messages get
/// logged, so they have to be redaction-safe by construction rather than by
/// remembering to be careful at each construction site.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{setting} must be a URL with a host")]
    MissingHost { setting: &'static str },

    #[error("{setting} has scheme \"{actual}\" but must be one of: {expected}")]
    Scheme {
        setting: &'static str,
        actual: String,
        expected: String,
    },

    #[error("{setting} is {actual} but must be between {min} and {max}")]
    Range {
        setting: &'static str,
        actual: u64,
        min: u64,
        max: u64,
    },
}

/// Asserted rather than merely documented. A pool of zero deadlocks on first use,
/// and a pool of several hundred is a way to make PostgreSQL slower rather than
/// faster, since every connection is a backend process.
const MAX_CONNECTIONS: (u64, u64) = (1, 200);
const CONNECT_TIMEOUT_SECS: (u64, u64) = (1, 120);
const RPC_RATE_LIMIT: (u64, u64) = (1, 1000);
const RPC_TIMEOUT_SECS: (u64, u64) = (1, 120);
const RPC_MAX_RETRIES: (u64, u64) = (0, 10);

impl Config {
    /// Resolve configuration from process arguments and the environment.
    ///
    /// Call this *after* loading `.env`, or values in that file will not be
    /// visible. clap prints usage and exits on a malformed argument, which is
    /// correct for a CLI; semantic problems come back as [`ConfigError`].
    pub fn load() -> Result<Self, ConfigError> {
        let config = Self::parse();
        config.validate()?;
        Ok(config)
    }

    /// The checks that clap's attributes cannot express.
    pub fn validate(&self) -> Result<(), ConfigError> {
        require_host("CHAINLENS_RPC_URL", &self.rpc_url)?;
        require_host("DATABASE_URL", &self.database_url)?;

        // ws:// and wss:// are deliberately rejected. Subscription-based
        // ingestion is on the cut list, and accepting a scheme the RPC client
        // cannot speak would defer the failure to first use.
        require_scheme("CHAINLENS_RPC_URL", &self.rpc_url, &["http", "https"])?;
        require_scheme(
            "DATABASE_URL",
            &self.database_url,
            &["postgres", "postgresql"],
        )?;

        require_range(
            "CHAINLENS_DB_MAX_CONNECTIONS",
            u64::from(self.db_max_connections),
            MAX_CONNECTIONS,
        )?;
        require_range(
            "CHAINLENS_DB_CONNECT_TIMEOUT_SECS",
            self.db_connect_timeout_secs,
            CONNECT_TIMEOUT_SECS,
        )?;

        require_range(
            "CHAINLENS_RPC_RATE_LIMIT",
            u64::from(self.rpc_rate_limit),
            RPC_RATE_LIMIT,
        )?;
        require_range(
            "CHAINLENS_RPC_TIMEOUT_SECS",
            self.rpc_timeout_secs,
            RPC_TIMEOUT_SECS,
        )?;
        require_range(
            "CHAINLENS_RPC_MAX_RETRIES",
            u64::from(self.rpc_max_retries),
            RPC_MAX_RETRIES,
        )?;

        Ok(())
    }

    #[must_use]
    pub fn db_connect_timeout(&self) -> Duration {
        Duration::from_secs(self.db_connect_timeout_secs)
    }
}

fn require_host(setting: &'static str, url: &RedactedUrl) -> Result<(), ConfigError> {
    if url.has_host() {
        Ok(())
    } else {
        Err(ConfigError::MissingHost { setting })
    }
}

fn require_scheme(
    setting: &'static str,
    url: &RedactedUrl,
    allowed: &[&str],
) -> Result<(), ConfigError> {
    if allowed.contains(&url.scheme()) {
        Ok(())
    } else {
        Err(ConfigError::Scheme {
            setting,
            actual: url.scheme().to_owned(),
            expected: allowed.join(", "),
        })
    }
}

fn require_range(
    setting: &'static str,
    actual: u64,
    (min, max): (u64, u64),
) -> Result<(), ConfigError> {
    if (min..=max).contains(&actual) {
        Ok(())
    } else {
        Err(ConfigError::Range {
            setting,
            actual,
            min,
            max,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RPC_KEY: &str = "sk_live_thisisthesecretapikey";
    const DB_PASSWORD: &str = "hunter2";

    fn valid() -> Config {
        Config {
            rpc_url: format!("https://eth-mainnet.g.alchemy.com/v2/{RPC_KEY}")
                .parse()
                .unwrap(),
            database_url: format!("postgres://chainlens:{DB_PASSWORD}@db.internal:5432/chainlens")
                .parse()
                .unwrap(),
            db_max_connections: 5,
            db_connect_timeout_secs: 5,
            log_format: LogFormat::Text,
            rpc_rate_limit: 10,
            rpc_timeout_secs: 30,
            rpc_max_retries: 3,
        }
    }

    /// The acceptance criterion for Phase 1 that is worth a test rather than an
    /// eyeball: no rendering of `Config` may contain a credential. If someone
    /// later swaps `RedactedUrl` for a plain `Url`, or derives `Debug` on it,
    /// this fails.
    #[test]
    fn no_rendering_of_config_contains_a_credential() {
        let config = valid();
        let renderings = [
            format!("{config:?}"),
            format!("{:?}", config.rpc_url),
            format!("{}", config.rpc_url),
            format!("{:?}", config.database_url),
            format!("{}", config.database_url),
        ];
        for rendered in &renderings {
            assert!(
                !rendered.contains(RPC_KEY),
                "leaked the RPC key: {rendered}"
            );
            assert!(
                !rendered.contains(DB_PASSWORD),
                "leaked the database password: {rendered}"
            );
        }
    }

    #[test]
    fn redaction_keeps_enough_to_identify_the_target() {
        let config = valid();
        assert_eq!(
            config.rpc_url.to_string(),
            "https://eth-mainnet.g.alchemy.com/..."
        );
        assert_eq!(
            config.database_url.to_string(),
            "postgres://db.internal:5432/..."
        );
    }

    #[test]
    fn a_url_with_nothing_to_hide_gets_no_ellipsis() {
        let url: RedactedUrl = "http://localhost:8545".parse().unwrap();
        assert_eq!(url.to_string(), "http://localhost:8545");
    }

    #[test]
    fn expose_returns_the_credential_bearing_url() {
        let config = valid();
        assert!(config.rpc_url.expose().as_str().contains(RPC_KEY));
    }

    #[test]
    fn accepts_a_valid_configuration() {
        assert!(valid().validate().is_ok());
    }

    #[test]
    fn rejects_a_database_url_that_is_not_postgres() {
        let mut config = valid();
        config.database_url = "mysql://db.internal:3306/chainlens".parse().unwrap();
        assert!(matches!(
            config.validate(),
            Err(ConfigError::Scheme {
                setting: "DATABASE_URL",
                ..
            })
        ));
    }

    #[test]
    fn rejects_a_websocket_rpc_url() {
        let mut config = valid();
        config.rpc_url = "wss://eth-mainnet.example.com/v2/key".parse().unwrap();
        assert!(matches!(config.validate(), Err(ConfigError::Scheme { .. })));
    }

    #[test]
    fn rejects_a_url_without_a_host() {
        let mut config = valid();
        config.rpc_url = "file:///tmp/not-an-endpoint".parse().unwrap();
        assert!(matches!(
            config.validate(),
            Err(ConfigError::MissingHost { .. })
        ));
    }

    #[test]
    fn rejects_an_empty_connection_pool() {
        let mut config = valid();
        config.db_max_connections = 0;
        assert!(matches!(
            config.validate(),
            Err(ConfigError::Range {
                setting: "CHAINLENS_DB_MAX_CONNECTIONS",
                actual: 0,
                ..
            })
        ));
    }

    #[test]
    fn rejects_a_zero_connect_timeout() {
        let mut config = valid();
        config.db_connect_timeout_secs = 0;
        assert!(matches!(config.validate(), Err(ConfigError::Range { .. })));
    }

    /// `localhost:5432` is the mistake people actually make, and it is worse than
    /// a syntax error: `Url` parses it happily as scheme `localhost` with path
    /// `5432`, so only the host check catches it.
    #[test]
    fn rejects_a_host_port_pair_mistaken_for_a_url() {
        let mut config = valid();
        config.database_url = "localhost:5432".parse().unwrap();
        assert!(matches!(
            config.validate(),
            Err(ConfigError::MissingHost { .. })
        ));
    }

    #[test]
    fn rejects_a_string_that_is_not_a_url_at_all() {
        assert!("not a url".parse::<RedactedUrl>().is_err());
    }

    /// Guards the CLI definition itself. clap panics on a malformed derive — a
    /// duplicate long flag, say — and without this the panic would only surface
    /// when the binary is run rather than when the tests are.
    #[test]
    fn the_cli_definition_is_well_formed() {
        use clap::CommandFactory;
        Config::command().debug_assert();
    }
}
