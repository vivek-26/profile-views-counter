mod state;

use axum::{response::Html, routing::get, Router};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use std::{net::SocketAddr, time::Duration};

use state::{PostgresDB, State};

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

    // setup connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&db_connection_str)
        .await
        .expect("cannot connect to database");

    tracing::info!("connected to database");

    let db = PostgresDB::new(pool);
    State::initialize(db).await.unwrap();

    // build our application with some routes
    let app = Router::new().route("/", get(handler));

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
