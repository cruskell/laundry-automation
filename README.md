# Laundry automation

Rust CLI for reading, decoding, and sending a normal wash-and-dry start through the Panasonic SmartLaundry cloud API.
The Rust client retrieved device-list and status data from NA-LX127ER-W with
HTTP 200 on 2026-10-09. Requires Rust 1.86 or later; dependencies are pinned in
`Cargo.lock`.

## Read device data

```sh
cargo run --locked -- devices
cargo run --locked -- info
cargo run --locked -- status
```

`status` saves the raw response to `.local/status.json`, the decoded response to
`.local/decoded-status.json`, and prints a summary. It requests fresh data using
`X-Cached: false`. `fetched_at` is the retrieval time, not the appliance measurement
time. The `start` command below sends an appliance control command.

Decode a saved snapshot without network access:

```sh
cargo run --locked -- decode
cargo run --locked -- decode --snapshot /path/to/status.json
```

Select an appliance explicitly when needed:

```sh
cargo run --locked -- status --appliance-id YOUR_APPLIANCE_ID
```

The decoder dictionary is specific to NA-LX127ER-W / NA-LX127E. Do not apply its
labels to unrelated models.

## Start normal wash-and-dry

Run these commands from the repository. The default course is normal wash-and-dry (21); wash-only and custom-course starts are not implemented.

```sh
# Read live conditions and generate the request without sending a write:
cargo run --locked -- start --dry-run
# Send the start request using current dosing/tank settings:
cargo run --locked -- start
```

The command fetches the registered device and current model capabilities, then reads power, door, fault, remote permission, operation phase, reservation, and dosing/tank settings directly with command type 03. It sends command type 02 only when the device is on, the door is closed, faults/reservations are absent, remote operation is permitted, and the machine is stopped in pre-operation standby. Unsupported or unknown settings fail before the write.

Private files: `.local/start-preflight.json`, `start-request.json`, `start-response.json`, and `start-attempt.json`. A request template is not evidence of delivery. The attempt record distinguishes API acceptance from uncertain delivery or unconfirmed acceptance; it is not a live-process indicator. The CLI validates the HTTP result, appliance/type/sequence, command result, and both API status codes. API acceptance does not independently confirm physical operation. Automatic transport retries and redirects are disabled for control requests. Following a timeout or unconfirmed response, check machine state before manually retrying.

Transmission was verified against a loopback mock server on 2026-10-10. No real start was sent during implementation. Existing `plan-start` remains an offline, non-sending command. See `START_API.md` for encoding and evidence.

## Additional API endpoints

`cargo run --locked -- api --help` lists 38 additional operations for notices,
notification settings, weather, prediction settings, purchase settings, device
registration/removal, and customer courses. Read operations accept explicit
`--param KEY=VALUE` arguments; updates accept `--body FILE`. All operations
support offline `--dry-run`. See [API_ENDPOINTS.md](API_ENDPOINTS.md) for the
complete list, examples, production-verified read corrections, and remaining
contract limitations. `api notification-terminals` retrieves registered terminal
IDs for the two notification settings reads. Device removal, notice read-state
updates, course add/edit/delete, Amazon/Alexa linking, and raw model-supported
control reads/writes follow the official app request formats. All 21 reference
operations have been checked against production; writes use mock-server tests.

## Authentication

Existing locally stored OAuth tokens remain usable after the Rust migration.
For a fresh login:

```sh
cargo run --locked -- login-bridge
```

Execute the function in `browser_login.js` through the local Playwright browser
tool. Sign in to CLUB Panasonic directly in the browser. The response handler
captures the registered Android OAuth callback and relays it to the loopback
Rust bridge. The bridge validates the state and exchanges the code using PKCE.
The JavaScript file is only browser integration; the client, decoder, dictionary
extractor, and authentication bridge are Rust.

Keep the bridge and browser session open until authentication completes, then
stop the bridge. Expired tokens currently require a fresh login. The Rust bridge
was checked locally; its token exchange still needs a fresh end-to-end browser
login to verify it.

Credentials and raw appliance data are stored in `.local/`, excluded from Git,
with owner-only file permissions. Do not share that directory.

## Decoding

`spec/property-map.json` contains 91 definitions extracted from official metadata
and the app bundle. The observed 83-property response has 68 known names and 64
interpreted values/structures, including 7 partial and 2 inferred interpretations (overlapping counts).
Unknown names and values retain their original bytes.

See [DECODING.md](DECODING.md) for property details, evidence, and limitations.
Regenerate the dictionary from downloaded official data:

```sh
cargo run --locked -- info
cargo run --locked -- extract-map
```

The extractor expects the inspected bundle in `.local/app.js`; the dictionary
records its URL and SHA-256. Rebuild after changing the dictionary, since it is
embedded in the executable.

## Validation

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

## API reference

`spec/openapi.yaml` is a third-party reference from
[s-zaizen/panasonic-smartlaundry-api](https://github.com/s-zaizen/panasonic-smartlaundry-api),
with its license in `spec/LICENSE`. It is not an official Panasonic specification.
Observed differences include the `atkn` cookie, timestamp headers, `product_code`
in device lists, and no trailing slash on the status endpoint.

The HTTP client identifies itself with `laundry-automation/0.1.0`. Omitting the
User-Agent initially produced HTTP 403; adding it resolved the request.

## Further protocol analysis

See `START_API.md` for the observed immediate wash-and-dry control request. `cargo run -- plan-start` generates an offline request; it does not send it. `cargo run -- catalogs` retrieves official read-only supply/course/version/customization catalogs. Supply code names are saved in `spec/supply-codes.json` from the public service catalog.

`cargo run -- inspect --baseline PATH` produces private byte-level comparisons for unresolved/partial/inferred fields. Successful status reads are archived in `.local/snapshots/`. The decoder currently interprets 64 of 83 captured properties, with partial and inferred fields counted explicitly. See `DECODING.md` for the report layout evidence and remaining unknowns.

`cargo run -- explore-models` reads official published-model metadata. `cargo run -- source-audit` compares that metadata and an official source-map file with captured properties. `cargo run -- live-read` sends only type-03 empty-parameter reads for eight model-supported properties. See `SOURCE_AUDIT.md` for evidence and limits.

`cargo run -- probe-unmapped --id ID` attempts a read-only request for one captured unresolved ID and preserves its response. The tested IDs 0201 and 010A were rejected with HTTP 400. See `SOURCE_AUDIT.md`; repeated identical probes do not establish payload semantics.

## Refresh credentials

Run `target/release/laundry-automation refresh-token` to exchange the stored refresh token. The response replaces credentials only after validation; a rotated refresh token is retained, and an omitted refresh token preserves the previous one. Refresh requests disable automatic retries. The daily-start script refreshes credentials before each guarded start and aborts if refresh fails. Credential files remain private and excluded from Git.
