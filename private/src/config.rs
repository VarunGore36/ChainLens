use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use clap::{Parser, ValueEnum};
use url::Url;

#[derive(Clone, PartialEq, Eq)]
pub struct RedactedUrl(Url);

impl RedactedUrl {
    #[must_use]
    pub fn expose(&self) -> &Url {
        &self.0
    }

    #[must_use]
    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }

    #[must_use]
    pub fn has_host(&self) -> bool {
        self.0.host_str().is_some_and(|host| !host.is_empty())
    }
}

impl fmt::Display for RedactedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://", self.0.scheme())?;
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
        write!(f, "{self}")
    }
}

impl FromStr for RedactedUrl {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Url::parse(s)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(
    name = "chainlens",
    version,
    about = "Ethereum indexer: reorg-safe, crash-resumable block ingestion"
)]
pub struct Config {
    #[arg(long, env = "CHAINLENS_RPC_URL")]
    pub rpc_url: RedactedUrl,

    #[arg(long, env = "DATABASE_URL")]
    pub database_url: RedactedUrl,

    #[arg(long, env = "CHAINLENS_DB_MAX_CONNECTIONS", default_value_t = 5)]
    pub db_max_connections: u32,

    #[arg(long, env = "CHAINLENS_DB_CONNECT_TIMEOUT_SECS", default_value_t = 5)]
    pub db_connect_timeout_secs: u64,

    #[arg(
        long,
        env = "CHAINLENS_LOG_FORMAT",
        value_enum,
        default_value_t = LogFormat::Text
    )]
    pub log_format: LogFormat,

    #[arg(long, env = "CHAINLENS_RPC_RATE_LIMIT", default_value_t = 10)]
    pub rpc_rate_limit: u32,

    #[arg(long, env = "CHAINLENS_RPC_TIMEOUT_SECS", default_value_t = 30)]
    pub rpc_timeout_secs: u64,

    #[arg(long, env = "CHAINLENS_RPC_MAX_RETRIES", default_value_t = 3)]
    pub rpc_max_retries: u32,

    #[arg(long, env = "CHAINLENS_BACKFILL_FROM", default_value_t = 0)]
    pub backfill_from: u64,

    #[arg(long, env = "CHAINLENS_HEAD_POLL_SECS", default_value_t = 4)]
    pub head_poll_secs: u64,

    #[arg(long, env = "CHAINLENS_MAX_REORG_DEPTH", default_value_t = 128)]
    pub max_reorg_depth: u64,
}

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

const MAX_CONNECTIONS: (u64, u64) = (1, 200);
const CONNECT_TIMEOUT_SECS: (u64, u64) = (1, 120);
const RPC_RATE_LIMIT: (u64, u64) = (1, 1000);
const RPC_TIMEOUT_SECS: (u64, u64) = (1, 120);
const RPC_MAX_RETRIES: (u64, u64) = (0, 10);

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config = Self::parse();
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        require_host("CHAINLENS_RPC_URL", &self.rpc_url)?;
        require_host("DATABASE_URL", &self.database_url)?;

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
            backfill_from: 0,
            head_poll_secs: 4,
            max_reorg_depth: 128,
        }
    }

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

    #[test]
    fn the_cli_definition_is_well_formed() {
        use clap::CommandFactory;
        Config::command().debug_assert();
    }
}
