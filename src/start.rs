use crate::storage;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

pub fn plan(appliance: &str, snapshot: &Value) -> Result<Value> {
    plan_mode(appliance, snapshot, false)
}

fn plan_mode(appliance: &str, snapshot: &Value, wash_only: bool) -> Result<Value> {
    ensure!(!appliance.is_empty(), "Missing appliance ID");
    ensure!(snapshot["http_status"] == 200, "Unsuccessful snapshot");
    let records = snapshot["data"]["status"]
        .as_array()
        .context("Missing status")?;
    let get = |id: &str| -> Result<String> {
        let value = records
            .iter()
            .find(|r| r["id"] == id)
            .and_then(|r| r["params"][0]["value"].as_str())
            .context("Missing command value")?;
        ensure!(
            !value.is_empty() && value.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid command value"
        );
        Ok(value.to_ascii_uppercase())
    };
    // Official Edt.WashingEntrust=61; WashingAndDryingEntrust=21.
    let mut cmds = vec![
        json!({"id":"00B2","params":["41"]}),
        json!({"id":"00D0","params":[if wash_only { "61" } else { "21" }]}),
    ];
    for id in ["0132", "0133", "013B", "0151"] {
        cmds.push(json!({"id":id,"params":[get(id)?]}));
    }
    Ok(json!({
        "analysis_only":true,"sent":false,"snapshot_fetched_at":snapshot["fetched_at"],
        "method":"POST","url":"https://app.wad.apws.panasonic.com/laundry/v5/device/control",
        "course":if wash_only { "61: normal wash only" } else { "21: normal wash and dry" },"body":{"requests":[{
            "appliance_id":appliance,"cmd_type":"02","cmds":cmds,"protocol":"echonetlite","seq_no":1
        }]},
        "preconditions_require_live_verification":{"remote_control_raw":get("0100")?,"door_raw":get("00B0")?,"abnormality_raw":get("0088")?,"power_raw":get("0080")?}
    }))
}

pub fn run(appliance: Option<&str>) -> Result<()> {
    let devices = storage::read(&storage::root().join("devices.json"))?;
    let records = devices["data"]["devices"]
        .as_array()
        .context("Missing devices")?;
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
        "Select exactly one NA-LX127ER appliance"
    );
    let id = matches[0]["appliance_id"]
        .as_str()
        .context("Missing appliance ID")?;
    let plan = plan(id, &storage::read(&storage::root().join("status.json"))?)?;
    storage::save("start-plan.json", &plan)?;
    println!("Offline wash-and-dry start plan saved to .local/start-plan.json; no request sent.");
    Ok(())
}

const READ_IDS: &[&str] = &[
    "0080", "00B0", "0088", "0100", "00E2", "0121", "0090", "00B2", "0132", "0133", "013B", "0151",
];

fn accepted<'a>(code: u16, data: &'a Value, appliance: &str, kind: &str) -> Result<&'a Value> {
    ensure!(
        code == 200,
        "Control rejected (HTTP {code}); error code: {}",
        data["code"].as_str().unwrap_or("unknown")
    );
    let results = data["results"]
        .as_array()
        .context("Missing control results")?;
    ensure!(results.len() == 1, "Expected exactly one control result");
    let result = &results[0];
    ensure!(
        result["appliance_id"] == appliance && result["cmd_type"] == kind && result["seq_no"] == 1,
        "Control result does not match the requested appliance/type/sequence"
    );
    ensure!(
        result["cmd_result"] == true,
        "Appliance rejected the command"
    );
    for field in ["status_code", "pcpf_status_code"] {
        ensure!(
            result[field] == "200" || result[field] == 200,
            "Control result contains an unsuccessful {field}"
        );
    }
    Ok(result)
}

fn raw<'a>(snapshot: &'a Value, id: &str) -> Result<&'a str> {
    let records = snapshot["data"]["status"]
        .as_array()
        .context("Missing status records")?;
    let matches: Vec<_> = records.iter().filter(|r| r["id"] == id).collect();
    ensure!(
        matches.len() == 1,
        "Missing or duplicate status property {id}"
    );
    matches[0]["params"][0]["value"]
        .as_str()
        .context("Missing raw property value")
}

