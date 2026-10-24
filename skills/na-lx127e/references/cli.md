# Repository CLI workflow

Paths below are relative to the repository root. Commands and implementation were inspected on 2026-10-24. Recheck current `src/main.rs`, `src/start.rs`, and CLI help when behavior may have changed.

## Common commands

Run from the repository root. `cargo run --locked --` builds/runs the checked-out source; an existing release executable can be used when known to match it.

| Intent | Command | Effect |
| --- | --- | --- |
| List registered devices | `cargo run --locked -- devices` | Cloud read; private local output |
| Fresh appliance state | `cargo run --locked -- status` | Cloud read; raw/decoded private files |
| Decode an existing snapshot | `cargo run --locked -- decode --snapshot PATH` | Offline; no new live state |
| Inspect wash-and-dry prerequisites | `cargo run --locked -- start --dry-run` | Live reads; no appliance write |
| Start normal wash-and-dry | `cargo run --locked -- start` | Appliance write; course 21 |
| Inspect wash-only prerequisites | `cargo run --locked -- start --wash-only --dry-run` | Live reads; no appliance write |
| Start normal wash-only | `cargo run --locked -- start --wash-only` | Appliance write; course 61 |
| Offline request planning | `cargo run --locked -- plan-start` | Saved-state plan; no live preflight or write |
| Refresh stored OAuth tokens | `cargo run --locked -- refresh-token` | Credential exchange/replacement; no appliance start |

Global selection: add `--appliance-id ID` when needed. Never choose arbitrarily among multiple devices.

`start` currently handles normal wash-and-dry and wash-only using current dosing/tank settings; custom courses and paused-cycle resume are not handled by this guarded path. It checks power on, door closed, no faults/reservations, remote operation allowed, stopped pre-operation standby, compatible model metadata and known dosing settings. Do not bypass these guards via raw control endpoints.

`--dry-run` is a real live preflight, so it needs authentication and network access; `plan-start` is offline and cannot prove present readiness. Neither a saved request nor a successful preflight proves a start was sent.

## Result handling

Inspect the command result first. Private records are `.local/start-preflight.json`, `start-request.json`, `start-response.json`, and `start-attempt.json`. Request templates are not delivery records; attempt records are not live-process indicators. Do not dump credentials or entire private files into answers.

The client validates HTTP status, appliance/type/sequence, command result and API status codes; redirects and automatic retries are disabled. Following a timeout, ambiguous acknowledgement, or unconfirmed delivery, fetch status and preserve uncertainty. Do not blindly repeat the control request. Even accepted requests need fresh observed state to establish physical operation.

## Repository evidence and specialized operations

- `README.md`: setup, private storage, common commands. Its early overview still says wash-only is unimplemented; `src/main.rs` and `src/start.rs` implement `--wash-only`. Prefer current source/help for this discrepancy.
- `START_API.md`: start encoding and acceptance checks; historical restrictions/authentication comments may predate wash-only and refresh-token support.
- `DECODING.md`, `spec/property-map.json`: meanings and limits of interpreted state. Unknown or inferred fields must not become definite hardware facts.
- `API_ENDPOINTS.md` and `api --help`: additional reads/updates. A command named `api` is not inherently read-only; check the selected operation and request before execution.
- `scripts/daily-start.sh`: refresh then one guarded start, private logs and locking. No argument starts wash-and-dry; `--wash-only` starts wash-only; `--dry-run` still refreshes credentials. It does not accept the combination `--wash-only --dry-run`; use the direct CLI for that preflight.

A documented schedule is not proof of installed/active scheduler configuration. Inspect the actual scheduler only when the task requires it; creating this skill does not authorize changing it. An expired-token failure may be resolved by the existing refresh flow; a fresh login uses `login-bridge` and `browser_login.js` with user sign-in.
