# SmartLaundry property decoding

Model: NA-LX127ER-W. Communication model: NA-LX127E.
Inspection date: 2026-10-09.

## Sources and confidence

The official model metadata endpoint is
`https://app.wad.apws.panasonic.com/laundry/v5/device/info`.
The inspected official app bundle is
`https://cot.wad.apws.panasonic.com/FY26_2nd/static/js/main.d8aa4ffb.js`.
The installed Android app is `com.panasonic.SmartLaundry`, version 2.14.00.
`spec/property-map.json` records the source URLs and bundle SHA-256.
Japanese API display labels are preserved as data.

Every decoded property retains its ID, raw value, raw parameters, and category.

| Category | Evidence |
| --- | --- |
| `official_enum` | Exact value-to-label match from model metadata |
| `official_hhmm_rule` | App hexadecimal hour/minute conversion |
| `unavailable_sentinel` | App `FFFF` sentinel, represented as null |
| `official_hex_integer_rule` | App hexadecimal integer conversion |
| `official_ascii_rule` | App version byte-to-character conversion |
| `official_bitfield_rule` | App `Bb` / `Hb` tank configuration extraction |
| `inferred_uint16_rule` | Official counter name; inferred integer encoding |
| `unresolved` | Interpretation not established; original bytes preserved |

## Useful properties

| ID | Meaning | Representation |
| --- | --- | --- |
| `0080` | Power | `30` on; `31` off |
| `0088` | Abnormality | `42` no abnormality |
| `0086` | Manufacturer error code | Raw; no error dictionary established |
| `0090` | Timer reservation | `41` enabled; `42` disabled |
| `00B0` | Door | `41` open; `42` closed |
| `00D0` | Configured course | `21` automatic wash and dry; other labels in metadata |
| `00E2` | Phase | `41` wash; `42` rinse; `43` spin; `44` paused; `52` dry/blow; `61` waiting |
| `0121` | Detailed state | `00` waiting for input; other labels in metadata |
| `0100` | Remote-control permission | Model-specific enumeration |
| `00DB`, `00DC`, `00ED` | Wash, dry, and combined time remaining | Hexadecimal hours and minutes |
| `0124`, `013E` | Current and washing process time remaining | Hexadecimal hours and minutes |
| `0136`, `0137`, `013D` | Tank 1, 2, and 3 remaining level | `00` present; `01` low; `FF` unknown |
| `0132`, `0133`, `013B` | Automatic dosing mode | Model-specific enumeration |
| `0134`, `0135`, `013C` | Dosing amount setting | Hexadecimal integer; no physical unit claimed |
| `0151` | Tank/adjustment configuration | Six-byte bitfield |
| `FF01`, `FF02` | Wireless module versions | ASCII bytes encoded as hexadecimal |
| `0160` | Operation count | Tentative unsigned big-endian integer |

As a time value, `013B` means one hour and 59 minutes, or 119 minutes.
`FFFF` is unavailable, not zero or a large duration. The configured course is
not evidence that a cycle is running.

## Tank configuration

App helper `Bb` pads `0151` to 48 bits and consumes fields from the least
significant end using helper `Hb`.

| Bit offset | Width | Meaning |
| --- | --- | --- |
| 0 | 4 | Tank 3 agent code |
| 4 | 1 | Tank 3 input setting |
| 5 | 4 | Tank 2 agent code |
| 9 | 1 | Tank 2 input setting |
| 10 | 4 | Tank 1 agent code |
| 14 | 1 | Tank 1 input setting |
| 15 | 1 | Reservation object setting |
| 16 | 4 | Margin time setting |
| 20 | 2 | Dry default setting |

Agent codes: 0 detergent, 1 softener, 2 delicates detergent, 3 oxygen bleach,
4 stain remover. Higher bits are preserved without interpretation.

Observed `0151 = 000000024223` selects detergent, softener, and oxygen bleach.
Thus `013D = 01` means low oxygen bleach, despite its generic API label referring
to delicates detergent. Tank levels are categories, not percentages or volumes.
Tank 3's mode was manual dosing.

## Coverage and remaining work

