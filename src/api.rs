use std::{collections::HashMap, fs};

use graphql_client::{GraphQLQuery, Response};
use log::{debug, info};
use orders::OrdersGetUserMarketOfferOrdersList;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{errors::ForbiddenError, traits::Signer};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/Orders.graphql",
    response_derives = "Debug, Deserialize"
)]
pub struct Orders;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/GetSignInfo.graphql",
    response_derives = "Debug, Deserialize"
)]
struct GetSignInfo;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/Login.graphql",
    response_derives = "Debug, Deserialize"
)]
struct Login;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/AcceptOrder.graphql",
    response_derives = "Debug, Deserialize"
)]
struct AcceptOrder;
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/RejectOrder.graphql",
    response_derives = "Debug, Deserialize"
)]
struct RejectOrder;
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/CancelOrder.graphql",
    response_derives = "Debug, Deserialize"
)]
pub struct CancelOrder;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/AddTransaction.graphql",
    response_derives = "Debug, Deserialize"
)]
struct AddTransaction;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/GetNodeAddress.graphql",
    response_derives = "Debug, Deserialize"
)]
struct GetNodeAddress;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/CreateApiKey.graphql",
    response_derives = "Debug, Deserialize"
)]
struct CreateApiKey;

#[derive(Debug, Deserialize, Clone)]
pub struct MagmaConfig {
    pub api_key: Option<String>,
    pub api_key_expiration: Option<f64>,
}

const API_KEY_FILE: &str = ".amboss_magma_bot.jwt";

pub struct Api {
    config: MagmaConfig,
}

fn log_cost(extensions: Option<HashMap<String, Value>>) {
    if let Some(extensions) = extensions {
        if let Some(cost) = extensions.get("cost") {
            debug!(
                " - Requested query cost: {}",
                cost.get("requestedQueryCost").unwrap()
            );

            if let Some(throttle) = cost.get("throttleStatus") {
                debug!(
                    " - Throttled query remaining: {}",
                    throttle.get("currentlyAvailable").unwrap()
                );
            }
        }
    }
}

impl Api {
    const API_URL: &str = "https://api.amboss.space/graphql";

    pub fn new(config: MagmaConfig) -> Self {
        Api { config }
    }

    pub async fn gen_new_api_key<T: Signer>(
        &mut self,
        node: &T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.login_with_node(node).await?;
        self.create_api_key().await?;
        self.write_api_key_to_file()?;
        Ok(())
    }

    pub fn load_api_key_from_file(&mut self) -> Result<(), std::io::Error> {
        fs::read_to_string(API_KEY_FILE).and_then(|api_key| {
            self.set_api_key(Some(api_key));
            Ok(())
        })
    }

    fn write_api_key_to_file(&self) -> Result<(), std::io::Error> {
        if let Some(api_key) = &self.config.api_key {
            fs::write(API_KEY_FILE, api_key)?;
        }
        Ok(())
    }

    async fn login_with_node<T: Signer>(
        &mut self,
        signer: &T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.set_api_key(None);

        // Get the sign challenge
        let info = self.get_sign_info().await?.get_sign_info;

        // Sign the challenge
        let signature = signer.sign(info.message.as_str()).await?;

        // Get the login token
        let api_key_from_node = self
            .login(info.identifier.as_str(), signature.as_str())
            .await?
            .login;

        info!("MAGMA API key acquired via login with node");

        self.set_api_key(Some(api_key_from_node));

        Ok(())
    }

    pub async fn create_api_key(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("CreateApiKey");
        let request_body = CreateApiKey::build_query(create_api_key::Variables {
            details: Some("Amboss Magma Bot".to_string()),
            seconds: self.config.api_key_expiration.or(Some(2592000.0)),
        });

        let api_key = self
            .request::<create_api_key::Variables, create_api_key::ResponseData>(request_body)
            .await?
            .create_api_key;

        info!("MAGMA API key created");

        self.set_api_key(Some(api_key));

        Ok(())
    }

