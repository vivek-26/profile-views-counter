use std::net::SocketAddr;

use axum::response::Html;
use axum::routing::get as GET;
use axum::Router;

#[tokio::main]
async fn main() {
    let counter_route = Router::new().route("/count.svg", GET(|| async { Html("Hello World!") }));

    // start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 9000));
    println!("listening on {addr}");
    axum::Server::bind(&addr)
        .serve(counter_route.into_make_service())
        .await
        .unwrap();
}
