#!/usr/bin/env bash
# tests/run_tests.sh — the shell test harness.
#
# What this tests:
#   1. Happy path: the E8 demo runs end to end and exits 0; the
#      transcript contains every beat label B1-B10; the story-expected
#      verify outcomes are present (B4 PASS, B9 old FAIL / new PASS,
#      B10 scrap PASS).
#   2. Tamper detection: flip a byte in a signed pack; verify exit
#      becomes 2 (FAIL) — the cryptographic pipeline caught it.
#   3. Missing anchor: verify without --anchor degrades (exit 1)
#      instead of fabricating a PASS.
#   4. Registry as-of: the EU lens binds for the bike type ref AFTER
#      2028-02-01 and not before (the dated-binding machinery).
#   5. Live services: registry + issuer + trust + log all run for
#      real; a B1-B4 subset verifies with the anchor pinned from the
#      trust service's /keyring and anchors packs in the log. SKIPS
#      (not fails) when the sibling binaries cannot be produced.
#   6-8. The adoption-path quickstarts (FW-6): verify-only (one
#      binary, zero services), publish-only (one issuer, nothing
#      else), augment-existing (the gateway as translation edge over
#      one issuer). Each SKIPS when its binaries cannot be produced.
#   9. Tenant isolation at the service level (SV-7): two registries
#      as two tenants; tenant A's bearer token is refused (401) by
#      tenant B's registry, and A's write never appears in B's
#      journal.
#   The B-INT interop beat (render to UNTP, ingest back through
#   unidpp-gateway) rides inside tests 1 and 5; both assert its label
#   and its identity-match check.
#
# A failing assertion prints the diagnostic and the script returns the
# assertion's exit status. `make test` chains them; a single failure
# stops the chain.

set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$HERE/.." && pwd)"
TEST_WORK="${UNIDPP_E2E_TEST_WORK:-$ROOT_DIR/build/test}"
DEMO="$HERE/../scripts/demo.sh"
UNIDPP="${UNIDPP_BIN:-$ROOT_DIR/../unidpp-cli/target/release/unidpp}"

pass=0
fail=0
skipped=0

assert() { # assert <description> <expected> <actual>
    assert_desc="$1"
    assert_expected="$2"
    assert_actual="$3"
    if [ "$assert_expected" = "$assert_actual" ]; then
        printf '  \033[32m[ok]\033[0m   %s\n' "$assert_desc"
        pass=$((pass + 1))
    else
        printf '  \033[31m[FAIL]\033[0m %s: expected %q, got %q\n' \
            "$assert_desc" "$assert_expected" "$assert_actual"
        fail=$((fail + 1))
    fi
}

assert_grep() { # assert_grep <file> <pattern> <description>
    assert_desc="$1"
    assert_file="$2"
    assert_pattern="$3"
    if grep -Eq "$assert_pattern" "$assert_file"; then
        printf '  \033[32m[ok]\033[0m   %s\n' "$assert_desc"
        pass=$((pass + 1))
    else
        printf '  \033[31m[FAIL]\033[0m %s: pattern not found: %s\n' \
            "$assert_desc" "$assert_pattern"
        fail=$((fail + 1))
    fi
}

# ---------------------------------------------------------------------------
# Test 1: happy path (the E8 demo)
# ---------------------------------------------------------------------------

