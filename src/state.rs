use std::{sync::Arc, time::Duration};

use tokio::sync::{Mutex, RwLock};
use tokio::time;
use tokio_stream::{wrappers::IntervalStream, StreamExt};

use super::badge::Fetcher;
use super::datastore::Datastore;

pub struct AppState<T: Datastore, F: Fetcher> {
    db: T,
    views: Arc<Mutex<u64>>,
    prev_views: RwLock<u64>,
    pub badge_fetcher: F,
}

impl<T, F> AppState<T, F>
where
    T: Datastore,
    F: Fetcher,
{
    pub async fn initialize(db: T, badge_fetcher: F) -> Option<AppState<T, F>> {
        match db.get_views().await {
            Ok(count) => {
                tracing::info!("state initialized, current views: {}", count);

                Some(AppState {
                    db,
                    views: Arc::new(Mutex::new(count as u64)),
                    prev_views: RwLock::new(count as u64),
                    badge_fetcher,
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
            let prev_views = self.prev_views.read().await;
            let current_views = self.views.lock().await;

            if *current_views == *prev_views {
                tracing::info!(
                    "no new updates for database, profile views: {}",
                    *current_views
                );
                continue;
            }

            drop(prev_views); // give away read lock

            let mut prev_views = self.prev_views.write().await;
            *prev_views = *current_views;

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

    pub async fn destroy(&self) {
        self.db.close_connection().await;
    }
}
