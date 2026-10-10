use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub fn definitions() -> Result<Value> {
    Ok(serde_json::from_str(include_str!(
        "../spec/property-map.json"
    ))?)
}

pub fn hours_minutes(raw: &str) -> Option<u32> {
    if raw.len() != 4 || !raw.is_ascii() || raw.eq_ignore_ascii_case("FFFF") {
        return None;
    }
    let hours = u32::from_str_radix(&raw[..2], 16).ok()?;
    let minutes = u32::from_str_radix(&raw[2..], 16).ok()?;
    (hours < 255 && minutes < 60).then_some(hours * 60 + minutes)
}

fn agent(code: u64) -> Option<&'static str> {
    match code {
        0 => Some("detergent"),
        1 => Some("softener"),
        2 => Some("delicates_detergent"),
        3 => Some("oxygen_bleach"),
        4 => Some("stain_remover"),
        _ => None,
    }
}

pub fn tank_configuration(raw: &str) -> Option<Value> {
    if raw.len() != 12 || !raw.is_ascii() {
        return None;
    }
    let bits = u64::from_str_radix(raw, 16).ok()?;
    let mut result = json!({});
    let mut offset = 0;
    // Official Bb/Hb helpers consume the low bits in this order.
    for (name, width) in [
        ("tank3_agent_code", 4),
        ("tank3_input_setting", 1),
        ("tank2_agent_code", 4),
        ("tank2_input_setting", 1),
        ("tank1_agent_code", 4),
        ("tank1_input_setting", 1),
        ("reservation_object_setting", 1),
        ("margin_time_setting", 4),
        ("dry_default_setting", 2),
    ] {
        result[name] = json!((bits >> offset) & ((1u64 << width) - 1));
        offset += width;
    }
    for tank in 1..=3 {
        let code = result[format!("tank{tank}_agent_code")].as_u64()?;
        result[format!("tank{tank}_agent")] = json!(agent(code));
    }
    result["uninterpreted_high_bits"] = json!(bits >> offset);
    result["dry_default"] = json!(match result["dry_default_setting"].as_u64()? {
        0 => Some("standard"),
        1 => Some("energy_saving"),
        _ => None,
    });
    Some(result)
}

fn decode_property(id: &str, raw_value: &Value, definition: &Value) -> Value {
    let mut entry = json!({
        "id": id, "key": definition["key"], "name": definition["name"],
        "raw": raw_value, "decoded": null, "decoding": "unresolved"
    });
    let Some(raw) = raw_value.as_str() else {
        return entry;
    };
    let raw = raw.to_ascii_uppercase();
    if let Some(label) = definition["enum"][&raw].as_str() {
        entry["decoded"] = json!(label);
        entry["decoding"] = json!("official_enum");
    } else if let Some((value, partial)) = crate::packed::decode(id, &raw) {
        entry["decoded"] = value;
        entry["decoding"] = json!(if partial {
            "documented_partial_rule"
        } else {
            "documented_packed_rule"
        });
        if id == "012C" {
            entry["decoding"] = json!("inferred_partial_rule");
        }
    } else if id == "00E5" {
        if let Some(minutes) = hours_minutes(&raw).filter(|minutes| *minutes <= 1439) {
            entry["decoded"] = json!(minutes);
            entry["unit"] = json!("minutes");
            entry["decoding"] = json!("documented_absolute_setting_rule");
        }
    } else if ["00E6", "00E8"].contains(&id) && raw.len() == 2 {
        if let Ok(value) = u8::from_str_radix(&raw, 16) {
            let maximum = if id == "00E6" { 8 } else { 59 };
            if value <= maximum {
                entry["decoded"] = json!(value);
                entry["unit"] = json!(if id == "00E6" {
                    "rinse_count"
                } else {
                    "minutes"
                });
                entry["decoding"] = json!("documented_absolute_setting_rule");
            }
        }
    } else if ["00DB", "00DC", "00ED", "0124", "013E", "00E9", "0127"].contains(&id) {
        if raw == "FFFF" {
            entry["decoding"] = json!("unavailable_sentinel");
        } else if let Some(minutes) = hours_minutes(&raw) {
            entry["decoded"] = json!(minutes);
            entry["unit"] = json!("minutes");
            entry["decoding"] = json!("official_hhmm_rule");
        }
    } else if ["0134", "0135", "013C"].contains(&id) && raw.len() == 2 {
        if let Ok(number) = u8::from_str_radix(&raw, 16) {
            entry["decoded"] = json!(number);
            entry["decoding"] = json!("official_hex_integer_rule");
        }
    } else if ["FF01", "FF02"].contains(&id) && raw.len() % 2 == 0 && raw.is_ascii() {
        let bytes: Option<Vec<u8>> = (0..raw.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).ok())
            .collect();
        if let Some(bytes) = bytes {
            if !bytes.is_empty() && bytes.iter().all(|b| *b == 0 || (32..127).contains(b)) {
                let text: String = bytes
                    .into_iter()
                    .filter(|b| *b != 0)
                    .map(char::from)
                    .collect();
                entry["decoded"] = json!(text);
                entry["decoding"] = json!("official_ascii_rule");
            }
        }
    } else if id == "0151" {
        if let Some(configuration) = tank_configuration(&raw) {
            entry["decoded"] = configuration;
            entry["decoding"] = json!("official_partial_rule");
        }
    } else if id == "0160" && raw.len() == 4 {
        if let Ok(number) = u16::from_str_radix(&raw, 16) {
            entry["decoded"] = json!(number);
            entry["decoding"] = json!("inferred_uint16_rule");
            entry["note"] = json!("The property name is official; integer encoding is inferred.");
        }
    }
    entry
}

