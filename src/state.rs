use axum::async_trait;
use sqlx::{PgPool, Row};

#[async_trait]
pub trait Datastore {
    async fn get_views(&self) -> Result<i64, DataAccessError>;
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct DataAccessError(#[from] sqlx::Error);

pub struct PostgresDB(PgPool);

impl PostgresDB {
    pub fn new(db: PgPool) -> PostgresDB {
        PostgresDB(db)
    }
}

#[async_trait]
impl Datastore for PostgresDB {
    async fn get_views(&self) -> Result<i64, DataAccessError> {
        let row = sqlx::query("SELECT count FROM profile_views")
            .fetch_one(&self.0)
            .await?;

        let count: i64 = row.try_get("count")?;
        Ok(count)
    }
}

pub struct State<T: Datastore> {
    pub db: T,
    views: u64,
}

impl<T: Datastore> State<T> {
    pub async fn initialize(db: T) -> Option<State<T>> {
        match db.get_views().await {
            Ok(count) => {
                tracing::info!("state initialized, current views: {}", count);
                Some(State {
                    db,
                    views: count as u64,
                })
            }
            Err(e) => {
                tracing::error!("failed to initialize state: {}", e);
                None
            }
        }
    }
}
