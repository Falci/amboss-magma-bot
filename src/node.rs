use crate::traits::Signer;
use lnd_grpc_rust::lnrpc;
use lnd_grpc_rust::walletrpc;
use lnd_grpc_rust::LndClient;
use log::debug;
use serde::de;
use serde::Deserialize;
use std::{cell::RefCell, fs, path::PathBuf};
use tonic::client;

#[derive(Debug, Deserialize)]
pub struct LNDConfig {
    pub host: String,
    pub macaroon_hex: Option<String>,
    pub macaroon_path: Option<String>,
    pub tls_cert_hex: Option<String>,
    pub tls_cert_path: Option<String>,
}

pub struct LNNode {
    client: RefCell<LndClient>,
}

impl LNNode {
    pub async fn new(config: LNDConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let macaroon_hex = match config.macaroon_hex {
            Some(val) => val,
            None => {
                let path = config
                    .macaroon_path
                    .as_deref()
                    .unwrap_or("~/.lnd/data/chain/bitcoin/mainnet/admin.macaroon");
                read_file_as_hex(path)?
            }
        };

        let tls_cert_hex = match config.tls_cert_hex {
            Some(cert) => hex::encode(cert.into_bytes()),
            None => {
                let path = config.tls_cert_path.as_deref().unwrap_or("~/.lnd/tls.cert");
                read_file_as_hex(path)?
            }
        };

        debug!("Connecting to LND at {}", config.host);

        let client = lnd_grpc_rust::connect(tls_cert_hex, macaroon_hex, config.host)
            .await
            .unwrap();

        debug!("Connected to LND");

        Ok(Self {
            client: RefCell::new(client),
        })
    }

    pub async fn sign_message(
        &self,
        message: impl AsRef<str>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let message_bytes = message.as_ref().as_bytes().to_vec();

        let mut client = self.client.borrow_mut();

        let request = lnrpc::SignMessageRequest {
            msg: message_bytes,
            single_hash: false,
        };

        let response = client.lightning().sign_message(request).await?;

        let signature = response.into_inner().signature;

        Ok(signature)
    }

    pub async fn connect_to_node(
        &self,
        host: &str,
        pubkey: &str,
    ) -> Result<lnrpc::ConnectPeerResponse, Box<dyn std::error::Error>> {
        let mut client = self.client.borrow_mut();

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
        let mut client = self.client.borrow_mut();

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
        let mut client = self.client.borrow_mut();

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
        let mut client = self.client.borrow_mut();

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

#[async_trait::async_trait(?Send)]
impl Signer for LNNode {
    async fn sign(&self, message: &str) -> Result<String, Box<dyn std::error::Error>> {
        debug!("Signing message: {}", message);
        self.sign_message(message).await
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(path.trim_start_matches("~/"));
        }
    }
    PathBuf::from(path)
}

fn read_file_as_hex(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let expanded = expand_tilde(path);
    let bytes = fs::read(&expanded)
        .map_err(|e| format!("Cannot read file {}: {}", expanded.display(), e))?;
    Ok(hex::encode(bytes))
}
