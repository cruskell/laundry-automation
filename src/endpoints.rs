//! Additional API operations from the bundled third-party reference.
//! Selected read operations are verified against production; see API_ENDPOINTS.md.
use anyhow::{Context, Result, ensure};
use clap::Args;
use reqwest::{Method, blocking::Client};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use url::Url;

use crate::{api, storage};

const BASE: &str = "https://app.wad.apws.panasonic.com/laundry/v5/";

pub use crate::generated_api::Operation;

#[derive(Args)]
pub struct Options {
    /// Named API operation; run `api --help` for the complete list.
    #[arg(value_enum)]
    operation: Operation,
    /// Query, path, or named header parameter, repeatable (e.g. --param term_id=ID).
    #[arg(long = "param", value_name = "KEY=VALUE")]
    params: Vec<String>,
    /// JSON object to send, read from a file; never put tokens in shell arguments.
    #[arg(long, value_name = "FILE")]
    body: Option<PathBuf>,
    /// Validate and save the request without authentication or network access.
    #[arg(long)]
    dry_run: bool,
}

struct Prepared {
    method: Method,
    url: Url,
    body: Option<Value>,
    headers: BTreeMap<String, String>,
    notice_id: Option<String>,
}

fn prepare(options: &Options, appliance: Option<&str>, base: &str) -> Result<Prepared> {
    let endpoint = options.operation.endpoint();
    ensure!(
        !endpoint.appliance || appliance.is_some_and(|id| !id.trim().is_empty()),
        "This operation requires --appliance-id"
    );
    if let Some(id) = appliance {
        ensure!(
            !id.trim().is_empty() && !id.contains(['\r', '\n']),
            "Invalid appliance ID"
        );
    }
    let mut params = BTreeMap::new();
    for param in &options.params {
        let (key, value) = param.split_once('=').context("Use --param KEY=VALUE")?;
        ensure!(
            endpoint.required.contains(&key) || endpoint.optional.contains(&key),
            "Unknown parameter: {key}"
        );
        ensure!(!value.trim().is_empty(), "Empty parameter: {key}");
        ensure!(
            params.insert(key, value).is_none(),
            "Duplicate parameter: {key}"
        );
        if ["limit", "offset", "from_index"].contains(&key) {
            value
                .parse::<u32>()
                .with_context(|| format!("{key} must be a nonnegative integer"))?;
        }
        if key == "osType" {
            ensure!(
                ["ios", "android"].contains(&value),
                "osType must be ios or android"
            );
        }
        if key == "supplies_type" {
            ensure!(
                ["detergent", "softener"].contains(&value),
                "supplies_type must be detergent or softener"
            );
        }
    }
    for key in endpoint.required {
        ensure!(params.contains_key(key), "Missing --param {key}=VALUE");
    }
    ensure!(
        !(params.contains_key("offset") && params.contains_key("from_index")),
        "Choose offset (reference) or from_index (observed app), not both"
    );
    let mut body = options.body.as_deref().map(storage::read).transpose()?;
    ensure!(
        endpoint.body == body.is_some(),
        "This operation {} --body FILE",
        if endpoint.body {
            "requires"
        } else {
            "does not accept"
        }
    );
    if let Some(body) = &body {
        validate_body(options.operation, body)?;
    }
    if let Some(generated) = options.operation.generated_body(appliance) {
        body = Some(generated);
    }
    if let Some(body) = &body {
        if let Some(id) = body.get("appliance_id") {
            if let Some(selected) = appliance {
                ensure!(
                    id.as_str() == Some(selected),
                    "Body appliance_id must match --appliance-id"
                );
            }
        }
        if matches!(
            options.operation,
            Operation::ControlRead | Operation::ControlWrite
        ) {
            ensure!(
                body["requests"][0]["appliance_id"].as_str() == appliance,
                "Control appliance_id must match --appliance-id"
            );
        }
    }
    let mut headers = BTreeMap::new();
    for &(param, header) in endpoint.headers {
        let value = params.remove(param).context("Missing header parameter")?;
        reqwest::header::HeaderValue::from_str(value).context("Invalid header parameter")?;
        headers.insert(header.to_owned(), value.to_owned());
    }
    let notice_id = if matches!(options.operation, Operation::Notice) {
        Some(params.remove("id").context("Missing notice ID")?.to_owned())
    } else {
        None
    };
    let mut url = Url::parse(base)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Invalid API base URL"))?;
        segments.pop_if_empty();
        for segment in endpoint.path.split('/') {
            if let Some(key) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                let value = params.remove(key).context("Missing path parameter")?;
                ensure!(value != "." && value != "..", "Invalid path parameter");
                segments.push(value);
            } else {
                segments.push(segment);
            }
        }
    }
    if !params.is_empty() {
        url.query_pairs_mut().extend_pairs(params);
    }
    Ok(Prepared {
        method: endpoint.method,
        url,
        body,
        headers,
        notice_id,
    })
}

