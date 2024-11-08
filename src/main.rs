use magma::Api;
use node::LNNode;

mod magma;
mod node; // Declare the module

fn main() {
    // let magma = magma::Api::from_env();
    // let data = magma.get_offer_orders().unwrap();
    // println!("{:#?}", data);
    
    let node = LNNode::new("api_key".to_string());
    let magma = Api::from_node(node);



}
