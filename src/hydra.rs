use std::collections::HashMap;
use serde_derive::{Serialize, Deserialize};
use actix_http::Response;

pub struct Hydra {}

const hydraUrl: &str = "http://localhost:4445";

#[derive(Serialize, Deserialize, Debug)]
pub struct HydraAcceptLoginRequest {
    pub subject: String,
    pub remember: bool,
    pub remember_for: i32
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HydraLoginResponse {
    pub challenge: String,
    pub requested_scope: Vec<String>,
    pub requested_access_token_audience: Option<String>,
    pub skip: bool,
    pub subject: String,
    // pub oidc_context: String,
    pub client: HydraClient,
    pub request_url: String,
    pub session_id: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HydraClient {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub scope: String,
    pub audience: Option<String>,
    pub owner: String,
    pub policy_uri: String,
    pub allowed_cors_origins: Option<String>,
    pub tos_uri: String,
    pub client_uri: String,
    pub logo_uri: String,
    pub contacts: Option<String>,
    pub client_secret_expires_at: i32,
    pub subject_type: String,
    pub token_endpoint_auth_method: String,
    pub userinfo_signed_response_alg: String,
    pub created_at: String,
    pub updated_at: String
}

impl Hydra {
    pub async fn get(flow: String, challenge: String) -> Result<String, failure::Error> {
        let url = format!("{}/oauth2/auth/requests/{}?{}_challenge={}", hydraUrl, flow, flow, challenge);
        let resp: reqwest::Response = reqwest::get(&url).await?;

        println!("DEBUG {:?}", resp);

        return Ok(resp.text().await?);
    }

    pub async fn put(flow: String, action: String, challenge: String, body: String) -> Result<String, failure::Error> {
        let url = format!("{}/oauth2/auth/requests/{}/{}?{}_challenge={}", hydraUrl, flow, action, flow, challenge);
        let client = reqwest::Client::new();
        println!("DEBUG {:?}", body);
        let resp: String = client.put(&url).body(body).send().await?.text().await?;

        println!("DEBUG {:?}", resp.clone());

        return Ok(resp.replace("\\u0026", "&"));
    }

    pub async fn get_login_request(challenge: String) -> Result<String, failure::Error> {
        Self::get("login".to_string(), challenge).await
    }

    pub async fn accept_login_request(challenge: String, body: String) -> Result<String, failure::Error> {
        Self::put("login".to_string(), "accept".to_string(), challenge, body).await
    }

    pub async fn reject_login_request(challenge: String, body: String) -> Result<String, failure::Error> {
        Self::put("login".to_string(), "reject".to_string(), challenge, body).await
    }

    pub async fn get_consent_request(challenge: String) -> Result<String, failure::Error> {
        Self::get("consent".to_string(), challenge).await
    }

    pub async fn accept_consent_request(challenge: String, body: String) -> Result<String, failure::Error> {
        Self::put("consent".to_string(), "accept".to_string(), challenge, body).await
    }

    pub async fn reject_consent_request(challenge: String, body: String) -> Result<String, failure::Error> {
        Self::put("consent".to_string(), "reject".to_string(), challenge, body).await
    }

    pub async fn get_logout_request(challenge: String) -> Result<String, failure::Error> {
        Self::get("logout".to_string(), challenge).await
    }

    pub async fn accept_logout_request(challenge: String, body: String) -> Result<String, failure::Error> {
        Self::put("logout".to_string(), "accept".to_string(), challenge, body).await
    }

    pub async fn reject_logout_request(challenge: String, body: String) -> Result<String, failure::Error> {
        Self::put("logout".to_string(), "reject".to_string(), challenge, body).await
    }
}