use dotenvy::dotenv;
use graphql_client::{GraphQLQuery, Response};
use reqwest::blocking::Client;
use serde::Serialize;
use std::env;

use crate::node::LNNode;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/schema.graphql",
    query_path = "resources/MyOfferOrders.graphql",
    response_derives = "Debug, Deserialize"
)]
struct MyOfferOrders;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/schema.graphql",
    query_path = "resources/GetSignInfo.graphql",
    response_derives = "Debug, Deserialize"
)]
struct GetSignInfo;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "resources/schema.graphql",
    query_path = "resources/Login.graphql",
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

    pub fn from_env() -> Api {
        dotenv().ok(); // Load environment variables from .env

        let api_key = env::var("MAGMA_API_KEY").expect("MAGMA_API_KEY not found in .env");

        Api::new(api_key)
    }

    pub fn from_node(node: LNNode) -> Api {
        let api = Api::new("".to_string());
        
        let info = api.get_sign_info().unwrap().get_sign_info;
        let signature = node.sign(info.message).unwrap();
        let api_key = api.login(info.identifier, signature).unwrap().login;

        Api::new(api_key)
    }

    fn request<Var, Res>(&self, request_body: graphql_client::QueryBody<Var>) -> Result<Res, Box<dyn std::error::Error>> 
    where
        Var: Serialize,
        Res: serde::de::DeserializeOwned,
    {
        let client = Client::new();

        // Send the request
        let res = client
            .post(Api::API_URL)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(&request_body)
            .send()?
            .json::<Response<Res>>()?;

        // Process the response
        if let Some(data) = res.data {
            // TOOD: handle throttleStatus
            Ok(data)
        } else if let Some(errors) = res.errors {
            Err(format!("GraphQL errors: {:?}", errors).into())
        } else {
            Err("Unknown error occurred".into())
        }
    }

    pub fn get_offer_orders(&self) -> Result<my_offer_orders::ResponseData, Box<dyn std::error::Error>> {
        let request_body = MyOfferOrders::build_query(my_offer_orders::Variables {});
        self.request::<my_offer_orders::Variables, my_offer_orders::ResponseData>(request_body) 
    }

    fn get_sign_info(&self) -> Result<get_sign_info::ResponseData, Box<dyn std::error::Error>> {
        let request_body = GetSignInfo::build_query(get_sign_info::Variables {});
        self.request::<get_sign_info::Variables, get_sign_info::ResponseData>(request_body) 
    }

    fn login(&self, identifier: String, signature: String) -> Result<login::ResponseData, Box<dyn std::error::Error>> {
        let request_body = Login::build_query(login::Variables {
            identifier,
            signature,
            token: Some(true),
        });
        self.request::<login::Variables, login::ResponseData>(request_body) 
    }
}