The observed response has 83 properties, 68 known names, and 44 interpreted values.
That count includes unavailable sentinels and one tentative integer conversion.
There are 39 unresolved values, including 15 properties with unknown names.

Unknown IDs: `012A`, `0202`, `0203`, `0201`, `021B`, `0215`, `0216`, `0213`,
`0214`, `0211`, `0212`, `0219`, `0218`, `010A`, `0221`.

Named but unresolved fields include reports, course history, brand settings,
downloaded content, firmware management, and packed adjustment data. Future
observations during normal user-operated cycles can validate changes and the
inferred counter. No physical cycle was started for this inspection.

The latest Rust request reported power on, waiting for input, door closed,
reservation disabled, and no abnormality. Detergent and softener were present;
oxygen bleach was low. All five remaining-time fields were unavailable.
These are snapshot observations, not permanent appliance settings.

## Extended decoding audit (2026-10-09)

The latest 83-property snapshot has 64 interpreted values or structures, including 7 partial structures and 2 inferred encodings. These counts overlap: the inferred report is partial. Unknown bits and tails remain visible.

| ID | Name | Interpretation |
|---|---|---|
| 0090 | ONタイマ予約設定 | official_enum |
| 00E9 | 乾燥時間設定値 | official_hhmm_rule |
| 012E | 予約時間変更有無 （予約開始相対時間含む） | unresolved |
| 00E7 | すすぎモード設定 | documented_packed_rule |
| 012C | 洗濯レポート | inferred_partial_rule |
| 00E8 | 脱水時間設定値 | official_enum |
| 012D | お手入れ通知 | official_enum |
| 00E5 | 洗い時間設定値 | official_enum |
| 012A | Unknown vendor property | unresolved |
| 00E6 | すすぎ回数設定値 | official_enum |
| 012B | コース履歴 | documented_partial_rule |
| 00EC | ソフトキープ設定 | official_enum |
| 0130 | 洗剤銘柄設定 | official_supply_catalog |
| 00ED | 洗濯乾燥残り時間 | unavailable_sentinel |
| 0131 | 柔軟剤銘柄設定 | official_supply_catalog |
| 00EA | 洗い温水設定値 | official_enum |
| 0092 | ONタイマ相対時間設定 | documented_packed_rule |
| 0127 | すすぎ時間設定値 | unavailable_sentinel |
| 0125 | プレ洗浄/プレ乾燥/ | official_enum |
| 0202 | Unknown vendor property | unresolved |
| 0203 | Unknown vendor property | unresolved |
| 0123 | 残時間カウントダウン禁止 | official_enum |
| 0124 | 工程残り時間 | unavailable_sentinel |
| 0201 | Unknown vendor property | unresolved |
| 0088 | 異常発生状態 | official_enum |
| 0121 | 運転状況詳細 | official_enum |
| 0122 | できたて通知許可 | official_enum |
| 00E4 | 水量設定２ | official_enum |
| 00E2 | 洗濯乾燥遷移状態 | official_enum |
| 0129 | 槽クリーン時間設定値 | official_enum |
| 013F | わが家流お好み設定 | documented_partial_rule |
| 013D | おしゃれ着洗剤自動投入タンク残量 | official_enum |
| 00F9 | チャイルドロック設定 | official_enum |
| 013E | 洗濯行程残り時間 | unavailable_sentinel |
| 021B | Unknown vendor property | unresolved |
| 00B2 | 洗濯・乾燥設定 | documented_packed_rule |
| 013B | おしゃれ着洗剤投入モード設定 | official_enum |
| 00F7 | 自動槽洗浄設定 | official_enum |
| 013C | おしゃれ着洗剤自動投入量設定 | official_hex_integer_rule |
| 0140 | 操作音・終了音設定 | unresolved |
| 00FA | その他独自コース設定 | documented_packed_rule |
| 0215 | Unknown vendor property | unresolved |
| 0216 | Unknown vendor property | unresolved |
| 0136 | 洗剤自動投入タンク残量 | official_enum |
| 0213 | Unknown vendor property | unresolved |
| 0137 | 柔軟剤自動投入タンク残量 | official_enum |
| 0214 | Unknown vendor property | unresolved |
| 0134 | 洗剤自動投入量設定 | official_hex_integer_rule |
| 0211 | Unknown vendor property | unresolved |
| 0135 | 柔軟剤自動投入量設定 | official_hex_integer_rule |
| 0212 | Unknown vendor property | unresolved |
| 0132 | 洗剤投入モード設定 | official_enum |
| 0133 | 柔軟剤投入モード設定 | official_enum |
| 00B0 | 扉・蓋開閉状態 | official_enum |
| 00F4 | 乾燥方法設定 | official_enum |
| 00F5 | ジェットほぐし・乾燥設定 | official_enum |
| 013A | おしゃれ着洗剤銘柄設定 | official_supply_catalog |
| 00F2 | 予約開始相対時間 | documented_packed_rule |
| 00F0 | NanoeSetting | documented_packed_rule |
| 0219 | Unknown vendor property | unresolved |
| 00F1 | わたし流プラスサブモード設定 | official_enum |
| 0217 | 乾燥終了予想AI関連 (布量判定完了時) | unresolved |
| 0218 | Unknown vendor property | unresolved |
| 010A | Unknown vendor property | unresolved |
| 01E6 | DLコース設定 | documented_packed_rule |
| 0152 | 終了時刻クラウド予約設定 | documented_partial_rule |
| 0153 | AI予測余裕時間設定 | unresolved |
| 0150 | 調整機能設定 | documented_partial_rule |
| 0151 | 調整機能設定2 | official_partial_rule |
| 0100 | 遠隔制御操作状態 | official_enum |
| 0221 | Unknown vendor property | unresolved |
| 01E2 | コースDL登録情報 | documented_packed_rule |
| 01E0 | コンテンツ確認 | documented_partial_rule |
| FF01 | 無線モジュールソフトバージョン | official_ascii_rule |
| FF02 | 無線モジュールハードバージョン | official_ascii_rule |
| 0086 | メーカ異常コード | documented_packed_rule |
| 00DB | 洗濯残り時間 | unavailable_sentinel |
| 00DC | 乾燥残り時間 | unavailable_sentinel |
| 0120 | 動作モード | official_enum |
| 0160 | 運転回数 | inferred_uint16_rule |
| 0080 | 動作状態 | official_enum |
| 00D0 | 洗濯・乾燥コース設定１ | official_enum |
| 01F0 | 本体ファーム管理 | documented_packed_rule |

