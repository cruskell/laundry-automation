use crate::{decoder, storage};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

pub fn run(map_path: &Path, models_path: &Path) -> Result<()> {
    let map_bytes = fs::read(map_path)?;
    let map: Value = serde_json::from_slice(&map_bytes)?;
    let sources = map["sources"]
        .as_array()
        .context("Missing source-map sources")?;
    let contents = map["sourcesContent"]
        .as_array()
        .context("Missing embedded sources")?;
    ensure!(
        sources.len() == contents.len(),
        "Mismatched source-map arrays"
    );
    let snapshot = storage::read(&storage::root().join("status.json"))?;
    let decoded = decoder::decode(&snapshot)?;
    let mut evidence = BTreeMap::new();
    for property in decoded["properties"]
        .as_array()
        .context("Missing properties")?
    {
        let id = property["id"].as_str().context("Missing ID")?;
        let mut references = Vec::new();
        for (source, content) in sources.iter().zip(contents) {
            let name = source.as_str().unwrap_or("");
            if name.contains("node_modules") || !(name.ends_with(".js") || name.ends_with(".json"))
            {
                continue;
            }
            for (line, text) in content.as_str().unwrap_or("").lines().enumerate() {
                if text.contains(&format!("\"{id}\"")) || text.contains(&format!("'{id}'")) {
                    references.push(json!({"source":name,"line":line+1}));
                }
            }
        }
        evidence.insert(id.to_owned(),json!({"name":property["name"],"decoding":property["decoding"],"source_references":references,"model_descriptions":[]}));
    }
    let mut model_count = 0;
    for entry in fs::read_dir(models_path)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let model = storage::read(&path)?;
        if model["http_status"] != 200 {
            continue;
        }
        model_count += 1;
        for device in model["data"]["devices"]
            .as_array()
            .context("Missing model devices")?
        {
            for block in device["supported_cmds"]
                .as_array()
                .context("Missing model commands")?
            {
                if block["protocol"] != "echonetlite" {
                    continue;
                }
                for cmd in block["cmds"].as_array().context("Missing command list")? {
                    if let Some(row) = cmd["id"].as_str().and_then(|id| evidence.get_mut(id)) {
                        row["model_descriptions"].as_array_mut().unwrap().push(json!({"com_id":model["com_id"],"name":cmd["name"],"params":cmd["params"]}));
                    }
                }
            }
        }
    }
    let source_hash = Sha256::digest(&map_bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let report = json!({"map_url":"https://cot.wad.apws.panasonic.com/FY26_2nd/static/js/main.d8aa4ffb.js.map",
        "map_sha256":source_hash,"embedded_source_count":sources.len(),"model_count":model_count,
        "note":"Absence of a literal reference does not prove the property is unused by other clients or backend services.","properties":evidence});
    storage::save("source-audit.json", &report)?;
    println!(
        "Audited {} properties against {} embedded sources and {model_count} published model descriptions.",
        evidence.len(),
        sources.len()
    );
    Ok(())
}
