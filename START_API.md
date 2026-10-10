# Immediate wash-and-dry start

Analyzed on 2026-10-09 from the official SmartLaundry web bundle, not verified by executing a physical cycle.

## Endpoint and encoding

`POST https://app.wad.apws.panasonic.com/laundry/v5/device/control`

The official `xh` function constructs this envelope; `IS.remoteOperation` constructs its commands and `HS` appends selected custom-course commands.

```json
{
  "requests": [{
    "appliance_id": "YOUR_APPLIANCE_ID",
    "cmd_type": "02",
    "cmds": [
      {"id": "00B2", "params": ["41"]},
      {"id": "00D0", "params": ["21"]},
      {"id": "0132", "params": ["02"]},
      {"id": "0133", "params": ["02"]},
      {"id": "013B", "params": ["00"]},
      {"id": "0151", "params": ["000000024223"]}
    ],
    "protocol": "echonetlite",
    "seq_no": 1
  }]
}
```

This example selects normal wash-and-dry (`00D0=21`). The dosing/configuration values illustrate the captured device state and must be derived from the intended settings. Request `params` is an array of strings, unlike status responses and course metadata, which use objects containing `value`.

`00B2=41` starts/resumes operation. The ECHONET standard also defines pause `42` and stop `43`. The app's immediate-start path puts the start command first. Starting a newly selected washing course adds course and dosing settings. Resuming an already configured operation uses only the start command. Custom courses can require additional commands from the course catalog; this example does not cover those courses.

The app uses write command type `02`, read command type `03`, and sequence number `1` for this path. These values were extracted from app constants, not guessed from the third-party OpenAPI document.

## Authentication and result handling

The app's HTTP interceptor supplies JSON content type, `Content-Language: ja_JP`, local `X-Timestamp` formatted as `YYYYMMDDHHmmss+0900`, and Bearer authorization. The existing read client additionally supplies cookie `atkn` and `X-ApplianceId`; which extra headers control strictly requires has not been tested.

The app checks `results[0].cmd_result` together with returned status/error codes. HTTP success alone is insufficient evidence that the machine accepted the command. Control has not been sent during this analysis, so a successful start response and physical execution remain unverified. A separate read-only control request confirmed the `results` envelope and success fields; see `SOURCE_AUDIT.md`.

`POST /laundry/v5/laundry/command` looks up course command metadata. It is not the start endpoint.

Before an actual start, verify live power, closed door, remote permission, absence of faults, and the intended course/settings. An old snapshot cannot establish these conditions.

## Offline Rust plan

```sh
cargo run -- plan-start
```

Creates private `.local/start-plan.json` using the registered NA-LX127ER and saved dosing configuration. It has no HTTP transport and never starts a cycle. It currently supports normal wash-and-dry only. `analysis_only`, `sent`, snapshot timestamp, and observed precondition values accompany the request for review.

## Evidence

- [Official app bundle](https://cot.wad.apws.panasonic.com/FY26_2nd/static/js/main.d8aa4ffb.js): `Qp`, `xh`, `IS.remoteOperation`, `HS`, and HTTP interceptor. The local prettified file is analysis material, not a checked-in dependency.
- [ECHONET Appendix Release R revision 3](https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/Release/Release_R/Appendix_Release_R_r3.pdf): washer/dryer operation setting property B2.
- `spec/openapi.yaml` is a third-party reference. Its control encoding differs from the observed app and must not be used as an executable contract.

## Rust transmission implementation (2026-10-10)

`cargo run --locked -- start --dry-run` performs authenticated capability and live-state reads, validates idle/start prerequisites, and saves the intended request without sending a write. `cargo run --locked -- start` performs those reads again and then sends one normal wash-and-dry start with current dosing/tank settings. No separate execution flag is needed: invoking `start` requests transmission.

This command is restricted to NA-LX127ER / NA-LX127E and normal wash-and-dry. It does not start wash-only/custom courses or resume paused cycles. Unknown or inconsistent conditions are rejected before the write. Expired credentials fail at the read stage and require the existing login flow.

The control client disables redirects and reqwest automatic retries, uses Bearer authorization plus the atkn cookie and appliance header, and supplies a JST timestamp. The acknowledgement must match the appliance, command type 02, and sequence 1; cmd_result must be true and both status codes must be 200. HTTP 200 alone is not acceptance. API acceptance is not proof of physical operation. Ambiguous transport/results are preserved in a private attempt record with no automatic resubmission.

Tests use a loopback server with dummy credentials to verify the actual HTTP write, headers, command order/string-array parameters, single request, and disabled redirects. Negative tests verify that unsuitable physical states block the write and that rejected/malformed/mismatched acknowledgements do not report success. The production endpoint was not used to send a start during this implementation.