## Washing report inference

The read-only `GET /laundry/v5/notices?limit=100&from_index=0` returned a laundry-report notification matching property 012C. The notification reports course 21, subcourse 000000, end time 2026-10-09T08:52:00Z, and duration 02:09. The raw property is 210000001A0A09113402090004. These independently match the following candidate layout:

| Byte offset | Length | Candidate field |
|---|---|---|
| 0 | 1 | Course code |
| 1 | 3 | Subcourse code |
| 4 | 1 | Year minus 2000 |
| 5 | 1 | Month |
| 6 | 1 | Day |
| 7 | 1 | Local hour |
| 8 | 1 | Local minute |
| 9 | 1 | Duration hours |
| 10 | 1 | Duration minutes |
| 11 | 2 | Uninterpreted tail |

This is a one-record inference, not an official layout. The decoder validates date/time components and retains the tail. The notification also contains an Econavi water-temperature comment, but the 0004 tail has not been assigned flag meanings from this single example.

Nanoe 00F0 values 00 through 05 are defined directly by the official app constant `ch`: unspecified, pre-cleaning, tank-rolling deodorizing, tank-stopped deodorizing, pollen, and virus care. A recognized code does not establish that this model supports executing that function.

## Remaining 19 payloads

The remaining IDs are 012E, 012A, 0202, 0203, 0201, 021B, 0140, 0215, 0216, 0213, 0214, 0211, 0212, 0219, 0217, 0218, 010A, 0153, 0221. Fifteen lack names in inspected official registries. Named unresolved properties are 012E (reservation change notification), 0140 (operation/end sound), 0153 (AI margin schedule), and 0217 (AI drying-end prediction at load assessment).

