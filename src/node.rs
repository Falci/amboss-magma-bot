pub struct LNNode {
    api_key: String,
}

impl LNNode {
    pub fn new(api_key: String) -> LNNode {
        LNNode { api_key }
    }

    pub fn sign(&self, message: String) -> Result<String, Box<dyn std::error::Error>> {
        // Sign the message
        Ok(message)
    }
}