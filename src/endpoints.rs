//! Additional API operations from the bundled third-party reference.
//! These operations have not been verified against production.
use anyhow::{Context, Result, ensure};
use clap::{Args, ValueEnum};
use reqwest::{Method, blocking::Client};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use url::Url;

use crate::{api, storage};

const BASE: &str = "https://app.wad.apws.panasonic.com/laundry/v5/";

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Operation {
    Login,
    UpdateUser,
    RegisterDevice,
    RemoveDevice,
    MenuItems,
    CustomerCourses,
    RegisterCourse,
    DeleteCourse,
    CustomView,
    ContentVersions,
    Notices,
    Notice,
    NoticeSettings,
    UpdateNoticeSettings,
    RegisterNotificationToken,
    NotificationSettings,
    UpdateNotificationSettings,
    WeatherForecast,
    UpdateWeatherSettings,
    PredictionSettings,
    UpdatePredictionSettings,
    Supplies,
    Advertisements,
    Versions,
    ServiceSites,
    PurchaseSettings,
    UpdatePurchaseSettings,
    RemovePurchaseSettings,
    Courses,
    CommandDetails,
}

#[derive(Args)]
pub struct Options {
    /// Named API operation; run `api --help` for the complete list.
    #[arg(value_enum)]
    operation: Operation,
    /// Query or path parameter, repeatable (e.g. --param com_id=MODEL).
    #[arg(long = "param", value_name = "KEY=VALUE")]
    params: Vec<String>,
    /// JSON object to send, read from a file; never put tokens in shell arguments.
    #[arg(long, value_name = "FILE")]
    body: Option<PathBuf>,
    /// Validate and save the request without authentication or network access.
    #[arg(long)]
    dry_run: bool,
}

struct Endpoint {
    method: Method,
    path: &'static str,
    appliance: bool,
    required: &'static [&'static str],
    optional: &'static [&'static str],
    body: bool,
}

impl Operation {
    fn endpoint(self) -> Endpoint {
        use Operation::*;
        let (method, path, appliance, required, optional, body): (_, _, _, &[&str], &[&str], _) =
            match self {
                Login => (Method::POST, "users", false, &[], &[], true),
                UpdateUser => (Method::POST, "user", false, &[], &[], true),
                RegisterDevice => (Method::POST, "user/devices", false, &[], &[], true),
                RemoveDevice => (Method::DELETE, "user/devices/remove", true, &[], &[], false),
                MenuItems => (Method::GET, "device/menuitem", true, &[], &[], false),
                CustomerCourses => (
                    Method::GET,
                    "customer/course",
                    false,
                    &["com_id", "product_number"],
                    &[],
                    false,
                ),
                RegisterCourse => (Method::POST, "customer/course", false, &[], &[], true),
                DeleteCourse => (
                    Method::DELETE,
                    "customer/course",
                    false,
                    &["id"],
                    &[],
                    false,
                ),
                CustomView => (
                    Method::GET,
                    "custom/view",
                    false,
                    &[
                        "wash_dry_course_code",
                        "com_id",
                        "view_id",
                        "command_param_value",
                    ],
                    &[],
                    false,
                ),
                ContentVersions => (
                    Method::GET,
                    "contents/versions",
                    false,
                    &["com_id", "data_id"],
                    &[],
                    false,
                ),
                Notices => (
                    Method::GET,
                    "notices",
                    false,
                    &[],
                    &["language", "limit", "offset", "from_index"],
                    false,
                ),
                Notice => (Method::GET, "notices/{id}", false, &["id"], &[], false),
                NoticeSettings => (Method::GET, "user/notices/setting", true, &[], &[], false),
                UpdateNoticeSettings => {
                    (Method::POST, "user/notices/setting", true, &[], &[], true)
                }
                RegisterNotificationToken => {
                    (Method::POST, "user/notifications", false, &[], &[], true)
                }
                NotificationSettings => (
                    Method::GET,
                    "user/notifications/settings",
                    true,
                    &[],
                    &[],
                    false,
                ),
                UpdateNotificationSettings => (
                    Method::POST,
                    "user/notifications/settings",
                    true,
                    &[],
                    &[],
                    true,
                ),
                WeatherForecast => (Method::GET, "weatherinfo/forecast", true, &[], &[], false),
                UpdateWeatherSettings => {
                    (Method::POST, "weatherinfo/settings", true, &[], &[], true)
                }
                PredictionSettings => (
                    Method::GET,
                    "user/drytime_prediction/settings",
                    true,
                    &[],
                    &[],
                    false,
                ),
                UpdatePredictionSettings => (
                    Method::POST,
                    "user/drytime_prediction/settings",
                    true,
                    &[],
                    &[],
                    true,
                ),
                Supplies => (
                    Method::GET,
                    "service/supplies",
                    false,
                    &[],
                    &["supplies_type", "limit", "offset"],
                    false,
                ),
                Advertisements => (Method::GET, "advertisements", false, &[], &[], false),
                Versions => (
                    Method::GET,
                    "versions/{osType}/{appVersion}",
                    false,
                    &["osType", "appVersion"],
                    &[],
                    false,
                ),
                ServiceSites => (Method::GET, "service/sites", false, &[], &[], false),
                PurchaseSettings => (Method::GET, "user/purchase/settings", true, &[], &[], false),
                UpdatePurchaseSettings => {
                    (Method::POST, "user/purchase/settings", true, &[], &[], true)
                }
                RemovePurchaseSettings => (
                    Method::POST,
                    "user/purchase/settings/remove",
                    true,
                    &[],
                    &[],
                    false,
                ),
                Courses => (
                    Method::GET,
                    "laundry/courses/search/{comId}",
                    false,
                    &["comId"],
                    &[],
                    false,
                ),
                CommandDetails => (Method::POST, "laundry/command", false, &[], &[], true),
            };
        Endpoint {
            method,
            path,
            appliance,
            required,
            optional,
            body,
        }
    }