fn validate_commands(cmds: &Value, writing: bool) -> Result<()> {
    let cmds = cmds.as_array().context("cmds must be an array")?;
    ensure!(!cmds.is_empty(), "cmds must not be empty");
    let mut seen = std::collections::BTreeSet::new();
    for cmd in cmds {
        let id = cmd["id"].as_str().context("Command requires id")?;
        ensure!(
            id.len() == 4 && id.bytes().all(|b| b.is_ascii_hexdigit()),
            "Command id must be four hex digits"
        );
        ensure!(seen.insert(id.to_uppercase()), "Duplicate command id");
        let params = cmd["params"]
            .as_array()
            .context("Command requires params array")?;
        if writing {
            ensure!(
                !params.is_empty()
                    && params
                        .iter()
                        .all(|v| v.as_str().is_some_and(|s| !s.is_empty()
                            && s.len() % 2 == 0
                            && s.bytes().all(|b| b.is_ascii_hexdigit()))),
                "Write params must be nonempty even-length hex strings"
            );
            if id.eq_ignore_ascii_case("00B2") {
                ensure!(
                    params.len() == 1 && matches!(params[0].as_str(), Some("42" | "43")),
                    "Use guarded start for starting/resuming operation; control-write supports pause 42 and stop 43"
                );
            }
        } else {
            ensure!(params.is_empty(), "Read commands require empty params");
        }
    }
    Ok(())
}

fn validate_body(operation: Operation, body: &Value) -> Result<()> {
    operation.validate_request(body)?;
    ensure!(
        body.as_object().is_some_and(|v| !v.is_empty()),
        "Body must be a nonempty JSON object"
    );
    // CLI safety policy is stricter than the server's multi-appliance contract.
    if matches!(operation, Operation::ControlRead | Operation::ControlWrite) {
        let requests = body["requests"]
            .as_array()
            .context("Missing control requests")?;
        ensure!(
            requests.len() == 1,
            "Control supports exactly one appliance request"
        );
        validate_commands(
            &requests[0]["cmds"],
            matches!(operation, Operation::ControlWrite),
        )?;
    }
    Ok(())
}

fn check_supported_commands(info: &Value, body: &Value, writing: bool) -> Result<()> {
    let models = info["devices"]
        .as_array()
        .context("Missing model metadata")?;
    ensure!(models.len() == 1, "Expected one model metadata record");
    let block = models[0]["supported_cmds"]
        .as_array()
        .context("Missing supported commands")?
        .iter()
        .find(|b| b["protocol"] == "echonetlite")
        .context("Missing ECHONET metadata")?;
    let supported = block[if writing { "set" } else { "get" }]
        .as_array()
        .context("Missing supported property list")?;
    for cmd in body["requests"][0]["cmds"].as_array().unwrap() {
        let id = cmd["id"].as_str().unwrap();
        ensure!(
            supported
                .iter()
                .any(|v| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case(id))),
            "Model does not support this command ID for the requested operation"
        );
    }
    Ok(())
}

fn control_capabilities(
    client: &Client,
    token: &str,
    appliance: &str,
    body: &Value,
    writing: bool,
) -> Result<()> {
    let (code, devices) = api::request(client, "user/devices", token, Some(appliance), false)?;
    check_response(code, &devices)?;
    let matches: Vec<_> = devices["devices"]
        .as_array()
        .context("Missing devices array")?
        .iter()
        .filter(|d| d["appliance_id"].as_str() == Some(appliance))
        .collect();
    ensure!(
        matches.len() == 1,
        "Select exactly one registered appliance"
    );
    let device = matches[0];
    let mut url = Url::parse(&format!("{BASE}device/info"))?;
    url.query_pairs_mut()
        .append_pair(
            "product_number",
            device["product_code"]
                .as_str()
                .context("Missing product code")?,
        )
        .append_pair(
            "com_id",
            device["com_id"]
                .as_str()
                .context("Missing communication model")?,
        );
    let (code, info) = api::request(
        client,
        &format!("device/info?{}", url.query().unwrap()),
        token,
        Some(appliance),
        false,
    )?;
    check_response(code, &info)?;
    ensure!(
        info["devices"][0]["com_id"] == device["com_id"],
        "Unexpected capability communication model"
    );
    storage::save(
        "api/control-capabilities.json",
        &json!({"http_status":code,"data":info}),
    )?;
    check_supported_commands(&info, body, writing)
}

