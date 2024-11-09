use magma::Api;
use node::LNNode;

mod magma;
mod node;

#[tokio::main]
async fn main() {
    let node = LNNode::from_env().await.unwrap();
    let signer = |msg: String| async { node.sign(msg).await };

    let magma = Api::from_signer(signer).await.unwrap();

    let offers = magma.get_orders().await.unwrap();

    println!("{:?}", offers);
}