test_happy_path() {
    printf '\n\033[1m== test 1 ==\033[0m  happy path: demo.sh walks B1-B10 successfully\n'
    mkdir -p "$TEST_WORK"
    rm -rf "$ROOT_DIR/build/e2e"

    if ! "$DEMO" >"$TEST_WORK/demo.stdout" 2>"$TEST_WORK/demo.stderr"; then
        printf '  \033[31m[FAIL]\033[0m demo.sh exited non-zero; tail of stderr:\n'
        tail -25 "$TEST_WORK/demo.stderr"
        fail=$((fail + 1))
        return 1
    fi

    transcript="$ROOT_DIR/build/e2e/transcript.txt"
    [ -f "$transcript" ] || { printf '  [FAIL] no transcript\n'; fail=$((fail + 1)); return 1; }

    # Strip ANSI escape sequences for stable pattern matching; the
    # transcript doubles as the human narration AND the assertion target.
    plain="$TEST_WORK/transcript.plain.txt"
    sed -E $'s/\x1B\\[[0-9;]*[A-Za-z]//g' "$transcript" >"$plain"

    beats_present="$(grep -cE '^B[0-9]+ — ' "$plain" || true)"
    assert "all ten STORY beats labelled" 10 "$beats_present"
    # The CTO composition beat rides between B2 and B3.
    cto_present="$(grep -cF 'B-CTO — The build-to-order variant' "$plain" || true)"
    assert "CTO composition beat labelled" 1 "$cto_present"
    cto_children="$(grep -cF 'CTO instance outgoing installs == 2 == 2' "$plain" || true)"
    assert "CTO composed instance shows its two children" 1 "$cto_children"

    # The S12 interop beat rides between B-CTO and B3: the gateway
    # renders the passport as the UNTP triad and ingests it back. The
    # identity-match check line only prints in its [ok] form on a real
    # round trip (the [FAIL] form carries "expected", not "==").
    int_present="$(grep -cF 'B-INT — S12 interop' "$plain" || true)"
    assert "S12 interop beat labelled" 1 "$int_present"
    assert_grep "B-INT identity round-trip check" "$plain" \
        'B-INT ingested identity round-trips to the source passport == '
    assert_grep "B-INT re-ingest matched" "$plain" \
        'B-INT re-ingest matches .idempotent per subject. == matched'

    # The grid beat rides between B-QUORUM and B10 (Phase 1 of the
    # build contract): the sealed segment proven from the spine alone.
    grid_present="$(grep -cF 'G-GRID — The grid' "$plain" || true)"
    assert "grid beat labelled" 1 "$grid_present"
    assert_grep "G-GRID sealed segment verified from the spine" "$plain" \
        'G-GRID the sealed segment verifies from the spine alone == ok'
    if grep -q "cycle_count=412" "$plain"; then
        assert "G-GRID sealed contents never leak" never leaked
    else
        assert "G-GRID sealed contents never leak" never never
    fi
    # Phase 2: the CN battery case (S13 offer, substitution, grading).
    assert_grep "G-GRID S13 attestation offer" "$plain" \
        'G-GRID S13 offers attestation, not data == ok'
    assert_grep "G-GRID sovereign substitution" "$plain" \
        "G-GRID substitution verifies under the verifier's own anchors == ok"
    assert_grep "G-GRID coverage-graded verdict" "$plain" \
        'G-GRID the verdict is a coverage report object .verified-direct . attested. == ok' 

    # The quorum beat rides between B9 and B10: retroactive distrust
    # of an authority as a quorate M-of-K act (2-of-3 jurisdictions).
    quorum_present="$(grep -cF 'B-QUORUM — Retroactive distrust' "$plain" || true)"
    assert "quorum beat labelled" 1 "$quorum_present"
    # The outcome checks hold only when the beat RAN (the trust and
    # quorum-ceremony binaries present); a narrated skip is honest.
    if grep -qF 'B-QUORUM narrated without running' "$plain"; then
        printf '  \033[33m[skip]\033[0m quorum beat narrated only (trust binaries absent)\n'
    else
        assert_grep "B-QUORUM single regulator refused" "$plain" \
            'B-QUORUM single regulator refused .422 — quorum attestation required. == 422'
        assert_grep "B-QUORUM quorate declaration accepted" "$plain" \
            'B-QUORUM quorate 2-of-3 declaration accepted .201. == 201'
        assert_grep "B-QUORUM verdict degrades through the standing overlay" "$plain" \
            'B-QUORUM the pack verdict degrades: in-window verifications no longer stand == false'
    fi

    for beat_label in 'B1 — Assembly in Kyoto' \
                      'B2 — Parts carry their own duties' \
                      'B3 — Placement in the EU' \
                      'B4 — The border moment' \
                      'B5 — Life in service' \
                      'B6 — Firmware update and the derestriction incident' \
                      'B7 — Repair' \
                      'B8 — Resale and auction' \
                      'B9 — A recall crosses the graph' \
                      'B10 — End of life'; do
        # Use python rather than grep: bash 3.2's regex loses the UTF-8
        # boundary of multi-byte characters in some locales.
        if python3 -c 'import sys,sys; sys.stdout.write("ok\n" if sys.argv[1].startswith(sys.argv[2]) else "no\n")' \
                "$(grep -F "${beat_label%% *}" "$plain" | grep -F "$beat_label" | head -1)" \
                "$beat_label" | grep -qx ok; then
            printf '  \033[32m[ok]\033[0m   beat label present: %s\n' "$beat_label"
            pass=$((pass + 1))
        else
            printf '  \033[31m[FAIL]\033[0m beat label missing: %s\n' "$beat_label"
            fail=$((fail + 1))
        fi
    done

    # Story-expected verify outcomes. ERE .* matching of multi-byte
    # em-dashes is reliable in grep when the locale is C; switch.
    LC_ALL=C
    export LC_ALL
    # The "PASS" assertion matches either the rendered verdict line or
    # the `check "verify verdict ..." == <code>` helper line (both end
    # in PASS / == 0 / == 2 respectively).
    assert_grep "B4 border-moment PASS"   "$plain" "B4 border moment.*(PASS|== 0)"
    assert_grep "B6 derestriction FAIL"  "$plain" "B6 derestriction.*(FAIL|== 2)"
    assert_grep "B7 new pack PASS"        "$plain" "B7 new pack .post-swap..*== 0"
    assert_grep "B8 post-auction PASS"    "$plain" "B8 post-auction.*== 0"
    assert_grep "B9 old pack FAIL"        "$plain" "B9 original pack.*(FAIL|== 2)"
    assert_grep "B9 new pack degraded"    "$plain" "B9 Vienna bike.*unaffected"
    assert_grep "B10 scrap EoW degraded"    "$plain" "B10 end-of-waste.*(DEGRADED|== 1)"
    assert_grep "B10 bike degraded"       "$plain" "B10 decomposed bike.*degraded"
    assert_grep "closing line"             "$plain" "DEMO PASSED"

    printf '  artifacts under: %s/build/e2e/\n' "$ROOT_DIR"
}

