---
name: na-lx127e
description: Operate and inspect this repository's Panasonic NA-LX127ER washer-dryer through its Rust CLI, including status, guarded wash-and-dry or wash-only starts, and operation troubleshooting. Use the NA-LX127E manuals as supporting hardware context for capacities, dosing, maintenance, and errors.
---

# NA-LX127E CLI operation

Use this repository's CLI as the primary interface. Hardware manual knowledge supports decisions about laundry and physical preparation; it does not establish which remote commands the CLI implements. Reply in the user's language and retain Japanese panel/course labels where useful.

## Operating workflow

1. Read [cli.md](references/cli.md) and the relevant current repository documentation/source before executing. Work from the repository root. Prefer supported CLI commands over constructing raw cloud requests.
2. For status questions, fetch fresh state with `status`; distinguish retrieval time from appliance measurement time. Keep unknown/partially decoded properties uncertain.
3. For an explicitly requested start, choose normal wash-and-dry or `--wash-only` according to intent. Use the CLI's matching `--dry-run` to inspect live prerequisites/settings, then execute the matching start once if they pass. The actual start rechecks state itself. Do not ask for redundant confirmation when the user has already authorized the operation.
4. Respect preflight failures. Do not bypass door, power, remote-permission, fault, reservation, idle-state, or dosing checks. If delivery is uncertain, inspect fresh state before any retry; never automatically repeat a start.
5. Report what is established: no write, API acceptance, or observed running state. API acceptance alone is not physical confirmation. A fresh status read after acceptance can provide additional evidence; unresolved state remains unresolved.

A request for advice/status does not authorize a physical start or schedule change. Authentication refresh and scheduling have their own effects: follow the requested scope and the current repository instructions. Keep `.local/` credentials, identifiers, and raw records private.

## Supporting hardware knowledge

Load only what the current task needs:

- [operation.md](references/operation.md): laundry preparation, course capacities, drying constraints.
- [care-and-dosing.md](references/care-and-dosing.md): detergent/tanks, maintenance timing and procedures.
- [sources.md](references/sources.md): error codes, manual provenance, original-page retrieval.

The manuals cover NA-LX127EL and NA-LX127ER; the current guarded CLI start specifically restricts the device to NA-LX127ER / NA-LX127E. Manual-only courses are not automatically remote features. For unsupported operations, explain the CLI limitation and provide relevant panel guidance when useful; do not silently substitute a different cycle.

When giving hardware advice, cite the document and **printed page**, e.g. `取扱説明書 p.20–21`. Use only the PDFs bundled under `references/manuals/`; no external manual files are needed. Inspect bundled page images for exact buttons, display codes, care symbols, tables and long-press sequences; text extraction omits these. Do not invent missing steps.

Never recommend drying oil-contaminated items even after washing, washing/spinning waterproof items, forcing locked doors, or removing a drain filter during operation. Keep dosing-path cleaning chemicals separate from drum-cleaning products. Check manual pp.4–6, 14–17 for garment exclusions; follow stop/service guidance for abnormal noise, burning smells and H errors.
