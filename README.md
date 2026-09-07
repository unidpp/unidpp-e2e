# unidpp-e2e

The public, end-to-end demonstration orchestrator — **the JTC 5 showpiece**.
Walks the [Momiji Mobility E8](../../isoiecjtc5/exemplar/STORY.md) story
beats (B1–B10) against the running registry, signed Tier-A packs, and the
offline verifier. Exits non-zero on any unexpected verify outcome.

Source of truth: [`isoiecjtc5/exemplar/STORY.md`](../../isoiecjtc5/exemplar/STORY.md).
This repo runs the story; it does not document it.

## What runs end-to-end today

```
$ make demo

UniDPP end-to-end — the Momiji Mobility E8 (STORY.md beats B1-B10)
    run: 2026-09-07T07:59:18Z
    issuance: unidpp-cli (TODO #11) — the unidpp-issuer service binary is
    not present yet; scripts/issuer-hook.sh switches to it automatically

[orchestrator] starting unidpp-registry on 127.0.0.1:8098
    unidpp-registry healthy at http://127.0.0.1:8098 (19135 item service, TODO #12)

======================================================================
B1 — Assembly in Kyoto (issuance where duty attaches)
----------------------------------------------------------------------
    what just happened: the JP type passport and the instance passport
    exist, and the build record lists every part at finest recorded
    granularity.
  $ unidpp create --id local:momiji:e8/type/2027.1 --capability S0 --eo momiji-mobility \
                   --passport-id urn:unidpp:passport:momiji-e8-type-2027-1 --out e8-type.json --resolver …
  $ unidpp create --id local:momiji:e8/J-000842 --type momiji:e8/type/2027.1 --capability S2 ...
  $ unidpp create --id local:rhine:du/M-771233 --capability S1 ...
  $ unidpp create --id local:weilian:wp/P-9904  --capability S2 ...
  $ unidpp create --id local:haichuan:cell/H-2231 --capability S0 ...
  $ unidpp event … --type issuance --data {"derived":false,"inputs":[]} …

…
======================================================================
B10 — End of life (the material loop closes)
----------------------------------------------------------------------
    out:  21.4 kg  ->  urn:unidpp:passport:scrap-steel-j000842
    out:   4.3 kg  ->  urn:unidpp:passport:recycle-pack-j000842
    in :  25.9 kg  (declared mass)
    loss = in - out = 0.2 kg  (auditable)

STORY COMPLETE — B1 through B10
    checks:   10/10 passed
    artifacts: build/e2e (passports, packs, registry responses)
DEMO PASSED — all verify outcomes matched the story.
```

## Beats → commands

Every beat is labeled with its STORY number and a one-line "what just
happened". The commands below are the exact `unidpp` / `curl` lines the
orchestrator runs.