# ---------------------------------------------------------------------------
# Test 2: tamper detection — flipping a byte in a signed pack must FAIL
# ---------------------------------------------------------------------------

test_tamper_detection() {
    printf '\n\033[1m== test 2 ==\033[0m  tamper detection: byte-flipped pack fails verification\n'
    pack="$ROOT_DIR/build/e2e/b4-border.pack"
    transcript="$ROOT_DIR/build/e2e/transcript.txt"
    plain="$TEST_WORK/transcript.plain.txt"
    [ -f "$pack" ] || { printf '  [FAIL] missing pack %s (run the happy path first)\n' "$pack"; fail=$((fail + 1)); return 1; }

    # Read the anchor from the plain transcript: the colored file has
    # ANSI escape sequences that would be captured into the hex.
    anchor="$(grep -oE 'anchor \(public key, hex\): [0-9a-fA-F]+' "$plain" \
        | head -1 | awk '{print $NF}')"
    [ -n "$anchor" ] || { printf '  [FAIL] could not read issuer anchor from transcript\n'; fail=$((fail + 1)); return 1; }

    bytes_file="$TEST_WORK/pack.bytes"
    # `unidpp verify` accepts hex text in the positional arg or a path to
    # a hex text file. Build a tampered hex file by flipping one hex
    # digit in the middle.
    orig_hex="$(cat "$pack")"
    mid=$(( ${#orig_hex} / 2 ))
    head="${orig_hex:0:mid}"
    tail="${orig_hex:mid}"
    # Flip one ASCII hex digit; the head and tail remain identical chars.
    first_char="${tail:0:1}"
    case "$first_char" in
        0) flipped=1 ;;
        *) flipped=0 ;;
    esac
    tampered_hex="${head}${flipped}${tail:1}"
    printf '%s' "$tampered_hex" >"$bytes_file"

    if "$UNIDPP" verify "$bytes_file" --anchor "$anchor" --as-of '2028-02-15T09:30:00Z' \
        >/dev/null 2>&1; then
        ve=0
    else
        ve=$?
    fi
    [ "$ve" = 127 ] && { printf '  [FAIL] unidpp CLI not found at %s\n' "$UNIDPP"; fail=$((fail + 1)); return 1; }
    assert "tampered pack yields FAIL (exit 2)" 2 "$ve"
}

# ---------------------------------------------------------------------------
# Test 3: missing anchor — degrades, never passes
# ---------------------------------------------------------------------------

test_missing_anchor() {
    printf '\n\033[1m== test 3 ==\033[0m  missing anchor: verifier degrades (exit 1), never PASS\n'
    pack="$ROOT_DIR/build/e2e/b4-border.pack"
    [ -f "$pack" ] || { printf '  [FAIL] missing pack %s\n' "$pack"; fail=$((fail + 1)); return 1; }

    if "$UNIDPP" verify "$pack" --as-of '2028-02-15T09:30:00Z' >/dev/null 2>&1; then
        ve=0
    else
        ve=$?
    fi
    assert "no anchor -> degraded (exit 1)" 1 "$ve"
}

# ---------------------------------------------------------------------------
# Test 4: registry dated binding
# ---------------------------------------------------------------------------

test_registry_binding() {
    printf '\n\033[1m== test 4 ==\033[0m  registry: EU lens binds after 2028-02-01 only\n'
    json_2027="$ROOT_DIR/build/e2e/reg-applicability-2027.json"
    json_2028="$ROOT_DIR/build/e2e/reg-applicability-2028.json"
    [ -f "$json_2027" ] && [ -f "$json_2028" ] \
        || { printf '  [FAIL] registry responses missing (run the happy path first)\n'; fail=$((fail + 1)); return 1; }

    n_2027="$(python3 -c 'import json,sys;print(len(json.load(open(sys.argv[1]))["applicability"]))' \
        "$json_2027")"
    n_2028="$(python3 -c 'import json,sys;print(len(json.load(open(sys.argv[1]))["applicability"]))' \
        "$json_2028")"
    assert "EU profiles bound before 2028-02-01" 0 "$n_2027"
    assert "EU profiles bound after 2028-02-01" 1 "$n_2028"
}

# ---------------------------------------------------------------------------
# Test 5: live services — B1+B4 subset against registry+issuer+trust+log
# ---------------------------------------------------------------------------

test_live_services() {
    printf '\n\033[1m== test 5 ==\033[0m  live services: B1-B4 subset over registry + issuer + trust + log\n'

    # The live test needs the three service binaries on top of the CLI
    # and registry the earlier tests already built, plus the gateway
    # (the B-INT beat rides inside the B1-B4 subset). Build any
    # missing one; SKIP (not FAIL) when a binary cannot be produced —
    # the sibling repos are developed in parallel and can be mid-edit.
    for live_repo in unidpp-issuer unidpp-trust unidpp-log unidpp-gateway; do
        live_bin="$ROOT_DIR/../$live_repo/target/release/$live_repo"
        [ -x "$live_bin" ] && continue
        printf '  ....... building missing %s\n' "$live_repo"
        cargo build --release --manifest-path \
            "$ROOT_DIR/../$live_repo/Cargo.toml" >/dev/null 2>&1 || true
    done
    for live_repo in unidpp-issuer unidpp-trust unidpp-log unidpp-gateway; do
        live_bin="$ROOT_DIR/../$live_repo/target/release/$live_repo"
        if [ ! -x "$live_bin" ]; then
            printf '  \033[33m[SKIP]\033[0m live test: %s binary unavailable (build failed or repo absent)\n' \
                "$live_repo"
            skipped=$((skipped + 1))
            return 0
        fi
    done

    live_work="$TEST_WORK/live"
    rm -rf "$live_work"
    if ! UNIDPP_E2E_LIVE_DIR="$live_work" UNIDPP_E2E_STOP_AFTER=B4 \
        "$ROOT_DIR/scripts/demo-live.sh" \
        >"$TEST_WORK/demo-live.stdout" 2>"$TEST_WORK/demo-live.stderr"; then
        printf '  \033[31m[FAIL]\033[0m demo-live.sh exited non-zero; tail of stderr:\n'
        tail -25 "$TEST_WORK/demo-live.stderr"
        fail=$((fail + 1))
        return 1
    fi

    transcript="$live_work/e2e/transcript.txt"
    [ -f "$transcript" ] || { printf '  [FAIL] no live transcript\n'; fail=$((fail + 1)); return 1; }
    plain="$TEST_WORK/transcript-live.plain.txt"
    sed -E $'s/\x1B\\[[0-9;]*[A-Za-z]//g' "$transcript" >"$plain"

    assert_grep "live topology header present" "$plain" 'topology: LIVE'
    assert_grep "topology lists the registry" "$plain" 'registry : http'
    assert_grep "topology lists the issuer" "$plain" 'issuer   : http.*server-minted packs'
    assert_grep "topology lists the trust service" "$plain" 'trust    : http.*verify anchor source'
    assert_grep "topology lists the log" "$plain" 'log      : http.*signed receipts'
    assert_grep "trust anchor pinned from /keyring" "$plain" 'anchor pinned from .*/keyring'
    assert_grep "issuer and trust anchors identical" "$plain" '\[ok\].*issuer pack anchor == trust-pinned anchor'
    assert_grep "B4 verify PASS under the live anchor" "$plain" 'B4 border moment.*(PASS|== 0)'
    assert_grep "log receipts narrated with ids" "$plain" 'log: +receipt [0-9]+ '
    assert_grep "B-INT beat in the live run" "$plain" 'B-INT — S12 interop'
    assert_grep "B-INT renders from the live issuer" "$plain" \
        'B-INT render source is the live issuer == issuer'
    assert_grep "B-INT identity round-trip in the live run" "$plain" \
        'B-INT ingested identity round-trips to the source passport == '
    assert_grep "live subset completed" "$plain" 'DEMO PASSED'

    # Independent receipt check, outside the orchestrator: the stored
    # B4 receipt's commitment must equal the sha256 of the pack file.
    receipt="$live_work/e2e/log-receipts/b4-border.receipt.json"
    pack="$live_work/e2e/b4-border.pack"
    if [ -f "$receipt" ] && [ -f "$pack" ]; then
        want="$(python3 -c 'import hashlib,sys;print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$pack")"
        got="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["commitment"])' "$receipt")"
        assert "b4 log receipt commitment == sha256(pack)" "$want" "$got"
    else
        printf '  \033[31m[FAIL]\033[0m missing b4 receipt or pack artifact\n'
        fail=$((fail + 1))
    fi
}

