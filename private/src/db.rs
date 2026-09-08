use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::config::Config;

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

pub async fn server_version(pool: &PgPool, target: &str) -> Result<String, DbError> {
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
