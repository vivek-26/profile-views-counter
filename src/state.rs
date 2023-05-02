use super::datastore::Datastore;
use futures::lock::Mutex;
use std::{sync::Arc, time::Duration};
use tokio::time;
use tokio_stream::{wrappers::IntervalStream, StreamExt};

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

    pub async fn update(&self) -> u64 {
        let mut views = self.views.lock().await;
        *views += 1;
        *views
    }

    pub async fn update_loop(&self) {
        let mut stream = IntervalStream::new(time::interval(Duration::from_secs(60)));

        while stream.next().await.is_some() {
            let current_views = self.views.lock().await;

            if let Err(err) = self.db.update_views(*current_views as i64).await {
                tracing::error!(
                    "failed to update database with latest profile view count: {}, reason: {}",
                    *current_views,
                    err
                );
            } else {
                tracing::info!(
                    "database updated with latest profile view count: {}",
                    *current_views
                );
            }
        }
    }
}
