use super::badge::ShieldsIoFetcher;
use super::datastore::DatastoreOperations;

pub struct AppState<T: DatastoreOperations, F: ShieldsIoFetcher> {
    pub db: T,
    pub badge: F,
}

impl<T, F> AppState<T, F>
where
    T: DatastoreOperations,
    F: ShieldsIoFetcher,
{
    pub fn new(db: T, badge: F) -> AppState<T, F> {
        AppState { db, badge }
    }
}
