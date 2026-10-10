use anyhow::{Context, Result, ensure};
use chrono::Utc;
use regex::Regex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

use crate::storage;

pub fn run(info_path: &Path, bundle_path: &Path, output: &Path) -> Result<()> {
    let info = storage::read(info_path)?;
    let bundle = fs::read_to_string(bundle_path)?;
    let registry_start = Regex::new(r#"(\w+)=\{OperatingState:\{id:"0080""#)?;
    let captures = registry_start
        .captures(&bundle)
        .context("Official registry not found")?;
    let start = captures.get(0).unwrap().start();
    let variable = captures.get(1).unwrap().as_str();
    let end = bundle[start..]
        .find(&format!(
            "idOf:function(e){{return Object.entries({variable})"
        ))
        .context("Registry boundary not found")?
        + start;
    let pattern = Regex::new(r#"(\w+):\{id:"([0-9A-F]{4})",message:"([^"]*)"\}"#)?;
    let mut properties = json!({});
    for captures in pattern.captures_iter(&bundle[start..end]) {
        properties[&captures[2]] = json!({"key":&captures[1],"name":null,
                                       "enum":{},"name_source":"official_web_registry"});
    }
    ensure!(
        info["http_status"] == 200,
        "Metadata request was unsuccessful"
    );
    let devices = info["data"]["devices"]
        .as_array()
        .context("Missing devices metadata")?;
    ensure!(
        devices.len() == 1,
        "Expected exactly one model metadata response"
    );
    let model = &devices[0];
    for block in model["supported_cmds"]
        .as_array()
        .context("Missing command metadata")?
    {
        if block["protocol"] != "echonetlite" {
            continue;
        }
        for command in block["cmds"].as_array().context("Missing commands")? {
            let id = command["id"].as_str().context("Missing property ID")?;
            let mut entry = if properties[id].is_null() {
                json!({"key":null})
            } else {
                properties[id].clone()
            };
            entry["name"] = json!(
                command["name"]
                    .as_str()
                    .unwrap_or("")
                    .replace("\r\n", " ")
                    .trim()
            );
            entry["name_source"] = json!("official_device_info");
            let mut enumeration = json!({});
            for parameter in command["params"].as_array().context("Missing parameters")? {
                if let (Some(value), Some(name)) =
                    (parameter["value"].as_str(), parameter["name"].as_str())
                {
                    if !value.is_empty() && !name.is_empty() {
                        enumeration[value.to_ascii_uppercase()] = json!(name);
                    }
                }
            }
            entry["enum"] = enumeration;
            properties[id] = entry;
        }
    }
    let hash = Sha256::digest(bundle.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let result: Value = json!({
        "model":model["product_number"],"com_id":model["com_id"],"generated_at":Utc::now().to_rfc3339(),
        "sources":{
            "metadata":"https://app.wad.apws.panasonic.com/laundry/v5/device/info",
            "web_bundle":"https://cot.wad.apws.panasonic.com/FY26_2nd/static/js/main.d8aa4ffb.js",
            "web_bundle_sha256":hash
        },"properties":properties
    });
    fs::write(output, serde_json::to_string_pretty(&result)? + "\n")?;
    println!(
        "Extracted property definitions: {}",
        result["properties"].as_object().unwrap().len()
    );
    Ok(())
}