The metadata-only POST `/laundry/v5/laundry/command` was queried with the captured named payload values. It returned HTTP 200 and empty parameter descriptions for all five queried IDs; the response does not establish their layouts. Prediction settings GET returned use_drytime_prediction=true, but that account setting does not decode 0217. Its captured value consists of 16 FF bytes; treating it as unavailable remains a hypothesis.

0153 is 25 bytes. Four aligned six-byte groups repeat in pairs, followed by one byte. The observed groups are 000000090A0E, 0000000B0C14, 000000090A0E, 0000000B0C14; the tail is 0A. Field units, group meaning, and tail semantics remain unresolved. Integer/byte views are deliberately excluded from semantic coverage.

## Reproducible comparison

Every successful `cargo run -- status` now stores a private timestamped snapshot under `.local/snapshots/` as well as updating `.local/status.json`. The fresh read made during this analysis had identical property values to the earlier snapshot.

```sh
cargo run -- inspect --baseline .local/baseline-status.json
# Or compare two archived captures:
cargo run -- inspect --snapshot .local/snapshots/NEW.json --baseline .local/snapshots/OLD.json
```

The private forensic report includes byte offsets, lengths, FF/zero patterns, competing aligned big/little-endian word views, and changed bytes including additions/removals. It does not invent semantic labels. Routine device use can supply later observations without sending control requests from this tool.

Source evidence: official main.d8aa4ffb.js helpers ab, zb, Jb, tv, ch and configuration getters; device-info and supplies catalogs; ECHONET Appendix Release R revision 3 for B2 and E7; and the authenticated notices read for the report inference. No physical operation or account-setting change was performed.

## Source-map and live-read follow-up

See `SOURCE_AUDIT.md` for the 964-source/60-model audit and successful direct read. The current 01F0 payload is complete at 23 bytes, not truncated; the earlier assertion was incorrect. Firmware now includes readiness, UTC timestamp, displayed version, ASCII suffix, and rollback semantics. 0151 is counted as partial because high bits remain unidentified. Overall coverage remains 64/83 with 19 unresolved payloads.

## First observed API-started cycle

An explicitly authorized normal wash-and-dry start was accepted on 2026-10-10. A subsequent status read independently reported wash phase `00E2=41` and washing detail `0121=01`. The snapshot at `2026-10-10T00:41:32.770019+00:00` was compared with the pre-start snapshot `20261010T004027.540162000Z.json`.

| Property | Pre-start | Washing | Evidence limit |
| --- | --- | --- | --- |
| 0217 | Sixteen FF bytes | `03000303070200020082001919191A04` | First populated load-assessment prediction payload; individual fields remain unknown |
| 010A | `00` | `02` | Correlated with this cycle transition; its name and enumeration remain unknown |
| 0152 | `0100` | `0101` | Second byte changes during an AI-enabled cycle; does not establish its meaning |
| 0153 | Groups `000000090A0E`, `0000000B0C14`, repeated; tail `0A` | Groups `00000008080C`, `0000000A0A11`, repeated; tail `0A` | Values changed when the cycle began; field units and group semantics remain unknown |

The archived `00:41:09` capture reports unavailable prediction (`I504`) and an all-FF 0217. The `00:41:32` capture reports a populated 0217 and an available 72-minute prediction. An earlier account incorrectly combined the populated later raw payload with the earlier unavailable prediction; the archived captures correct that chronology. Fresh command-detail metadata again supplied no field descriptions. Semantic coverage is unchanged.

At `2026-10-10T00:42:23.546792+00:00`, the server prediction remained available with `drytime=72`, `unreliability=0`, and `decrease_acc=false`. The 0217 payload and all unresolved/partial/inferred property bytes were identical to the `00:41:32` washing snapshot. Its big-endian bytes 8–9 equal 130, whereas the reported prediction is 72; this rules out interpreting those bytes directly as that prediction in minutes for this observation. Other encodings and field meanings remain unresolved. The simultaneous availability/payload transition supports investigating a relationship, but does not prove the payload layout.

At `00:42:44`, unnamed property 0216 changed from `001500` to `120000` while the wash phase and 72-minute prediction remained unchanged. Neither decimal interpretations nor byte positions have assigned semantics yet. Later captures must distinguish counters, timers, packed flags, and unrelated state changes.

