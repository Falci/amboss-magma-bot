use env_logger::Env;
use log::debug;
use std::env;
use tokio::time::{sleep, Duration};

use api::Api;
use node::LNNode;
use service::Service;

mod api;
mod mempool;
mod node;
mod service;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(Env::default().default_filter_or("debug"));

    let node = LNNode::from_env().await.unwrap();
    let magma = Api::from_signer(|msg: String| async { node.sign(msg).await })
        .await
        .unwrap();

    let interval = env::var("INTERVAL")
        .unwrap_or_else(|_| "10".to_string())
        .parse::<u64>()
        .unwrap();

    let service = Service::new(node, magma);

    loop {
        service.run().await.unwrap();

        debug!("Sleeping for {} seconds...", interval);
        sleep(Duration::from_secs(interval)).await;
    }
}
