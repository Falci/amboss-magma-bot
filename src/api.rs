use std::collections::HashMap;

use graphql_client::{GraphQLQuery, Response};
use log::{debug, info, trace, warn};
use orders::{OrdersGetUserMarket, OrdersGetUserMarketOfferOrdersList};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

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
    query_path = "resources/graphql/AddTransaction.graphql",
    response_derives = "Debug, Deserialize"
)]
struct AddTransaction;

pub struct Api {
    api_key: String,
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
    const API_URL: &'static str = "https://api.amboss.space/graphql";

    fn new(api_key: String) -> Api {
        Api { api_key }
    }

    pub async fn from_signer<F, S>(signer: F) -> Result<Api, Box<dyn std::error::Error>>
    where
        F: Fn(String) -> S,
        S: std::future::Future<Output = Result<String, Box<dyn std::error::Error>>>,
    {
        let api = Api::new("".to_string());

        let info = api.get_sign_info().await?.get_sign_info;
        let signature = signer(info.message).await?;
        let api_key = api.login(info.identifier, signature).await?.login;

        println!("MAGMA API key acquired");

        Ok(Api::new(api_key))
    }

    async fn request<Var, Res>(
        &self,
        request_body: graphql_client::QueryBody<Var>,
    ) -> Result<Res, Box<dyn std::error::Error>>
    where
        Var: Serialize,
        Res: serde::de::DeserializeOwned,
    {
        // Send the request asynchronously
        let res = Client::new()
            .post(Api::API_URL)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(&request_body)
            .send()
            .await?
            .json::<Response<Res>>()
            .await?;

        // Process the response
        if let Some(data) = res.data {
            log_cost(res.extensions);

            Ok(data)
        } else if let Some(errors) = res.errors {
            Err(format!("GraphQL errors: {:?}", errors).into())
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
        identifier: String,
        signature: String,
    ) -> Result<login::ResponseData, Box<dyn std::error::Error>> {
        debug!("Login");
        let request_body = Login::build_query(login::Variables {
            identifier,
            signature,
            token: Some(true),
        });
        self.request::<login::Variables, login::ResponseData>(request_body)
            .await
    }

    pub async fn accept_order(
        &self,
        order_id: String,
        invoice: String,
    ) -> Result<accept_order::ResponseData, Box<dyn std::error::Error>> {
        debug!("AcceptOrder");
        let request_body = AcceptOrder::build_query(accept_order::Variables { order_id, invoice });
        self.request::<accept_order::Variables, accept_order::ResponseData>(request_body)
            .await
    }

    pub async fn reject_order(
        &self,
        order_id: String,
    ) -> Result<reject_order::ResponseData, Box<dyn std::error::Error>> {
        debug!("RejectOrder");
        let request_body = RejectOrder::build_query(reject_order::Variables { order_id });
        self.request::<reject_order::Variables, reject_order::ResponseData>(request_body)
            .await
    }

    pub async fn confirm_channel_open(
        &self,
        order_id: String,
        tx_point: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("ConfirmChannelOpen");
        let request_body =
            AddTransaction::build_query(add_transaction::Variables { order_id, tx_point });
        self.request::<add_transaction::Variables, add_transaction::ResponseData>(request_body)
            .await?;

        Ok(())
    }
}