At `2026-10-10T01:03:08.556430+00:00`, 0216 returned from `120000` to `001500` while the machine remained in rinsing. The sequence is therefore `001500` (standby/early washing), `120000` (later washing and earlier rinsing), then `001500` (later rinsing). This rules out a monotonically increasing whole-payload cycle counter and a one-to-one mapping to the top-level wash/rinse phase. It may encode a substate or a reusable setting, but neither interpretation is established by these captures.

`cargo run -- timeline` now creates `.local/timeline.json` from archived snapshots sorted by capture time. Every event retains phase, operation detail, remaining-time fields, server prediction, changed unresolved/partial/inferred payloads with byte offsets, and disappeared IDs. Newly observed properties are retained even without a previous value. This report is an evidence index; it does not increase semantic coverage.

A focused UI source trace confirms the 0152 boundary: `0706-CloudAiPredictionSettings.js` lines 64–66 split its first byte into `settingStatus` and preserve the remaining bytes in a variable named `isUpdating`. Lines 150, 169, and 197 concatenate them again when changing the setting. Official constants define first-byte ON=`01` and OFF=`00`. The source therefore confirms preservation of the tail, but never interprets its values in this component. The variable name alone is insufficient to label `01` as an active update flag. The decoder deliberately continues reporting an uninterpreted tail. This also explains why changing the setting must preserve that byte rather than replacing the entire property with a one-byte ON/OFF value.

### Absolute cycle settings observed after load assessment

The user independently observed the app displaying 94 minutes overall and 22 minutes remaining during spinning. The nearby `2026-10-10T01:14:21.607316+00:00` capture contains `0124=0016` and `013E=0016` (both 22 minutes), plus server `drytime=72`. The app's `0775-RunningWashing.js` lines 570–587 add washing-process remaining time to the server drying prediction on supported post-FY23 drum machines before drying. Thus `22 + 72 = 94` reproduces the reported app total. In contrast, `00ED=0206` is 126 minutes: the machine's raw remaining-time estimate differs from the cloud-based display. Do not label 00ED as the app's currently displayed total. The app can also count down locally between reads, so exact minute comparisons require close timestamps.

The first archived rinse observation is `2026-10-10T00:54:57.418744+00:00`. The decoded phase and detail both report rinsing, but none of the unresolved, partial, or inferred payloads changed relative to the last washing capture. In particular, 010A remains `02`, 0216 remains `120000`, and 0217 retains its populated load-assessment value. These observations contradict treating 010A or 0216 as a direct wash-versus-rinse phase enumeration for this cycle; they do not identify their actual meaning.

The first archived spin observation is `2026-10-10T01:06:11.952566+00:00`. Both phase and operation detail report spinning. None of the unresolved/partial/inferred payloads changed across this rinse-to-spin transition. 010A remains `02`, 0216 remains `001500`, and 0217 remains the populated load-assessment payload. The same unknown values therefore span multiple independently identified top-level phases; direct phase labels would be unsupported.

The user independently reported drying had started. The first archived drying observation is `2026-10-10T01:38:28.850726+00:00`. There is a capture gap after the last `01:29:38` spin snapshot, so the precise transition time is not established. Unnamed 0219 changed from `FFFFFFFFFFFFFFFFFFFFFF00` to `4D723E4F575B506400040004`; this is its first populated drying payload in this run. Unnamed 0218 changed from `0076BFCB009618730C014A4E043218292C2C6800` to `006AB09D0077104516014B4F043318272A2A6800`. Their field meanings remain unresolved. Unlike 0217, these two properties changed across the observed spin/drying boundary.

At `01:39:35`, the phase reports drying/airflow and detail reports jet drying. Washing-process remaining time is zero, raw current-process and total remaining time are 99 minutes, and the server drying prediction is 69 minutes (70 at `01:38:28`). No unresolved payload bytes changed between these drying observations. Thus the server prediction can decrease while 0217/0218/0219 remain unchanged; none should be equated directly with a continuously updated predicted remaining-minute field without additional evidence.

