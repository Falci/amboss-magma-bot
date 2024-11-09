use dotenvy::dotenv;
use lnd_grpc_rust::{lnrpc::MacaroonPermission, LndClient};
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

        node.check_permissions().await?;

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

    async fn get_necessary_permissions(&self) -> Vec<MacaroonPermission> {
        let mut client = self.client().await;

        let permissions = vec![
            "https://api.amboss.space/graphql",
            "/invoicesrpc.Invoices/AddHoldInvoice",
            "/lnrpc.Lightning/CheckMacaroonPermissions",
            "/lnrpc.Lightning/ConnectPeer",
            "/lnrpc.Lightning/OpenChannel",
        ];

        let perm = client
            .lightning()
            .list_permissions(lnd_grpc_rust::lnrpc::ListPermissionsRequest {})
            .await
            .expect("failed to list permissions")
            .into_inner()
            .method_permissions;

        perm.iter()
            .filter(|p| permissions.contains(&p.0.as_str()))
            .flat_map(|p| p.1.permissions.clone())
            .collect()
    }

    async fn check_permissions(&self) -> Result<(), Box<dyn std::error::Error>> {
        // let mut client = self.client().await;

        // let permissions = vec![
        //     "https://api.amboss.space/graphql",
        //     "/invoicesrpc.Invoices/AddHoldInvoice",
        //     "/lnrpc.Lightning/CheckMacaroonPermissions",
        //     "/lnrpc.Lightning/ConnectPeer",
        //     "/lnrpc.Lightning/OpenChannel",
        // ];

        // let perm = client
        //     .lightning()
        //     .list_permissions(lnd_grpc_rust::lnrpc::ListPermissionsRequest {})
        //     .await
        //     .expect("failed to list permissions")
        //     .into_inner()
        //     .method_permissions;

        // let filtered = perm.iter().filter(|p| permissions.contains(&p.0.as_str()));

        // let permissions = filtered.flat_map(|p| p.1.permissions.clone()).collect();

        // let check = client
        //     .lightning()
        //     .check_macaroon_permissions(lnd_grpc_rust::lnrpc::CheckMacPermRequest {
        //         permissions: vec![],
        //         full_method: "/lnrpc.Lightning/OpenChannel".to_string(),
        //         macaroon: self.macaroon.clone().into_bytes(),
        //     })
        //     .await?;

        // println!("Permissions: {:?}", check);

        Ok(())
    }

    pub async fn sign(&self, message: String) -> Result<String, Box<dyn std::error::Error>> {
        let mut client = self.client().await;

        let signature = client
            .lightning()
            .sign_message(lnd_grpc_rust::lnrpc::SignMessageRequest {
                msg: message.into_bytes(),
                single_hash: false,
            })
            .await?
            .into_inner()
            .signature;

        Ok(signature)
    }
}