# ---------------------------------------------------------------------------
# Tests 6-8: the adoption-path quickstarts (FW-6) — each script is
# self-asserting; the harness only records its exit. Exit 77 = SKIP
# (a binary the path needs is unavailable — a sibling repo mid-edit,
# never a failure of the path itself).
# ---------------------------------------------------------------------------

run_quickstart() { # run_quickstart <number> <label> <script>
    rq_n="$1"
    rq_label="$2"
    rq_script="$3"
    printf '\n\033[1m== test %s ==\033[0m  %s\n' "$rq_n" "$rq_label"
    if "$rq_script" >"$TEST_WORK/quickstart-$rq_n.stdout" 2>&1; then
        cat "$TEST_WORK/quickstart-$rq_n.stdout"
    else
        rq_code=$?
        if [ "$rq_code" -eq 77 ]; then
            cat "$TEST_WORK/quickstart-$rq_n.stdout"
            printf '  \033[33m[SKIP]\033[0m %s\n' "$rq_label"
            skipped=$((skipped + 1))
        else
            cat "$TEST_WORK/quickstart-$rq_n.stdout"
            printf '  \033[31m[FAIL]\033[0m %s exited %s\n' "$rq_label" "$rq_code"
            fail=$((fail + 1))
        fi
    fi
}

