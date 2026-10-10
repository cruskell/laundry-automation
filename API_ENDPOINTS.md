# Additional API endpoints

The `api` command exposes 31 named operations alongside the existing `devices`,
`info`, `status`, `start`, and analysis commands. The initial endpoint list is based
on `spec/openapi.yaml`, a **third-party reference**, not a Panasonic contract.
The corrected read operations below follow the official app source map and have
been checked against production. Other operations, especially writes, remain
unverified. Tests also use local mock servers.

## Usage

```sh
cargo run --locked -- api --help
cargo run --locked -- api notices --param limit=20 --param from_index=0
cargo run --locked -- --appliance-id DEVICE api notice --param id=NOTICE_ID
cargo run --locked -- --appliance-id DEVICE api customer-courses --param wash_dry_course_code=01 --param base_course=21
cargo run --locked -- api service-sites
cargo run --locked -- --appliance-id DEVICE api weather-forecast
cargo run --locked -- api notification-terminals
cargo run --locked -- --appliance-id DEVICE api notification-settings --param term_id=TERMINAL_ID
cargo run --locked -- --appliance-id DEVICE api notice-settings --param term_id=TERMINAL_ID
cargo run --locked -- api advertisements --param product_number=NA-LX127ER-W
```

Parameters must be supplied explicitly. Required appliance headers use the
existing global `--appliance-id`; no device is silently selected. `term_id` is
sent as `X-TermId` for the two settings reads, and
`product_number` as `X-Product-Number` for advertisements. Header parameters
are validated and excluded from the URL. Path and query values are URL encoded,
and unknown, duplicate, empty, and missing parameters
are rejected. `notices` accepts `from_index` as observed by the existing client,
and `offset` from the reference, but rejects using both together.

Updates take a JSON file. `--dry-run` validates and saves the request **without
network access or authentication**. Without `--dry-run`, the named operation is
sent once, except that `notice` may read multiple pages while searching. There
is no additional confirmation flag.

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
| `login` | POST `users` | Body: `auth0_token` string (unverified reference) | |
| `notification-terminals` | POST `users` | Built-in `{"type":0}` user-info request; returns `notification_term_ids` | |
| `update-user` | POST `user` | Body: `tos_agreed_version` string and/or `stop_service` boolean | |
| `register-device` | POST `user/devices` | Body: `appliance_id`; optionally registration/nickname/purchase fields from the reference | |
| `remove-device` | DELETE `user/devices/remove` | No body | Yes |
| `menu-items` | GET `device/menuitem` | None | Yes |
| `customer-courses` | GET `customer/course` | `wash_dry_course_code`, `base_course` | Yes |
| `register-course` | POST `customer/course` | Body: `name`, `course_code`, optional `settings` | |
| `delete-course` | DELETE `customer/course` | `id` | |
| `custom-view` | GET `custom/view` | `wash_dry_course_code`, `com_id`, `view_id`, `command_param_value` | |
| `content-versions` | GET `contents/versions` | `com_id`, `data_id` | |
| `notices` | GET `notices` | Optional `language`, `limit`, `offset` or `from_index` | |
| `notice` | GET `notices` (paginated lookup) | `id` to find in the list | Yes |
| `notice-settings` | GET `user/notices/setting` | `term_id` header parameter | Yes |
| `update-notice-settings` | POST `user/notices/setting` | Body: `settings` object | Yes |
| `register-notification-token` | POST `user/notifications` | Body: `token`, `os_type` (`ios` or `android`) | |
| `notification-settings` | GET `user/notifications/settings` | `term_id` header parameter | Yes |
| `update-notification-settings` | POST `user/notifications/settings` | Body: `enabled` boolean and/or `types` string array | Yes |
| `weather-forecast` | GET `weatherinfo/forecast` | None | Yes |
| `update-weather-settings` | POST `weatherinfo/settings` | Body: `location` with numeric `latitude` and `longitude` | Yes |
| `prediction-settings` | GET `user/drytime_prediction/settings` | None | Yes |
| `update-prediction-settings` | POST `user/drytime_prediction/settings` | Body: `use_drytime_prediction` boolean | Yes |
| `supplies` | GET `service/supplies` | Optional `supplies_type` (`detergent` or `softener`), `limit`, `offset` | |
| `advertisements` | GET `advertisements` | `product_number` header parameter | |
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

## Verified read corrections (2026-10-10)

The official [app source map](https://cot.wad.apws.panasonic.com/FY26_2nd/static/js/main.d8aa4ffb.js.map)
contains `customerCourseAPI.js`, `noticeAPI.js`, `notificationAPI.js`,
`noticesSettingAPI.js`, and `loginAPI.js`. These establish the corrected parameters,
headers, and user-info request. With the registered NA-LX127ER-W and one existing
notification terminal, all five previously failing reads returned HTTP 200:

- `customer-courses`: both course parameters and `X-ApplianceId`; the list was empty.
- `advertisements`: `X-Product-Number`; the list was empty.
- `notice`: list lookup returned the requested notification with `read_state` unchanged.
- `notice-settings`: `X-TermId` plus `X-ApplianceId`; returned `pana_notice_setting`.
- `notification-settings`: the same headers; returned `all_status`, `categories`, and `items`.

`notification-terminals` returned one registered terminal. It follows the app's
`POST /users` with `type: 0`, used to fetch current user information, and retains
only `notification_term_ids` from the response. It does not register a terminal.
The implementation does not assert that every POST is an update.

The apparent `GET /notices/{id}` reference is incorrect. The official app uses
`POST /notices/{id}` to mark a notice read. The `notice` CLI command instead
searches `GET /notices?limit=100&from_index=...`, returning `{"notice":...,"read_only":true}`.
It never calls the mark-read endpoint. A missing ID returns a nonzero exit; searches
stop after the reported total, an empty page, or 100 pages. Notification settings
require an explicit registered terminal ID, even when the account has just one.