At `2026-10-10T01:43:34.998084+00:00`, 0219 changed again during drying, from `4D723E4F575B506400040004` to `537A55554F6C4F5F00090004`, while the server prediction reached 64 minutes. All first eight bytes changed; bytes 8–9 changed from `0004` to `0009`, and bytes 10–11 remained `0004`. This establishes that 0219 is not merely an immutable drying-start record in this run. Its field units, refresh trigger, and sensor/timer interpretations remain unverified. The absence of per-minute changes at intermediate captures argues against identifying its update cadence with the server prediction countdown.

At `2026-10-10T01:48:40.593293+00:00`, 0219 became `59876C631869595A000E0004`. The server prediction increased from 59 to 62 minutes at this capture, so it is not a simple wall-clock countdown. Across the three populated 0219 observations, big-endian bytes 8–9 are 4, 9, and 14, separated by approximately five minutes; bytes 10–11 remain 4. This supports an elapsed-minute counter candidate refreshed in roughly five-minute steps. The exact drying-start time is unavailable because of the capture gap, and a firmware specification or additional independently timed observations are still required to establish its semantics. These candidate interpretations are not included in decoded coverage.

The fourth populated observation, at `2026-10-10T01:53:45.846151+00:00`, is `6193757607685A5500130004`: bytes 8–9 are now 19. The observed sequence 4/9/14/19 tracks approximately 15 minutes of independently timestamped captures; the final two bytes remain 4. This strengthens the elapsed-minute hypothesis for bytes 8–9 without identifying what event starts that counter. The prediction dropped from 58 to 53 minutes at this refresh, whereas the other unknown payloads remained unchanged. Sampling establishes temporal co-occurrence, not that this property causes the cloud prediction update.

At `2026-10-10T01:58:52.071505+00:00`, 0219 became `699A837A796B595000180004`, extending the candidate counter to 24 (`0018`). Five populated observations now span approximately 20 minutes and advance by 20 in bytes 8–9; the trailing `0004` remains constant. The independent server prediction is 47 minutes, excluding a direct remaining-time interpretation of that candidate counter for this capture. The elapsed-time hypothesis remains unconfirmed as to its starting event and behavior outside this single cycle.

At `2026-10-10T02:03:58.042896+00:00`, 0219 became `72A28F834F65594B001D0004`, extending the candidate counter to 29. Unnamed 0215 also changed for the first time in this cycle: `4A004E0400` to `4B004F0400`. A cross-property relation is worth investigating: 0215 bytes 0, 2, and 3 now match 0218 bytes 10, 11, and 12 (`4B`, `4F`, `04`); the same positions matched the previous 0218 value (`4A`, `4E`, `04`) before the cycle. Their update times differ, so these matching values do not establish a shared sensor layout or field units. Both payloads remain unresolved. The server prediction at this capture is 40 minutes.

The first populated washing capture contains `00E5=0009`, `00E6=02`, and `00E8=0D`. ECHONET Appendix Release R rev.3, washer/dryer class sections 20, 21, and 23 define E5 absolute hours/minutes (00:00 through 17:3B), E6 absolute rinse count (0 through 8), and E8 absolute spin minutes (0 through 59). Together with the official model property names, these support wash time 9 minutes, rinse count 2, and spin time 13 minutes. They are configured values, not elapsed time or total cycle duration.

The decoder now handles these absolute values with range validation while preserving the model-specific automatic enumeration `00E5=00FF`. Relative settings outside the implemented absolute ranges remain unresolved rather than being mistaken for long durations. Existing automatic values already counted as decoded, so total coverage remains 64/83; this change expands supported observed states rather than adding three new property IDs. Tests cover real captured values, upper bounds, reserved ranges, and preservation of automatic enumeration.

A bounded read-only capture process was started for this authorized cycle: up to 150 status reads, separated by 60 seconds. Each successful read archives all raw properties and the server prediction. It sends no start, pause, stop, reservation, or setting writes. Process completion and additional snapshots must be checked before claiming later stages were observed.

