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
    # and registry the earlier tests already built. Build any missing
    # one; SKIP (not FAIL) when a binary cannot be produced — the
    # sibling repos are developed in parallel and can be mid-edit.
    for live_repo in unidpp-issuer unidpp-trust unidpp-log; do
        live_bin="$ROOT_DIR/../$live_repo/target/release/$live_repo"
        [ -x "$live_bin" ] && continue
        printf '  ....... building missing %s\n' "$live_repo"
        cargo build --release --manifest-path \
            "$ROOT_DIR/../$live_repo/Cargo.toml" >/dev/null 2>&1 || true
    done
    for live_repo in unidpp-issuer unidpp-trust unidpp-log; do
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

main() {
    mkdir -p "$TEST_WORK"

    test_happy_path
    test_tamper_detection
    test_missing_anchor
    test_registry_binding
    test_live_services

    printf '\n\033[1m== summary ==\033[0m  %d passed, %d failed, %d skipped\n' \
        "$pass" "$fail" "$skipped"
    [ "$fail" = 0 ]
}

main "$@"
