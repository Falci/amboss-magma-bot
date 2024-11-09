use graphql_client::{GraphQLQuery, Response};
use orders::{OrdersGetUserMarket, OrdersGetUserMarketOfferOrdersList};
use reqwest::Client;
use serde::Serialize;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/graphql/schema.graphql",
    query_path = "resources/graphql/Orders.graphql",
    response_derives = "Debug, Deserialize"
)]
struct Orders;

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

pub struct Api {
    api_key: String,
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
            // TODO: handle throttleStatus
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
        let request_body = GetSignInfo::build_query(get_sign_info::Variables {});
        self.request::<get_sign_info::Variables, get_sign_info::ResponseData>(request_body)
            .await
    }

    async fn login(
        &self,
        identifier: String,
        signature: String,
    ) -> Result<login::ResponseData, Box<dyn std::error::Error>> {
        let request_body = Login::build_query(login::Variables {
            identifier,
            signature,
            token: Some(true),
        });
        self.request::<login::Variables, login::ResponseData>(request_body)
            .await
    }
}
