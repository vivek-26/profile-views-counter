mod datastore;
mod state;

use axum::{extract::State as StateExtractor, response::Html, routing::get, Router};
use datastore::PostgresDB;
use dotenv::dotenv;
use state::State;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // load environment variables from .env file if we are running locally
    if std::env::var("PRODUCTION").is_err() {
        dotenv().ok();
    }

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_tokio_postgres=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_connection_str = std::env::var("DATABASE_URL").unwrap();

    // setup database connection pool
    let db = PostgresDB::new(&db_connection_str).await;

    // initialize state
    let state = State::initialize(db).await.unwrap();

    // build our application with some routes
    let app = Router::new()
        .route("/count.svg", get(handler))
        .with_state(state);

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn handler(StateExtractor(state): StateExtractor<State<PostgresDB>>) -> Html<String> {
    let views = state.update();
    Html(format!("<h1>Profile views = {}</h1>", views))
}
