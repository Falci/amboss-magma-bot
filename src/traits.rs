#[async_trait::async_trait(?Send)]
pub trait Signer {
    async fn sign(&self, message: &str) -> Result<String, Box<dyn std::error::Error>>;
}