    async fn request<Var, Res>(
        &self,
        request_body: graphql_client::QueryBody<Var>,
    ) -> Result<Res, Box<dyn std::error::Error>>
    where
        Var: Serialize,
        Res: serde::de::DeserializeOwned,
    {
        let client = Client::new();
        let mut request = client.post(Api::API_URL).json(&request_body);

        if let Some(api_key) = &self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let res = request.send().await?.json::<Response<Res>>().await?;

        // Process the response
        if let Some(data) = res.data {
            log_cost(res.extensions);

            Ok(data)
        } else if let Some(errors) = res.errors {
            // check if "errors[].extensions.code === FORBIDDEN"
            if errors.iter().any(|e| {
                e.extensions.as_ref().map_or(false, |ext| {
                    ext.get("code")
                        .and_then(|code| code.as_str())
                        .map_or(false, |code| code == "FORBIDDEN")
                })
            }) {
                Err(ForbiddenError {}.into())
            } else {
                Err(format!("GraphQL error: {:?}", errors).into())
            }
        } else {
            Err("Unknown error occurred".into())
        }
    }

    pub async fn get_orders(
        &self,
    ) -> Result<Vec<OrdersGetUserMarketOfferOrdersList>, Box<dyn std::error::Error>> {
        debug!("GetOrders");
        let request_body = Orders::build_query(orders::Variables {});

        let orders = self
            .request::<orders::Variables, orders::ResponseData>(request_body)
            .await?
            .get_user
            .market
            .map_or_else(|| Vec::new(), |market| market.offer_orders.list);

        Ok(orders)
    }

    async fn get_sign_info(
        &self,
    ) -> Result<get_sign_info::ResponseData, Box<dyn std::error::Error>> {
        debug!("GetSignInfo");
        let request_body = GetSignInfo::build_query(get_sign_info::Variables {});
        self.request::<get_sign_info::Variables, get_sign_info::ResponseData>(request_body)
            .await
    }

    async fn login(
        &self,
        identifier: &str,
        signature: &str,
    ) -> Result<login::ResponseData, Box<dyn std::error::Error>> {
        debug!("Login");
        let request_body = Login::build_query(login::Variables {
            identifier: identifier.to_string(),
            signature: signature.to_string(),
            token: Some(true),
        });
        self.request::<login::Variables, login::ResponseData>(request_body)
            .await
    }

    pub async fn accept_order(
        &self,
        order_id: &str,
        invoice: &str,
    ) -> Result<accept_order::ResponseData, Box<dyn std::error::Error>> {
        debug!("AcceptOrder");
        let request_body = AcceptOrder::build_query(accept_order::Variables {
            order_id: order_id.to_string(),
            invoice: invoice.to_string(),
        });
        self.request::<accept_order::Variables, accept_order::ResponseData>(request_body)
            .await
    }

    pub async fn reject_order(
        &self,
        order_id: &str,
    ) -> Result<reject_order::ResponseData, Box<dyn std::error::Error>> {
        debug!("RejectOrder");
        let request_body = RejectOrder::build_query(reject_order::Variables {
            order_id: order_id.to_string(),
        });
        self.request::<reject_order::Variables, reject_order::ResponseData>(request_body)
            .await
    }

    pub async fn cancel_order(
        &self,
        order_id: &str,
        reason: cancel_order::OrderCancellationReason,
    ) -> Result<cancel_order::ResponseData, Box<dyn std::error::Error>> {
        debug!("CancelOrder");
        let request_body = CancelOrder::build_query(cancel_order::Variables {
            order_id: order_id.to_string(),
            reason,
        });
        self.request::<cancel_order::Variables, cancel_order::ResponseData>(request_body)
            .await
    }

    pub async fn confirm_channel_open(
        &self,
        order_id: &str,
        tx_point: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("ConfirmChannelOpen");
        let request_body = AddTransaction::build_query(add_transaction::Variables {
            order_id: order_id.to_string(),
            tx_point: tx_point.to_string(),
        });
        self.request::<add_transaction::Variables, add_transaction::ResponseData>(request_body)
            .await?;

        Ok(())
    }

    pub async fn get_node_addresses(
        &self,
        pubkey: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        debug!("GetNodeAddress");
        let request_body = GetNodeAddress::build_query(get_node_address::Variables {
            pubkey: pubkey.to_string(),
        });
        let addresses = self
            .request::<get_node_address::Variables, get_node_address::ResponseData>(request_body)
            .await?
            .get_node
            .graph_info
            .node
            .expect("Node not found")
            .addresses
            .iter()
            .map(|a| a.addr.clone())
            .collect::<Vec<String>>();

        Ok(addresses)
    }

    pub fn set_api_key(&mut self, new_key: Option<String>) {
        self.config.api_key = new_key;
    }
}
