//! # Stellar Federation
//!
//! The `stellar-federation` crate provides functions to map Stellar
//! addresses to more information about a user. For more information,
//! see the [official Stellar
//! documentation](https://developers.stellar.org/docs/glossary/federation/)
//! and
//! [SEP-0002](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0002.md).
//!
//! ## Example Usage
//!
//! ```rust
//! use stellar_federation::resolve_stellar_address;
//!
//! # async fn run() -> std::result::Result<(), stellar_federation::Error> {
//! let address = resolve_stellar_address("with-text-memo*ceccon.me").await?;
//! println!("Address = {:?}", address);
//! # Ok(())
//! # }
//! ```
#[macro_use]
extern crate serde_derive;

use hyper::Client;
use hyper_tls::HttpsConnector;
use serde::de::{Deserialize, Deserializer, Error as SerdeError};
use stellar_base::{Memo, PublicKey};
use url::Url;

/// Stellar federation response.
#[derive(Debug, Clone)]
pub struct FederationResponse {
    /// The Stellar address, for example `example*stellar.org`.
    pub stellar_address: String,
    /// The Stellar account id.
    pub account_id: PublicKey,
    /// An optional memo to include when sending payments to the address.
    pub memo: Option<Memo>,
}

/// Resolves a Stellar address, automatically discovering the federation server to use.
pub async fn resolve_stellar_address(address: &str) -> Result<FederationResponse, Error> {
    let mut address_parts = address.split("*").into_iter();
    match (
        address_parts.next(),
        address_parts.next(),
        address_parts.next(),
    ) {
        (Some(_name), Some(domain), None) => {
            let toml = stellar_toml::resolve(&domain).await?;
            if let Some(federation_server) = toml.federation_server {
                let url: Url = federation_server.to_string().parse()?;
                resolve_stellar_address_from_server(&address, &url).await
            } else {
                Err(Error::MissingFederationServer)
            }
        }
        _ => Err(Error::InvalidStellarAddress),
    }
}

/// Resolves a Stellar address using the specified federation server.
pub async fn resolve_stellar_address_from_server(
    address: &str,
    server: &Url,
) -> Result<FederationResponse, Error> {
    let url = stellar_address_request_url(address, server);
    resolve_url(&url).await
}

/// Returns the url for a Stellar address federation request.
pub fn stellar_address_request_url(address: &str, server: &Url) -> Url {
    let mut url = server.clone();
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("type", "name");
        query.append_pair("q", address);
    }
    url
}

/// Resolves the `account_id` using the specified federation server.
pub async fn resolve_stellar_account_id(
    account_id: &PublicKey,
    server: &Url,
) -> Result<FederationResponse, Error> {
    let url = stellar_account_id_request_url(account_id, server);
    resolve_url(&url).await
}

/// Returns the url for a Stellar account id request.
pub fn stellar_account_id_request_url(public_key: &PublicKey, server: &Url) -> Url {
    let mut url = server.clone();
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("type", "id");
        query.append_pair("q", &public_key.account_id());
    }
    url
}

/// Resolves the `tx_id` using the specified federation server.
pub async fn resolve_stellar_transaction_id(
    tx_id: &str,
    server: &Url,
) -> Result<FederationResponse, Error> {
    let url = stellar_transaction_id_request_url(tx_id, server);
    resolve_url(&url).await
}

/// Returns the url for a Stellar transaction id request.
pub fn stellar_transaction_id_request_url(tx_id: &str, server: &Url) -> Url {
    let mut url = server.clone();
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("type", "txid");
        query.append_pair("q", tx_id);
    }
    url
}

/// Resolves to the information to send a payment to a different network or institution.
///
/// The `forward_parameters` parameters will vary depending on what
/// institution is the destination of the payment. The `stellar.toml`
/// file of the institution should specify which parameters to
/// include.
pub async fn resolve_stellar_forward<'a, K>(
    forward_parameters: K,
    server: &Url,
) -> Result<FederationResponse, Error>
where
    K: IntoIterator<Item = (&'a str, &'a str)>,
{
    let url = stellar_forward_request_url(forward_parameters, server);
    resolve_url(&url).await
}

/// Returns the url for a forward request.
pub fn stellar_forward_request_url<'a, K>(forward_parameters: K, server: &Url) -> Url
where
    K: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut url = server.clone();
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("type", "forward");
        for (k, v) in forward_parameters.into_iter() {
            query.append_pair(k, v);
        }
    }
    url
}

async fn resolve_url(url: &Url) -> Result<FederationResponse, Error> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let uri: hyper::Uri = url.to_string().parse()?;
    let response = client.get(uri).await?;

    if response.status().is_success() {
        let bytes = hyper::body::to_bytes(response).await?;
        let result: FederationResponse = serde_json::from_slice(&bytes)?;
        Ok(result)
    } else if response.status().is_client_error() {
        Err(Error::ClientError(response))
    } else {
        Err(Error::ServerError(response))
    }
}

