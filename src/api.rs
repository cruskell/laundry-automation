use anyhow::{Context, Result, ensure};
use chrono::{Local, Utc};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::time::Duration;
use url::Url;

use crate::{decoder, storage};

const BASE: &str = "https://app.wad.apws.panasonic.com/laundry/v5/";
pub(crate) const CONTROL_URL: &str = "https://app.wad.apws.panasonic.com/laundry/v5/device/control";

pub(crate) fn control_client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(concat!("laundry-automation/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .timeout(Duration::from_secs(25))
        .build()?)
}

pub(crate) fn control_request(
    client: &Client,
    endpoint: &str,
    token: &str,
    appliance: &str,
    body: &Value,
) -> Result<(u16, Value)> {
    let timestamp = Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).unwrap())
        .format("%Y%m%d%H%M%S%z")
        .to_string();
    let response = client
        .post(endpoint)
        .bearer_auth(token)
        .header("Cookie", format!("atkn={token}"))
        .header("X-ApplianceId", appliance)
        .header("Content-Language", "ja_JP")
        .header("X-Timestamp", timestamp)
        .json(body)
        .send()?;
    let code = response.status().as_u16();
    let data = response
        .json::<Value>()
        .unwrap_or_else(|_| json!({"error":"Non-JSON control response"}));
    Ok((code, data))
}

pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(concat!("laundry-automation/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(25))
        .build()?)
}

pub(crate) fn request(
    client: &Client,
    path: &str,
    token: &str,
    appliance: Option<&str>,
    fresh: bool,
) -> Result<(u16, Value)> {
    let mut request = client
        .get(format!("{BASE}{path}"))
        .bearer_auth(token)
        .header("Cookie", format!("atkn={token}"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Content-Language", "ja")
        .header(
            "X-Timestamp",
            Local::now().format("%Y%m%d%H%M%S%z").to_string(),
        );
    if let Some(appliance) = appliance {
        request = request.header("X-ApplianceId", appliance);
    }
    if fresh {
        request = request.header("X-Cached", "false");
    }
    let response = request.send()?;
    let code = response.status().as_u16();
    let body = response
        .json::<Value>()
        .unwrap_or_else(|_| json!({"error":"Non-JSON response"}));
    Ok((code, body))
}

pub fn print_decoded(result: &Value) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"summary":result["summary"], "coverage":result["coverage"]})
        )?
    );
    Ok(())
}

pub fn run(action: &str, appliance: Option<&str>) -> Result<()> {
    run_selected(action, appliance, None)
}

pub fn probe(appliance: Option<&str>, property: Option<&str>) -> Result<()> {
    run_selected("probe-unmapped", appliance, property)
}

