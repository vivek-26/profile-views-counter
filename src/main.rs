mod datastore;
mod handler;
// mod keepalive;
mod fetcher;
mod state;

use axum::routing::{get, head};
use axum::{Extension, Router};
use datastore::PostgresDB;
use dotenv::dotenv;
use fetcher::BadgeFetcher;
use state::State;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{signal, task};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let is_production = std::env::var("PRODUCTION").is_ok();
    match is_production {
        // local env
        false => {
            dotenv().ok();

            tracing::subscriber::set_global_default(
                tracing_subscriber::fmt()
                    .pretty()
                    .with_env_filter(EnvFilter::from_default_env())
                    .finish(),
            )
            .expect("failed to set global default subscriber");
        }
        // production env
        true => {
            tracing::subscriber::set_global_default(
                tracing_subscriber::fmt()
                    .json()
                    .with_env_filter(EnvFilter::from_default_env())
                    .with_target(false)
                    .finish(),
            )
            .expect("failed to set global default subscriber");
        }
    }

    let db_connection_str =
        std::env::var("DATABASE_URL").expect("missing env variable DATABASE_URL");

    // setup database connection pool
    let db = PostgresDB::new(&db_connection_str).await;

    // initialize state
    let state = Arc::new(State::initialize(db).await.unwrap());
    let state_clone = state.clone();
    let state_destory_clone = state.clone();

    // initialize badge fetcher
    let badge_fetcher = Arc::new(BadgeFetcher::new());

    // async thread to update profile views in database at regular intervals
    let _update_loop_handle = task::spawn(async move {
        state_clone.update_loop().await;
    });

    // build our application with some routes
    let app = Router::new()
        .route("/healthz", head(handler::health_check_handler))
        .route("/count.svg", get(handler::profile_views_handler))
        .layer(Extension(state))
        .layer(Extension(badge_fetcher));

    // async thread to keep server alive by hitting health check route at regular intervals
    // let _server_keep_alive_loop_handle = task::spawn(async move {
    //     server_keep_alive.health_check_loop().await;
    // });

    // run it with hyper
    let port = std::env::var("PORT")?
        .parse::<u16>()
        .expect("missing env variable PORT");

    let addr: SocketAddr = match is_production {
        false => format!("127.0.0.1:{}", port)
            .parse()
            .expect("could not parse socket address"),
        true => format!("[::]:{}", port) // for fly.io
            .parse()
            .expect("could not parse socket address"),
    };

    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal());

    tracing::info!("server running on {}", addr);

    if let Err(err) = server.await {
        tracing::error!("server encountered an error: {}", err);
    }

    // cleanup resources
    state_destory_clone.destroy().await;
    tracing::info!("database connection closed, cleanup complete");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install ctrl+c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::interrupt())
            .expect("failed to install interrupt signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