    fn name(self) -> String {
        self.to_possible_value()
            .expect("visible operation")
            .get_name()
            .to_owned()
    }
}

struct Prepared {
    method: Method,
    url: Url,
    body: Option<Value>,
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
    let body = options.body.as_deref().map(storage::read).transpose()?;
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
    })
}

fn validate_body(operation: Operation, body: &Value) -> Result<()> {
    use Operation::*;
    ensure!(
        body.is_object() && !body.as_object().unwrap().is_empty(),
        "Body must be a nonempty JSON object"
    );
    let required: &[(&str, &str)] = match operation {
        Login => &[("auth0_token", "string")],
        RegisterDevice => &[("appliance_id", "string")],
        RegisterCourse => &[("name", "string"), ("course_code", "string")],
        UpdateNoticeSettings => &[("settings", "object")],
        RegisterNotificationToken => &[("token", "string"), ("os_type", "string")],
        UpdateWeatherSettings => &[("location", "object")],
        UpdatePredictionSettings => &[("use_drytime_prediction", "boolean")],
        UpdatePurchaseSettings => &[("site_id", "string")],
        CommandDetails => &[
            ("product_number", "string"),
            ("com_id", "string"),
            ("cmds", "array"),
        ],
        _ => &[],
    };
    for (key, kind) in required {
        let value = &body[*key];
        let valid = match *kind {
            "string" => value.as_str().is_some_and(|s| !s.trim().is_empty()),
            "boolean" => value.is_boolean(),
            "object" => value.is_object(),
            "array" => value.is_array(),
            _ => false,
        };
        ensure!(valid, "Body requires {key} of type {kind}");
    }
    if matches!(operation, UpdateUser) {
        ensure!(
            body.get("tos_agreed_version").is_some() || body.get("stop_service").is_some(),
            "Supply tos_agreed_version or stop_service"
        );
        if let Some(v) = body.get("tos_agreed_version") {
            ensure!(
                v.as_str().is_some_and(|s| !s.is_empty()),
                "tos_agreed_version must be a nonempty string"
            );
        }
        if let Some(v) = body.get("stop_service") {
            ensure!(v.is_boolean(), "stop_service must be a boolean");
        }
    }
    if matches!(operation, RegisterNotificationToken) {
        ensure!(
            ["ios", "android"].contains(&body["os_type"].as_str().unwrap_or("")),
            "os_type must be ios or android"
        );
    }
    if matches!(operation, UpdateNotificationSettings) {
        ensure!(
            body.get("enabled").is_some() || body.get("types").is_some(),
            "Supply enabled or types"
        );
        if let Some(v) = body.get("enabled") {
            ensure!(v.is_boolean(), "enabled must be a boolean");
        }
        if let Some(v) = body.get("types") {
            ensure!(
                v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
                "types must be an array of strings"
            );
        }
    }
    if matches!(operation, UpdateWeatherSettings) {
        for (key, max) in [("latitude", 90.0), ("longitude", 180.0)] {
            ensure!(
                body["location"][key]
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n.abs() <= max),
                "Invalid location.{key}"
            );
        }
    }
    if matches!(operation, CommandDetails) {
        ensure!(
            body["cmds"].as_array().unwrap().iter().all(|cmd| {
                cmd["id"]
                    .as_str()
                    .is_some_and(|id| id.len() == 4 && id.bytes().all(|b| b.is_ascii_hexdigit()))
                    && cmd["params"].is_array()
            }),
            "cmds must use the observed id/params-array encoding"
        );
    }
    Ok(())
}

