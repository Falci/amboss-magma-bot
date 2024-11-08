use reqwest::Client;

use dotenvy::dotenv;
use serde::Serialize;
use std::env;

#[derive(Debug)]
pub struct LNNode {
    host: String,
    macaroon: String,
}

impl LNNode {

    pub async fn from_env() -> Result<LNNode, Box<dyn std::error::Error>> {
        dotenv().ok(); // Load environment variables from .env
        
        let host = env::var("LND_HOST").unwrap_or("localhost:8080".to_string());

        let macaroon = LNNode::get_from_env_or_path(
            "LND_MACAROON".to_string(), 
            "~/.lnd/data/chain/bitcoin/mainnet/admin.macaroon".to_string()
        );

        let node = LNNode { host, macaroon };

        node.check_connection().await?;
        
        Ok(node)
    }

    async fn get(&self, path: String) -> Result<reqwest::Response, reqwest::Error> {
        let client = Client::builder()
            .danger_accept_invalid_certs(true) 
            .build()?;

        let url = format!("https://{}{}", &self.host, path);
        client
            .get(&url)
            .header("Grpc-Metadata-macaroon", &self.macaroon)
            .send()
            .await
    }

    async fn post<T>(&self, path: String, json: T) -> Result<reqwest::Response, reqwest::Error> 
    where
        T: Serialize,
    {
        let client = Client::builder()
            .danger_accept_invalid_certs(true) 
            .build()?;

        let url = format!("https://{}{}", &self.host, path);
        client
            .post(&url)
            .json(&json)
            .header("Grpc-Metadata-macaroon", &self.macaroon)
            .send()
            .await
    }


    fn get_from_env_or_path(env_var: String, default_path: String) -> String {
        match env::var(&env_var) {
            Ok(val) => val,
            Err(_) => {
                let path = env::var(format!("{}_PATH", env_var)).unwrap_or(default_path);
                // Read the file content and return it as a string
                std::fs::read_to_string(path).expect("Failed to read file")
            }
        }
    }

    pub async fn check_connection(&self) -> Result<(), Box<dyn std::error::Error>> {
       let res = self.get("/v1/macaroon/permissions".to_string()).await?.json::<serde_json::Value>().await?;

    //    println!("Response: {:?}", res);
    
        Ok(())
    }


    pub async fn sign(&self, message: String) -> Result<String, Box<dyn std::error::Error>> {
        let response = self.post(
            "/v1/signmessage".to_string(),
            serde_json::json!({ "msg": message })
        ).await?.json::<serde_json::Value>().await?;

        Ok(response["signature"].as_str().unwrap().to_string())
    }
}