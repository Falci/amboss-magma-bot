use magma::Api;
use node::LNNode;

mod magma;
mod node; // Declare the module

#[tokio::main]
async fn main() {
    let node = LNNode::from_env().await.unwrap();
        
    Api::from_node(node).await;
}
