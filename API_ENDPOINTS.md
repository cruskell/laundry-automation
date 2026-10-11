# Additional API endpoints

The `api` command exposes 38 operations alongside `devices`, `info`, `status`,
`start`, and the existing analysis commands. The implementation follows the
[official app source map](https://cot.wad.apws.panasonic.com/FY26_2nd/static/js/main.d8aa4ffb.js.map).
`spec/openapi.yaml` now reflects these observed contracts (29 paths / 35 HTTP
operations). The 38 CLI operations and their request schemas are generated from it;
see README.md for regeneration. Verification is recorded per operation with
`x-verification`; writes have not been executed against production.

## Usage and result handling

```sh
cargo run --locked -- api --help
cargo run --locked -- api user-info
cargo run --locked -- api notification-terminals
cargo run --locked -- api notices --param limit=20 --param from_index=0
cargo run --locked -- --appliance-id DEVICE api notice --param id=NOTICE_ID
cargo run --locked -- --appliance-id DEVICE api customer-courses \
  --param wash_dry_course_code=01 --param base_course=21
cargo run --locked -- --appliance-id DEVICE api notification-settings --param term_id=TERMINAL_ID
cargo run --locked -- api advertisements --param product_number=NA-LX127ER-W
cargo run --locked -- api content-versions --param com_id=NA-LX127E --param data_id=02
```

All operations use existing Bearer/`atkn` credentials from `.local/auth/tokens.json`.
`user-info` and `notification-terminals` perform the app's user-info POST with
`{"type":0}`. `login` accepts `{"type":0}` for user info or `{"type":1}` for
login, with the same authentication. These service calls do not replace OAuth
credentials; use `login-bridge` and `refresh-token` for credential management.

Parameters are explicit. Required appliance headers use global `--appliance-id`.
`term_id` becomes `X-TermId` for settings reads, and advertisement `product_number`
becomes `X-Product-Number`; header parameters are validated and omitted from the
URL. Path and query values are URL encoded. Unknown, duplicate, empty, or missing
parameters fail locally. Body appliance IDs must agree with a supplied global ID.
`notices` supports `offset` from the reference or `from_index` from the official
app, but not both together.

Operations with custom payloads require `--body FILE`, containing a JSON object.
`--dry-run` validates and saves the request with no authentication or network
access. Without it, invoking an operation sends the request. There is no extra
execution flag. Writes are never automatically retried or redirected. `notice`
can read multiple pages; control operations also fetch device/capability metadata
before sending one control request.

Private requests/responses are saved in `.local/api/<operation>-request.json`
and `<operation>-response.json`, replacing previous records for that operation.
They can contain authorization codes, push tokens, location, and account data.
HTTP errors, non-JSON replies, and recognized error/status fields produce a nonzero
exit. Transport failures record uncertain delivery; check current state before
retrying a write. Control success additionally requires a matching appliance,
command type, sequence, `cmd_result=true`, and successful appliance/API status
codes. API acceptance does not independently establish physical operation.

## Operation reference

Paths are relative to `/laundry/v5/`. “Device” means `--appliance-id` is required.
A supplied appliance header is also sent for other operations.

| Operation | Method / path | Parameters or body | Device |
|---|---|---|---|
| `login` | POST `users` | Body: numeric `type` (0 or 1) | |
| `user-info` | POST `users` | Generated body `{"type":0}` | |
| `notification-terminals` | POST `users` | Generated body `{"type":0}`; retains `notification_term_ids` | |
| `update-user` | POST `user` | Body: `setting_type` and mode-specific fields | |
| `register-device` | POST `user/devices` | Body: registration fields below | |
| `remove-device` | POST `user/devices/remove` | Generated `appliance_ids` array from selected device | Yes |
| `menu-items` | GET `device/menuitem` | None; returns purchase-area/shop-category choices | Yes |
| `customer-courses` | GET `customer/course` | `wash_dry_course_code`, `base_course` | Yes |
| `register-course` | POST `customer/course` | Body: course envelope, `api_type=1` | Yes |
| `edit-course` | POST `customer/course` | Body: course envelope, `api_type=2` | Yes |
| `delete-course` | POST `customer/course` | Body: course envelope, `api_type=3` | Yes |
| `custom-view` | GET `custom/view` | `wash_dry_course_code`, `com_id`, `view_id`, `command_param_value` | |
| `content-versions` | GET `contents/versions` | `com_id`, `data_id` (`02` for download sequence versions) | |
| `notices` | GET `notices` | Optional `language`, `limit`, `offset` or `from_index` | |
| `notice` | GET `notices` (paginated lookup) | `id` to find | Yes |
| `mark-notice-read` | POST `notices/{id}` | `id`; generated `{"read_state":true}` | Yes |
| `notice-settings` | GET `user/notices/setting` | `term_id` header parameter | Yes |
| `update-notice-settings` | POST `user/notices/setting` | Body: `term_id`, `appliance_id`, `pana_notice_setting` | Yes |
| `register-notification-token` | POST `user/notifications` | Body: `reg_id`, numeric `type`, `device_id` | |
| `notification-settings` | GET `user/notifications/settings` | `term_id` header parameter | Yes |
| `update-notification-settings` | POST `user/notifications/settings` | Body: terminal/device, `all_status`, `items`, `pana_notice_setting` | Yes |
| `weather-forecast` | GET `weatherinfo/forecast` | None | Yes |
| `update-weather-settings` | POST `weatherinfo/settings` | Body: locale/region information below | Yes |
| `prediction-settings` | GET `user/drytime_prediction/settings` | None | Yes |
| `update-prediction-settings` | POST `user/drytime_prediction/settings` | Body: boolean `use_drytime_prediction` | Yes |
| `supplies` | GET `service/supplies` | Optional `supplies_type`, `limit`, `offset` | |
| `advertisements` | GET `advertisements` | `product_number` header parameter | |
| `versions` | GET `versions/{osType}/{appVersion}` | `osType` (`ios` or `android`), `appVersion` | |
| `service-sites` | GET `service/sites` | None | |
| `purchase-settings` | GET `user/purchase/settings` | None | Yes |
| `update-purchase-settings` | POST `user/purchase/settings` | Body: `site_id` | Yes |
| `remove-purchase-settings` | POST `user/purchase/settings/remove` | No body | Yes |
| `courses` | GET `laundry/courses/search/{comId}` | `comId` | |
| `command-details` | POST `laundry/command` | Body: `product_number`, `com_id`, `cmds`; metadata only | |
| `amazon-account-link` | POST `amazon/account/link` | Body: `author_code`, `appliance_id` | Yes |
| `amazon-skill-enable` | POST `amazon/skill/enable` | Body: `author_code`, `appliance_id` | Yes |
| `control-read` | POST `device/control` | Body: type-03 ECHONET request | Yes |
| `control-write` | POST `device/control` | Body: type-02 ECHONET request | Yes |

The account-link operation needs an Amazon authorization code; skill enablement
needs the corresponding Auth0 authorization code. Obtaining those codes is an
external account-linking flow; the CLI implements submission, not browser OAuth.

## User, notification, and region payloads

`update-user` uses numeric `setting_type` from the official app:

| Value | Operation | Additional required fields |
|---|---|---|
| 1 | Terms agreement | `tos_agreed_version` string |
| 2 | Change selected appliance | `appliance_id` string |
| 3 | Suspend service | `tos_agreed_version` string |
| 4 | Record read information | `read_message_ids` string array |
| 5 | Acknowledge cloud-reservation information | None |
| 6 | Last login / sleep setting | `term_id`, `app_sleep_setting` (native-provided value) |

Examples of body files (dummy identifiers):

```json
{"setting_type":2,"appliance_id":"DEVICE"}
```

```json
{"reg_id":"PUSH_TOKEN","type":1,"device_id":"PHONE_ID"}
```

Notification token type is 1 for Android, 2 for iOS. Settings updates use booleans:

```json
{"term_id":"TERMINAL_ID","appliance_id":"DEVICE","pana_notice_setting":false}
```

```json
{"term_id":"TERMINAL_ID","appliance_id":"DEVICE","all_status":true,"items":[{"id":"ITEM_ID","status":true}],"pana_notice_setting":false}
```

Additional item fields such as `category_id` and `name` are preserved. Region
settings use codes and names from the weather metadata/region catalog:

```json
{"locale_id":"ja_JP","region_id":"REGION_CODE","sub_region_1":{"PREFECTURE_CODE":"PREFECTURE_NAME"},"sub_region_2":{"CITY_CODE":"CITY_NAME"}}
```

The former latitude/longitude body is not the official app's region-setting
format. Device registration accepts `appliance_id`, `com_id`, `hashed_guid`,
`nickname`, `product_code`, and a `purchase_info` object; purchase fields include
`purchase_date`, `shop_category_id`, `shop_area_id`, `shop_name`, and `memo`.

## Course add/edit/delete

All three operations use a full course envelope, not a DELETE query:

```json
{
  "wash_dry_course_code":"01",
  "appliance_id":"DEVICE",
  "api_type":2,
  "view_id":"11310",
  "base_course":"BASE_COURSE_VALUE",
  "courses":[{"id":"COURSE_ID","name":"Example","cmds":[{"id":"00D0","params":[{"value":"21"}]}]}]
}
```

Choose `api_type` 1/2/3 to match `register-course`/`edit-course`/`delete-course`.
The official app uses view `11300` for addition and `11310` for editing/deletion.
Use the actual base-course value from the course data, including its submode.
Editing/deleting requires course IDs; adding/editing requires names and command
arrays. Course configuration uses `params` objects containing `value`, unlike
raw device-control writes, which use arrays of hex strings.

## Device-control requests

`control-read` and `control-write` accept one explicit appliance request. Before
sending, the client fetches the registered device and current model capabilities
and checks each ID against `get` or `set`. Offline dry runs validate only the
request structure, not current capabilities. Reads have empty parameters:

```json
{"requests":[{"appliance_id":"DEVICE","cmd_type":"03","cmds":[{"id":"0140","params":[]},{"id":"01F0","params":[]}],"protocol":"echonetlite","seq_no":1}]}
```

Example pause request for `control-write`:

```json
{"requests":[{"appliance_id":"DEVICE","cmd_type":"02","cmds":[{"id":"00B2","params":["42"]}],"protocol":"echonetlite","seq_no":1}]}
```

Stop uses `00B2=43`. Other supported settings, reservation, and firmware commands
can be submitted using their known property IDs and encoded values. This is raw
transport, not a high-level encoder for every setting or firmware operation.
Writes require nonempty arrays of even-length hex strings. Unknown IDs, duplicate
IDs, malformed values, and appliance mismatches fail before control transmission.
`00B2=41` starts/resumes operation and is rejected here; use existing guarded
`start` for the implemented normal wash-and-dry start. No raw control command is
retried automatically. Physical state and command-specific semantics are not
validated for arbitrary raw settings; supply values supported by the appliance.

`notice` remains read-only, searching pages of 100 until it finds the ID, reaches
the total/empty page, or reads 100 pages. It returns `{"notice":...,"read_only":true}`.
Only `mark-notice-read` writes the notification's read state.

## Validation and coverage

All 21 reference operations were executed with the registered NA-LX127ER-W and
existing notification terminal and returned HTTP 200:

`user-info`, `login` with `type:0`, `notification-terminals`, `control-read`,
`menu-items`, `customer-courses`, `custom-view`, `content-versions`, `notices`,
`notice`, `notice-settings`, `notification-settings`, `weather-forecast`,
`prediction-settings`, `supplies`, `advertisements`, `versions`, `service-sites`,
`purchase-settings`, `courses`, and `command-details`.

Download content `data_id=02` returned 11 versions; notification settings returned
22 items. Custom courses and advertisements were valid empty lists. Control read
returned successful, matching acknowledgements for 0140/01F0. Raw responses and a
summary are stored privately in `.local/api/read-coverage-verification.json`.
The source for `data_id=02` is `CourseDownload.js` and
`EdtDownloadContentsSetting.sequence` in `models/Edt.js`.

Updates, deletions, account linking, `login type:1`, and physical control writes
were not executed against production. Local mock tests verify their HTTP methods,
authentication, appliance headers, body encoding, single transmission, and error
handling. User-mode validation, model command support, control acknowledgements,
and read-only notification lookup have regression tests. Legacy v4 functions
commented out in the official app are not implemented; native-only and unknown
server endpoints are outside the audited coverage.
