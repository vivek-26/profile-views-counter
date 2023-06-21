use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::{get, head};
use axum::Router;
use dotenv::dotenv;
use tokio::signal;
use tracing_subscriber::EnvFilter;

use badge::Shields;
use datastore::Xata;
use state::AppState;

mod badge;
mod datastore;
mod handler;
// mod keepalive;
mod state;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let is_production_env = std::env::var("PRODUCTION").is_ok();
    setup_logger(is_production_env);

    // setup xata serverless db client
    let db = Xata::new()?;

    // initialize shields io badge
    let shields_io_badge = Shields::new()?;

    // initialize state
    let app_state = Arc::new(AppState::new(db, shields_io_badge));

    // setup application routes
    let app = Router::new()
        .route("/healthz", head(handler::health_check_handler))
        .route(
            "/:user_name/counter.svg",
            get(handler::profile_views_handler),
        )
        .with_state(app_state);

    // async thread to keep server alive by hitting health check route at regular intervals
    // let _server_keep_alive_loop_handle = task::spawn(async move {
    //     server_keep_alive.health_check_loop().await;
    // });

    // read port from env variable
    let port = std::env::var("PORT")
        .expect("missing env variable PORT")
        .parse::<u16>()?;

    let addr: SocketAddr = match is_production_env {
        false => format!("127.0.0.1:{}", port).parse()?,
        true => format!("[::]:{}", port).parse()?, // for fly.io
    };

    // start server
    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal());

    tracing::info!("server running on {}", addr);

    // block until server shuts down
    if let Err(err) = server.await {
        tracing::error!("server encountered an error: {}", err);
    }

    Ok(())
}

fn setup_logger(is_production_env: bool) {
    match is_production_env {
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
