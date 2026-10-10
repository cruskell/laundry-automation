use crate::{decoder, storage};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::Path;

fn bytes(raw: &str) -> Option<Vec<u8>> {
    if !raw.is_ascii() || raw.len() % 2 != 0 {
        return None;
    }
    (0..raw.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).ok())
        .collect()
}

pub fn inspect(snapshot: &Value, baseline: Option<&Value>) -> Result<Value> {
    let decoded = decoder::decode(snapshot)?;
    let previous = baseline.and_then(|b| b["data"]["status"].as_array());
    let mut rows = Vec::new();
    for property in decoded["properties"]
        .as_array()
        .context("Missing decoded properties")?
    {
        let method = property["decoding"].as_str().unwrap_or("");
        if method != "unresolved" && !method.contains("partial") && !method.starts_with("inferred")
        {
            continue;
        }
        let raw = property["raw"].as_str().context("Missing raw value")?;
        let current = bytes(raw).context("Invalid hexadecimal payload")?;
        let prev = previous
            .and_then(|rows| rows.iter().find(|r| r["id"] == property["id"]))
            .and_then(|r| r["params"][0]["value"].as_str())
            .and_then(bytes);
        let changed = prev.as_ref().map(|old| {
            (0..current.len().max(old.len()))
                .filter_map(|offset| {
                    let before = old.get(offset);
                    let after = current.get(offset);
                    (before != after)
                        .then(|| json!({"byte_offset":offset,"before":before,"after":after}))
                })
                .collect::<Vec<_>>()
        });
        let words = |width: usize| {
            current
                .chunks_exact(width)
                .enumerate()
                .map(|(i, chunk)| {
                    let be = chunk.iter().fold(0u32, |n, b| (n << 8) | u32::from(*b));
                    let le = chunk
                        .iter()
                        .rev()
                        .fold(0u32, |n, b| (n << 8) | u32::from(*b));
                    json!({"byte_offset":i*width,"big_endian":be,"little_endian":le})
                })
                .collect::<Vec<_>>()
        };
        rows.push(
            json!({"id":property["id"],"name":property["name"],"decoding":method,
            "raw":raw,"length_bytes":current.len(),"bytes":current,
            "all_ff":!current.is_empty()&&current.iter().all(|b|*b==255),
            "all_zero":!current.is_empty()&&current.iter().all(|b|*b==0),
            "aligned_uint16_candidates":words(2),"aligned_uint32_candidates":words(4),
            "changed_bytes":changed}),
        );
    }
    Ok(
        json!({"fetched_at":snapshot["fetched_at"],"baseline_fetched_at":baseline.map(|b|&b["fetched_at"]),
        "note":"Integer views are competing representations, not semantic decodings. Missing baseline yields null changes.","properties":rows}),
    )
}

pub fn run(snapshot: Option<&Path>, baseline: Option<&Path>) -> Result<()> {
    let current = storage::read(
        &snapshot.map_or_else(|| storage::root().join("status.json"), Path::to_path_buf),
    )?;
    let previous = baseline.map(storage::read).transpose()?;
    let report = inspect(&current, previous.as_ref())?;
    storage::save("forensic-report.json", &report)?;
    println!(
        "Forensic byte/word views saved to .local/forensic-report.json; semantic coverage unchanged."
    );
    Ok(())
}

pub fn timeline(directory: &Path) -> Result<()> {
    let mut captures = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let snapshot = storage::read(&path)?;
            let time = chrono::DateTime::parse_from_rfc3339(
                snapshot["fetched_at"]
                    .as_str()
                    .context("Missing capture time")?,
            )?;
            captures.push((time, snapshot));
        }
    }
    captures.sort_by_key(|(time, _)| *time);
    let snapshots: Vec<_> = captures.into_iter().map(|(_, snapshot)| snapshot).collect();
    let report = timeline_report(&snapshots)?;
    storage::save("timeline.json", &report)?;
    println!(
        "{} captures correlated in .local/timeline.json; correlations do not establish field meanings.",
        snapshots.len()
    );
    Ok(())
}

fn timeline_report(snapshots: &[Value]) -> Result<Value> {
    let mut events = Vec::new();
    let mut previous: Option<&Value> = None;
    for snapshot in snapshots {
        let decoded = decoder::decode(snapshot)?;
        let report = inspect(snapshot, previous)?;
        let properties: Vec<_> = report["properties"]
            .as_array()
            .context("Missing forensic properties")?
            .iter()
            .filter(|row| {
                previous.is_none()
                    || row["changed_bytes"].is_null()
                    || row["changed_bytes"]
                        .as_array()
                        .is_some_and(|v| !v.is_empty())
            })
            .cloned()
            .collect();
        let disappeared: Vec<_> = previous
            .and_then(|old| old["data"]["status"].as_array())
            .into_iter()
            .flatten()
            .filter(|row| {
                !snapshot["data"]["status"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|r| r["id"] == row["id"]))
            })
            .map(|row| row["id"].clone())
            .collect();
        events.push(json!({
            "fetched_at":snapshot["fetched_at"],
            "phase":decoded["summary"]["phase"],
            "operation_detail":decoded["summary"]["operation_detail"],
            "remaining_minutes":decoded["summary"]["remaining_minutes"],
            "server_prediction":snapshot["data"]["drytime_prediction"],
            "changed_unresolved_properties":properties,
            "disappeared_property_ids":disappeared
        }));
        previous = Some(snapshot);
    }
    Ok(
        json!({"note":"Temporal co-occurrence is evidence for investigation, not a semantic decoding. The first capture includes the baseline raw payloads.","capture_count":snapshots.len(),"events":events}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeline_preserves_new_missing_and_unchanged_prediction_observations() {
        let capture = |rows: Value, prediction: u32| json!({"http_status":200,"data":{"status":rows,"drytime_prediction":{"drytime":prediction}}});
        let row = |id: &str| json!({"id":id,"params":[{"value":"01"}]});
        let report = timeline_report(&[
            capture(json!([row("ABCD")]), 0),
            capture(json!([row("ABCD"), row("ABCE")]), 72),
            capture(json!([row("ABCE")]), 71),
        ])
        .unwrap();
        assert_eq!(
            report["events"][1]["changed_unresolved_properties"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            report["events"][1]["changed_unresolved_properties"][0]["id"],
            "ABCE"
        );
        assert_eq!(report["events"][1]["server_prediction"]["drytime"], 72);
        assert_eq!(
            report["events"][2]["disappeared_property_ids"],
            json!(["ABCD"])
        );
        assert_eq!(
            report["events"][2]["changed_unresolved_properties"],
            json!([])
        );
    }
    #[test]
    fn difference_tracks_byte_offsets_and_length_changes() {
        let snapshot = |raw: &str| json!({"http_status":200,"data":{"status":[{"id":"ABCD","params":[{"value":raw}]}]}});
        let report = inspect(&snapshot("0102FF"), Some(&snapshot("0103"))).unwrap();
        assert_eq!(
            report["properties"][0]["changed_bytes"],
            json!([
                {"byte_offset":1,"before":3,"after":2},{"byte_offset":2,"before":null,"after":255}
            ])
        );
        assert_eq!(
            report["properties"][0]["aligned_uint16_candidates"][0]["big_endian"],
            258
        );
        assert_eq!(
            report["properties"][0]["aligned_uint16_candidates"][0]["little_endian"],
            513
        );
        assert!(bytes("😀").is_none());
    }
}