pub fn decode(snapshot: &Value) -> Result<Value> {
    ensure!(
        snapshot["http_status"] == 200,
        "Cannot decode an unsuccessful status response"
    );
    let definitions = definitions()?;
    let records = snapshot["data"]["status"]
        .as_array()
        .context("Missing status array")?;
    let mut properties = Vec::new();
    for record in records {
        let id = record["id"]
            .as_str()
            .context("Missing property ID")?
            .to_ascii_uppercase();
        let mut property = decode_property(
            &id,
            &record["params"][0]["value"],
            &definitions["properties"][&id],
        );
        property["raw_params"] = record["params"].clone();
        properties.push(property);
    }
    let supplies: Value = serde_json::from_str(include_str!("../spec/supply-codes.json"))?;
    let third_agent = properties
        .iter()
        .find(|p| p["id"] == "0151")
        .and_then(|p| p["decoded"]["tank3_agent"].as_str())
        .unwrap_or("")
        .to_owned();
    for property in &mut properties {
        let category = match property["id"].as_str().unwrap_or("") {
            "0130" => "detergent",
            "0131" => "softener",
            "013A" => match third_agent.as_str() {
                "oxygen_bleach" => "bleach",
                "stain_remover" => "stainRemovers",
                "delicates_detergent" => "neutralDetergent",
                _ => continue,
            },
            _ => continue,
        };
        if let Some(raw) = property["raw"].as_str() {
            let matched = &supplies[category][raw.to_ascii_uppercase()];
            if !matched.is_null() {
                property["decoded"] = matched.clone();
                property["decoding"] = json!("official_supply_catalog");
            }
        }
    }
    let by_id: BTreeMap<&str, &Value> = properties
        .iter()
        .filter_map(|v| Some((v["id"].as_str()?, v)))
        .collect();
    let value = |id: &str| by_id.get(id).map_or(Value::Null, |v| v["decoded"].clone());
    let raw = |id: &str| by_id.get(id).map_or(Value::Null, |v| v["raw"].clone());
    let configuration = value("0151");
    let mut tanks = json!({});
    for (tank, amount, mode) in [
        (1, "0136", "0132"),
        (2, "0137", "0133"),
        (3, "013D", "013B"),
    ] {
        tanks[format!("tank{tank}")] = json!({
            "agent": configuration[format!("tank{tank}_agent")],
            "remaining": value(amount), "input_mode": value(mode)
        });
    }
    let summary = json!({
        "power": value("0080"), "phase": value("00E2"),
        "operation_detail": value("0121"), "door": value("00B0"),
        "reservation": value("0090"), "remote_control": value("0100"),
        "abnormality": value("0088"), "abnormal_code_raw": raw("0086"),
        "configured_course": value("00D0"),
        "remaining_minutes": {
            "wash": value("00DB"), "dry": value("00DC"),
            "wash_and_dry": value("00ED"), "current_process": value("0124"),
            "washing_process": value("013E")
        },
        "tanks": tanks,
        "wireless_module": {"software": value("FF01"), "hardware": value("FF02")},
        "machine_firmware": value("01F0"),
        "drytime_prediction": snapshot["data"]["drytime_prediction"]
    });
    let named =
        |entry: &&Value| entry["name"].as_str().is_some() || entry["key"].as_str().is_some();
    let unknown: Vec<_> = properties
        .iter()
        .filter(|e| !named(e))
        .map(|e| e["id"].clone())
        .collect();
    let unresolved: Vec<_> = properties
        .iter()
        .filter(|e| e["decoding"] == "unresolved")
        .map(|e| e["id"].clone())
        .collect();
    Ok(json!({
        "model": definitions["model"], "fetched_at": snapshot["fetched_at"],
        "summary": summary,
        "coverage": {
            "total": properties.len(), "named": properties.iter().filter(named).count(),
            "decoded": properties.len() - unresolved.len(),
            "inferred": properties.iter().filter(|e| e["decoding"].as_str().unwrap_or("").starts_with("inferred_")).count(),
            "partial": properties.iter().filter(|e| e["decoding"].as_str().unwrap_or("").contains("partial_rule")).count(),
            "unknown_ids": unknown, "unresolved_values": unresolved
        },
        "properties": properties
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_sentinel_is_not_a_large_duration() {
        assert_eq!(hours_minutes("FFFF"), None);
        assert_eq!(hours_minutes("013B"), Some(119));
        assert_eq!(hours_minutes("0000"), Some(0));
        assert_eq!(hours_minutes("013C"), None);
        assert_eq!(hours_minutes("010"), None);
        assert_eq!(hours_minutes("😀"), None);
    }

    #[test]
    fn absolute_cycle_settings_validate_units_and_reserved_ranges() {
        let defs = definitions().unwrap();
        for (id, raw, value, unit) in [
            ("00E5", "0009", 9, "minutes"),
            ("00E5", "173B", 1439, "minutes"),
            ("00E6", "02", 2, "rinse_count"),
            ("00E8", "0D", 13, "minutes"),
        ] {
            let result = decode_property(id, &json!(raw), &defs["properties"][id]);
            assert_eq!(result["decoded"], value);
            assert_eq!(result["unit"], unit);
        }
        for (id, raw) in [
            ("00E5", "1800"),
            ("00E5", "003C"),
            ("00E6", "09"),
            ("00E8", "3C"),
            ("00E8", "A0"),
        ] {
            let result = decode_property(id, &json!(raw), &defs["properties"][id]);
            assert_eq!(result["decoding"], "unresolved");
        }
        let auto = decode_property("00E5", &json!("00FF"), &defs["properties"]["00E5"]);
        assert_eq!(auto["decoding"], "official_enum");
    }

    #[test]
    fn third_tank_content_overrides_the_generic_property_name() {
        let configuration = tank_configuration("000000024223").unwrap();
        assert_eq!(configuration["tank3_agent"], "oxygen_bleach");
        assert_eq!(configuration["tank2_agent"], "softener");
        assert_eq!(configuration["tank1_agent"], "detergent");
        assert_eq!(configuration["tank3_input_setting"], 0);
        assert!(tank_configuration("0000").is_none());
        assert!(tank_configuration("GG0000024223").is_none());
    }

    #[test]
    fn unrecognized_values_are_preserved_without_inventing_a_label() {
        let defs = definitions().unwrap();
        let decoded = decode_property("00B2", &json!("44"), &defs["properties"]["00B2"]);
        assert_eq!(decoded["raw"], "44");
        assert_eq!(decoded["decoding"], "unresolved");
        assert_eq!(decoded["decoded"], Value::Null);
    }

    #[test]
    fn synthetic_cycle_retains_active_phase_and_unavailable_time() {
        let snapshot = json!({"http_status": 200, "data": {"status": [
            {"id": "0080", "params": [{"value": "30"}]},
            {"id": "00E2", "params": [{"value": "52"}]},
            {"id": "00ED", "params": [{"value": "FFFF"}]},
            {"id": "ABCD", "params": [{"value": "DEAD"}]}
        ]}});
        let result = decode(&snapshot).unwrap();
        assert_eq!(result["summary"]["power"], "ON");
        assert_eq!(result["summary"]["phase"], "運転中・乾燥・送風工程");
        assert_eq!(
            result["summary"]["remaining_minutes"]["wash_and_dry"],
            Value::Null
        );
        assert_eq!(result["coverage"]["unknown_ids"], json!(["ABCD"]));
        assert!(decode(&json!({"http_status":401})).is_err());
    }
}