The start preflight also exposed an incorrect assumption: `00B2=41` can coexist with confirmed pre-operation standby (`00E2=61`, `0121=00`). B2 is a start/pause/stop command setting, not independent proof of active operation. The sender now checks the actual phase and detail, accepts recognized start/stop settings while idle, and continues rejecting pause or unknown settings. A regression test covers standby with B2=41 and rejection after the phase becomes active.

At `2026-10-10T02:09:04.121197+00:00`, 0219 changed to `79A6988870694D4600220004`, extending bytes 8–9 to 34. Seven populated observations now span 30 minutes 35.270471 seconds while this candidate advances by 30. Subtracting the candidate number of minutes from each capture time produces hypothetical origins between `01:34:28.850726` and `01:35:04.121197` UTC, a 35.27-second spread. This is consistent with an elapsed-minute quantity plus sampling delay, but cannot distinguish a device timer from a reporting-batch index or establish the starting event. The capture gap still prevents independent verification of the exact drying transition. At this latest refresh the cloud prediction increased from 36 to 38 minutes; bytes 8–9 therefore remain incompatible with a direct remaining-minute interpretation. No semantic coverage was added.

At `2026-10-10T02:14:10.591999+00:00`, 0219 refreshed to `7FA8948B9D64384100270004`. Bytes 8–9 advanced to 39, continuing the five-unit sequence over another approximately five-minute interval. Bytes 10–11 remain `0004`; the other unresolved properties did not change. Cloud prediction remained 34 minutes across the adjacent captures despite this payload refresh. Together with earlier increases and decreases at refreshes, this excludes a fixed one-to-one prediction delta tied to every 0219 update. Individual fields remain unresolved.

At `2026-10-10T02:18:14.983503+00:00`, 0219 became `85A98A893C66463C002C0004` and bytes 8–9 advanced from 39 to 44. This change was captured only 244.391504 seconds after the previous changed value, unlike the earlier approximately 305-second intervals. Thus the observed refresh interval is not fixed at five wall-clock minutes; polling phase, update timing, and quantization remain possible contributors. The counter-derived hypothetical origin is now `01:34:14.983503` UTC, widening the observed origin range to 49.137694 seconds. This remains compatible with a quantized elapsed-minute candidate but must not be treated as an exact clock. Cloud prediction again stayed at 31 minutes across adjacent captures, and no other unresolved property changed.

### Counter hypothesis contradicted by a later drying observation

At `2026-10-10T02:23:21.177330+00:00`, 0219 changed to `89A782880E663D2800400004`. Big-endian bytes 8–9 jumped from 44 to 64 in 306.193827 seconds. This contradicts interpreting that quantity as unadjusted real elapsed minutes: it advanced 20 units in approximately five minutes. The earlier near-clock sequence was insufficient to establish semantics. A corrected or model-derived time, adjustable counter, or packed representation remains possible, but none is verified. In particular, the earlier hypothetical origins must not be used to infer a drying-start timestamp. Cloud prediction fell from 27 to 22 minutes across the adjacent captures; temporal co-occurrence alone does not establish a formula connecting the two. The trailing `0004` and all other unresolved properties remained unchanged. Semantic coverage remains unchanged.

At `2026-10-10T02:28:27.198828+00:00`, 0219 refreshed to `8DA781880A663E1B004D0004`. Bytes 8–9 advanced from 64 to 77 in 306.021498 seconds, a second advance exceeding real elapsed minutes. This reinforces the rejection of an unadjusted elapsed-minute interpretation. Bytes 10–11 remain `0004`; no other unresolved payload changed. Cloud prediction increased from 18 to 19 minutes, whereas the previous oversized advance accompanied a prediction decrease. These two observations do not support a simple fixed sign relationship between this quantity and the prediction adjustment. The underlying field semantics remain unknown.

At `2026-10-10T02:33:33.583163+00:00`, 0219 changed to `90A68088F066401100570004`. Bytes 8–9 advanced from 77 to 87 in 306.384335 seconds, again exceeding elapsed real minutes. The first eight bytes continue to vary, including byte 4 changing from `0A` to `F0`; this alone does not establish signedness, overflow, or a physical sensor unit. The final word remains `0004`, other unresolved properties are unchanged, and the cloud prediction remains 15 minutes across adjacent captures. No new field semantics are established.