fn check_control_response(request: &Value, data: &Value) -> Result<()> {
    let results = data["results"]
        .as_array()
        .context("Missing control results")?;
    ensure!(results.len() == 1, "Expected one control result");
    let r = &results[0];
    let expected = &request["requests"][0];
    for key in ["appliance_id", "cmd_type", "seq_no"] {
        ensure!(
            r[key] == expected[key],
            "Control acknowledgement mismatch: {key}"
        );
    }
    ensure!(r["cmd_result"] == true, "Control command rejected");
    for key in ["status_code", "pcpf_status_code"] {
        ensure!(
            r[key] == "200" || r[key] == 200,
            "Control reported unsuccessful {key}"
        );
    }
    Ok(())
}

fn transmit_once(
    _client: &Client,
    prepared: &Prepared,
    token: Option<&str>,
    appliance: Option<&str>,
) -> Result<(u16, Value)> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Language", "ja_JP".parse()?);
        let timestamp = chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).unwrap())
            .format("%Y%m%d%H%M%S%z")
            .to_string();
        headers.insert("X-Timestamp", timestamp.parse()?);
        if let Some(token) = token {
            let mut bearer = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))?;
            bearer.set_sensitive(true);
            headers.insert(reqwest::header::AUTHORIZATION, bearer);
            let mut cookie = reqwest::header::HeaderValue::from_str(&format!("atkn={token}"))?;
            cookie.set_sensitive(true);
            headers.insert(reqwest::header::COOKIE, cookie);
        }
        if let Some(id) = appliance {
            headers.insert("X-ApplianceId", id.parse()?);
        }
        let http = reqwest::Client::builder()
            .user_agent(concat!("laundry-automation/", env!("CARGO_PKG_VERSION")))
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .timeout(std::time::Duration::from_secs(25))
            .build()?;
        let origin = prepared.url.origin().ascii_serialization();
        let generated = crate::panasonic_api::Client::new_with_client(&origin, http);
        crate::generated_api::dispatch(
            &generated,
            &prepared.method,
            &prepared.url,
            prepared.body.as_ref(),
            &prepared.headers,
            appliance,
        )
        .await
    })
}

// The app's /notices/{id} POST marks a notice read. Fetch details through
// paginated GET /notices instead so that `notice` remains read-only.
fn transmit(
    client: &Client,
    prepared: &Prepared,
    token: Option<&str>,
    appliance: Option<&str>,
) -> Result<(u16, Value)> {
    let Some(id) = &prepared.notice_id else {
        return transmit_once(client, prepared, token, appliance);
    };
    let mut offset = 0;
    for _ in 0..100 {
        let mut url = prepared.url.clone();
        url.query_pairs_mut()
            .append_pair("limit", "100")
            .append_pair("from_index", &offset.to_string());
        let page = Prepared {
            method: Method::GET,
            url,
            body: None,
            headers: prepared.headers.clone(),
            notice_id: None,
        };
        let (code, data) = transmit_once(client, &page, token, appliance)?;
        // Preserve server errors for the caller's response record.
        if check_response(code, &data).is_err() {
            return Ok((code, data));
        }
        let Some(notices) = data["notices"].as_array() else {
            return Ok((
                code,
                json!({"error":"Missing notices array", "response":data}),
            ));
        };
        if let Some(notice) = notices.iter().find(|n| n["id"].as_str() == Some(id)) {
            return Ok((code, json!({"notice":notice,"read_only":true})));
        }
        offset += notices.len();
        let total = data["pagination_info"]["total"].as_u64();
        if notices.is_empty() || total.is_some_and(|n| offset as u64 >= n) {
            return Ok((
                404,
                json!({"error":"Notice not found in the accessible list", "read_only":true}),
            ));
        }
    }
    Ok((
        422,
        json!({"error":"Notice search exceeded 100 pages", "read_only":true}),
    ))
}

fn check_response(code: u16, data: &Value) -> Result<()> {
    ensure!(
        (200..300).contains(&code),
        "API request failed (HTTP {code}); inspect the saved response"
    );
    ensure!(
        data.get("non_json_response").is_none(),
        "Non-JSON response; inspect the saved response"
    );
    if let Some(status) = data.get("status_code") {
        let code = status
            .as_u64()
            .or_else(|| status.as_str().and_then(|s| s.parse().ok()));
        ensure!(
            code.is_some_and(|n| (200..300).contains(&n)),
            "API reported unsuccessful status_code"
        );
    }
    ensure!(
        data.get("error").is_none_or(|e| e.is_null() || e == false),
        "API reported an error; inspect the saved response"
    );
    Ok(())
}