/// Crate error type.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Invalid stellar address.
    #[error("invalid stellar address")]
    InvalidStellarAddress,
    /// Federation server is missing.
    #[error("missing federation server")]
    MissingFederationServer,
    /// The client sent a bad request.
    #[error("client response error")]
    ClientError(hyper::Response<hyper::Body>),
    /// Server error response.
    #[error("server response error")]
    ServerError(hyper::Response<hyper::Body>),
    /// Error resolving `stellar.toml` file.
    #[error("toml resolve error")]
    TomlResolveError(#[from] stellar_toml::Error),
    /// Error parsing json.
    #[error("json error")]
    JsonError(#[from] serde_json::error::Error),
    /// Http error.
    #[error("hyper error")]
    HyperError(#[from] hyper::Error),
    /// Invalid url format.
    #[error("invalid url")]
    InvalidUrl(#[from] url::ParseError),
    /// Invalid uri format.
    #[error("invalid uri")]
    InvalidUri(#[from] http::uri::InvalidUri),
}

#[derive(Deserialize, Debug)]
struct IntermediateFederationResponse {
    pub stellar_address: String,
    pub account_id: String,
    pub memo_type: Option<String>,
    pub memo: Option<String>,
}

impl<'de> Deserialize<'de> for FederationResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let intermediate = IntermediateFederationResponse::deserialize(deserializer)?;

        let account_id = PublicKey::from_account_id(intermediate.account_id.trim())
            .map_err(|_| SerdeError::custom("Malformed account_id"))?;

        let memo = match (intermediate.memo_type, intermediate.memo) {
            (None, None) => Ok(None),
            (Some(ref t), Some(value)) if t == "text" => {
                let memo =
                    Memo::new_text(value).map_err(|_| SerdeError::custom("Malformed text memo"))?;
                Ok(Some(memo))
            }
            (Some(ref t), Some(value)) if t == "id" => {
                let id: u64 = value
                    .parse()
                    .map_err(|_| SerdeError::custom("Malformed id memo"))?;
                let memo = Memo::new_id(id);
                Ok(Some(memo))
            }
            (Some(ref t), Some(value)) if t == "hash" => {
                let hash = base64::decode(value)
                    .map_err(|_| SerdeError::custom("Malformed base64 hash memo"))?;
                let memo =
                    Memo::new_hash(&hash).map_err(|_| SerdeError::custom("Malformed hash memo"))?;
                Ok(Some(memo))
            }
            _ => Err(SerdeError::custom("Invalid memo_type or memo")),
        }?;

        let response = FederationResponse {
            stellar_address: intermediate.stellar_address.clone(),
            account_id,
            memo,
        };
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use stellar_base::PublicKey;
    use url::Url;

    #[test]
    fn test_stellar_address_request_url() {
        let server: Url = "https://example.org/federation".parse().unwrap();
        let url = stellar_address_request_url("test*example.org", &server);
        assert_eq!("/federation", url.path());
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(Some(&"test*example.org".to_string()), query.get("q"));
        assert_eq!(Some(&"name".to_string()), query.get("type"));
    }

    #[test]
    fn test_account_id_request_url() {
        let server: Url = "https://example.org/federation".parse().unwrap();
        let public_key =
            PublicKey::from_account_id("GBUFHFEIMKTBQQFDSCAZFOC6MAUE3EHBVE4S4RYKMX62PMWDIDSD44CP")
                .unwrap();
        let url = stellar_account_id_request_url(&public_key, &server);
        assert_eq!("/federation", url.path());
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(
            Some(&"GBUFHFEIMKTBQQFDSCAZFOC6MAUE3EHBVE4S4RYKMX62PMWDIDSD44CP".to_string()),
            query.get("q")
        );
        assert_eq!(Some(&"id".to_string()), query.get("type"));
    }

    #[test]
    fn test_transaction_id_request_url() {
        let server: Url = "https://example.org/federation".parse().unwrap();
        let url = stellar_transaction_id_request_url(
            "39be7a5a001bc542c297bf7594f75bd2a8093f024ba598f19f2e71b2745b51f6",
            &server,
        );
        assert_eq!("/federation", url.path());
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(
            Some(&"39be7a5a001bc542c297bf7594f75bd2a8093f024ba598f19f2e71b2745b51f6".to_string()),
            query.get("q")
        );
        assert_eq!(Some(&"txid".to_string()), query.get("type"));
    }

    #[test]
    fn test_forward_request_url() {
        let server: Url = "https://example.org/federation".parse().unwrap();
        let parameters: HashMap<&str, &str> = [
            ("forward_type", "bank_account"),
            ("swift", "BOPBPHMM"),
            ("acct", "2382376"),
        ]
        .iter()
        .cloned()
        .collect();

        let url = stellar_forward_request_url(parameters, &server);
        assert_eq!("/federation", url.path());
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(None, query.get("q"));
        assert_eq!(Some(&"forward".to_string()), query.get("type"));
        assert_eq!(Some(&"bank_account".to_string()), query.get("forward_type"));
        assert_eq!(Some(&"BOPBPHMM".to_string()), query.get("swift"));
        assert_eq!(Some(&"2382376".to_string()), query.get("acct"));
    }
}
