use super::datastore::Datastore;

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
