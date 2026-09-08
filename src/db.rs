//! PostgreSQL connection setup.
//!
//! Phase 1 proves reachability and nothing more. Schema, migrations, and the
//! single-transaction commit arrive in Phase 4, at which point this module grows
//! into `store/` behind the `BlockStore` trait.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::config::Config;

/// Failures reaching the database.
///
/// `target` is always the *redacted* rendering of the connection URL. These
/// messages are printed and logged, so they must not be able to carry a password
/// even when constructed carelessly.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("PostgreSQL at {target} did not answer within {}s", timeout.as_secs())]
    Timeout { target: String, timeout: Duration },

    #[error("could not connect to PostgreSQL at {target}")]
    Connect {
        target: String,
        #[source]
        source: sqlx::Error,
    },

    #[error("connected to PostgreSQL at {target} but the first query failed")]
    Query {
        target: String,
        #[source]
        source: sqlx::Error,
    },
}

/// Open the connection pool, refusing to hang.
///
/// `PgPoolOptions::connect` establishes one connection eagerly, so a wrong host
/// or a stopped database is a startup failure rather than a surprise on the first
/// query hours later.
///
/// The outer `tokio::time::timeout` is not redundant with `acquire_timeout`. It
/// makes the deadline this program's own rather than a property of how sqlx
/// happens to apply pool timeouts during the initial connect, and a TCP connect
/// to an unroutable address will otherwise sit in the kernel's SYN retry
/// schedule for over a minute before returning anything at all.
pub async fn connect(config: &Config) -> Result<PgPool, DbError> {
    let target = config.database_url.to_string();
    let timeout = config.db_connect_timeout();

    let opened = tokio::time::timeout(
        timeout,
        PgPoolOptions::new()
            .max_connections(config.db_max_connections)
            .acquire_timeout(timeout)
            .connect(config.database_url.expose().as_str()),
    )
    .await;

    match opened {
        Err(_elapsed) => Err(DbError::Timeout { target, timeout }),
        Ok(Err(source)) => Err(DbError::Connect { target, source }),
        Ok(Ok(pool)) => Ok(pool),
    }
}

/// The server's own version string.
///
/// Logged at startup, and not as decoration: Phase 11 requires every benchmark
/// result to name the PostgreSQL version that produced it, and a value read from
/// the running server cannot drift out of step the way a number copied into a
/// document can.
pub async fn server_version(pool: &PgPool, target: &str) -> Result<String, DbError> {
    // `query_scalar`, not the `query_scalar!` macro. The macro validates SQL
    // against a live database at compile time, which would make `cargo build`
    // require a running PostgreSQL and turn CI into a much larger thing.
    // Compile-time checking earns that cost in Phase 4, where the queries are
    // long enough to get wrong; `SELECT version()` is not.
    sqlx::query_scalar::<sqlx::Postgres, String>("SELECT version()")
        .fetch_one(pool)
        .await
        .map_err(|source| DbError::Query {
            target: target.to_owned(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timeout_names_both_the_target_and_the_deadline() {
        let error = DbError::Timeout {
            target: "postgres://db.internal:5432/...".to_owned(),
            timeout: Duration::from_secs(5),
        };
        let rendered = error.to_string();
        assert!(rendered.contains("db.internal:5432"), "{rendered}");
        assert!(rendered.contains("within 5s"), "{rendered}");
    }
}
