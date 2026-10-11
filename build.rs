fn main() {
    println!("cargo:rerun-if-changed=spec/openapi.yaml");
    let input = std::fs::File::open("spec/openapi.yaml").unwrap();
    let raw: serde_json::Value = serde_yaml::from_reader(input).unwrap();
    // Progenitor 0.11 cannot combine a typed 200 and a differently typed default.
    // Restrict the default error decoder to error ranges in the generation input;
    // unexpected statuses are preserved by the adapter, and the source spec stays intact.
    let mut generation_input = raw.clone();
    for item in generation_input["paths"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        for operation in item.as_object_mut().unwrap().values_mut() {
            let responses = operation["responses"].as_object_mut().unwrap();
            if let Some(error) = responses.remove("default") {
                responses.insert("4XX".into(), error.clone());
                responses.insert("5XX".into(), error);
            }
        }
    }
    let spec = serde_json::from_value(generation_input).unwrap();
    let mut settings = progenitor::GenerationSettings::default();
    settings.with_interface(progenitor::InterfaceStyle::Builder);
    // Preserve unknown/model-specific response fields and error payloads.
    for item in raw["paths"].as_object().unwrap().values() {
        for operation in item.as_object().unwrap().values() {
            for response in operation["responses"].as_object().unwrap().values() {
                if let Some(reference) =
                    response["content"]["application/json"]["schema"]["$ref"].as_str()
                {
                    let name = reference.rsplit('/').next().unwrap();
                    settings.with_replacement(name, "serde_json::Value", std::iter::empty());
                }
            }
        }
    }

    let mut generator = progenitor::Generator::new(&settings);
    let tokens = generator
        .generate_tokens(&spec)
        .expect("OpenAPI client generation failed");
    let ast = syn::parse2(tokens).unwrap();
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("panasonic_api.rs"), prettyplease::unparse(&ast)).unwrap();
    generate_cli(&raw, &output);
}