fn preflight(snapshot: &Value) -> Result<()> {
    ensure!(snapshot["http_status"] == 200, "Unsuccessful live snapshot");
    for (id, expected, message) in [
        ("0080", "30", "Appliance power must be on"),
        ("00B0", "42", "Door must be closed"),
        ("0088", "42", "Appliance must have no active fault"),
        (
            "0100",
            "01",
            "Remote operation must be permitted on the appliance",
        ),
        ("00E2", "61", "Appliance must be in pre-operation standby"),
        ("0121", "00", "Appliance must be waiting for input"),
        ("0090", "42", "An existing reservation must not be active"),
    ] {
        ensure!(raw(snapshot, id)? == expected, "{message}");
    }
    // This property is a command setting; the actual phase is checked above.
    ensure!(
        ["41", "43"].contains(&raw(snapshot, "00B2")?),
        "Unsupported or paused operation command setting"
    );
    for id in ["0132", "0133", "013B"] {
        ensure!(
            ["00", "01", "02", "03"].contains(&raw(snapshot, id)?),
            "Unsupported dosing setting for {id}"
        );
    }
    let tanks = crate::decoder::tank_configuration(raw(snapshot, "0151")?)
        .context("Invalid tank configuration")?;
    ensure!(
        tanks["tank1_agent_code"] == 0
            && tanks["tank2_agent_code"] == 1
            && matches!(tanks["tank3_agent_code"].as_u64(), Some(2..=4)),
        "Unsupported agent configuration for this model"
    );
    Ok(())
}

#[cfg(test)]
fn write_once(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    token: &str,
    appliance: &str,
    snapshot: &Value,
) -> Result<(u16, Value)> {
    write_once_mode(client, endpoint, token, appliance, snapshot, false)
}

fn write_once_mode(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    token: &str,
    appliance: &str,
    snapshot: &Value,
    wash_only: bool,
) -> Result<(u16, Value)> {
    preflight(snapshot)?;
    let plan = plan_mode(appliance, snapshot, wash_only)?;
    crate::api::control_request(client, endpoint, token, appliance, &plan["body"])
        .context("Start delivery is uncertain; inspect appliance state before retrying. The client did not retry the start request")
}