test_quickstart_verify_only() {
    run_quickstart 6 "adoption path: verify-only (one binary, zero services)" \
        "$HERE/../scripts/quickstart-verify-only.sh"
}

test_quickstart_publish_only() {
    run_quickstart 7 "adoption path: publish-only (one issuer, nothing else)" \
        "$HERE/../scripts/quickstart-publish-only.sh"
}

test_quickstart_gateway() {
    run_quickstart 8 "adoption path: augment-existing (the gateway translation edge)" \
        "$HERE/../scripts/quickstart-gateway.sh"
}

# ---------------------------------------------------------------------------
# Test 9: SV-7 at the service level — two registries as two tenants;
# credentials do not cross, journals do not mix.
# ---------------------------------------------------------------------------

test_tenant_isolation_services() {
    printf '\n\033[1m== test 9 ==\033[0m  tenant isolation: cross-tenant credentials refused at the service level\n'

    registry_bin="$ROOT_DIR/../unidpp-registry/target/release/unidpp-registry"
    if [ ! -x "$registry_bin" ]; then
        printf '  \033[33m[SKIP]\033[0m registry binary unavailable\n'
        skipped=$((skipped + 1))
        return 0
    fi

    iso_work="$TEST_WORK/isolation"
    rm -rf "$iso_work"
    mkdir -p "$iso_work"

    UNIDPP_REGISTRY_BIND=127.0.0.1:18541 \
    UNIDPP_REGISTRY_STATE_FILE="$iso_work/tenant-a.json" \
    UNIDPP_REGISTRY_ADMIN_TOKEN=tenant-a-token \
        "$registry_bin" >"$iso_work/tenant-a.log" 2>&1 &
    pid_a=$!
    UNIDPP_REGISTRY_BIND=127.0.0.1:18542 \
    UNIDPP_REGISTRY_STATE_FILE="$iso_work/tenant-b.json" \
    UNIDPP_REGISTRY_ADMIN_TOKEN=tenant-b-token \
        "$registry_bin" >"$iso_work/tenant-b.log" 2>&1 &
    pid_b=$!
    trap 'kill $pid_a $pid_b 2>/dev/null; wait $pid_a $pid_b 2>/dev/null' RETURN

    ready=0
    for _ in $(seq 1 50); do
        curl -sf http://127.0.0.1:18541/healthz >/dev/null 2>&1 && \
        curl -sf http://127.0.0.1:18542/healthz >/dev/null 2>&1 && { ready=1; break; }
        sleep 0.2
    done
    if [ "$ready" != 1 ]; then
        printf '  \033[31m[FAIL]\033[0m the two tenant registries never became healthy\n'
        fail=$((fail + 1))
        return 0
    fi
    assert "both tenant registries healthy" 1 1

    body_a='{"register_id":"tenant-a","item_id":"shared-name","version":"1","definition":"tenant A item","class":"data-element"}'
    body_b='{"register_id":"tenant-b","item_id":"shared-name","version":"1","definition":"tenant B item","class":"data-element"}'

    # A's own credential writes at A.
    code_a="$(curl -s -o "$iso_work/a-own.json" -w '%{http_code}' -X POST \
        http://127.0.0.1:18541/items -H 'authorization: Bearer tenant-a-token' \
        -H 'content-type: application/json' --data "$body_a")"
    assert "tenant A writes at its own registry" 201 "$code_a"

    # A's credential is REFUSED at B (the cross-tenant probe).
    code_cross="$(curl -s -o "$iso_work/a-at-b.json" -w '%{http_code}' -X POST \
        http://127.0.0.1:18542/items -H 'authorization: Bearer tenant-a-token' \
        -H 'content-type: application/json' --data "$body_a")"
    assert "tenant A's credential refused at tenant B (401)" 401 "$code_cross"

    # B's own write succeeds at B — the refusal was the credential,
    # not the item name.
    code_b="$(curl -s -o "$iso_work/b-own.json" -w '%{http_code}' -X POST \
        http://127.0.0.1:18542/items -H 'authorization: Bearer tenant-b-token' \
        -H 'content-type: application/json' --data "$body_b")"
    assert "tenant B writes the same item id at its own registry" 201 "$code_b"

    # The journals do not mix: B's store holds B's item, not A's.
    b_def="$(curl -s http://127.0.0.1:18542/items/shared-name | \
        python3 -c 'import json,sys; print(json.load(sys.stdin).get("register", "absent"))')"
    assert "tenant B's journal holds B's item only" "tenant-b" "$b_def"
    a_def="$(curl -s http://127.0.0.1:18541/items/shared-name | \
        python3 -c 'import json,sys; print(json.load(sys.stdin).get("register", "absent"))')"
    assert "tenant A's journal holds A's item only" "tenant-a" "$a_def"

    kill $pid_a $pid_b 2>/dev/null
    wait $pid_a $pid_b 2>/dev/null
}

main() {
    mkdir -p "$TEST_WORK"

    test_happy_path
    test_tamper_detection
    test_missing_anchor
    test_registry_binding
    test_live_services
    test_quickstart_verify_only
    test_quickstart_publish_only
    test_quickstart_gateway
    test_tenant_isolation_services

    printf '\n\033[1m== summary ==\033[0m  %d passed, %d failed, %d skipped\n' \
        "$pass" "$fail" "$skipped"
    [ "$fail" = 0 ]
}

main "$@"
