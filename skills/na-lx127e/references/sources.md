# Sources, page lookup and errors

## Supplied sources

The supplied PDFs are bundled in [manuals/](manuals/). All manual lookups use these repository files; no external source directory is required.

| Document | Filename | PDF pages | SHA-256 |
| --- | --- | --- | --- |
| ご使用ガイド | [洗濯機_ご使用ガイド_NA-LX127E.pdf](manuals/洗濯機_ご使用ガイド_NA-LX127E.pdf) | 8 | e33416d7f1a2e602d4619d5c50bb496a6f3428f9abab36151d4cc31dcb08334d |
| 取扱説明書 | [洗濯機_取扱説明書_NA-LX127E.pdf](manuals/洗濯機_取扱説明書_NA-LX127E.pdf) | 27 | bdd419de3f1ea87830c197c23343d7de4305e350a7a0e00ea525c62f1f228c3e |

The main manual is issued July 2025 (AWW9901ADJA0); the guide is AWW9960ADJA0-0A0, with detergent examples dated March 2025. These are supplied-document facts, not claims about current product formulations or service availability.

## Page retrieval

`manual_page.py` requires Python 3 and PyMuPDF. Use an available environment or install `pymupdf` in a temporary virtual environment. From the repository root:

```sh
python3 skills/na-lx127e/scripts/manual_page.py manual 25 --render /tmp/na-lx127e-errors.png
python3 skills/na-lx127e/scripts/manual_page.py guide 6 --render /tmp/na-lx127e-dosing-care.png
```

The number is a **one-based PDF page**. Inspect the resulting PNG for visual content. The helper resolves bundled PDFs relative to its own location, independently of the current working directory. Do not fall back to files outside this repository.

The main manual has a cover at PDF page 1; printed pp.2–51 are two-page spreads. For printed page p (2 through 51), PDF page = floor(p / 2) + 1. PDF page 27 is the back cover. Guide printed and PDF page numbers match.

| Topic | Printed manual pages | PDF pages |
| --- | --- | --- |
| Safety and limitations | 4–6 | 3–4 |
| Panel, door/child locks | 8–9 | 5 |
| Features, dosing overview | 10–13 | 6–7 |
| Clothing and bedding compatibility/preparation | 14–17 | 8–9 |
| Course compatibility/capacity table | 18–19 | 10 |
| Basic operation and reservation | 20–21 | 11 |
| Special courses / custom settings / individual stages | 22–25 | 12–13 |
| Drying | 26–27 | 14 |
| Down jacket / steam care | 28–29 | 15 |
| Persistent settings | 30–31 | 16 |
| Drum care | 32–33 | 17 |
| Automatic dosing Q&A | 34–35 | 18 |
| Laundry tips / manual dosage | 36–38 | 19–20 |
| Symptoms and troubleshooting | 39–45 | 20–23 |
| Cycle times and reservation ranges | 46–47 | 24 |
| Errors / specifications | 48–49 | 25 |
| Accessories/service / app connectivity | 50–51 | 26 |

Guide: basic use p.1; dosing setup pp.2–3; compatible dosing courses/tanks p.4; tank/path and body cleaning pp.5–6; filters p.7; manual dosage p.8.

## Hardware errors

Source: 取扱説明書 p.48 (PDF p.25), visually checked; filter instructions in ご使用ガイド p.7. Use actual panel errors; do not equate HTTP/cloud failures with appliance U/H codes.

| Display | Meaning and first checks |
| --- | --- |
| U04 | Drying filter clogged. Power off; remove and wash both main and rear drying filters, dry thoroughly and reinstall. |
| U11 | Cannot drain. Check drain hose/outlet/filter, kinks, freezing, submerged end, rise above 10 cm or extension over 2 m. If unresolved, contact dealer. Do not remove a water-filled filter unsafely. |
| U12 | Door lock problem. Close until it clicks; if already closed, open and push laundry inside, then close. If it will not open, the manual says press the door firmly; do not force it open. |
| U13 | Cannot spin. Check too little laundry (add 1–2 bath towels), tangled/excess large items, nets-only load, unstable installation and drainage. Check spin-start adjustment p.30 if relevant. |
| U14 | Cannot fill. Check tap, water outage/freezing and inlet-filter blockage (p.45). |
| U18 | Drain filter not properly installed. Refit correctly. |
| H plus two digits | Close tap immediately, record exact digits, unplug and contact dealer/service. Do not diagnose by guessing or repeatedly restart. |

For U codes other than U04/U18, the manual says `スタート/一時停止` restarts after correction. If unresolved, unplug and contact the dealer. Do not apply that instruction as an automatic CLI retry policy. App errors U81–U86 require manual p.51, not the table above.
