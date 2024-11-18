use dotenvy::dotenv;

use lnd_grpc_rust::lnrpc;
use lnd_grpc_rust::walletrpc;
use lnd_grpc_rust::LndClient;
use log::debug;
use std::env;

fn hex(bytes: Vec<u8>) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

fn get_host() -> String {
    env::var("LND_HOST").unwrap_or("localhost:10009".to_string())
}

fn get_macaroon() -> String {
    env::var("LND_MACAROON").unwrap_or_else(|_| {
        let path = env::var("LND_MACAROON_PATH")
            .unwrap_or_else(|_| "~/.lnd/data/chain/bitcoin/mainnet/admin.macaroon".to_string());

        let value =
            std::fs::read(&path).expect(format!("Cannot load macaroon file {}", path).as_str());

        hex(value)
    })
}

fn get_tls_cert() -> String {
    env::var("LND_TLS_CERT").unwrap_or_else(|_| {
        let path = env::var("LND_TLS_CERT_PATH").unwrap_or_else(|_| "~/.lnd/tls.cert".to_string());

        std::fs::read_to_string(&path)
            .expect(format!("Cannot load TLS certificate file {}", path).as_str())
    })
}

#[derive(Debug)]
pub struct LNNode {
    host: String,
    macaroon: String,
    tls_cert: String,
}

impl LNNode {
    pub async fn from_env() -> Result<LNNode, Box<dyn std::error::Error>> {
        dotenv().ok(); // Load environment variables from .env

        let host = get_host();
        let macaroon = get_macaroon();
        let tls_cert = get_tls_cert();

        let node = LNNode {
            host,
            macaroon,
            tls_cert,
        };

        // node.check_permissions().await?;

        Ok(node)
    }

    async fn client(&self) -> LndClient {
        let tls_cert = self.tls_cert.clone();
        let macaroon = self.macaroon.clone();
        let host = self.host.clone();

        lnd_grpc_rust::connect(hex(tls_cert.into_bytes()), macaroon, host)
            .await
            .unwrap()
    }

    pub async fn sign(&self, message: String) -> Result<String, Box<dyn std::error::Error>> {
        let mut client = self.client().await;

        let signature = client
            .lightning()
            .sign_message(lnrpc::SignMessageRequest {
                msg: message.into_bytes(),
                single_hash: false,
            })
            .await?
            .into_inner()
            .signature;

        Ok(signature)
    }

    pub async fn connect_to_node(
        &self,
        host: &str,
        pubkey: &str,
    ) -> Result<lnrpc::ConnectPeerResponse, Box<dyn std::error::Error>> {
        let mut client = self.client().await;

        debug!("Connecting to peer: {}", host);

        let peer = client
            .lightning()
            .connect_peer(lnrpc::ConnectPeerRequest {
                addr: Some(lnrpc::LightningAddress {
                    host: host.to_string(),
                    pubkey: pubkey.to_string(),
                }),
                perm: false,
                timeout: 120,
            })
            .await?
            .into_inner();

        debug!("Peer: {:?}", peer);

        Ok(peer)
    }

    pub async fn check_connect_to_node(
        &self,
        host: &str,
        pubkey: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.connect_to_node(host, pubkey).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.to_string().contains("already connected to peer") {
                    debug!("Already connected to peer: {}", host);
                    Ok(())
                } else {
                    debug!("Error connecting to peer: {}", e);
                    Err(e)
                }
            }
        }
    }

    pub async fn create_invoice(
        &self,
        amount: i64,
        expiry: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut client = self.client().await;

        let invoice = client
            .lightning()
            .add_invoice(lnrpc::Invoice {
                value: amount,
                expiry: expiry,
                ..Default::default()
            })
            .await?
            .into_inner()
            .payment_request;

        Ok(invoice)
    }

    pub async fn open_channel(
        &self,
        node_pubkey: Vec<u8>,
        sat_per_vbyte: u64,
        local_funding_amount: i64,
        outpoints: Vec<lnrpc::OutPoint>,
    ) -> Result<lnrpc::ChannelPoint, Box<dyn std::error::Error>> {
        let mut client = self.client().await;

        let channel = client
            .lightning()
            .open_channel_sync(lnrpc::OpenChannelRequest {
                sat_per_vbyte,
                node_pubkey,
                local_funding_amount,
                outpoints,

                ..Default::default()
            })
            .await?
            .into_inner();

        Ok(channel)
    }

    pub async fn list_unspent(&self) -> Result<Vec<lnrpc::Utxo>, Box<dyn std::error::Error>> {
        let mut client = self.client().await;

        let unspent = client
            .wallet()
            .list_unspent(walletrpc::ListUnspentRequest {
                min_confs: 3,
                ..Default::default()
            })
            .await?
            .into_inner()
            .utxos;

        Ok(unspent)
    }
}