fn literal(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

fn generate_cli(spec: &serde_json::Value, output: &std::path::Path) {
    use heck::ToSnakeCase;
    let mut variants = String::new();
    let mut endpoints = String::new();
    let mut bodies = String::new();
    let mut validation = String::new();
    let mut dispatch = String::new();
    for (path, item) in spec["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            let parameters = operation["parameters"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let request_schema = operation["requestBody"]["content"]["application/json"]["schema"]
                ["$ref"]
                .as_str()
                .map(|r| r.rsplit('/').next().unwrap());
            for alias in operation["x-cli-operations"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let variant = alias["variant"].as_str().unwrap();
                variants += &format!(
                    "#[value(name = {})] {variant},",
                    literal(alias["name"].as_str().unwrap())
                );
                let mut params = parameters
                    .iter()
                    .filter(|p| p["name"] != "X-ApplianceId")
                    .cloned()
                    .collect::<Vec<_>>();
                params.extend(
                    alias["extra_params"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default(),
                );
                if let Some(only) = alias["only_params"].as_array() {
                    params.retain(|p| only.contains(&p["name"]));
                }
                let mut required = Vec::new();
                let mut optional = Vec::new();
                let mut headers = Vec::new();
                for param in params {
                    let name = param["x-cli-name"]
                        .as_str()
                        .unwrap_or_else(|| param["name"].as_str().unwrap());
                    if param["required"] == true {
                        required.push(literal(name));
                    } else {
                        optional.push(literal(name));
                    }
                    if param["in"] == "header" {
                        headers.push(format!(
                            "({}, {})",
                            literal(name),
                            literal(param["name"].as_str().unwrap())
                        ));
                    }
                }
                let appliance = alias["appliance"] == true
                    || parameters
                        .iter()
                        .any(|p| p["name"] == "X-ApplianceId" && p["required"] == true);
                let body = request_schema.is_some() && alias.get("generated_body").is_none();
                endpoints += &format!(
                    "Self::{variant} => Endpoint {{ method: reqwest::Method::{}, path: {}, appliance: {appliance}, required: &[{}], optional: &[{}], body: {body}, headers: &[{}] }},",
                    method.to_uppercase(),
                    literal(path.strip_prefix("/laundry/v5/").unwrap()),
                    required.join(","),
                    optional.join(","),
                    headers.join(",")
                );
                if let Some(body) = alias.get("generated_body") {
                    bodies += &format!(
                        "Self::{variant} => serde_json::from_str({}).expect(\"generated body\"),",
                        literal(&body.to_string())
                    );
                }
                if let Some(schema) = alias["request_schema"].as_str().or(request_schema) {
                    validation += &format!(
                        "Self::{variant} => {{ let _: crate::panasonic_api::types::{schema} = serde_json::from_value(body.clone())?; {} }},",
                        literal(schema)
                    );
                }
            }
            let mut pattern = Vec::new();
            for segment in path.trim_start_matches('/').split('/') {
                if segment.starts_with('{') {
                    pattern.push(segment.trim_matches(['{', '}']).to_snake_case());
                } else {
                    pattern.push(literal(segment));
                }
            }
            dispatch += &format!(
                "({}, [{}]) => {{ let {}request = client.{}();",
                literal(&method.to_uppercase()),
                pattern.join(","),
                if parameters.is_empty() && request_schema.is_none() {
                    ""
                } else {
                    "mut "
                },
                operation["operationId"].as_str().unwrap().to_snake_case()
            );
            for param in parameters {
                let name = param["name"].as_str().unwrap();
                let setter = name.to_snake_case();
                if param["in"] == "path" {
                    dispatch += &format!("request = request.{setter}(*{setter});");
                    continue;
                }
                let source = if name == "X-ApplianceId" {
                    "appliance".into()
                } else if param["in"] == "header" {
                    format!("headers.get({}).map(String::as_str)", literal(name))
                } else {
                    format!("query.get({}).map(String::as_str)", literal(name))
                };
                let converted = match param["schema"]["type"].as_str() {
                    Some("integer") => "value.parse::<u64>()?",
                    Some("boolean") => "value.parse::<bool>()?",
                    _ => "value",
                };
                dispatch += &format!(
                    "if let Some(value) = {source} {{ request = request.{setter}({converted}); }}"
                );
            }
            if let Some(schema) = request_schema {
                dispatch += &format!(
                    "let typed: crate::panasonic_api::types::{schema} = serde_json::from_value(body.context(\"Missing request body\")?.clone())?; request = request.body(typed);"
                );
            }
            dispatch += "request.send().await },";
        }
    }
    let source = format!(
        r#"
use clap::ValueEnum;
use serde_json::Value;
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Operation {{ {variants} }}
pub struct Endpoint {{
    pub method: reqwest::Method, pub path: &'static str, pub appliance: bool,
    pub required: &'static [&'static str], pub optional: &'static [&'static str],
    pub body: bool, pub headers: &'static [(&'static str, &'static str)],
}}
impl Operation {{
    pub fn endpoint(self) -> Endpoint {{ match self {{ {endpoints} }} }}
    pub fn name(self) -> String {{ self.to_possible_value().expect("visible operation").get_name().to_owned() }}
    pub fn generated_body(self, appliance: Option<&str>) -> Option<Value> {{
        let mut body = match self {{ {bodies} _ => return None }};
        fn substitute(value: &mut Value, appliance: Option<&str>) {{
            match value {{
                Value::String(s) if s == "$appliance_id" => *s = appliance.expect("validated appliance").to_owned(),
                Value::Array(items) => items.iter_mut().for_each(|v| substitute(v, appliance)),
                Value::Object(fields) => fields.values_mut().for_each(|v| substitute(v, appliance)),
                _ => {{}}
            }}
        }}
        substitute(&mut body, appliance); Some(body)
    }}
    pub fn validate_request(self, body: &Value) -> anyhow::Result<()> {{
        let schema = match self {{ {validation} _ => return Ok(()) }};
        crate::schema_validation::validate(schema, body)
    }}
}}
pub async fn dispatch(
    client: &crate::panasonic_api::Client, method: &reqwest::Method, url: &url::Url,
    body: Option<&Value>, headers: &std::collections::BTreeMap<String, String>, appliance: Option<&str>,
) -> anyhow::Result<(u16, Value)> {{
    use anyhow::Context;
    let segments = url.path_segments().context("Missing URL segments")?
        .map(|s| percent_encoding::percent_decode_str(s).decode_utf8().map(|s| s.into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let parts = segments.iter().map(String::as_str).collect::<Vec<_>>();
    let query = url.query_pairs().map(|(k,v)| (k.into_owned(),v.into_owned())).collect::<std::collections::BTreeMap<_,_>>();
    let result = match (method.as_str(), parts.as_slice()) {{ {dispatch} _ => anyhow::bail!("Unknown OpenAPI operation") }};
    match result {{
        Ok(response) => Ok((response.status().as_u16(), response.into_inner())),
        Err(progenitor_client::Error::ErrorResponse(response)) => Ok((response.status().as_u16(), response.into_inner())),
        Err(progenitor_client::Error::UnexpectedResponse(response)) => {{
            let code = response.status().as_u16();
            let text = response.text().await?;
            let data = if text.trim().is_empty() {{ Value::Null }} else {{ serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({{"non_json_response":text}})) }};
            Ok((code, data))
        }}
        Err(_) => anyhow::bail!("Generated API client failed; check delivery before retrying"),
    }}
}}
"#
    );
    let ast = syn::parse_file(&source).expect("CLI adapter syntax");
    std::fs::write(output.join("cli_api.rs"), prettyplease::unparse(&ast)).unwrap();
    std::fs::write(
        output.join("api_schemas.json"),
        serde_json::to_string(&spec["components"]["schemas"]).unwrap(),
    )
    .unwrap();
}