| Beat | STORY.md section | What the demo does |
|---|---|---|
| **B1** | issuance where duty attaches | `unidpp create` for the JP type, instance, drive unit, pack, and lot passports; `unidpp event --type issuance` on each. |
| **B2** | parts carry their own duties | `unidpp event --type install` on the bike (outgoing edges to drive + pack) and on each part (incoming); dormant identifiers recorded to the lot at absorbing recoverability; **registry POST /transforms** registers the CCC ≅ IEC-62368-1 equivalence. |
| **B3** | placement in the EU | **registry POST /items** (EU LMT profile) + **POST /applicability** (effective 2028-02-01); queries at 2027-06-01 and 2028-06-01 prove the dated binding (`/applicability?at=`); then `unidpp event --type custody.transfer` for the placement + sale. |
| **B4** | the border moment | `unidpp pack --key e8-demo-pack-seed` mints a Tier-A pack with REAL ECDSA-P256 signature (anchor hex printed); **officer: offline**; `unidpp verify pack --anchor <pinned> --as-of 2028-02-15T09:30:00Z` → **verdict PASS** with the three readings (cryptographic / evidentiary / current-state) and 10/10 field coverage. |
| **B5** | edge state | `unidpp event --type milestone.record` (BMS cycle count, odometer); `unidpp event --type inspection.stamp` (service-lens stamp). |
| **B6** | the derestriction | `unidpp event --type software.update` (OTA); `product.modify` (dongle, derived_type set) + `status.change issued→non-conformant`; `unidpp pack` + **verify → FAIL** (the legal-class change); then `product.modify` (dongle off) + `status.change non-conformant→issued` for the re-evaluation. |
| **B7** | repair | `uninstall` (old pack, harvested) + `install` (new Voltaro EU pack) on the bike; mirror on the new pack; provenance custody-transfer to refurbisher. `unidpp pack` the new pack; **verify → PASS**. |
| **B8** | resale/auction | `custody.transfer` to auction (counterparty_signed); `inspection.stamp` (auction lens); `custody.transfer` to buyer. **Verify → PASS**. |
| **B9** | recall | `unidpp event --type recall.campaign` on the **lot passport** (predicate `FactContains bom.lots = H-2231`); the same on the **original pack** (the local custodian's evaluation). Mint and **verify both packs**: original → **FAIL** (recall-active), current → **degraded** (only freshness; safety finding is clean — degradation is explicit, never silent). |
| **B10** | end of life | New material passports (steel scrap, pack recycling) issued; `decompose` on the bike with mass-balance carve-outs; `end-of-waste` on the scrap passport. Verify scrap → **degraded** (end-of-waste status, not by trust); verify bike under `--max-age 0` (archival) → **degraded** (transformed). Mass balance printed from the artifact. |

The full annotated transcript is in `build/e2e/transcript.txt`.

## Layout

```
unidpp-e2e/
├── Makefile                demo | test | deps | up (compose) | down | clean
├── docker-compose.yml     optional: registry + (profile=issuer) issuer
├── scripts/
│   ├── demo.sh             the orchestrator (~700 lines, shellcheck-clean)
│   └── issuer-hook.sh      SINGLE clearly-marked issuer integration point
└── tests/
    └── run_tests.sh        the shell test harness (4 tests)
```

The sibling repos (`../unidpp-registry`, `../unidpp-cli`,
`../unidpp-issuer`) must live side-by-side; the build paths assume the
unippp workspace layout.

## Running

```sh
# Run the demo end to end.
make demo

# Run the test harness (happy path + tamper + missing-anchor + registry).
make test
```

The orchestrator builds `unidpp-cli` and `unidpp-registry` from source
on first run (release profile). Subsequent runs are fast (incremental).

Useful environment variables:

| Variable | Default | Meaning |
|---|---|---|
| `UNIDPP_REGISTRY_URL` | `http://127.0.0.1:8098` | external registry (skips the local start) |
| `UNIDPP_REGISTRY_BIND` | `127.0.0.1:8098` | bind address when starting locally |
| `UNIDPP_ISSUER_URL` | unset | activates the `unidpp-issuer` service mode (TODO #10) |
| `UNIDPP_ISSUER_BIND` | `127.0.0.1:8096` | issuer bind address when starting locally |
| `UNIDPP_ISSUER_ADMIN_TOKEN` | unset (open) | bearer token sent to the issuer when set |
| `UNIDPP_E2E_WORK_DIR` | `build/e2e` | where artifacts land |

### Compose

```sh
docker compose up -d registry
make demo UNIDPP_REGISTRY_URL=http://localhost:8098

docker compose --profile issuer up -d
make demo UNIDPP_REGISTRY_URL=http://localhost:8098 \
            UNIDPP_ISSUER_URL=http://localhost:8096
```

## Integration status — what waits on the issuer binary

| Piece | Source | Status |
|---|---|---|
| `unidpp-registry` (TODO #12) | `../unidpp-registry` | **RUNNING locally today.** Axum service, 19135 items + 6 subregisters, JSONL journal, 17 unit + 7 integration tests. |
| `unidpp-cli` (TODO #11) | `../unidpp-cli` | **RUNNING locally today** (the default driver). Real ECDSA-P256 pack signatures, 50 unit + 14 integration tests. |
| `unidpp-issuer` (TODO #10) | `../unidpp-issuer` | **In flight (parallel agent) — and already integrated.** The hook at `scripts/issuer-hook.sh` speaks its JSON wire protocol (POST `/passports`, `/passports/{id}/events`, `/pack`) and was verified green against the in-tree debug binary: `UNIDPP_ISSUER_URL=http://…:8096 make demo` runs the full story with server-signed events (TrustMarker::Attested) and server-minted packs; re-runs against a stateful issuer journal hit the 409 fallback (identity never re-minted, I1). |
| `unidpp-trust`, `unidpp-resolver`, `unidpp-py`, … | sibling repos | not in scope for the demo. |

The `scripts/issuer-hook.sh` file is the **single, clearly-marked issuer
integration point**: every issuance step in the demo goes through its
three verbs (`issuer_create`, `issuer_event`, `issuer_mint_pack`). When
the issuer binary lands, switch modes by setting `UNIDPP_ISSUER_URL=…`
(the hook then auto-starts the binary if none is listening, drives the
whole story through its real HTTP API, and syncs the local mirror
documents after each call). Nothing else in the orchestrator changes.

## Tests

`make test` runs four checks against a freshly-built demo:

1. **Happy path.** The E8 demo runs end-to-end; the transcript contains
   every STORY beat label B1–B10 and the story-expected verify outcomes.
2. **Tamper detection.** Flipping a byte in the signed Tier-A pack makes
   `unidpp verify` return **FAIL** (exit 2) — the cryptographic pipeline
   caught the tamper.
3. **Missing anchor.** Verifying without `--anchor` degrades (exit 1),
   never silently PASSes.
4. **Registry dated binding.** EU profile applicability is empty before
   2028-02-01 and non-empty after — the dated-binding machinery.

## Conventions

- The orchestrator is **shellcheck-clean** under `bash 3.2+` (macOS
  default), `set -u` + `set -o pipefail` + `set -o nounset`, no
  associative arrays or `mapfile`.
- No `jq` dependency: JSON is read with `python3` (always present in
  the family toolchain).
- Exit codes mirror the CLI: `0 = pass`, `1 = degraded`, `2 = fail`,
  `3 = usage`. Unexpected verify outcomes fail the demo loudly rather
  than fabricating a pass.
- Artifacts land in `build/e2e/`. The transcript doubles as the human
  narration and the assertion target (ANSI-stripped by the test harness).

## License

Apache-2.0, matching the rest of the UniDPP workspace.
