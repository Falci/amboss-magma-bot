#![allow(unused)]
use std::fs;

use api::Api;
use config::load as load_config;
use errors::ForbiddenError;
use log::debug;
use node::LNNode;
use service::Service;

mod api;
mod config;
mod errors;
mod mempool;
mod node;
mod service;
mod traits;

#[tokio::main]
async fn main() {
    env_logger::init();

    let mut service = Service::new().await;
    service.start().await;
}
