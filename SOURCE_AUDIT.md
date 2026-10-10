# Source and protocol audit

Analyzed the advertised official source map (`main.d8aa4ffb.js.map`): 964 embedded source entries, including 335 application files extracted locally. Source-map SHA-256: `28e1fc15b6781fb7c722b33afb5c9a47489928b5d8304460d8e1d97f4ac3ea1f`. Compared 60 communication models listed in the official models catalog.

## Unresolved properties

| ID | Captured length (bytes) | Literal app references | Model descriptions | Known name |
|---|---|---|---|---|
| 012E | 3 | 0 | 30 | 予約時間変更有無 （予約開始相対時間含む） |
| 012A | 1 | 0 | 0 | Unknown vendor field |
| 0202 | 42 | 0 | 0 | Unknown vendor field |
| 0203 | 62 | 0 | 0 | Unknown vendor field |
| 0201 | 62 | 0 | 0 | Unknown vendor field |
| 021B | 3 | 0 | 0 | Unknown vendor field |
| 0140 | 1 | 1 | 60 | 操作音・終了音設定 |
| 0215 | 5 | 0 | 0 | Unknown vendor field |
| 0216 | 3 | 0 | 0 | Unknown vendor field |
| 0213 | 20 | 0 | 0 | Unknown vendor field |
| 0214 | 9 | 0 | 0 | Unknown vendor field |
| 0211 | 1 | 0 | 0 | Unknown vendor field |
| 0212 | 14 | 0 | 0 | Unknown vendor field |
| 0219 | 12 | 0 | 0 | Unknown vendor field |
| 0217 | 16 | 0 | 11 | 乾燥終了予想AI関連 (布量判定完了時) |
| 0218 | 20 | 0 | 0 | Unknown vendor field |
| 010A | 1 | 0 | 0 | Unknown vendor field |
| 0153 | 25 | 0 | 5 | AI予測余裕時間設定 |
| 0221 | 28 | 0 | 0 | Unknown vendor field |

No literal references were found for the 15 unnamed vendor fields in application JavaScript/JSON, and none of the 60 model descriptions named them. This does not prove the fields have no function: native code, dynamic access, server-side decoding, or device firmware may use them. The Android registry inspected earlier also lacks their names.

## Direct-read verification

`POST /laundry/v5/device/control` with hard-coded read type `03`, empty command parameters, ECHONET protocol, and sequence 1 returned HTTP 200, cmd_result=true, status_code="200", and pcpf_status_code="200". The returned values for 013F, 012B, 0150, 01E0, 0140, 0151, 01F0, and 0217 matched the cached state. This confirms the read transport and response envelope (`results`, plural), not the correctness of any unverified interpretation.

01F0 contains 23 bytes, including a rollback byte 00. The earlier documentation incorrectly described this captured payload as truncated. It is now decoded as firmware 1.00 (ASCII suffix 0), no update ready, and no rollback. App source `actions/updateFirmwareAction.js`, `constants.js`, and `utils/Utils.js` establishes readiness/rollback enums, ASCII locale `ja_JP`, UTC update timestamps, and version display. Zero previous-version data is omitted by the official UI.

0151 dry-default codes 0 and 1 map to drying methods 01 (standard) and 05 (energy-saving) in the official DRYDEFAULTMAP. Other codes are retained without labels. Its high bits remain uninterpreted and the property is explicitly counted as partial.

## Next evidence needed

Static sources have not supplied the remaining binary layouts. Sound settings require contrasting observations of 0140 with known operation/end-sound states. Reservation notifications require 012E observations with known changed/unchanged and relative-time states. AI margin settings require 0153 observations tied to actual settings. AI assessment telemetry requires 0217 observations when load assessment and prediction are applicable. The unnamed vendor fields require labeled state transitions or a firmware/protocol source. Raw byte/word views alone are not semantic evidence. No setting write, cycle start, or firmware update was sent.

## Reproduction

```sh
cargo run -- catalogs
cargo run -- explore-models
cargo run -- source-audit
cargo run -- live-read
cargo run -- inspect --baseline .local/baseline-status.json
```

Source-map input is stored privately at `.local/app.js.map`. The source audit includes original file names and line references, with per-model descriptions, under `.local/source-audit.json`. Live device readings are private under `.local/live-read.json`.

Sources: [official source map](https://cot.wad.apws.panasonic.com/FY26_2nd/static/js/main.d8aa4ffb.js.map), [official model catalog](https://cot.wad.apws.panasonic.com/common/models/9/models.csv), authenticated device/info API, and authenticated direct-read responses. The official NA-LX127E manual was consulted to identify available settings; it does not provide these API encodings.

## Unmapped-read follow-up

A later authenticated status capture at 2026-10-09T13:46:53Z produced no changed unresolved/partial/inferred payload bytes relative to the 12:40:29Z baseline. The continued standby state supplies no distinguishing input for competing interpretations.

A type-03 read containing the 19 captured unresolved IDs returned HTTP 400 with error code L2E05-018-0001. Isolated type-03 reads for 0201 and 010A independently returned the same rejection. Previously supported reads succeeded with the same request encoding. The rejection does not disclose an internal binary layout or prove why these IDs were refused. It shows that the attempted direct-read path cannot retrieve additional values for those two IDs under the current conditions. No write or start commands were sent.

```sh
cargo run -- probe-unmapped --id 0201
```

The probe restricts input to IDs already present and unresolved in the saved snapshot, uses hard-coded command type 03 with empty parameters, saves each response privately under `.local/probes/`, and reports unsuccessful HTTP responses as errors. An omitted `--id` requests all unresolved IDs, which was rejected in this capture. Repeating the same rejected request is not a decoding strategy.

The documented `GET /device/menuitem` endpoint was also inspected. Its response contains only shop_area and shop_category registration choices, not appliance sound/AI setting encodings; it has been excluded from routine protocol catalog reads. An exact quoted-ID scan of all decompiled Java sources found no references to the 15 unnamed vendor IDs.

Progress remains incomplete: 19 payloads have no verified semantic layout, and partial/inferred decoders also require more evidence. The next useful evidence is an observation tied to a known setting or normal operation transition, or an authoritative device/firmware protocol definition. An optional request for the current sound and AI margin settings remains unanswered. This audit must not be read as successful full decoding.

## Blocking-evidence revalidation

At 2026-10-09T13:53:45Z, another successful status read still showed pre-operation standby and waiting for input. No byte changes were found in unresolved, partial, or inferred payloads against the saved baseline. Coverage remains 19 unresolved payloads, 7 partial interpretations, and 2 inferred interpretations (overlapping counts).

The same missing evidence has persisted across the consecutive goal turns: static source/model audits did not define the layouts, targeted read probes were rejected, and revalidation supplied no labeled state transitions. No running observation job is being awaited. Full semantic decoding remains unproven. Meaningful continuation requires new observations paired with known settings/normal operation, or a device/firmware protocol source. Repeating identical standby reads cannot resolve this ambiguity.
