use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tiny_http::{Header, Response, Server};
use url::Url;

use crate::{api, storage};

const CLIENT: &str = "2wSeRcOi0MoAv5ByV1tSUusr5VW3CP4v";
const REDIRECT: &str =
    "auth0://auth.digital.panasonic.com/android/com.panasonic.SmartLaundry/callback";

fn random_token() -> String {
    let mut bytes = [0u8; 48];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn merge_refreshed_tokens(previous: &Value, mut updated: Value) -> Result<Value> {
    ensure!(
        updated["access_token"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "Refresh response lacks an access token"
    );
    ensure!(
        updated["expires_in"].as_i64().is_some_and(|n| n > 0),
        "Refresh response lacks a valid lifetime"
    );
    if updated.get("refresh_token").is_none() {
        updated["refresh_token"] = previous["refresh_token"].clone();
    }
    ensure!(
        updated["refresh_token"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "Refresh response lacks a valid refresh token"
    );
    updated["obtained_at"] = json!(chrono::Utc::now().timestamp());
    Ok(updated)
}

pub fn refresh() -> Result<()> {
    let previous = storage::read(&storage::root().join("auth/tokens.json"))?;
    let refresh_token = previous["refresh_token"]
        .as_str()
        .context("Login required: no refresh token")?;
    // Disable retries because refresh token rotation can make delivery ambiguous.
    let response = api::control_client()?
        .post("https://auth.digital.panasonic.com/oauth/token")
        .json(&json!({"grant_type":"refresh_token", "client_id":CLIENT, "refresh_token":refresh_token}))
        .send()?;
    ensure!(
        response.status().is_success(),
        "Token refresh failed (HTTP {}); credentials were not replaced",
        response.status()
    );
    let tokens = merge_refreshed_tokens(&previous, response.json()?)?;
    storage::save("auth/tokens.json", &tokens)?;
    let expires = tokens["obtained_at"].as_i64().unwrap() + tokens["expires_in"].as_i64().unwrap();
    println!(
        "Token refreshed; expires at {}",
        chrono::DateTime::from_timestamp(expires, 0)
            .context("Invalid expiry")?
            .with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).unwrap())
    );
    Ok(())
}

fn exchange(location: &str, state: &str, verifier: &str) -> Result<()> {
    let location = Url::parse(location)?;
    let redirect = Url::parse(REDIRECT)?;
    ensure!(
        location.scheme() == redirect.scheme()
            && location.host_str() == redirect.host_str()
            && location.path() == redirect.path(),
        "Unexpected OAuth redirect"
    );
    let query: std::collections::HashMap<_, _> = location.query_pairs().into_owned().collect();
    ensure!(
        query.get("state").map(String::as_str) == Some(state),
        "Invalid OAuth state"
    );
    let code = query
        .get("code")
        .context("Authorization code was not returned")?;
    let response = api::client()?
        .post("https://auth.digital.panasonic.com/oauth/token")
        .json(
            &json!({"grant_type":"authorization_code","client_id":CLIENT,
                      "code":code,"redirect_uri":REDIRECT,"code_verifier":verifier}),
        )
        .send()?;
    ensure!(
        response.status().is_success(),
        "Token exchange failed (HTTP {})",
        response.status()
    );
    let tokens: Value = response.json()?;
    ensure!(
        tokens["access_token"].is_string(),
        "Access token was not returned"
    );
    storage::save("auth/tokens.json", &tokens)?;
    storage::save("auth/result.json", &json!({"authenticated":true}))?;
    Ok(())
}

pub fn run(port: u16) -> Result<()> {
    let verifier = random_token();
    let state = random_token();
    let mut authorize = Url::parse("https://auth.digital.panasonic.com/authorize")?;
    authorize
        .query_pairs_mut()
        .append_pair("client_id", CLIENT)
        .append_pair("redirect_uri", REDIRECT)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid offline_access smartlaundry.control")
        .append_pair(
            "audience",
            &format!("https://club.panasonic.jp/{CLIENT}/api/v1/"),
        )
        .append_pair("state", &state)
        .append_pair("nonce", &random_token())
        .append_pair(
            "code_challenge",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        )
        .append_pair("code_challenge_method", "S256");
    let server = Server::http(format!("127.0.0.1:{port}"))
        .map_err(|e| anyhow::anyhow!("Start login bridge: {e}"))?;
    println!(
        "Login bridge listening on 127.0.0.1:{port}. Complete the browser login; Ctrl-C stops it."
    );
    let mut completed = false;
    for mut request in server.incoming_requests() {
        let (status, body) = match (request.method().as_str(), request.url()) {
            ("GET", "/start") => (200, json!({"url":authorize.as_str(),"redirect":REDIRECT})),
            ("POST", "/callback") if completed => (200, json!({"authenticated":true})),
            ("POST", "/callback") => {
                let mut input = String::new();
                let parse = request
                    .as_reader()
                    .take(16_385)
                    .read_to_string(&mut input)
                    .map_err(anyhow::Error::from)
                    .and_then(|_| {
                        ensure!(input.len() <= 16_384, "Callback too large");
                        let data: Value = serde_json::from_str(&input)?;
                        exchange(
                            data["location"]
                                .as_str()
                                .context("Missing callback location")?,
                            &state,
                            &verifier,
                        )
                    });
                match parse {
                    Ok(()) => {
                        completed = true;
                        println!("Authentication completed. Credentials saved locally.");
                        (200, json!({"authenticated":true}))
                    }
                    Err(_) => {
                        storage::save(
                            "auth/result.json",
                            &json!({"authenticated":false,"error":"OAuth callback failed"}),
                        )?;
                        (
                            400,
                            json!({"authenticated":false,"error":"OAuth callback failed"}),
                        )
                    }
                }
            }
            _ => (404, json!({"error":"Not found"})),
        };
        let response = Response::from_string(serde_json::to_string(&body)?)
            .with_status_code(status)
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        request.respond(response)?;
    }
    Ok(())
}

use std::io::Read;

#[cfg(test)]
mod refresh_tests {
    use super::*;

    #[test]
    fn preserves_or_rotates_refresh_token_and_rejects_invalid_response() {
        let old = json!({"refresh_token":"old"});
        let retained =
            merge_refreshed_tokens(&old, json!({"access_token":"new", "expires_in":86400}))
                .unwrap();
        assert_eq!(retained["refresh_token"], "old");
        let rotated = merge_refreshed_tokens(
            &old,
            json!({"access_token":"new", "refresh_token":"rotated", "expires_in":86400}),
        )
        .unwrap();
        assert_eq!(rotated["refresh_token"], "rotated");
        assert!(merge_refreshed_tokens(&old, json!({"error":"invalid_grant"})).is_err());
        assert!(
            merge_refreshed_tokens(&old, json!({"access_token":"new", "expires_in":0})).is_err()
        );
    }
}