fn run_selected(action: &str, appliance: Option<&str>, property: Option<&str>) -> Result<()> {
    let tokens = storage::read(&storage::root().join("auth/tokens.json"))
        .context("Login required: complete the browser OAuth flow first")?;
    let token = tokens["access_token"]
        .as_str()
        .context("Missing access token")?;
    let client = client()?;
    let (code, devices) = request(&client, "user/devices", token, appliance, false)?;
    storage::save("devices.json", &json!({"http_status":code,"data":devices}))?;
    println!("Device list HTTP status: {code}");
    ensure!(
        code == 200,
        "Device list failed (HTTP {code}); error code: {}",
        devices["code"].as_str().unwrap_or("unknown")
    );
    if action == "devices" {
        return Ok(());
    }
    let records = devices["devices"]
        .as_array()
        .context("Missing devices array")?;
    let matches: Vec<_> = records
        .iter()
        .filter(|d| match appliance {
            Some(id) => d["appliance_id"].as_str() == Some(id),
            None => d["product_code"]
                .as_str()
                .unwrap_or("")
                .starts_with("NA-LX127ER"),
        })
        .collect();
    ensure!(
        matches.len() == 1,
        "Specify --appliance-id to select exactly one registered device"
    );
    let selected = matches[0];
    if action == "status" {
        let definitions = decoder::definitions()?;
        ensure!(
            selected["com_id"] == definitions["com_id"],
            "The decoder dictionary does not support this communication model"
        );
    }
    let id = selected["appliance_id"]
        .as_str()
        .context("Missing appliance ID")?;
    if action == "live-read" || action == "probe-unmapped" {
        let info = storage::read(&storage::root().join("device-info.json"))?;
        let blocks = info["data"]["devices"][0]["supported_cmds"]
            .as_array()
            .context("Missing supported commands")?;
        let block = blocks
            .iter()
            .find(|b| b["protocol"] == "echonetlite")
            .context("Missing ECHONET metadata")?;
        let supported = block["get"]
            .as_array()
            .context("Missing readable properties")?;
        let usual_ids = [
            "01F0", "0140", "0217", "0150", "0151", "013F", "01E0", "012B",
        ];
        let saved = storage::read(&storage::root().join("status.json"))?;
        let decoded = decoder::decode(&saved)?;
        let probe_ids: Vec<_> = decoded["coverage"]["unresolved_values"]
            .as_array()
            .context("Missing unresolved IDs")?
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let selected_ids = property.map(|id| vec![id]);
        ensure!(
            property.is_none_or(|id| probe_ids.contains(&id)),
            "Probe ID must be present and unresolved in the saved snapshot"
        );
        let ids: &[&str] = if action == "probe-unmapped" {
            selected_ids.as_deref().unwrap_or(&probe_ids)
        } else {
            &usual_ids
        };
        ensure!(
            ids.iter()
                .all(|id| id.len() == 4 && id.bytes().all(|b| b.is_ascii_hexdigit())),
            "Invalid captured ID"
        );
        if action == "live-read" {
            ensure!(
                ids.iter().all(|id| supported.contains(&json!(id))),
                "Unsupported read property"
            );
        }
        let body = read_body(id, ids);
        let response = client
            .post(format!("{BASE}device/control"))
            .bearer_auth(token)
            .header("Cookie", format!("atkn={token}"))
            .header("X-ApplianceId", id)
            .header("Content-Language", "ja_JP")
            .header(
                "X-Timestamp",
                Local::now().format("%Y%m%d%H%M%S%z").to_string(),
            )
            .json(&body)
            .send()?;
        let code = response.status().as_u16();
        let data = response.json::<Value>()?;
        let filename = if action == "probe-unmapped" {
            property.map_or_else(
                || "probe-unmapped.json".to_owned(),
                |id| format!("probes/{id}.json"),
            )
        } else {
            "live-read.json".to_owned()
        };
        storage::save(
            &filename,
            &json!({"http_status":code,"fetched_at":Utc::now().to_rfc3339(),"data":data}),
        )?;
        println!(
            "Direct read HTTP {code}; result: {}, command status: {}",
            data["results"][0]["cmd_result"], data["results"][0]["status_code"]
        );
        ensure!(
            code == 200,
            "Direct read rejected (HTTP {code}); error code: {}",
            data["code"].as_str().unwrap_or("unknown")
        );
        return Ok(());
    }
    if action == "explore-models" {
        let versions = storage::read(&storage::root().join("catalogs/versions.json"))?;
        let model_url = versions["data"]["models"]["url"]
            .as_str()
            .context("Missing model catalog URL")?;
        let url = Url::parse(model_url)?;
        ensure!(
            url.scheme() == "https" && url.host_str() == Some("cot.wad.apws.panasonic.com"),
            "Unexpected model catalog origin"
        );
        let csv = client.get(url).send()?.error_for_status()?.text()?;
        let normalized = csv.replace("\\r\\n", "\n");
        let mut seen = std::collections::BTreeSet::new();
        for line in normalized.lines().skip(1) {
            let fields: Vec<_> = line.trim().split(',').collect();
            if fields.len() < 2 || !seen.insert(fields[1].to_owned()) {
                continue;
            }
            let product = fields[0];
            let com = fields[1];
            let mut url = Url::parse(&format!("{BASE}device/info"))?;
            url.query_pairs_mut()
                .append_pair("product_number", product)
                .append_pair("com_id", com);
            let path = format!("device/info?{}", url.query().unwrap_or(""));
            let (code, body) = request(&client, &path, token, Some(id), false)?;
            let filename = com
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            storage::save(
                &format!("models/{filename}.json"),
                &json!({"http_status":code,"product_number":product,"com_id":com,"data":body}),
            )?;
            println!("{com}: metadata HTTP {code}");
        }
        return Ok(());
    }
    if action == "catalogs" {
        let com_id = selected["com_id"]
            .as_str()
            .context("Missing communication model")?;
        for (name, path) in [
            ("notices", "notices?limit=100&from_index=0".to_string()),
            (
                "prediction-settings",
                "user/drytime_prediction/settings".to_string(),
            ),
            ("supplies", "service/supplies".to_string()),
            ("courses", format!("laundry/courses/search/{com_id}")),
            ("versions", "versions/android/25".to_string()),
            (
                "custom-view",
                format!(
                    "custom/view?wash_dry_course_code=01&com_id={com_id}&view_id=11300&command_param_value=4F"
                ),
            ),
        ] {
            let (code, body) = request(&client, &path, token, Some(id), false)?;
            storage::save(
                &format!("catalogs/{name}.json"),
                &json!({"http_status":code,"data":body}),
            )?;
            println!("{name} metadata HTTP status: {code}");
        }
        // This POST queries command metadata; it does not control the appliance.
        let saved = storage::read(&storage::root().join("status.json"))?;
        let cmds: Vec<_> = ["012E", "012C", "0140", "0153", "0217"]
            .iter()
            .map(|id| {
                let params = saved["data"]["status"]
                    .as_array()
                    .and_then(|rows| rows.iter().find(|row| row["id"] == *id))
                    .map_or(json!([]), |row| row["params"].clone());
                json!({"id":id,"params":params})
            })
            .collect();
        let response = client
            .post(format!("{BASE}laundry/command"))
            .bearer_auth(token)
            .header("Cookie", format!("atkn={token}"))
            .header("X-ApplianceId", id)
            .header("Content-Language", "ja_JP")
            .header(
                "X-Timestamp",
                Local::now().format("%Y%m%d%H%M%S%z").to_string(),
            )
            .json(&json!({"product_number":selected["product_code"],"com_id":com_id,"cmds":cmds}))
            .send()?;
        let code = response.status().as_u16();
        storage::save(
            "catalogs/command-details.json",
            &json!({"http_status":code,"data":response.json::<Value>()?}),
        )?;
        println!("Command details metadata HTTP status: {code}");
        return Ok(());
    }
    let (path, fresh, filename) = if action == "info" {
        let mut url = Url::parse(&format!("{BASE}device/info"))?;
        url.query_pairs_mut()
            .append_pair(
                "product_number",
                selected["product_code"].as_str().context("Missing model")?,
            )
            .append_pair(
                "com_id",
                selected["com_id"]
                    .as_str()
                    .context("Missing communication model")?,
            );
        (
            format!("device/info?{}", url.query().unwrap_or("")),
            false,
            "device-info.json",
        )
    } else {
        ("device/status".to_string(), true, "status.json")
    };
    let (code, body) = request(&client, &path, token, Some(id), fresh)?;
    let snapshot = json!({"http_status":code,"fetched_at":Utc::now().to_rfc3339(),"data":body});
    if action == "status" && code == 200 {
        storage::save(
            &format!("snapshots/{}.json", Utc::now().format("%Y%m%dT%H%M%S%.9fZ")),
            &snapshot,
        )?;
    }
    storage::save(filename, &snapshot)?;
    println!("{action} HTTP status: {code}");
    ensure!(
        code == 200,
        "Request failed (HTTP {code}); error code: {}",
        body["code"].as_str().unwrap_or("unknown")
    );
    if action == "status" {
        let result = decoder::decode(&snapshot)?;
        storage::save("decoded-status.json", &result)?;
        print_decoded(&result)?;
    }
    Ok(())
}

pub(crate) fn read_body(appliance: &str, ids: &[&str]) -> Value {
    json!({"requests":[{"appliance_id":appliance,"cmd_type":"03","cmds":ids.iter().map(|id|json!({"id":id,"params":[]})).collect::<Vec<_>>(),"protocol":"echonetlite","seq_no":1}]})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_read_has_only_read_commands_and_empty_parameters() {
        let body = read_body("test", &["0140", "01F0"]);
        let request = &body["requests"][0];
        assert_eq!(request["cmd_type"], "03");
        assert_eq!(
            request["cmds"],
            json!([{"id":"0140","params":[]},{"id":"01F0","params":[]}])
        );
    }
}
