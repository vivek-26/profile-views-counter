use reqwest::{Client, StatusCode};
use std::time::Duration;
use tokio::time;
use tokio_stream::{wrappers::IntervalStream, StreamExt};

pub struct KeepAlive {
    http_client: Client,
    port: u16,
    interval: u64,
}

impl KeepAlive {
    pub fn new(port: u16, interval: u64) -> KeepAlive {
        let http_client = reqwest::ClientBuilder::new()
            .pool_max_idle_per_host(5)
            .pool_idle_timeout(Duration::from_secs(600))
            .build()
            .expect("failed to initialize server keep alive client");

        KeepAlive {
            http_client,
            port,
            interval,
        }
    }

    pub async fn health_check_loop(&self) {
        let mut stream = IntervalStream::new(time::interval(Duration::from_secs(self.interval)));

        while stream.next().await.is_some() {
            let response = self
                .http_client
                .get(format!("http://127.0.0.1:{}/healthz", self.port))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status() == StatusCode::OK {
                        tracing::info!("health check succeeded");
                    } else {
                        tracing::error!(
                            "health check returned non ok response, received status code: {}",
                            resp.status()
                        );
                    }
                }
                Err(err) => {
                    tracing::error!("health check failed, reason: {}", err);
                }
            }
        }
    }
}