fn transmit(
    client: &Client,
    prepared: &Prepared,
    token: Option<&str>,
    appliance: Option<&str>,
) -> Result<(u16, Value)> {
    let timestamp = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).unwrap())
        .format("%Y%m%d%H%M%S%z")
        .to_string();
    let mut request = client
        .request(prepared.method.clone(), prepared.url.clone())
        .header("Accept", "application/json")
        .header("Content-Language", "ja_JP")
        .header("X-Timestamp", timestamp);
    if let Some(token) = token {
        request = request
            .bearer_auth(token)
            .header("Cookie", format!("atkn={token}"));
    }
    if let Some(id) = appliance {
        request = request.header("X-ApplianceId", id);
    }
    if let Some(body) = &prepared.body {
        request = request.json(body);
    }
    let response = request.send()?;
    let code = response.status().as_u16();
    let text = response.text()?;
    let data = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or_else(|_| json!({"non_json_response":text}))
    };
    Ok((code, data))
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
            "appliance_id":appliance, "body":prepared.body, "dry_run":options.dry_run
        }),
    )?;
    if options.dry_run {
        println!(
            "Dry run: {} {}. Request saved to .local/api/{name}-request.json; nothing sent.",
            prepared.method, prepared.url
        );
        return Ok(());
    }
    let token = if matches!(options.operation, Operation::Login) {
        None
    } else {
        let tokens = storage::read(&storage::root().join("auth/tokens.json"))
            .context("Login required: complete the browser OAuth flow first")?;
        Some(
            tokens["access_token"]
                .as_str()
                .filter(|s| !s.is_empty())
                .context("Missing access token")?
                .to_owned(),
        )
    };
    let client = api::control_client()?;
    let filename = format!("api/{name}-response.json");
    storage::save(
        &filename,
        &json!({"state":"pending", "automatic_retry":false}),
    )?;
    let result = transmit(&client, &prepared, token.as_deref(), appliance);
    let (code, data) = match result {
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
    storage::save(&filename, &json!({"http_status":code,"fetched_at":chrono::Utc::now().to_rfc3339(),"data":data,"automatic_retry":false}))
        .context("Request was sent but saving its response failed; check current state before retrying")?;
    check_response(code, &data)?;
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

    #[test]
    fn parameters_cannot_change_the_origin_or_escape_a_path_segment() {
        let prepared = prepare(
            &options(Operation::Notice, &["id=a/b?x=1&y=日本語"]),
            None,
            BASE,
        )
        .unwrap();
        assert_eq!(prepared.url.host_str(), Some("app.wad.apws.panasonic.com"));
        assert_eq!(prepared.url.query(), None);
        assert!(prepared.url.path().contains("a%2Fb%3Fx=1&y="));
        for value in ["id=.", "id=.."] {
            assert!(prepare(&options(Operation::Notice, &[value]), None, BASE).is_err());
        }
        let query = prepare(
            &options(
                Operation::CustomerCourses,
                &["com_id=a&evil=1", "product_number=A B"],
            ),
            None,
            BASE,
        )
        .unwrap();
        let pairs: BTreeMap<_, _> = query.url.query_pairs().collect();
        assert_eq!(pairs.len(), 2);
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
    fn mock_server_checks_get_post_delete_and_does_not_follow_redirects() {
        for (method, status, body) in [
            (Method::GET, 200, None),
            (Method::POST, 200, Some(json!({"enabled":false}))),
            (Method::DELETE, 204, None),
            (Method::POST, 307, Some(json!({"enabled":false}))),
            (Method::POST, 400, Some(json!({"enabled":false}))),
        ] {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let prepared = Prepared {
                method: method.clone(),
                url: Url::parse(&format!(
                    "http://{}/settings?id=a%26b",
                    server.server_addr()
                ))
                .unwrap(),
                body: body.clone(),
            };
            let worker = std::thread::spawn(move || {
                let mut request = server
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap()
                    .unwrap();
                assert_eq!(request.method().as_str(), method.as_str());
                assert_eq!(request.url(), "/settings?id=a%26b");
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
