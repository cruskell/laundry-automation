# Additional API endpoints

The `api` command exposes 30 named operations alongside the existing `devices`,
`info`, `status`, `start`, and analysis commands. These new endpoints are based
on `spec/openapi.yaml`, a **third-party reference**, not a verified Panasonic
contract. Their production behavior has not been tested. Tests use a local mock
server; implementation does not imply successful appliance or account changes.

## Usage

```sh
cargo run --locked -- api --help
cargo run --locked -- api notices --param limit=20 --param from_index=0
cargo run --locked -- api notice --param id=NOTICE_ID
cargo run --locked -- api customer-courses --param com_id=MODEL --param product_number=PRODUCT
cargo run --locked -- api service-sites
cargo run --locked -- --appliance-id DEVICE api weather-forecast
cargo run --locked -- --appliance-id DEVICE api notification-settings
```

Parameters must be supplied explicitly. Required appliance headers use the
existing global `--appliance-id`; no device is silently selected. Path and query
values are URL encoded, and unknown, duplicate, empty, and missing parameters
are rejected. `notices` accepts `from_index` as observed by the existing client,
and `offset` from the reference, but rejects using both together.

Updates take a JSON file. `--dry-run` validates and saves the request **without
network access or authentication**. Without `--dry-run`, the named operation is
sent once. There is no additional confirmation flag.

```sh
# Create .local/prediction-update.json containing:
# {"use_drytime_prediction": true}
cargo run --locked -- --appliance-id DEVICE api update-prediction-settings \
  --body .local/prediction-update.json --dry-run
# To submit, run the same command without --dry-run.
```

Requests and responses are saved privately in `.local/api/<operation>-request.json`
and `<operation>-response.json`. They may include tokens, location, or account
information. Transport failures record uncertain delivery; inspect current
settings before retrying a write. Retries and redirects are disabled. HTTP
errors, non-JSON responses, and recognized API error/status fields cause a
nonzero exit. Unknown API response semantics are not asserted as success:
output reports the HTTP result, not confirmed physical execution. A new request
replaces the previous files for the same operation.

All operations except `login` use `.local/auth/tokens.json` and the existing
Bearer/`atkn` authentication. `login` sends `auth0_token` from its body file to
`users`; it saves the service response but does not replace OAuth credentials.
Use `login-bridge` for the existing OAuth login flow.

## Operation reference

Paths below are relative to `/laundry/v5/`. “Device” requires `--appliance-id`.
A supplied appliance ID is also sent for operations where it is optional.

| Operation | Method / path | Parameters or body | Device |
|---|---|---|---|
| `login` | POST `users` | Body: `auth0_token` string | |
| `update-user` | POST `user` | Body: `tos_agreed_version` string and/or `stop_service` boolean | |
| `register-device` | POST `user/devices` | Body: `appliance_id`; optionally registration/nickname/purchase fields from the reference | |
| `remove-device` | DELETE `user/devices/remove` | No body | Yes |
| `menu-items` | GET `device/menuitem` | None | Yes |
| `customer-courses` | GET `customer/course` | `com_id`, `product_number` | |
| `register-course` | POST `customer/course` | Body: `name`, `course_code`, optional `settings` | |
| `delete-course` | DELETE `customer/course` | `id` | |
| `custom-view` | GET `custom/view` | `wash_dry_course_code`, `com_id`, `view_id`, `command_param_value` | |
| `content-versions` | GET `contents/versions` | `com_id`, `data_id` | |
| `notices` | GET `notices` | Optional `language`, `limit`, `offset` or `from_index` | |
| `notice` | GET `notices/{id}` | `id` | |
| `notice-settings` | GET `user/notices/setting` | None | Yes |
| `update-notice-settings` | POST `user/notices/setting` | Body: `settings` object | Yes |
| `register-notification-token` | POST `user/notifications` | Body: `token`, `os_type` (`ios` or `android`) | |
| `notification-settings` | GET `user/notifications/settings` | None | Yes |
| `update-notification-settings` | POST `user/notifications/settings` | Body: `enabled` boolean and/or `types` string array | Yes |
| `weather-forecast` | GET `weatherinfo/forecast` | None | Yes |
| `update-weather-settings` | POST `weatherinfo/settings` | Body: `location` with numeric `latitude` and `longitude` | Yes |
| `prediction-settings` | GET `user/drytime_prediction/settings` | None | Yes |
| `update-prediction-settings` | POST `user/drytime_prediction/settings` | Body: `use_drytime_prediction` boolean | Yes |
| `supplies` | GET `service/supplies` | Optional `supplies_type` (`detergent` or `softener`), `limit`, `offset` | |
| `advertisements` | GET `advertisements` | None | |
| `versions` | GET `versions/{osType}/{appVersion}` | `osType` (`ios` or `android`), `appVersion` | |
| `service-sites` | GET `service/sites` | None | |
| `purchase-settings` | GET `user/purchase/settings` | None | Yes |
| `update-purchase-settings` | POST `user/purchase/settings` | Body: `site_id` | Yes |
| `remove-purchase-settings` | POST `user/purchase/settings/remove` | No body | Yes |
| `courses` | GET `laundry/courses/search/{comId}` | `comId` | |
| `command-details` | POST `laundry/command` | Body: `product_number`, `com_id`, `cmds` | |

`command-details` queries course command metadata; it does not start a cycle.
It uses the existing client's observed `cmds` format: each entry has a four-digit
hex `id` and a `params` array. The reference's `value` encoding is not used.

The `api` command intentionally does not expose unrestricted `device/control`
writes. Existing `start` remains the supported, guarded normal wash-and-dry
operation. JSON bodies are checked for core required fields and selected value
types, not against a complete production schema. Unknown additional body fields
are preserved to support model-specific payloads; use the reference and actual
service responses to determine them.