pub fn send(appliance: Option<&str>, dry_run: bool, wash_only: bool) -> Result<()> {
    use crate::api;
    use chrono::Utc;
    let tokens = storage::read(&storage::root().join("auth/tokens.json"))?;
    let token = tokens["access_token"]
        .as_str()
        .context("Login required: missing access token")?;
    let client = api::control_client()?;
    let (code, devices) = api::request(&client, "user/devices", token, appliance, false)?;
    ensure!(
        code == 200,
        "Device list failed (HTTP {code}); login may need renewal"
    );
    let records = devices["devices"].as_array().context("Missing devices")?;
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
        "Select exactly one NA-LX127ER appliance"
    );
    let selected = matches[0];
    ensure!(
        selected["product_code"]
            .as_str()
            .unwrap_or("")
            .starts_with("NA-LX127ER")
            && selected["com_id"] == "NA-LX127E",
        "Start is supported only for NA-LX127ER / NA-LX127E"
    );
    let id = selected["appliance_id"]
        .as_str()
        .context("Missing appliance ID")?;
    let mut url = url::Url::parse("https://app.wad.apws.panasonic.com/laundry/v5/device/info")?;
    url.query_pairs_mut()
        .append_pair("product_number", selected["product_code"].as_str().unwrap())
        .append_pair("com_id", "NA-LX127E");
    let (code, info) = api::request(
        &client,
        &format!("device/info?{}", url.query().unwrap()),
        token,
        Some(id),
        false,
    )?;
    ensure!(code == 200, "Model metadata failed (HTTP {code})");
    let models = info["devices"]
        .as_array()
        .context("Missing metadata devices")?;
    ensure!(
        models.len() == 1 && models[0]["com_id"] == "NA-LX127E",
        "Unexpected model metadata"
    );
    let block = models[0]["supported_cmds"]
        .as_array()
        .context("Missing supported commands")?
        .iter()
        .find(|b| b["protocol"] == "echonetlite")
        .context("Missing ECHONET metadata")?;
    let readable = block["get"]
        .as_array()
        .context("Missing readable properties")?;
    let writable = block["set"]
        .as_array()
        .context("Missing writable properties")?;
    ensure!(
        READ_IDS.iter().all(|id| readable.contains(&json!(id))),
        "Preflight contains an unsupported read property"
    );
    ensure!(
        ["00B2", "00D0", "0132", "0133", "013B", "0151"]
            .iter()
            .all(|id| writable.contains(&json!(id))),
        "Start contains an unsupported write property"
    );
    let (code, data) = api::control_request(
        &client,
        api::CONTROL_URL,
        token,
        id,
        &api::read_body(id, READ_IDS),
    )?;
    let result = accepted(code, &data, id, "03")?;
    let snapshot = json!({"http_status":200,"fetched_at":Utc::now().to_rfc3339(),"data":{"status":result["cmds"]}});
    storage::save("start-preflight.json", &snapshot)?;
    preflight(&snapshot)?;
    let mut request = plan_mode(id, &snapshot, wash_only)?;
    let mode = if wash_only {
        "wash-only"
    } else {
        "wash-and-dry"
    };
    request["analysis_only"] = json!(dry_run);
    // Delivery status belongs to the separate attempt record, not this request template.
    request.as_object_mut().unwrap().remove("sent");
    storage::save("start-request.json", &request)?;
    if dry_run {
        println!(
            "Live preflight passed. Normal {mode} request saved to .local/start-request.json; no write sent."
        );
        return Ok(());
    }
    storage::save(
        "start-attempt.json",
        &json!({"state":"sending","started_at":Utc::now().to_rfc3339(),"course":if wash_only { "61" } else { "21" },"automatic_retry":false}),
    )?;
    let response = match write_once_mode(&client, api::CONTROL_URL, token, id, &snapshot, wash_only)
    {
        Ok((code, data)) => {
            json!({"http_status":code,"received_at":Utc::now().to_rfc3339(),"data":data})
        }
        Err(error) => {
            // A transport failure cannot prove that the appliance did not receive the write.
            storage::save(
                "start-attempt.json",
                &json!({"state":"delivery_uncertain","automatic_retry":false}),
            )?;
            return Err(error);
        }
    };
    storage::save("start-response.json", &response).context("Start was transmitted but response storage failed; inspect appliance state before retrying")?;
    if let Err(error) = accepted(
        response["http_status"].as_u64().unwrap() as u16,
        &response["data"],
        id,
        "02",
    ) {
        storage::save("start-attempt.json", &json!({"state":"acceptance_not_confirmed","automatic_retry":false}))
            .context("Start was transmitted but recording its unconfirmed result failed; inspect appliance state before retrying")?;
        return Err(error).context("Start was transmitted but acceptance was not confirmed; inspect appliance state before retrying");
    }
    storage::save(
        "start-attempt.json",
        &json!({"state":"api_accepted","automatic_retry":false}),
    )
    .context("API accepted the start, but recording acceptance failed; do not resend")?;
    println!(
        "Normal {mode} start accepted by the API. Physical operation has not been independently confirmed. Response saved to .local/start-response.json."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> Value {
        let values = [
            "30",
            "42",
            "42",
            "01",
            "61",
            "00",
            "42",
            "43",
            "02",
            "02",
            "00",
            "000000024223",
        ];
        json!({"http_status":200,"data":{"status":READ_IDS.iter().zip(values).map(|(id,value)|json!({"id":id,"params":[{"value":value}]})).collect::<Vec<_>>()}})
    }

    fn success() -> Value {
        json!({"results":[{"appliance_id":"test-device","cmd_type":"02","seq_no":1,"cmd_result":true,"status_code":"200","pcpf_status_code":"200"}]})
    }

    #[test]
    fn wash_only_sender_transmits_the_selected_course_once() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/device/control", server.server_addr());
        let worker = std::thread::spawn(move || {
            let mut request = server
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
                .unwrap();
            let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
            let expected = plan_mode("test-device", &healthy(), true).unwrap();
            assert_eq!(body, expected["body"]);
            request
                .respond(tiny_http::Response::from_string("{}"))
                .unwrap();
            assert!(
                server
                    .recv_timeout(std::time::Duration::from_millis(100))
                    .unwrap()
                    .is_none()
            );
        });
        let (code, _) = write_once_mode(
            &crate::api::control_client().unwrap(),
            &endpoint,
            "dummy",
            "test-device",
            &healthy(),
            true,
        )
        .unwrap();
        assert_eq!(code, 200);
        worker.join().unwrap();
    }

    #[test]
    fn wash_only_selects_61_and_preserves_dosing_and_start_guards() {
        let snapshot = healthy();
        let normal = plan("test-device", &snapshot).unwrap();
        let wash = plan_mode("test-device", &snapshot, true).unwrap();
        assert_eq!(
            wash["body"]["requests"][0]["cmds"][1],
            json!({"id":"00D0","params":["61"]})
        );
        let mut expected = normal["body"].clone();
        expected["requests"][0]["cmds"][1]["params"][0] = json!("61");
        assert_eq!(wash["body"], expected);
        let mut unsafe_snapshot = snapshot;
        unsafe_snapshot["data"]["status"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["id"] == "0100")
            .unwrap()["params"][0]["value"] = json!("00");
        assert!(preflight(&unsafe_snapshot).is_err());
    }

    #[test]
    fn standby_accepts_a_latched_start_command_setting() {
        let mut snapshot = healthy();
        let row = snapshot["data"]["status"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["id"] == "00B2")
            .unwrap();
        row["params"][0]["value"] = json!("41");
        assert!(preflight(&snapshot).is_ok());
        let phase = snapshot["data"]["status"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["id"] == "00E2")
            .unwrap();
        phase["params"][0]["value"] = json!("41");
        assert!(preflight(&snapshot).is_err());
    }

    #[test]
    fn physical_preconditions_block_the_write_before_network_access() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/device/control", server.server_addr());
        for (id, value) in [
            ("0080", "31"),
            ("00B0", "41"),
            ("0088", "41"),
            ("0100", "02"),
            ("0100", "41"),
            ("00E2", "41"),
            ("0121", "20"),
            ("0090", "41"),
            ("00B2", "42"),
            ("0132", "FF"),
            ("0151", "FFFF"),
            ("0151", "00000002422F"),
        ] {
            let mut snapshot = healthy();
            let row = snapshot["data"]["status"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| r["id"] == id)
                .unwrap();
            row["params"][0]["value"] = json!(value);
            assert!(
                write_once(
                    &crate::api::control_client().unwrap(),
                    &endpoint,
                    "test-token",
                    "test-device",
                    &snapshot
                )
                .is_err(),
                "{id}={value}"
            );
        }
        assert!(
            server
                .recv_timeout(std::time::Duration::from_millis(50))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn sender_posts_authenticated_app_encoding_exactly_once() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/device/control", server.server_addr());
        let worker = std::thread::spawn(move || {
            let mut request = server
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
                .unwrap();
            assert_eq!(request.method(), &tiny_http::Method::Post);
            assert_eq!(request.url(), "/device/control");
            for (name, value) in [
                ("Authorization", "Bearer test-token"),
                ("Cookie", "atkn=test-token"),
                ("X-ApplianceId", "test-device"),
                ("Content-Language", "ja_JP"),
            ] {
                assert!(
                    request
                        .headers()
                        .iter()
                        .any(|h| h.field.equiv(name) && h.value.as_str() == value)
                );
            }
            let timestamp = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("X-Timestamp"))
                .unwrap()
                .value
                .as_str();
            assert!(timestamp.ends_with("+0900"));
            assert!(chrono::DateTime::parse_from_str(timestamp, "%Y%m%d%H%M%S%z").is_ok());
            let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
            assert_eq!(body, plan("test-device", &healthy()).unwrap()["body"]);
            request
                .respond(
                    tiny_http::Response::from_string(success().to_string()).with_header(
                        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                    ),
                )
                .unwrap();
            assert!(
                server
                    .recv_timeout(std::time::Duration::from_millis(100))
                    .unwrap()
                    .is_none()
            );
        });
        let (code, data) = write_once(
            &crate::api::control_client().unwrap(),
            &endpoint,
            "test-token",
            "test-device",
            &healthy(),
        )
        .unwrap();
        accepted(code, &data, "test-device", "02").unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn http_success_does_not_hide_rejection_or_mismatched_acknowledgement() {
        for (key, value) in [
            ("cmd_result", json!(false)),
            ("pcpf_status_code", json!("500")),
            ("status_code", json!("400")),
            ("status_code", Value::Null),
            ("appliance_id", json!("other-device")),
            ("cmd_type", json!("03")),
            ("seq_no", json!(2)),
        ] {
            let mut data = success();
            data["results"][0][key] = value;
            assert!(accepted(200, &data, "test-device", "02").is_err(), "{key}");
        }
        assert!(accepted(503, &success(), "test-device", "02").is_err());
        assert!(accepted(200, &json!({}), "test-device", "02").is_err());
        let mut data = success();
        data["results"]
            .as_array_mut()
            .unwrap()
            .push(success()["results"][0].clone());
        assert!(accepted(200, &data, "test-device", "02").is_err());
    }

    #[test]
    fn control_post_does_not_follow_redirects() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/device/control", server.server_addr());
        let redirect = format!("http://{}/duplicate", server.server_addr());
        let worker = std::thread::spawn(move || {
            let request = server
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
                .unwrap();
            request
                .respond(
                    tiny_http::Response::from_string("{}")
                        .with_status_code(307)
                        .with_header(tiny_http::Header::from_bytes("Location", redirect).unwrap()),
                )
                .unwrap();
            assert!(
                server
                    .recv_timeout(std::time::Duration::from_millis(100))
                    .unwrap()
                    .is_none()
            );
        });
        let (code, data) = write_once(
            &crate::api::control_client().unwrap(),
            &endpoint,
            "test-token",
            "test-device",
            &healthy(),
        )
        .unwrap();
        assert_eq!(code, 307);
        assert!(accepted(code, &data, "test-device", "02").is_err());
        worker.join().unwrap();
    }
    #[test]
    fn app_command_order_and_string_array_encoding_are_preserved() {
        let snapshot = json!({"http_status":200,"data":{"status":[
            {"id":"0132","params":[{"value":"02"}]},{"id":"0133","params":[{"value":"02"}]},
            {"id":"013B","params":[{"value":"00"}]},{"id":"0151","params":[{"value":"000000024223"}]},
            {"id":"0100","params":[{"value":"41"}]},{"id":"00B0","params":[{"value":"42"}]},
            {"id":"0088","params":[{"value":"42"}]},{"id":"0080","params":[{"value":"30"}]}
        ]}});
        let p = plan("test-device", &snapshot).unwrap();
        let request = &p["body"]["requests"][0];
        assert_eq!(request["cmd_type"], "02");
        assert_eq!(request["cmds"][0], json!({"id":"00B2","params":["41"]}));
        assert_eq!(request["cmds"][1], json!({"id":"00D0","params":["21"]}));
        assert_eq!(request["cmds"].as_array().unwrap().len(), 6);
        assert_eq!(p["sent"], false);
        assert!(plan("", &snapshot).is_err());
    }
}