pub fn run(options: &Options, appliance: Option<&str>) -> Result<()> {
    let prepared = prepare(options, appliance, BASE)?;
    let name = options.operation.name();
    storage::save(
        &format!("api/{name}-request.json"),
        &json!({
            "method":prepared.method.as_str(), "url":prepared.url.as_str(),
            "appliance_id":appliance, "body":prepared.body, "headers":prepared.headers,
            "notice_id":prepared.notice_id, "dry_run":options.dry_run
        }),
    )?;
    if options.dry_run {
        println!(
            "Dry run: {} {}. Request saved to .local/api/{name}-request.json; nothing sent.",
            prepared.method, prepared.url
        );
        return Ok(());
    }
    let tokens = storage::read(&storage::root().join("auth/tokens.json"))
        .context("Login required: complete the browser OAuth flow first")?;
    let token = tokens["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .context("Missing access token")?;
    let client = api::control_client()?;
    if matches!(
        options.operation,
        Operation::ControlRead | Operation::ControlWrite
    ) {
        control_capabilities(
            &client,
            token,
            appliance.unwrap(),
            prepared.body.as_ref().unwrap(),
            matches!(options.operation, Operation::ControlWrite),
        )?;
    }
    let filename = format!("api/{name}-response.json");
    storage::save(
        &filename,
        &json!({"state":"pending", "automatic_retry":false}),
    )?;
    let result = transmit(&client, &prepared, Some(token), appliance);
    let (code, mut data) = match result {
        Ok(result) => result,
        Err(_) => {
            storage::save(
                &filename,
                &json!({"state":"delivery_uncertain", "automatic_retry":false}),
            )?;
            anyhow::bail!(
                "Request transport failed; delivery is uncertain. Check current state before retrying."
            );
        }
    };
    if matches!(options.operation, Operation::NotificationTerminals)
        && check_response(code, &data).is_ok()
    {
        data = match data.get("notification_term_ids") {
            Some(ids) if ids.is_array() => json!({"notification_term_ids":ids}),
            _ => json!({"error":"User response lacks notification_term_ids array"}),
        };
    }
    storage::save(&filename, &json!({"http_status":code,"fetched_at":chrono::Utc::now().to_rfc3339(),"data":data,"automatic_retry":false}))
        .context("Request was sent but saving its response failed; check current state before retrying")?;
    check_response(code, &data)?;
    if matches!(
        options.operation,
        Operation::ControlRead | Operation::ControlWrite
    ) {
        check_control_response(prepared.body.as_ref().unwrap(), &data)?;
    }
    println!(
        "{name}: HTTP {code}; response saved to .local/{filename}. This is an HTTP/API result, not confirmation of a physical operation."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(operation: Operation, params: &[&str]) -> Options {
        Options {
            operation,
            params: params.iter().map(|s| s.to_string()).collect(),
            body: None,
            dry_run: true,
        }
    }

    fn prepare_body(operation: Operation, body: &Value) -> Result<Prepared> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "laundry-api-test-{}-{}.json",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, body.to_string())?;
        let opts = Options {
            operation,
            params: vec![],
            body: Some(path.clone()),
            dry_run: true,
        };
        let result = prepare(&opts, Some("device"), BASE);
        std::fs::remove_file(path)?;
        result
    }

    fn course_body(kind: u64) -> Value {
        json!({"wash_dry_course_code":"01","appliance_id":"device","api_type":kind,"view_id":if kind==1 {"11300"} else {"11310"},"base_course":"2102","courses":[{"id":"000001","name":"example","cmds":[{"id":"00D0","params":[{"value":"21"}]}]}]})
    }

    #[test]
    fn official_mutation_contracts_are_posted_once_with_exact_bodies() {
        let cases = vec![
            (
                Operation::Login,
                "users",
                Some(json!({"type":0})),
                json!({"type":0}),
                vec![],
            ),
            (
                Operation::RemoveDevice,
                "user/devices/remove",
                None,
                json!({"appliance_ids":["device"]}),
                vec![],
            ),
            (
                Operation::MarkNoticeRead,
                "notices/notice-id",
                None,
                json!({"read_state":true}),
                vec!["id=notice-id"],
            ),
            (
                Operation::AmazonAccountLink,
                "amazon/account/link",
                Some(json!({"author_code":"dummy-code","appliance_id":"device"})),
                json!({"author_code":"dummy-code","appliance_id":"device"}),
                vec![],
            ),
            (
                Operation::AmazonSkillEnable,
                "amazon/skill/enable",
                Some(json!({"author_code":"dummy-code","appliance_id":"device"})),
                json!({"author_code":"dummy-code","appliance_id":"device"}),
                vec![],
            ),
            (
                Operation::RegisterCourse,
                "customer/course",
                Some(course_body(1)),
                course_body(1),
                vec![],
            ),
            (
                Operation::EditCourse,
                "customer/course",
                Some(course_body(2)),
                course_body(2),
                vec![],
            ),
            (
                Operation::DeleteCourse,
                "customer/course",
                Some(course_body(3)),
                course_body(3),
                vec![],
            ),
            (
                Operation::RegisterNotificationToken,
                "user/notifications",
                Some(json!({"reg_id":"dummy-push","type":1,"device_id":"phone"})),
                json!({"reg_id":"dummy-push","type":1,"device_id":"phone"}),
                vec![],
            ),
            (
                Operation::UpdateNoticeSettings,
                "user/notices/setting",
                Some(
                    json!({"term_id":"terminal","appliance_id":"device","pana_notice_setting":false}),
                ),
                json!({"term_id":"terminal","appliance_id":"device","pana_notice_setting":false}),
                vec![],
            ),
            (
                Operation::UpdateNotificationSettings,
                "user/notifications/settings",
                Some(
                    json!({"term_id":"terminal","appliance_id":"device","all_status":false,"items":[{"id":"item","status":false}],"pana_notice_setting":true}),
                ),
                json!({"term_id":"terminal","appliance_id":"device","all_status":false,"items":[{"id":"item","status":false}],"pana_notice_setting":true}),
                vec![],
            ),
            (
                Operation::UpdateWeatherSettings,
                "weatherinfo/settings",
                Some(
                    json!({"locale_id":"ja_JP","region_id":"region","sub_region_1":{"pref":"Prefecture"},"sub_region_2":{"city":"City"}}),
                ),
                json!({"locale_id":"ja_JP","region_id":"region","sub_region_1":{"pref":"Prefecture"},"sub_region_2":{"city":"City"}}),
                vec![],
            ),
        ];
        for (operation, path, input, expected, params) in cases {
            let mut prepared = if let Some(body) = input {
                prepare_body(operation, &body).unwrap()
            } else {
                prepare(&options(operation, &params), Some("device"), BASE).unwrap()
            };
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            prepared.url = Url::parse(&format!(
                "http://{}/laundry/v5/{path}",
                server.server_addr()
            ))
            .unwrap();
            let worker = std::thread::spawn(move || {
                let mut request = server
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap()
                    .unwrap();
                assert_eq!(request.method(), &tiny_http::Method::Post);
                assert_eq!(request.url(), format!("/laundry/v5/{path}"));
                assert!(
                    request
                        .headers()
                        .iter()
                        .any(|h| h.field.equiv("Authorization")
                            && h.value.as_str() == "Bearer dummy")
                );
                assert!(
                    request
                        .headers()
                        .iter()
                        .any(|h| h.field.equiv("X-ApplianceId") && h.value.as_str() == "device")
                );
                let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
                assert_eq!(body, expected);
                request
                    .respond(tiny_http::Response::from_string("{}"))
                    .unwrap();
                assert!(
                    server
                        .recv_timeout(std::time::Duration::from_millis(30))
                        .unwrap()
                        .is_none()
                );
            });
            let (code, data) = transmit(
                &api::control_client().unwrap(),
                &prepared,
                Some("dummy"),
                Some("device"),
            )
            .unwrap();
            check_response(code, &data).unwrap();
            worker.join().unwrap();
        }
    }

    #[test]
    fn all_user_setting_modes_accept_official_fields_and_reject_missing_data() {
        for body in [
            json!({"setting_type":1,"tos_agreed_version":"9"}),
            json!({"setting_type":2,"appliance_id":"device"}),
            json!({"setting_type":3,"tos_agreed_version":"9"}),
            json!({"setting_type":4,"read_message_ids":["message"]}),
            json!({"setting_type":5}),
            json!({"setting_type":6,"term_id":"terminal","app_sleep_setting":false}),
        ] {
            assert!(prepare_body(Operation::UpdateUser, &body).is_ok());
        }
        for mode in [1, 2, 3, 4, 6, 7] {
            assert!(validate_body(Operation::UpdateUser, &json!({"setting_type":mode})).is_err());
        }
        assert!(
            prepare_body(
                Operation::UpdateNoticeSettings,
                &json!({"term_id":"terminal","appliance_id":"other","pana_notice_setting":true})
            )
            .is_err()
        );
        assert!(
            validate_body(
                Operation::RegisterNotificationToken,
                &json!({"token":"old","os_type":"android"})
            )
            .is_err()
        );
        assert!(validate_body(Operation::RegisterCourse, &course_body(3)).is_err());
        assert!(validate_body(Operation::DeleteCourse, &json!({"id":"old-reference"})).is_err());
    }

    fn control_body(writing: bool, value: &str) -> Value {
        json!({"requests":[{"appliance_id":"device","cmd_type":if writing {"02"} else {"03"},"cmds":[{"id":"00B2","params":if writing {json!([value])} else {json!([])}}],"protocol":"echonetlite","seq_no":1}]})
    }

    #[test]
    fn raw_control_validates_model_support_start_guard_and_acknowledgements() {
        let info = json!({"devices":[{"supported_cmds":[{"protocol":"echonetlite","get":["00B2"],"set":["00B2"]}]}]});
        for (op, body) in [
            (Operation::ControlRead, control_body(false, "")),
            (Operation::ControlWrite, control_body(true, "42")),
            (Operation::ControlWrite, control_body(true, "43")),
        ] {
            let prepared = prepare_body(op, &body).unwrap();
            check_supported_commands(&info, &body, matches!(op, Operation::ControlWrite)).unwrap();
            assert_eq!(prepared.method, Method::POST);
            let good = json!({"results":[{"appliance_id":"device","cmd_type":body["requests"][0]["cmd_type"],"seq_no":1,"cmd_result":true,"status_code":"200","pcpf_status_code":200}]});
            check_control_response(&body, &good).unwrap();
            for (key, value) in [
                ("cmd_result", json!(false)),
                ("appliance_id", json!("other")),
                ("seq_no", json!(2)),
                ("pcpf_status_code", json!(500)),
                ("cmd_type", json!("FF")),
            ] {
                let mut bad = good.clone();
                bad["results"][0][key] = value;
                assert!(check_control_response(&body, &bad).is_err());
            }
        }
        assert!(validate_body(Operation::ControlWrite, &control_body(true, "41")).is_err());
        assert!(validate_body(Operation::ControlRead, &control_body(true, "42")).is_err());
        assert!(validate_body(Operation::ControlWrite, &control_body(true, "4")).is_err());
        let mut unknown = control_body(false, "");
        unknown["requests"][0]["cmds"][0]["id"] = json!("FFFF");
        assert!(check_supported_commands(&info, &unknown, false).is_err());
        let mut mismatch = control_body(false, "");
        mismatch["requests"][0]["appliance_id"] = json!("other");
        assert!(prepare_body(Operation::ControlRead, &mismatch).is_err());
    }

    #[test]
    fn terminal_list_uses_the_official_authenticated_user_info_request() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let prepared = prepare(
            &options(Operation::NotificationTerminals, &[]),
            None,
            &format!("http://{}/laundry/v5/", server.server_addr()),
        )
        .unwrap();
        let worker = std::thread::spawn(move || {
            let mut request = server
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
                .unwrap();
            assert_eq!(request.method(), &tiny_http::Method::Post);
            assert_eq!(request.url(), "/laundry/v5/users");
            assert!(
                request
                    .headers()
                    .iter()
                    .any(|h| h.field.equiv("Authorization") && h.value.as_str() == "Bearer dummy")
            );
            let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
            assert_eq!(body, json!({"type":0}));
            request
                .respond(tiny_http::Response::from_string(
                    json!({"notification_term_ids":["existing-terminal"]}).to_string(),
                ))
                .unwrap();
        });
        let (code, data) = transmit(
            &api::control_client().unwrap(),
            &prepared,
            Some("dummy"),
            None,
        )
        .unwrap();
        assert_eq!(code, 200);
        assert_eq!(data["notification_term_ids"], json!(["existing-terminal"]));
        worker.join().unwrap();
    }

    #[test]
    fn required_header_parameters_are_validated_before_network_access() {
        for operation in [Operation::NoticeSettings, Operation::NotificationSettings] {
            assert!(prepare(&options(operation, &[]), Some("device"), BASE).is_err());
            assert!(
                prepare(
                    &options(operation, &["term_id=bad\r\nInjected: x"]),
                    Some("device"),
                    BASE
                )
                .is_err()
            );
            let prepared = prepare(
                &options(operation, &["term_id=registered-terminal"]),
                Some("device"),
                BASE,
            )
            .unwrap();
            assert_eq!(prepared.headers["X-TermId"], "registered-terminal");
            assert_eq!(prepared.url.query(), None);
        }
        assert!(prepare(&options(Operation::Advertisements, &[]), None, BASE).is_err());
        assert!(
            prepare(
                &options(Operation::CustomerCourses, &["wash_dry_course_code=01"]),
                None,
                BASE
            )
            .is_err()
        );
    }

    #[test]
    fn official_read_contracts_send_parameters_in_the_correct_places() {
        for (operation, params, expected_path, header) in [
            (
                Operation::Advertisements,
                vec!["product_number=NA-LX127ER-W"],
                "/laundry/v5/advertisements",
                Some(("X-Product-Number", "NA-LX127ER-W")),
            ),
            (
                Operation::NoticeSettings,
                vec!["term_id=terminal"],
                "/laundry/v5/user/notices/setting",
                Some(("X-TermId", "terminal")),
            ),
            (
                Operation::NotificationSettings,
                vec!["term_id=terminal"],
                "/laundry/v5/user/notifications/settings",
                Some(("X-TermId", "terminal")),
            ),
            (
                Operation::CustomerCourses,
                vec!["wash_dry_course_code=01", "base_course=21"],
                "/laundry/v5/customer/course?base_course=21&wash_dry_course_code=01",
                None,
            ),
        ] {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let prepared = prepare(
                &options(operation, &params),
                Some("device"),
                &format!("http://{}/laundry/v5/", server.server_addr()),
            )
            .unwrap();
            let worker = std::thread::spawn(move || {
                let request = server
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap()
                    .unwrap();
                assert_eq!(request.method(), &tiny_http::Method::Get);
                assert_eq!(request.url(), expected_path);
                assert!(
                    request
                        .headers()
                        .iter()
                        .any(|h| h.field.equiv("X-ApplianceId") && h.value.as_str() == "device")
                );
                if let Some((name, value)) = header {
                    assert!(
                        request
                            .headers()
                            .iter()
                            .any(|h| h.field.equiv(name) && h.value.as_str() == value)
                    );
                }
                request
                    .respond(tiny_http::Response::from_string("{}"))
                    .unwrap();
            });
            assert_eq!(
                transmit(
                    &api::control_client().unwrap(),
                    &prepared,
                    Some("dummy"),
                    Some("device")
                )
                .unwrap()
                .0,
                200
            );
            worker.join().unwrap();
        }
    }

    #[test]
    fn notice_details_search_pages_without_marking_any_notice_read() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let prepared = prepare(
            &options(Operation::Notice, &["id=target"]),
            Some("device"),
            &format!("http://{}/laundry/v5/", server.server_addr()),
        )
        .unwrap();
        let worker = std::thread::spawn(move || {
            for (offset, id) in [(0, "other"), (1, "target")] {
                let request = server
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap()
                    .unwrap();
                assert_eq!(request.method(), &tiny_http::Method::Get);
                assert_eq!(
                    request.url(),
                    format!("/laundry/v5/notices?from_index={offset}&limit=100")
                );
                request.respond(tiny_http::Response::from_string(json!({"notices":[{"id":id,"read_state":false}],"pagination_info":{"total":2}}).to_string())).unwrap();
            }
            assert!(
                server
                    .recv_timeout(std::time::Duration::from_millis(100))
                    .unwrap()
                    .is_none()
            );
        });
        let (code, data) = transmit(
            &api::control_client().unwrap(),
            &prepared,
            Some("dummy"),
            Some("device"),
        )
        .unwrap();
        assert_eq!(code, 200);
        assert_eq!(data["notice"], json!({"id":"target","read_state":false}));
        assert_eq!(data["read_only"], true);
        worker.join().unwrap();
    }

    #[test]
    fn notice_search_returns_missing_and_server_errors_without_writes() {
        for (status, body, expected) in [
            (
                200,
                json!({"notices":[],"pagination_info":{"total":0}}),
                404,
            ),
            (403, json!({"code":"denied"}), 403),
            (200, json!({"unexpected":[]}), 200),
        ] {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let prepared = prepare(
                &options(Operation::Notice, &["id=absent"]),
                Some("device"),
                &format!("http://{}/laundry/v5/", server.server_addr()),
            )
            .unwrap();
            let worker = std::thread::spawn(move || {
                let request = server
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap()
                    .unwrap();
                assert_eq!(request.method(), &tiny_http::Method::Get);
                request
                    .respond(
                        tiny_http::Response::from_string(body.to_string()).with_status_code(status),
                    )
                    .unwrap();
            });
            let (code, data) = transmit(
                &api::control_client().unwrap(),
                &prepared,
                Some("dummy"),
                Some("device"),
            )
            .unwrap();
            assert_eq!(code, expected);
            assert!(check_response(code, &data).is_err());
            worker.join().unwrap();
        }
    }

    #[test]
    fn parameters_cannot_change_the_origin_or_escape_a_path_segment() {
        let prepared = prepare(
            &options(Operation::Courses, &["comId=a/b?x=1&y=日本語"]),
            None,
            BASE,
        )
        .unwrap();
        assert_eq!(prepared.url.host_str(), Some("app.wad.apws.panasonic.com"));
        assert_eq!(prepared.url.query(), None);
        assert!(prepared.url.path().contains("a%2Fb%3Fx=1&y="));
        for value in ["comId=.", "comId=.."] {
            assert!(prepare(&options(Operation::Courses, &[value]), None, BASE).is_err());
        }
        let query = prepare(
            &options(
                Operation::CustomView,
                &[
                    "com_id=a&evil=1",
                    "wash_dry_course_code=01",
                    "view_id=A B",
                    "command_param_value=4F",
                ],
            ),
            None,
            BASE,
        )
        .unwrap();
        let pairs: BTreeMap<_, _> = query.url.query_pairs().collect();
        assert_eq!(pairs.len(), 4);
        assert_eq!(pairs["com_id"], "a&evil=1");
    }

    #[test]
    fn invalid_requests_fail_before_transport() {
        for (op, params, appliance) in [
            (Operation::Notice, vec![], None),
            (Operation::Notices, vec!["limit=-1"], None),
            (Operation::Notices, vec!["limit=1", "limit=2"], None),
            (Operation::Notices, vec!["offset=1", "from_index=1"], None),
            (Operation::Notices, vec!["typo=1"], None),
            (
                Operation::Versions,
                vec!["osType=windows", "appVersion=25"],
                None,
            ),
            (Operation::RemoveDevice, vec![], None),
            (Operation::RemoveDevice, vec![], Some("\n")),
            (Operation::UpdatePredictionSettings, vec![], Some("test")),
        ] {
            assert!(prepare(&options(op, &params), appliance, BASE).is_err());
        }
        assert!(
            validate_body(
                Operation::UpdatePredictionSettings,
                &json!({"use_drytime_prediction":"false"})
            )
            .is_err()
        );
        assert!(
            validate_body(
                Operation::UpdatePredictionSettings,
                &json!({"use_drytime_prediction":false})
            )
            .is_ok()
        );
        assert!(
            validate_body(
                Operation::UpdateWeatherSettings,
                &json!({"location":{"latitude":91,"longitude":0}})
            )
            .is_err()
        );
        assert!(validate_body(Operation::RegisterCourse, &json!({"name":"test"})).is_err());
        assert!(
            validate_body(
                Operation::UpdateNotificationSettings,
                &json!({"types":[false]})
            )
            .is_err()
        );
    }

    #[test]
    fn generated_client_preserves_error_statuses_and_does_not_follow_redirects() {
        for (operation, status, body) in [
            (Operation::Notices, 200, None),
            (
                Operation::UpdatePredictionSettings,
                200,
                Some(json!({"use_drytime_prediction":false})),
            ),
            (Operation::Notices, 204, None),
            (
                Operation::UpdatePredictionSettings,
                307,
                Some(json!({"use_drytime_prediction":false})),
            ),
            (
                Operation::UpdatePredictionSettings,
                400,
                Some(json!({"use_drytime_prediction":false})),
            ),
        ] {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let reading = matches!(operation, Operation::Notices);
            let method = if reading { Method::GET } else { Method::POST };
            let expected_path = if reading {
                "/laundry/v5/notices?language=a%26b"
            } else {
                "/laundry/v5/user/drytime_prediction/settings"
            };
            let prepared = Prepared {
                method: method.clone(),
                url: Url::parse(&format!("http://{}{expected_path}", server.server_addr()))
                    .unwrap(),
                body: body.clone(),
                headers: BTreeMap::new(),
                notice_id: None,
            };
            let worker = std::thread::spawn(move || {
                let mut request = server
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap()
                    .unwrap();
                assert_eq!(request.method().as_str(), method.as_str());
                assert_eq!(request.url(), expected_path);
                for (name, value) in [
                    ("Authorization", "Bearer dummy"),
                    ("Cookie", "atkn=dummy"),
                    ("X-ApplianceId", "device"),
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
                if let Some(body) = body {
                    let received: Value = serde_json::from_reader(request.as_reader()).unwrap();
                    assert_eq!(received, body);
                }
                request
                    .respond(
                        tiny_http::Response::from_string(if status == 204 { "" } else { "{}" })
                            .with_status_code(status)
                            .with_header(
                                tiny_http::Header::from_bytes("Location", "/redirected").unwrap(),
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
            let (code, data) = transmit(
                &api::control_client().unwrap(),
                &prepared,
                Some("dummy"),
                Some("device"),
            )
            .unwrap();
            assert_eq!(code, status);
            assert_eq!(
                check_response(code, &data).is_ok(),
                (200..300).contains(&status)
            );
            worker.join().unwrap();
        }
    }

    #[test]
    fn http_success_does_not_hide_application_errors() {
        for (code, data) in [
            (401, json!({})),
            (200, json!({"status_code":"500"})),
            (200, json!({"error":"rejected"})),
            (200, json!({"non_json_response":"html"})),
            (200, json!({"status_code":"unknown"})),
        ] {
            assert!(check_response(code, &data).is_err());
        }
        assert!(check_response(204, &Value::Null).is_ok());
        assert!(check_response(200, &json!({"status_code":"200"})).is_ok());
    }
}
