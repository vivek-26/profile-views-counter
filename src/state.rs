use super::datastore::Datastore;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct State<T: Datastore> {
    db: T,
    views: Arc<Mutex<u64>>,
}

impl<T: Datastore> State<T> {
    pub async fn initialize(db: T) -> Option<State<T>> {
        match db.get_views().await {
            Ok(count) => {
                tracing::info!("state initialized, current views: {}", count);
                Some(State {
                    db,
                    views: Arc::new(Mutex::new(count as u64)),
                })
            }
            Err(e) => {
                tracing::error!("failed to initialize state: {}", e);
                None
            }
        }
    }

    pub fn update(&self) -> u64 {
        let mut views = self.views.lock().unwrap();
        *views += 1;
        *views
    }
}