At `2026-10-10T02:36:36.331863+00:00`, 0219 changed to `93A681887E6A460E005A0004`. Bytes 8–9 advanced from 87 to 90 in 182.748700 seconds, approximately three minutes. This differs from the preceding five-minute refreshes and oversized increments, establishing that both observed interval and increment vary within this cycle. The last word stays `0004`, and the cloud prediction is 14 minutes. Other unresolved properties did not change. The elapsed-minute interpretation remains contradicted by the earlier jumps despite this locally near-clock increment.

At `2026-10-10T02:38:38.024734+00:00`, 0219 refreshed to `95A781895067430C005C0004`. Bytes 8–9 advanced from 90 to 92 in 121.692871 seconds, approximately two minutes. Observed refresh intervals therefore shorten near this part of drying, but one cycle does not establish a phase-specific refresh policy. The last word is still `0004`; no other unresolved property changed. Cloud prediction is 11 minutes. The earlier oversized increments still rule out an unadjusted elapsed-minute interpretation across the full observed run.

At `2026-10-10T02:43:41.954259+00:00`, 0219 changed to `99A77F89E6673E0700610004`. Bytes 8–9 advanced from 92 to 97 over 303.929525 seconds, returning to a locally near-five-minute increment. This does not restore the rejected unadjusted-clock interpretation across the whole run. The final word remains `0004`, no other unresolved property changed, and cloud prediction is 6 minutes. All raw values remain archived for comparison with completion.

At `2026-10-10T02:48:46.160076+00:00`, 0219 changed to `9AA577870B67310400640004`. Bytes 8–9 advanced from 97 to 100 in 304.205817 seconds, while cloud prediction reached 1 minute. A value of 100 is suggestive of a bounded progress quantity, but an endpoint coincidence in one cycle is not evidence of a percentage encoding. Completion, saturation, and reset behavior must be observed before evaluating that candidate further. The last word remains `0004`, other unresolved properties did not change, and the machine still reports drying rather than completion.

## Observed wash-and-dry completion

At `2026-10-10T02:49:46.764105+00:00`, phase changed from drying to operation-ended/fluffy-keep, with detail fluffy-keep. The immediately preceding capture was at `02:48:46.160076`; completion is bounded by those observations rather than assigned an exact device timestamp. Current-process and wash-and-dry remaining minutes became zero. The server prediction still reported 1 minute after completion, showing that its value alone must not determine active-operation state.

Changed unresolved/partial/inferred properties at completion:

| ID | Raw value after completion |
| --- | --- |
| 012C | `210000001A0A0A0B3102090004` |
| 0202 | `007E021009020DFFFF0303140A0A007D24021100290049038B03030100000001788100003800650000CC` |
| 0203 | `00300001330000000000004900290E6A6B6A20B0549D20066E0D7C500277104516885A99A669E03C875B88A766313C846272A36525585546537B6BD95003` |
| 0201 | `00360033000600260009000069005700970007000000000000000000000000000000000000000000000000000000000000000000000036014C01BE000000` |
| 0212 | `2904440251024403368AC1480141` |
| 0221 | `5A5CB0546D8812A8F002D04C11ACB002A04C11ACB002130A07026702` |
| 0160 | `0059` |

0160 advanced from `0058` (88) to `0059` (89) after this single observed cycle, strengthening the big-endian cycle-count inference. This does not establish overflow or reset behavior. 012C now records course 21 and an end time consistent with 2026-10-10 11:49 JST, plus duration 129 minutes; its final `0004` remains uninterpreted. The unnamed 0201/0202/0203/0212/0221 changes show that these payloads can be completion-updated records, but their individual layouts are not established.

0219 retained `9AA577870B67310400640004`, including candidate value 100, across completion and the following `02:50:47` capture. No immediate reset was observed. This is consistent with a completed progress quantity but does not prove percentage units. 010A, 0152, 0215, 0216, 0217, 0218, and 0153 likewise did not change at the observed completion boundary. Fluffy-keep is an operation-ended phase even though the appliance may continue periodic drum movement. Semantic coverage remains incomplete.
