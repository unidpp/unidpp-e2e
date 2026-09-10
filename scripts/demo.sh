#!/usr/bin/env bash
# demo.sh — the UniDPP end-to-end story: the Momiji Mobility E8 walks
# the ten STORY.md beats against real services and real artifacts.
#
# Source of truth: the UniDPP E8 exemplar story (beats B1–B10).
# Every beat is labeled with its STORY number and a one-line "what just
# happened". Any command that does not produce its expected outcome
# aborts the run with a non-zero exit; every `unidpp verify` result is
# asserted against the beat's expected verdict (pass / degraded / fail)
# — an unexpected verify outcome fails the demo.
#
# Services: unidpp-registry (../unidpp-registry, ) is started
# locally and seeded with the story's profile applicability bindings.
# Issuance goes through scripts/issuer-hook.sh — today driven by the
# unidpp-cli , automatically switching to the unidpp-issuer
# service  once its binary exists (see that file).
# unidpp-gateway (../unidpp-gateway) runs the B-INT beat: the S12
# interop round trip — render the passport as the UNTP VC triad, feed
# the triad back through POST /untp/ingest. Issuer-driver runs inherit
# UNIDPP_ISSUER_URL so the gateway renders the REAL story passports;
# local-driver runs use its seeded fixtures (it binds :8398 — clear of
# the fixed live-service ports).
#
# LIVE mode (make demo-live / scripts/demo-live.sh): all four sibling
# services run for real — UNIDPP_ISSUER_URL (server-signed events,
# server-minted packs), UNIDPP_TRUST_URL (verify anchors pinned from
# GET /keyring), UNIDPP_LOG_URL (every minted pack's commitment
# anchored in the transparency log; receipts kept + narrated).

set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FAMILY_DIR="$(cd "$ROOT_DIR/.." && pwd)"
WORK_DIR="${UNIDPP_E2E_WORK_DIR:-$ROOT_DIR/build/e2e}"

UNIDPP="${UNIDPP_BIN:-$FAMILY_DIR/unidpp-cli/target/release/unidpp}"
REGISTRY_BIN="${UNIDPP_REGISTRY_BIN:-$FAMILY_DIR/unidpp-registry/target/release/unidpp-registry}"
REGISTRY_BIND="${UNIDPP_REGISTRY_BIND:-127.0.0.1:8098}"
REGISTRY_URL="${UNIDPP_REGISTRY_URL:-http://$REGISTRY_BIND}"

# The interop gateway (B-INT). :8398 stays clear of the fixed
# live-service ports (8092/8096/8098/8194) and of the gateway's own
# :8094 default, which a parallel deployment may hold.
GATEWAY_BIN="${UNIDPP_GATEWAY_BIN:-$FAMILY_DIR/unidpp-gateway/target/release/unidpp-gateway}"
GATEWAY_BIND="${UNIDPP_GATEWAY_BIND:-127.0.0.1:8398}"
GATEWAY_URL="http://$GATEWAY_BIND"
GATEWAY_PID=""

# The B-QUORUM beat's trust base: the live trust service in live mode
# (UNIDPP_TRUST_URL), else an ephemeral unidpp-trust started below.
# :8097 stays clear of the fixed live-service ports (8092/8096/8098/
# 8194) and of the gateway's :8398. The quorum-ceremony binary runs
# the REAL threshold ceremony (Feldman VSS + threshold Schnorr -> one
# standard Ed25519 group signature) and emits the HTTP bodies.
TRUST_BIN="${UNIDPP_TRUST_BIN:-$FAMILY_DIR/unidpp-trust/target/release/unidpp-trust}"
QUORUM_CEREMONY_BIN="${UNIDPP_QUORUM_CEREMONY_BIN:-$FAMILY_DIR/unidpp-trust/target/release/quorum-ceremony}"
QUORUM_BIND="${UNIDPP_QUORUM_BIND:-127.0.0.1:8097}"
QUORUM_URL="http://$QUORUM_BIND"
QUORUM_PID=""

# Live-service wiring (make demo-live / scripts/demo-live.sh):
#   UNIDPP_TRUST_URL — verify anchors are pinned from the trust
#     service's GET /keyring instead of the mint-returned fixture
#     anchor (see pin_trust_anchor);
#   UNIDPP_LOG_URL — every minted pack's commitment is anchored in
#     the transparency log (POST /commitments) and the signed
#     inclusion receipt is stored + narrated (see log_anchor_pack).
TRUST_URL="${UNIDPP_TRUST_URL:-}"
LOG_URL="${UNIDPP_LOG_URL:-}"
TRUST_ANCHOR=""
TRUST_KEY_ID=""
TRUST_MODE=""
LOG_ID=""
LOG_RECEIPTS="$WORK_DIR/log-receipts"

# STORY cast (STORY.md section 0 — the identities of every beat).
BIKE_ID="local:momiji:e8/J-000842"
TYPE_ID="local:momiji:e8/type/2027.1"
BIKE_URN="urn:unidpp:passport:momiji-e8-j000842"
TYPE_URN="urn:unidpp:passport:momiji-e8-type-2027-1"
DRIVE_URN="urn:unidpp:passport:rhine-du-m771"
PACK_URN="urn:unidpp:passport:weilian-wp-p9904"
LOT_URN="urn:unidpp:passport:haichuan-cell-h2231"
NEWPACK_URN="urn:unidpp:passport:voltaro-wp-eu7781"
SCRAP_URN="urn:unidpp:passport:scrap-steel-j000842"
RECYCLE_URN="urn:unidpp:passport:recycle-pack-j000842"

BIKE_TYPE_REF="momiji:e8/type/2027.1"

FAILED=0
CHECKS_TOTAL=0
CHECKS_OK=0
REGISTRY_PID=""
TRANSCRIPT="$WORK_DIR/transcript.txt"

# ---------------------------------------------------------------------------
# Narration helpers
# ---------------------------------------------------------------------------

hr() { printf '\n\033[1m%s\033[0m\n' "$*"; }

beat() { # beat <B-number> <title>
    printf '\n\033[1m======================================================================\033[0m\n'
    printf '\033[1m%s — %s\033[0m\n' "$1" "$2"
    printf '\033[1m----------------------------------------------------------------------\033[0m\n'
}

what() { # what just happened — the one-line beat narration
    printf '\033[36m    what just happened: %s\033[0m\n' "$*"
}

say() { # narrator voice (facts read out of the artifacts)
    printf '    %s\n' "$*"
}

note() { # orchestration notes (not story narration)
    printf '\033[33m    [orchestrator] %s\033[0m\n' "$*"
}

show() { # show the command the story is about to run
    printf '  \033[32m$ %s\033[0m\n' "$*"
}

# Run a command quietly: echo it, run it, abort the demo on failure.
run_quiet() {
    show "$*"
    if ! "$@" >/dev/null; then
        fail "command failed: $*"
    fi
}

check() { # check <label> <expected> <actual>
    check_label="$1"
    check_expected="$2"
    check_actual="$3"
    CHECKS_TOTAL=$((CHECKS_TOTAL + 1))
    if [ "$check_expected" = "$check_actual" ]; then
        CHECKS_OK=$((CHECKS_OK + 1))
        printf '  \033[32m[ok]\033[0m   %s == %s\n' "$check_label" "$check_expected"
    else
        FAILED=$((FAILED + 1))
        printf '  \033[31m[FAIL]\033[0m %s: expected %s, got %s\n' \
            "$check_label" "$check_expected" "$check_actual"
    fi
}

fail() {
    printf '\n\033[31mE8 STORY ABORTED: %s\033[0m\n' "$1" >&2
    exit 1
}

# Extract a top-level JSON field with python3 (no jq dependency).
json_get() { # json_get <file> <field>
    python3 - "$1" "$2" <<'PYEOF'
import json, sys
with open(sys.argv[1]) as fh:
    doc = json.load(fh)
print(doc[sys.argv[2]])
PYEOF
}

# Count the length of a JSON array field in a file.
python3_count() { # python3_count <file> <field>
    python3 - "$1" "$2" <<'PYEOF'
import json, sys
print(len(json.load(open(sys.argv[1]))[sys.argv[2]]))
PYEOF
}

# Count the outgoing installation edges in a passport document (the
# CTO composition check: the config vector resolves through the graph).
python3_count_installs() { # python3_count_installs <file>
    python3 - "$1" <<'PYEOF'
import json, sys
doc = json.load(open(sys.argv[1]))
events = doc.get("log", {}).get("sealed", [])
installs = [
    e["event"]["payload"]["Install"]["target"]["Open"]
    for e in events
    if e["event"].get("event_type") == "install"
]
print(len([i for i in installs if i["direction"] == "outgoing"]))
PYEOF
}

# The `other` passport of the n-th installation edge (either direction;
# the CTO child/parent knowledge check).
python3_install_child() { # python3_install_child <file> <index>
    python3 - "$1" "$2" <<'PYEOF'
import json, sys
doc = json.load(open(sys.argv[1]))
index = int(sys.argv[2])
events = doc.get("log", {}).get("sealed", [])
installs = [
    e["event"]["payload"]["Install"]["target"]["Open"]
    for e in events
    if e["event"].get("event_type") == "install"
]
print(installs[index]["other"] if index < len(installs) else "")
PYEOF
}

# The UNTP identifier value a rendered triad carries for its subject
# (the B-INT round trip: this value must parse back to the same core
# identity the source passport holds).
python3_triad_identifier() { # python3_triad_identifier <triad-file>
    python3 - "$1" <<'PYEOF'
import json, sys
doc = json.load(open(sys.argv[1]))
print(doc["passport"]["productIdentifiers"][0]["value"])
PYEOF
}

# Did every conformity standard the triad rendered land as a profile
# binding on the ingested passport? (standardsConformance -> profiles)
python3_untp_bindings() { # python3_untp_bindings <triad-file> <ingest-file>
    python3 - "$1" "$2" <<'PYEOF'
import json, sys
triad = json.load(open(sys.argv[1]))
ingest = json.load(open(sys.argv[2]))
standards = [c.get("standard", "") for c in
             triad["passport"].get("standardsConformance") or []]
bound = [p.get("id", "") for p in ingest.get("profiles") or []]
missing = [s for s in standards if s not in bound]
if not standards:
    print("no standardsConformance rendered")
elif missing:
    print("unbound: " + ",".join(missing))
else:
    print("ok")
PYEOF
}

# Extract a dotted JSON path (one nesting level per dot) with python3.
json_path() { # json_path <file> <dotted.path>
    python3 - "$1" "$2" <<'PYEOF'
import json, sys
doc = json.load(open(sys.argv[1]))
for part in sys.argv[2].split("."):
    doc = doc[part]
if isinstance(doc, (dict, list)):
    print(json.dumps(doc))
else:
    print(doc)
PYEOF
}

# SHA-256 of a file's exact bytes, hex (the commitment we anchor in
# the transparency log).
sha256_hex() { # sha256_hex <file>
    python3 -c 'import hashlib,sys;print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$1"
}

# ---------------------------------------------------------------------------
# Live trust service — pin the verify anchor from GET /keyring
# ---------------------------------------------------------------------------

# When UNIDPP_TRUST_URL is set (and issuance runs through the issuer
# service), fetch the public anchor a verifier pins from the trust
# service's /keyring — role sign-ecdsa-p256 — and use it as the
# `--anchor` of every verify step below, instead of the anchor each
# pack mint prints. The pinned key covers the issuer's pack signer
# because both services derive it from the same ceremony seed
# (scripts/demo-live.sh aligns UNIDPP_TRUST_SIGN_SEED_P256 with
# UNIDPP_ISSUER_PACK_SEED); B4 asserts the two anchors are
# byte-identical, so a seed drift fails the demo loudly.
pin_trust_anchor() {
    [ -n "$TRUST_URL" ] || return 0
    if [ "$(detect_issuer_mode)" != issuer ]; then
        note "UNIDPP_TRUST_URL set but issuance is CLI-driven — the trust-pinned"
        note "anchor covers the issuer's pack signer; keeping the mint-returned anchors"
        return 0
    fi
    show "GET $TRUST_URL/keyring"
    curl -sf "$TRUST_URL/keyring" >"$WORK_DIR/trust-keyring.json" \
        || fail "trust /keyring unreachable on $TRUST_URL"
    TRUST_ANCHOR="$(json_path "$WORK_DIR/trust-keyring.json" roles.sign-ecdsa-p256.public)"
    TRUST_KEY_ID="$(json_path "$WORK_DIR/trust-keyring.json" roles.sign-ecdsa-p256.key_id)"
    TRUST_MODE="$(json_path "$WORK_DIR/trust-keyring.json" mode)"
    [ -n "$TRUST_ANCHOR" ] || fail "trust /keyring carried no sign-ecdsa-p256 public anchor"
    say "trust:    anchor pinned from $TRUST_URL/keyring"
    say "          role sign-ecdsa-p256, key id $TRUST_KEY_ID ($TRUST_MODE mode)"
    say "          every verify below passes this pinned key as --anchor"
}

# Read the transparency-log identity once (UNIDPP_LOG_URL mode).
probe_log() {
    [ -n "$LOG_URL" ] || return 0
    show "GET $LOG_URL/tree/head"
    curl -sf "$LOG_URL/tree/head" >"$WORK_DIR/log-head.json" \
        || fail "log /tree/head unreachable on $LOG_URL"
    LOG_ID="$(json_get "$WORK_DIR/log-head.json" log_id)"
    say "log:      $LOG_URL (log id $LOG_ID) — every minted pack anchors here"
}

# Anchor a minted pack's commitment in the live transparency log:
# POST /commitments {subject, sha256(pack)}; assert the receipt's
# commitment echoes ours and that GET /receipt/{seq} re-serves it
# byte-identically; store the signed receipt and narrate its id.
log_anchor_pack() { # log_anchor_pack <pack-file> <label> <subject>
    [ -n "$LOG_URL" ] || return 0
    lap_file="$1"
    lap_label="$2"
    lap_subject="$3"
    mkdir -p "$LOG_RECEIPTS"
    lap_hash="$(sha256_hex "$lap_file")"
    show "POST $LOG_URL/commitments  (subject $lap_subject, commitment = sha256 of $(basename "$lap_file"))"
    printf '{"subject":"%s","commitment":"%s"}' "$lap_subject" "$lap_hash" \
        >"$LOG_RECEIPTS/$lap_label.request.json"
    curl -sSf -X POST "$LOG_URL/commitments" \
        -H 'content-type: application/json' \
        --data @"$LOG_RECEIPTS/$lap_label.request.json" \
        >"$LOG_RECEIPTS/$lap_label.receipt.json" \
        || fail "log refused the $lap_label pack commitment"
    rm -f "$LOG_RECEIPTS/$lap_label.request.json"
    lap_rid="$(json_get "$LOG_RECEIPTS/$lap_label.receipt.json" receipt_id)"
    lap_seq="$(json_get "$LOG_RECEIPTS/$lap_label.receipt.json" seq)"
    lap_size="$(json_path "$LOG_RECEIPTS/$lap_label.receipt.json" tree_head.tree_size)"
    check "log receipt commitment echoes pack hash ($lap_label)" \
        "$lap_hash" "$(json_get "$LOG_RECEIPTS/$lap_label.receipt.json" commitment)"
    curl -sf "$LOG_URL/receipt/$lap_seq" >"$LOG_RECEIPTS/$lap_label.reserved.json"
    if cmp -s "$LOG_RECEIPTS/$lap_label.receipt.json" "$LOG_RECEIPTS/$lap_label.reserved.json"; then
        check "log receipt $lap_rid re-served byte-identically" ok ok
    else
        check "log receipt $lap_rid re-served byte-identically" ok changed
    fi
    rm -f "$LOG_RECEIPTS/$lap_label.reserved.json"
    say "log:      receipt $lap_rid (seq $lap_seq, tree size $lap_size) — GET $LOG_URL/receipt/$lap_seq"
}

# ---------------------------------------------------------------------------
# Service helpers
# ---------------------------------------------------------------------------

registry_post() { # registry_post <path> <json-body> [out-file]
    rp_path="$1"
    rp_body="$2"
    rp_out="${3:-/dev/null}"
    show "POST $REGISTRY_URL$rp_path  $rp_body"
    # 4xx is fine for re-runs of the test harness (idempotent item
    # registration + applicability bind of the same triple returns 409
    # "already registered"); only server errors abort the demo.
    rp_http_code="$(curl -s -o "$rp_out" -w '%{http_code}' \
        -X POST "$REGISTRY_URL$rp_path" \
        -H 'content-type: application/json' \
        --data "$rp_body")"
    if [ "$rp_http_code" -ge 500 ]; then
        fail "registry POST $rp_path returned $rp_http_code (see $(cat "$rp_out" 2>/dev/null))"
    fi
}

# Has the registry already seen this exact applicability triple
# (profile × subject × effective_from)? The registry itself does NOT
# deduplicate applicability bindings (the dated-binding model allows
# multiple windows for the same profile), so re-runs accumulate a
# duplicate audit record. Detect the duplicate client-side and skip
# the POST so the demo's as-of assertions remain deterministic.
binding_already_seen() {
    # Silent query (no show line): this function's stdout is captured.
    binding_already_seen_product="$1"
    binding_already_seen_path="$WORK_DIR/reg-applicability-check.json"
    curl -s "$REGISTRY_URL/applicability?product_type=$binding_already_seen_product&at=2028-06-01T00:00:00Z" \
        >"$binding_already_seen_path"
    python3 - "$binding_already_seen_path" <<'PYEOF'
import json, sys
doc = json.load(open(sys.argv[1]))
print(len(doc.get("applicability", [])))
PYEOF
}

registry_get() { # registry_get <path> [out-file]
    rg_path="$1"
    rg_out="${2:-/dev/null}"
    show "GET $REGISTRY_URL$rg_path"
    if ! curl -s "$REGISTRY_URL$rg_path" >"$rg_out"; then
        fail "registry GET failed: $rg_path"
    fi
}

start_registry() {
    if [ -n "${UNIDPP_REGISTRY_URL:-}" ]; then
        note "using external registry at $REGISTRY_URL (not starting one)"
    else
        [ -x "$REGISTRY_BIN" ] || fail "registry binary missing: $REGISTRY_BIN (run: make deps)"
        note "starting unidpp-registry on $REGISTRY_BIND"
        UNIDPP_REGISTRY_BIND="$REGISTRY_BIND" "$REGISTRY_BIN" >/dev/null 2>&1 &
        REGISTRY_PID=$!
    fi

    registry_ready=0
    registry_try=0
    while [ "$registry_try" -lt 50 ]; do
        if curl -sf "$REGISTRY_URL/healthz" >/dev/null 2>&1; then
            registry_ready=1
            break
        fi
        registry_try=$((registry_try + 1))
        sleep 0.2
    done
    [ "$registry_ready" = 1 ] || fail "registry did not become healthy on $REGISTRY_URL"
    say "unidpp-registry healthy at $REGISTRY_URL (19135 item service, )"
}

stop_registry() {
    if [ -n "$REGISTRY_PID" ]; then
        kill "$REGISTRY_PID" 2>/dev/null
        wait "$REGISTRY_PID" 2>/dev/null
        REGISTRY_PID=""
    fi
}

# ---------------------------------------------------------------------------
# unidpp-trust — the B-QUORUM beat (retroactive distrust as a quorum act)
# ---------------------------------------------------------------------------

# The trust base for the quorum beat: the live trust service in live
# mode (UNIDPP_TRUST_URL, already healthy), else an ephemeral instance
# (fresh journal under the work dir, no seed fixtures — the beat seeds
# its own quorum node). Returns non-zero when it cannot run; callers
# narrate honestly instead of failing the story.
start_quorum_trust() {
    if [ -n "$TRUST_URL" ]; then
        QUORUM_URL="$TRUST_URL"
        note "B-QUORUM targets the live trust service at $TRUST_URL"
        return 0
    fi
    [ -x "$TRUST_BIN" ] || return 1
    note "starting ephemeral unidpp-trust on $QUORUM_BIND (B-QUORUM only)"
    rm -f "$WORK_DIR/quorum-trust.journal.jsonl"
    UNIDPP_TRUST_BIND="$QUORUM_BIND" \
    UNIDPP_TRUST_STATE_FILE="$WORK_DIR/quorum-trust.journal.jsonl" \
    UNIDPP_TRUST_NO_SEED_FIXTURES=1 \
        "$TRUST_BIN" >/dev/null 2>&1 &
    QUORUM_PID=$!
    quorum_try=0
    while [ "$quorum_try" -lt 50 ]; do
        if curl -sf "$QUORUM_URL/healthz" >/dev/null 2>&1; then
            return 0
        fi
        quorum_try=$((quorum_try + 1))
        sleep 0.2
    done
    stop_quorum_trust
    return 1
}

stop_quorum_trust() {
    if [ -n "$QUORUM_PID" ]; then
        kill "$QUORUM_PID" 2>/dev/null
        wait "$QUORUM_PID" 2>/dev/null
        QUORUM_PID=""
    fi
}

quorum_post() { # quorum_post <path> <body-file> <out-file> -> echoes the status code
    qp_path="$1"
    qp_body="$2"
    qp_out="$3"
    show "POST $QUORUM_URL$qp_path  ($(basename "$qp_body"))" >&2
    qp_code="$(curl -s -o "$qp_out" -w '%{http_code}' -X POST "$QUORUM_URL$qp_path" \
        -H 'content-type: application/json' --data-binary @"$qp_body")"
    printf '%s' "$qp_code"
}

# A dotted field of the first standing entry for the beat's subject
# (GET /revocations?subject=node:haichuan-cn); booleans print lowercase.
quorum_standing() { # quorum_standing <query-suffix> <dotted.field>
    curl -sf "$QUORUM_URL/revocations?$1&subject=node:haichuan-cn" \
        | python3 -c '
import json, sys
revs = json.load(sys.stdin)["revocations"]
if not revs:
    print("none")
    raise SystemExit
doc = revs[0]
for part in sys.argv[1].split("."):
    doc = doc[part]
if isinstance(doc, bool):
    doc = str(doc).lower()
print(doc)
' "$2"
}

# ---------------------------------------------------------------------------
# unidpp-gateway — the B-INT interop beat (render to UNTP, ingest back)
# ---------------------------------------------------------------------------

# Start the interop gateway. An issuer-driver run inherits
# UNIDPP_ISSUER_URL from the environment (demo-live exports it), so
# the gateway renders the REAL story passports from the issuer's
# document API; a local-driver run starts it fixture-backed (the
# story's CLI-file passports are not behind an HTTP issuer there).
start_gateway() {
    if curl -sf "$GATEWAY_URL/healthz" >/dev/null 2>&1; then
        note "using already-running unidpp-gateway at $GATEWAY_URL"
        return 0
    fi
    [ -x "$GATEWAY_BIN" ] || fail "gateway binary missing: $GATEWAY_BIN (run: make deps)"
    if [ "$(detect_issuer_mode)" = issuer ]; then
        note "starting unidpp-gateway on $GATEWAY_BIND (issuer upstream: $ISSUER_URL)"
    else
        note "starting unidpp-gateway on $GATEWAY_BIND (seeded fixtures — no issuer upstream)"
    fi
    UNIDPP_GATEWAY_BIND="$GATEWAY_BIND" "$GATEWAY_BIN" >/dev/null 2>&1 &
    GATEWAY_PID=$!
    gateway_ready=0
    gateway_try=0
    while [ "$gateway_try" -lt 50 ]; do
        if curl -sf "$GATEWAY_URL/healthz" >/dev/null 2>&1; then
            gateway_ready=1
            break
        fi
        gateway_try=$((gateway_try + 1))
        sleep 0.2
    done
    [ "$gateway_ready" = 1 ] || fail "unidpp-gateway did not become healthy on $GATEWAY_URL"
    say "unidpp-gateway healthy at $GATEWAY_URL (UNTP triad render + ingest)"
}

stop_gateway() {
    if [ -n "$GATEWAY_PID" ]; then
        kill "$GATEWAY_PID" 2>/dev/null
        wait "$GATEWAY_PID" 2>/dev/null
        GATEWAY_PID=""
    fi
}

gateway_get() { # gateway_get <path> <out-file>
    gg_path="$1"
    gg_out="$2"
    show "GET $GATEWAY_URL$gg_path"
    if ! curl -sf "$GATEWAY_URL$gg_path" >"$gg_out"; then
        fail "gateway GET failed: $gg_path"
    fi
}

# POST a body file to /untp/ingest. The HTTP status code is printed;
# the response body lands in <out-file>.
gateway_ingest() { # gateway_ingest <body-file> <out-file>
    gi_body="$1"
    gi_out="$2"
    curl -s -o "$gi_out" -w '%{http_code}' -X POST "$GATEWAY_URL/untp/ingest" \
        -H 'content-type: application/json' --data-binary @"$gi_body"
}

cleanup() { stop_gateway; stop_registry; stop_quorum_trust; }
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------
# Verify helper — run `unidpp verify` and ASSERT the expected verdict.
# Exit codes: 0 pass, 1 degraded, 2 fail (never silently accepted).
# ---------------------------------------------------------------------------

verify_and_expect() { # verify_and_expect <pack> <anchor> <as-of> <expected> <why> [extra-args...]
    ve_pack="$1"
    ve_anchor="$2"
    ve_asof="$3"
    ve_expected="$4"
    ve_why="$5"
    shift 5
    ve_extra="$*"

    # Live trust mode: the anchor is the one pinned from the trust
    # service's /keyring (pin_trust_anchor); the per-pack argument
    # stays as the CLI-driver fallback.
    if [ -n "$TRUST_ANCHOR" ]; then
        ve_anchor="$TRUST_ANCHOR"
    fi

    case "$ve_expected" in
        0) ve_expected_label="pass" ;;
        1) ve_expected_label="degraded" ;;
        2) ve_expected_label="fail" ;;
        *) fail "internal: bad expected verdict $ve_expected" ;;
    esac

    show "unidpp verify $ve_pack --anchor <pinned> --as-of $ve_asof $ve_extra   # expect: $ve_expected_label"
    # shellcheck disable=SC2086
    "$UNIDPP" verify "$ve_pack" --anchor "$ve_anchor" --as-of "$ve_asof" $ve_extra
    ve_code=$?
    [ "$ve_code" -eq 127 ] && fail "unidpp CLI not found at $UNIDPP (run: make deps)"
    check "verify verdict ($ve_why)" "$ve_expected" "$ve_code"
    if [ "$ve_code" != "$ve_expected" ]; then
        fail "verify returned $ve_code, expected $ve_expected ($ve_expected_label): $ve_why"
    fi
}

# ---------------------------------------------------------------------------
# Payload builders (typed JSON per unidpp-core/crates/event/src/payload.rs)
# ---------------------------------------------------------------------------

# install_data <direction> <other-urn> <from> <alteration|-> <method>
#               <recoverability> <pairing>
install_data() {
    id_direction="$1"
    id_other="$2"
    id_from="$3"
    id_alteration="$4"
    id_method="$5"
    id_recoverability="$6"
    id_pairing="$7"

    id_alterations='[]'
    if [ "$id_alteration" != "-" ]; then
        id_alterations="[\"Known\",\"$id_alteration\"]"
    fi

    printf '{"target":{"Open":{"link_type":"installation","other":"%s","direction":"%s","interval":{"from":"%s","to":null},"binding":{"method":"%s","recoverability":"%s","visibility":{"edge":"public","audiences":[]},"slot_id":null,"pairing":"%s","alterations":%s}}}}' \
        "$id_other" "$id_direction" "$id_from" "$id_method" \
        "$id_recoverability" "$id_pairing" "$id_alterations"
}

# uninstall_data <other-urn> <interval-from> <interval-to> — closes the
# installation interval; outcome harvested (provenance carries forward).
uninstall_data() {
    printf '{"link":{"link_type":"installation","other":"%s","direction":"outgoing","interval":{"from":"%s","to":"%s"},"binding":{"method":"fastened","recoverability":"restorable","visibility":{"edge":"public","audiences":[]},"slot_id":null,"pairing":"firmware","alterations":[]}},"outcome":"harvested"}' \
        "$1" "$2" "$3"
}

# stamp_data <subject-urn> <attester> <at> — a lens-scoped, dated, signed
# condition stamp (B8 auction lens; B5 service lens).
stamp_data() {
    printf '{"stamp":{"attester":"%s","subject":"%s","subject_state_commitment":"0000000000000000000000000000000000000000000000000000000000000000","lens":"urn:unidpp:profile:lens-auction","lens_version":"1","mode":"snapshot","verdict_summary":"grade A-","coverage_report":null,"log_anchored_at":"%s","quantity_context":null}}' \
        "$2" "$1" "$3"
}

# decompose_data — B10 mass balance: 25.9 kg in; 21.4 + 4.3 out; 0.2 loss.
decompose_data() {
    printf '{"outputs":[{"child":"%s","quantity":{"amount":"21.4","unit":{"uom":"kg","registry_uri":"https://unitsml.org/units/kg"}}},{"child":"%s","quantity":{"amount":"4.3","unit":{"uom":"kg","registry_uri":"https://unitsml.org/units/kg"}}}],"accredited_for_claims":true}' \
        "$SCRAP_URN" "$RECYCLE_URN"
}

# Registry seed bodies ( wire shapes).
reg_transform_body() {
    printf '{"register_id":"unidpp-e2e","item_id":"gb4943-1-2022-eq-iec-62368-1","class":"transform","definition":"GB 4943.1-2022 ~= IEC 62368-1 certificate equivalence (attester: cqc)","version":"1.0.0"}'
}

reg_profile_body() {
    printf '{"register_id":"unidpp-e2e","item_id":"eu-battery-lmt","class":"profile","definition":"EU battery passport profile - LMT class (Reg. (EU) 2023/1542)","version":"1.0.0","effective_from":"2027-02-18T00:00:00Z","manifest":{"version":"1.0.0","issuer_class":"law","issuer":"ec-espr","signature":{"signature":"seeded-dev-signature"}}}'
}

reg_binding_body() {
    printf '{"profile_id":"eu-battery-lmt","product_type":"%s","effective_from":"2028-02-01T00:00:00Z"}' \
        "$BIKE_TYPE_REF"
}

# B10 mass balance narration: in - out = loss, computed from the artifact.
mass_balance() {
    python3 - "$WORK_DIR/e8-instance.json" <<'PYEOF'
import json, sys
doc = json.load(open(sys.argv[1]))
decompose = [e for e in doc["log"]["sealed"]
             if e["event"]["event_type"] == "decompose"][-1]
payload = decompose["event"]["payload"]
outs = payload.get("Decompose", payload).get("outputs", [])
total_in = 25.9
total_out = sum(float(o["quantity"]["amount"]) for o in outs)
loss = total_in - total_out
for o in outs:
    print("    out: {:>5} kg  ->  {}".format(o["quantity"]["amount"], o["child"]))
print("    in : {:>5} kg  (declared mass)".format(total_in))
print("    loss = in - out = {:.1f} kg  (auditable)".format(loss))
PYEOF
}

# Narrate the last milestone counters (B5) from the artifact.
b5_counters() {
    python3 - "$WORK_DIR/e8-instance.json" <<'PYEOF'
import json, sys
doc = json.load(open(sys.argv[1]))
milestones = [e for e in doc["log"]["sealed"]
              if e["event"]["event_type"] == "milestone.record"]
if not milestones:
    print("    (no milestone events recorded)")
else:
    counters = milestones[-1]["event"]["payload"]
    counters = counters.get("MilestoneRecord", counters).get("counters", {})
    for key, value in counters.items():
        print("    counter: {} = {}".format(key, value))
PYEOF
}

# ---------------------------------------------------------------------------
# The story
# ---------------------------------------------------------------------------

main() {
    mkdir -p "$WORK_DIR"

    [ -x "$UNIDPP" ] || fail "unidpp CLI missing: $UNIDPP (run: make deps)"
    [ -x "$(command -v python3)" ] || fail "python3 is required"
    [ -x "$(command -v curl)" ] || fail "curl is required"

    hr "UniDPP end-to-end — the Momiji Mobility E8 (STORY.md beats B1-B10)"
    say "run:      $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    say "story:    UniDPP E8 exemplar (beats B1-B10)"
    say "artifacts: $WORK_DIR"

    . "$SCRIPT_DIR/issuer-hook.sh"

    issuer_driver="$(detect_issuer_mode)"
    if [ "$issuer_driver" = issuer ]; then
        say "issuance: unidpp-issuer service  at $ISSUER_URL"
    else
        say "issuance: unidpp-cli (local driver) — set UNIDPP_ISSUER_URL or run"
        say "make demo-live to issue through the unidpp-issuer service"
    fi

    start_registry

    # Live-service mode: pin the verify anchor from the trust service
    # and identify the transparency log before the story starts, so
    # the transcript names every dependency up front.
    pin_trust_anchor
    probe_log

    if [ -n "${UNIDPP_ISSUER_URL:-}$TRUST_URL$LOG_URL" ]; then
        if [ -n "$TRUST_URL" ] && [ -n "$LOG_URL" ] && [ "$issuer_driver" = issuer ]; then
            say "topology: LIVE — four sibling services (started by scripts/demo-live.sh)"
        else
            say "topology: LIVE (partial — live service URLs detected)"
        fi
        say "  registry : $REGISTRY_URL (items, applicability, transforms)"
        if [ "$issuer_driver" = issuer ]; then
            say "  issuer   : $ISSUER_URL (server-signed events, server-minted packs)"
        fi
        if [ -n "$TRUST_URL" ]; then
            say "  trust    : $TRUST_URL (verify anchor source: GET /keyring)"
        fi
        if [ -n "$LOG_URL" ]; then
            say "  log      : $LOG_URL (pack commitments -> signed receipts)"
        fi
    fi

    # =====================================================================
    beat "B1" "Assembly in Kyoto (issuance where duty attaches)"
    # =====================================================================
    what "the JP type passport and the instance passport exist, and the build record lists every part at finest recorded granularity."

    issuer_create "$TYPE_ID" - S0 momiji-mobility \
        "https://resolver.unidpp.org/r/momiji-e8-type-2027-1" \
        "$TYPE_URN" "$WORK_DIR/e8-type.json"
    issuer_create "$BIKE_ID" "$BIKE_TYPE_REF" S2 momiji-mobility \
        "https://resolver.unidpp.org/r/momiji-e8-j000842" \
        "$BIKE_URN" "$WORK_DIR/e8-instance.json"
    issuer_create "local:rhine:du/M-771233" - S1 rhine-drives-de \
        "https://resolver.unidpp.org/r/rhine-du-m771" \
        "$DRIVE_URN" "$WORK_DIR/drive-unit.json"
    issuer_create "local:weilian:wp/P-9904" - S2 weilian-shenzhen \
        "https://resolver.unidpp.org/r/weilian-wp-p9904" \
        "$PACK_URN" "$WORK_DIR/pack-original.json"
    issuer_create "local:haichuan:cell/H-2231" - S0 haichuan-cn \
        "https://resolver.unidpp.org/r/haichuan-cell-h2231" \
        "$LOT_URN" "$WORK_DIR/cell-lot.json"

    say "instance passport   $(json_get "$WORK_DIR/e8-instance.json" passport_id) ($(json_get "$WORK_DIR/e8-instance.json" product_id))"
    say "type passport       $(json_get "$WORK_DIR/e8-type.json" passport_id) ($(json_get "$WORK_DIR/e8-type.json" product_id))"
    say "type ref + version  $BIKE_TYPE_REF (the 2028.1 hardware revision exists — type-version visibility)"
    say "dormant identifiers: the 40 Haichuan cells of lot H-2231 have no passports yet — absorption is regime-temporal"

    issuer_event "$WORK_DIR/e8-type.json" issuance \
        '{"derived":false,"inputs":[]}' momiji-type-approval "issuing authority" \
        "2027-04-12T09:00:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" issuance \
        '{"derived":false,"inputs":[]}' momiji-mobility "issuing authority" \
        "2027-04-12T09:30:00Z"
    issuer_event "$WORK_DIR/pack-original.json" issuance \
        '{"derived":false,"inputs":[]}' weilian-shenzhen "issuing authority" \
        "2027-03-02T08:00:00Z"
    issuer_event "$WORK_DIR/cell-lot.json" issuance \
        '{"derived":false,"inputs":[]}' haichuan-cn "issuing authority" \
        "2027-01-15T08:00:00Z"
    issuer_event "$WORK_DIR/drive-unit.json" issuance \
        '{"derived":false,"inputs":[]}' rhine-drives-de "issuing authority" \
        "2027-03-20T08:00:00Z"

    what "invariant I7 in one line: the passport is issued where the legal duty attaches (JP road-traffic/EPAC type facts) — not where the server is."

    # =====================================================================
    beat "B2" "Parts carry their own duties (mixed-profile children)"
    # =====================================================================
    what "installation edges R3 are typed: pairing, alteration, recoverability — bidirectional parent/child knowledge."

    # The bike's outgoing side of each installation edge (R3).
    issuer_event "$WORK_DIR/e8-instance.json" install \
        "$(install_data outgoing "$DRIVE_URN" 2027-04-12T10:00:00Z torque-to-yield fastened harvestable none)" \
        momiji-assembly installer "2027-04-12T10:00:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" install \
        "$(install_data outgoing "$PACK_URN" 2027-04-12T10:05:00Z - fastened restorable firmware)" \
        momiji-assembly installer "2027-04-12T10:05:00Z"
    # The parts' incoming side (bidirectional knowledge).
    issuer_event "$WORK_DIR/pack-original.json" install \
        "$(install_data incoming "$BIKE_URN" 2027-04-12T10:05:00Z - fastened restorable firmware)" \
        momiji-assembly installer "2027-04-12T10:05:00Z"
    # The pack's cells: dormant identifiers recorded as an installation
    # edge to the lot identity at finest recorded granularity (I3).
    issuer_event "$WORK_DIR/pack-original.json" install \
        "$(install_data incoming "$LOT_URN" 2027-03-02T08:30:00Z - potted absorbing none)" \
        weilian-shenzhen installer "2027-03-02T08:30:00Z"

    # The charger's CCC certificate joins the subregister as a registered
    # equivalence transform (GB 4943.1-2022 ~= IEC 62368-1) — B2's
    # "equivalence as a registered transform".
    registry_post /items "$(reg_transform_body)" "$WORK_DIR/reg-transform-response.json"
    registry_get /transforms/gb4943-1-2022-eq-iec-62368-1 "$WORK_DIR/reg-transform.json"

    say "bike -> drive unit:       torque-mount alteration, harvestable (recoverability spectrum)"
    say "bike -> pack:             battery swappable = restorable, CAN/firmware pairing"
    say "pack -> cell lot H-2231:  absorbing (dormant identifiers — the cell-passport ratchet adopts them one day)"
    say "certificate join:         $(json_get "$WORK_DIR/reg-transform.json" identifier) (class $(json_get "$WORK_DIR/reg-transform.json" item_class))"

    what "a foreign verifier reads a CCC certificate without adopting CN rules — equivalence claims are registered transforms, not re-issuance."


    # =====================================================================
    beat "B-CTO" "The build-to-order variant (configuration-vector composition)"
    # =====================================================================
    what "CTO = model + configuration vector: each option is a registered model with its own passport; the instance composes through the same R3 edges."

    CTO_ID="local:momiji:e8/J-000843"
    CTO_URN="urn:unidpp:passport:momiji-e8-j000843"
    CTO_BATT_STD_TYPE_URN="urn:unidpp:passport:weilian-wp-type-std"
    CTO_BATT_LR_TYPE_URN="urn:unidpp:passport:voltaro-wp-type-lr9"
    CTO_RACK_TYPE_URN="urn:unidpp:passport:arca-rack-type-r200"
    CTO_BATT_LR_URN="urn:unidpp:passport:voltaro-lr-0001"
    CTO_RACK_URN="urn:unidpp:passport:arca-r200-0007"
    CTO_CONFIG="urn:unidpp:option:battery:long-range,urn:unidpp:option:rack:yes"

    # Option families: one model passport per option (the catalogue).
    issuer_create "local:weilian:wp/type-STD" - S0 weilian-shenzhen \
        "https://resolver.unidpp.org/r/weilian-wp-type-std" \
        "$CTO_BATT_STD_TYPE_URN" "$WORK_DIR/cto-batt-std-type.json"
    issuer_create "local:voltaro:wp/type-LR9" - S0 voltaro-eu \
        "https://resolver.unidpp.org/r/voltaro-wp-type-lr9" \
        "$CTO_BATT_LR_TYPE_URN" "$WORK_DIR/cto-batt-lr-type.json"
    issuer_create "local:arca:rack/type-R200" - S0 arca-cycles \
        "https://resolver.unidpp.org/r/arca-rack-type-r200" \
        "$CTO_RACK_TYPE_URN" "$WORK_DIR/cto-rack-type.json"
    issuer_event "$WORK_DIR/cto-batt-std-type.json" issuance \
        '{"derived":false,"inputs":[]}' weilian-shenzhen "issuing authority" "2027-04-01T08:00:00Z"
    issuer_event "$WORK_DIR/cto-batt-lr-type.json" issuance \
        '{"derived":false,"inputs":[]}' voltaro-eu "issuing authority" "2027-04-01T08:30:00Z"
    issuer_event "$WORK_DIR/cto-rack-type.json" issuance \
        '{"derived":false,"inputs":[]}' arca-cycles "issuing authority" "2027-04-01T09:00:00Z"

    # The CTO instance: the 2027.1 frame plus the ordered configuration
    # vector [battery: long-range, rack: yes] — a second bike off the
    # same line, composed, not re-engineered.
    issuer_create "$CTO_ID" "$BIKE_TYPE_REF" S2 momiji-mobility \
        "https://resolver.unidpp.org/r/momiji-e8-j000843" \
        "$CTO_URN" "$WORK_DIR/e8-cto.json" "$CTO_CONFIG"
    issuer_event "$WORK_DIR/e8-cto.json" issuance \
        '{"derived":false,"inputs":[]}' momiji-mobility "issuing authority" "2027-04-12T11:00:00Z"

    # Option instances: the chosen long-range pack and the rack.
    issuer_create "local:voltaro:wp/LR-0001" "voltaro:wp/type/LR9" S2 voltaro-eu \
        "https://resolver.unidpp.org/r/voltaro-lr-0001" \
        "$CTO_BATT_LR_URN" "$WORK_DIR/cto-batt-lr.json"
    issuer_create "local:arca:rack/R-200-0007" "arca:rack/type/R200" S1 arca-cycles \
        "https://resolver.unidpp.org/r/arca-r200-0007" \
        "$CTO_RACK_URN" "$WORK_DIR/cto-rack.json"
    issuer_event "$WORK_DIR/cto-batt-lr.json" issuance \
        '{"derived":false,"inputs":[]}' voltaro-eu "issuing authority" "2027-04-12T10:30:00Z"
    issuer_event "$WORK_DIR/cto-rack.json" issuance \
        '{"derived":false,"inputs":[]}' arca-cycles "issuing authority" "2027-04-12T10:45:00Z"

    # Composition through the SAME typed R3 edges (bidirectional).
    issuer_event "$WORK_DIR/e8-cto.json" install \
        "$(install_data outgoing "$CTO_BATT_LR_URN" 2027-04-12T11:10:00Z - fastened restorable firmware)" \
        momiji-assembly installer "2027-04-12T11:10:00Z"
    issuer_event "$WORK_DIR/cto-batt-lr.json" install \
        "$(install_data incoming "$CTO_URN" 2027-04-12T11:10:00Z - fastened restorable firmware)" \
        momiji-assembly installer "2027-04-12T11:10:00Z"
    issuer_event "$WORK_DIR/e8-cto.json" install \
        "$(install_data outgoing "$CTO_RACK_URN" 2027-04-12T11:15:00Z - fastened restorable none)" \
        momiji-assembly installer "2027-04-12T11:15:00Z"
    issuer_event "$WORK_DIR/cto-rack.json" install \
        "$(install_data incoming "$CTO_URN" 2027-04-12T11:15:00Z - fastened restorable none)" \
        momiji-assembly installer "2027-04-12T11:15:00Z"

    # The composition manifest: the config vector as a build artifact.
    python3 - "$WORK_DIR/cto-config.json" "$CTO_CONFIG" <<'PYEOF'
import json, sys
out, config = sys.argv[1], sys.argv[2]
vector = [token.strip() for token in config.split(",") if token.strip()]
with open(out, "w") as fh:
    json.dump({
        "model": "momiji:e8/type/2027.1",
        "instance": "urn:unidpp:passport:momiji-e8-j000843",
        "config": vector,
    }, fh, indent=2)
PYEOF

    say "frame model         momiji:e8/type/2027.1 (the same type passport as J-000842)"
    say "configuration       $CTO_CONFIG"
    say "composed children   $CTO_BATT_LR_URN (battery LR) + $CTO_RACK_URN (rack)"
    say "not chosen          the STD battery model exists in the catalogue — no instance, no edge"

    # The composed instance's installation edges resolve to exactly the
    # configured children (the projector's traversal, read from the
    # graph itself).
    cto_children="$(python3_count_installs "$WORK_DIR/e8-cto.json")"
    check "CTO instance outgoing installs == 2" 2 "$cto_children"
    check "CTO battery edge names the LR pack" "$CTO_BATT_LR_URN"         "$(python3_install_child "$WORK_DIR/e8-cto.json" 0)"
    check "CTO rack edge names the rack" "$CTO_RACK_URN"         "$(python3_install_child "$WORK_DIR/e8-cto.json" 1)"
    # Bidirectional knowledge: each child's incoming edge names the CTO.
    check "LR pack incoming edge names the CTO instance" "$CTO_URN"         "$(python3_install_child "$WORK_DIR/cto-batt-lr.json" 0)"
    check "rack incoming edge names the CTO instance" "$CTO_URN"         "$(python3_install_child "$WORK_DIR/cto-rack.json" 0)"
    # The rejected option never enters this build's graph.
    if grep -q "$CTO_BATT_STD_TYPE_URN" "$WORK_DIR/e8-cto.json"; then
        check "STD battery absent from the CTO graph" absent present
    else
        check "STD battery absent from the CTO graph" absent absent
    fi
    # The configuration vector is queryable on the live document.
    if [ "$(detect_issuer_mode)" = issuer ]; then
        show "GET $ISSUER_URL/passports/$CTO_URN"
        curl -sf "$ISSUER_URL/passports/$CTO_URN" >"$WORK_DIR/e8-cto-view.json" \
            || fail "cannot query the CTO instance"
        check "config vector queryable == 2 options" 2 \
            "$(python3_count "$WORK_DIR/e8-cto-view.json" config)"
    else
        say "config vector:      local driver mode — the sidecar manifest $WORK_DIR/cto-config.json"
    fi

    # The composed instance verifies like any other: one identity, one
    # pack, the same pipeline.
    cto_anchor="$(issuer_mint_pack "$WORK_DIR/e8-cto.json" "$WORK_DIR/b-cto.pack")"
    verify_and_expect "$WORK_DIR/b-cto.pack" "$cto_anchor" \
        "2027-04-12T11:30:00Z" 0 \
        "B-CTO composed instance — issued, both children recorded"
    log_anchor_pack "$WORK_DIR/b-cto.pack" b-cto "$CTO_URN"

    what "the CTO variant is composition, not re-issuance: the same frame type, one config vector, R3 edges to each chosen option's own passport."

    # =====================================================================
    beat "B-INT" "S12 interop: render to UNTP, ingest back"
    # =====================================================================
    what "the S12 seam is bidirectional: the core passport renders as the UNTP verifiable-credential triad, and the triad ingests back as a core passport — the same subject identity, conformity credentials landed as profile bindings, idempotent per subject."

    start_gateway

    if [ "$(detect_issuer_mode)" = issuer ]; then
        bint_render_id="$CTO_URN"
        bint_source_identity="$(json_get "$WORK_DIR/e8-cto.json" product_id)"
        say "subject:   the B-CTO instance the story just composed ($bint_source_identity),"
        say "           rendered by the gateway from the live issuer document"
    else
        gateway_get / "$WORK_DIR/b-int-discovery.json"
        bint_render_id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["source"]["fixtures"][0]["product_id"])' "$WORK_DIR/b-int-discovery.json")"
        bint_source_identity="$bint_render_id"
        say "subject:   the gateway's seeded pilot fixture ($bint_render_id) — the local"
        say "           driver keeps the story passports as CLI files, not behind an HTTP"
        say "           issuer; run make demo-live to round-trip the real B-CTO passport"
    fi

    gateway_get "/untp/product/$bint_render_id" "$WORK_DIR/b-int-triad.json"

    check "B-INT triad renders under the UNTP profile" \
        "urn:unidpp:profile:render:untp" \
        "$(json_path "$WORK_DIR/b-int-triad.json" rendering.profile)"
    if [ "$(detect_issuer_mode)" = issuer ]; then
        check "B-INT render source is the live issuer" issuer \
            "$(json_path "$WORK_DIR/b-int-triad.json" rendering.source)"
        say "verdict:   the gateway verified the issuer's Ed25519 event signatures against"
        say "           the anchors pinned from GET $ISSUER_URL/keyring"
    else
        check "B-INT render source is the seeded fixture" fixture \
            "$(json_path "$WORK_DIR/b-int-triad.json" rendering.source)"
    fi
    check "B-INT render keeps the source product identity" \
        "$bint_source_identity" \
        "$(python3_triad_identifier "$WORK_DIR/b-int-triad.json")"

    show "POST $GATEWAY_URL/untp/ingest  (the triad just rendered)"
    bint_code1="$(gateway_ingest "$WORK_DIR/b-int-triad.json" "$WORK_DIR/b-int-ingest-1.json")"
    bint_status1="$(json_get "$WORK_DIR/b-int-ingest-1.json" status)"
    # A gateway this run started answers 201/imported (fresh store); one
    # still holding a previous run's ingests answers 200/matched. Both
    # honor the contract; anything else fails loudly.
    if { [ "$bint_code1" = 201 ] && [ "$bint_status1" = imported ]; } \
        || { [ "$bint_code1" = 200 ] && [ "$bint_status1" = matched ]; }; then
        check "B-INT first ingest imports (201), or matches on re-runs (200)" ok ok
    else
        check "B-INT first ingest imports (201), or matches on re-runs (200)" \
            "201/imported or 200/matched" "$bint_code1/$bint_status1"
    fi
    bint_pid1="$(json_get "$WORK_DIR/b-int-ingest-1.json" passport_id)"
    check "B-INT ingested identity round-trips to the source passport" \
        "$bint_source_identity" \
        "$(json_get "$WORK_DIR/b-int-ingest-1.json" identity)"
    check "B-INT standardsConformance lands as profile bindings" ok \
        "$(python3_untp_bindings "$WORK_DIR/b-int-triad.json" "$WORK_DIR/b-int-ingest-1.json")"
    say "imported:  $(json_get "$WORK_DIR/b-int-ingest-1.json" passport_id)"
    say "bindings:  $(python3_count "$WORK_DIR/b-int-ingest-1.json" profiles) profile bindings from the triad's conformity credentials"

    # The second ingest of the same subject must match, never duplicate (I1).
    show "POST $GATEWAY_URL/untp/ingest  (again — idempotence per subject)"
    bint_code2="$(gateway_ingest "$WORK_DIR/b-int-triad.json" "$WORK_DIR/b-int-ingest-2.json")"
    check "B-INT re-ingest HTTP status" 200 "$bint_code2"
    check "B-INT re-ingest matches (idempotent per subject)" matched \
        "$(json_get "$WORK_DIR/b-int-ingest-2.json" status)"
    check "B-INT re-ingest returns the same passport id" \
        "$bint_pid1" "$(json_get "$WORK_DIR/b-int-ingest-2.json" passport_id)"

    stop_gateway

    what "render and ingest are inverse projections over one identity: the UNTP consumer and the UniDPP core agree on the subject — no parallel-universe passport."

    # =====================================================================
    beat "B3" "Placement in the EU (profile growth by dated binding)"
    # =====================================================================
    what "the EU lens set binds onto the SAME identity by a registry applicability event — no re-minting, no parallel-universe passport (I1)."

    registry_post /items "$(reg_profile_body)" "$WORK_DIR/reg-profile-response.json"
    # Idempotence: only POST the binding when this exact triple has not
    # yet been recorded (the registry does not deduplicate bindings).
    existing_bindings="$(binding_already_seen "$BIKE_TYPE_REF")"
    if [ "$existing_bindings" = 0 ]; then
        registry_post /applicability "$(reg_binding_body)" "$WORK_DIR/reg-binding-response.json"
    else
        note "applicability binding already present ($existing_bindings) — skipped (registry journal carries it)"
    fi

    registry_get "/applicability?product_type=$BIKE_TYPE_REF&at=2027-06-01T00:00:00Z" "$WORK_DIR/reg-applicability-2027.json"
    say "at 2027-06-01 (JP market only): no EU duty applies yet"
    registry_get "/applicability?product_type=$BIKE_TYPE_REF&at=2028-06-01T00:00:00Z" "$WORK_DIR/reg-applicability-2028.json"
    say "at 2028-06-01: the EU battery-lens binding is in force for the same type ref"

    b3_before="$(python3_count "$WORK_DIR/reg-applicability-2027.json" applicability)"
    b3_after="$(python3_count "$WORK_DIR/reg-applicability-2028.json" applicability)"
    check "EU profiles bound at 2027-06-01 (before placement)" 0 "$b3_before"
    check "EU profiles bound at 2028-06-01 (after placement)" 1 "$b3_after"

    # The importer becomes the battery producer (LMT duty): the
    # placement custody edge, dated for the border moment.
    issuer_event "$WORK_DIR/e8-instance.json" custody.transfer \
        '{"from":"momiji-mobility","to":"dusseldorf-importer","counterparty_signed":true}' \
        momiji-mobility custodian "2028-02-14T18:00:00Z"

    what "jurisdiction growth is a registry event + a custody edge — the JP lens stays mounted; manifest history stays as-of-reconstructable."

    # =====================================================================
    beat "B4" "The border moment (offline, degraded origins)"
    # =====================================================================
    what "the officer's terminal verifies the signed Tier-A pack OFFLINE — nothing is fetched — and the three readings print."

    b4_anchor="$(issuer_mint_pack "$WORK_DIR/e8-instance.json" "$WORK_DIR/b4-border.pack")"
    if [ -n "$TRUST_ANCHOR" ]; then
        say "issuer pins anchor (public key, hex): $b4_anchor"
        say "verifier pins anchor from unidpp-trust /keyring: $TRUST_ANCHOR"
        check "issuer pack anchor == trust-pinned anchor" "$TRUST_ANCHOR" "$b4_anchor"
    else
        say "issuer pins anchor (public key, hex): $b4_anchor"
    fi
    say "officer terminal: offline (Shenzhen host unreachable, EU registry mid-outage)"

    verify_and_expect "$WORK_DIR/b4-border.pack" "$b4_anchor" \
        "2028-02-15T09:30:00Z" 0 \
        "B4 border moment — PASS, as-of stamped, coverage states what was not reachable"

    log_anchor_pack "$WORK_DIR/b4-border.pack" b4-border "$BIKE_URN"

    what "verdict PASS with the three readings named (cryptographic / evidentiary / current-state) and full field coverage — honesty is the feature."

    # Optional subset exit (tests/run_tests.sh test 5 drives B1 + B4
    # against the live services): the minimum live circuit is issuance
    # (B1) plus an offline verify under the trust-pinned anchor (B4).
    if [ "${UNIDPP_E2E_STOP_AFTER:-}" = B4 ]; then
        hr "SUBSET COMPLETE — B1 through B4 (UNIDPP_E2E_STOP_AFTER=B4)"
        say "checks:   $CHECKS_OK/$CHECKS_TOTAL passed"
        say "artifacts: $WORK_DIR"
        if [ "$FAILED" -gt 0 ] || [ "$CHECKS_OK" != "$CHECKS_TOTAL" ]; then
            printf '\033[31mDEMO FAILED (%s failing checks)\033[0m\n' "$FAILED"
            exit 1
        fi
        printf '\033[32mDEMO PASSED — B1-B4 subset (live smoke).\033[0m\n'
        return 0
    fi

    # =====================================================================
    beat "B5" "Life in service (edge state, capability classes)"
    # =====================================================================
    what "the S2 BMS commits its log prefix and reveals at the dealer visit (commit-now / reveal-later); staleness becomes bounded."

    # Cleared the border, sold to the first owner (Duesseldorf).
    issuer_event "$WORK_DIR/e8-instance.json" custody.transfer \
        '{"from":"dusseldorf-importer","to":"owner-1-duesseldorf","counterparty_signed":true}' \
        dusseldorf-importer custodian "2028-03-01T10:00:00Z"

    issuer_event "$WORK_DIR/e8-instance.json" milestone.record \
        '{"counters":{"bms.cycle_count":"412","odometer.km":"2871.4"}}' \
        e8-bms-controller device "2028-11-05T11:00:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" inspection.stamp \
        "$(stamp_data "$BIKE_URN" dealer-service-duesseldorf 2028-11-05T11:00:00Z)" \
        dealer-service-duesseldorf verifier "2028-11-05T11:20:00Z"

    b5_counters

    say "capability classes on one bike: BMS logs (S2), optional Connect module (S3), silent rack/frame (S0)"
    say "who measured what, with which unit, under whose model — SoH is a derived verdict whose transform is a registered item"

    what "truth becomes bounded and auditable: the service-center era had episodic, undetectable staleness."

    # =====================================================================
    beat "B6" "Firmware update and the derestriction incident"
    # =====================================================================
    what "the OTA is a software.update (declared values change, no physical change); the dongle is a product.modify that CHANGES THE LEGAL CLASS."

    issuer_event "$WORK_DIR/e8-instance.json" software.update \
        '{"versions":{"controller":"2.4.1"},"unlocked_features":["range-algorithm-v2"]}' \
        momiji-mobility "economic operator" "2029-03-02T04:00:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" custody.transfer \
        '{"from":"owner-1-duesseldorf","to":"owner-2","counterparty_signed":true}' \
        owner-1-duesseldorf custodian "2029-05-20T15:00:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" product.modify \
        '{"description":"derestriction dongle: assistance cutoff 25 -> 45 km/h","derived_type":"momiji:e8/type/2027.1#moped-2029","reevaluation_required":true}' \
        owner-2 "accredited modifier" "2029-06-18T16:00:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" status.change \
        '{"from":"issued","to":"non-conformant","authority":"jp-road-traffic"}' \
        jp-road-traffic regulator "2029-06-19T09:00:00Z"

    what "derived type spawns (E4b), profiles re-evaluate (JP: non-conformant; EU: type-approval regime required) — graded, never binary."

    b6_anchor="$(issuer_mint_pack "$WORK_DIR/e8-instance.json" "$WORK_DIR/b6-derestricted.pack")"
    verify_and_expect "$WORK_DIR/b6-derestricted.pack" "$b6_anchor" \
        "2029-06-20T10:00:00Z" 2 \
        "B6 derestriction — the modified machine is legally an unregistered moped (expected FAIL: non-conformant)"
    log_anchor_pack "$WORK_DIR/b6-derestricted.pack" b6-derestricted "$BIKE_URN"

    # The dongle comes off at the next dealer visit; re-evaluation passes.
    issuer_event "$WORK_DIR/e8-instance.json" product.modify \
        '{"description":"dongle removed at service: assistance cutoff restored to 25 km/h","derived_type":null,"reevaluation_required":true}' \
        dealer-service-duesseldorf "accredited modifier" "2029-09-03T10:00:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" status.change \
        '{"from":"non-conformant","to":"issued","authority":"jp-road-traffic"}' \
        jp-road-traffic regulator "2029-09-04T09:00:00Z"

    what "history is never rewritten: the incident stays in the log; the state machine recovered through a legal re-evaluation event."

    # =====================================================================
    beat "B7" "Repair (regulated child swap, cross-jurisdiction install)"
    # =====================================================================
    what "water damage: uninstall + install (two events); the old pack's interval closes CARRYING ITS HISTORY."

    issuer_create "local:voltaro:wp/EU-7781" - S2 voltaro-eu \
        "https://resolver.unidpp.org/r/voltaro-wp-eu7781" \
        "$NEWPACK_URN" "$WORK_DIR/pack-2029.json"
    issuer_event "$WORK_DIR/pack-2029.json" issuance \
        '{"derived":false,"inputs":[]}' voltaro-eu "issuing authority" "2029-09-10T08:00:00Z"

    issuer_event "$WORK_DIR/e8-instance.json" uninstall \
        "$(uninstall_data "$PACK_URN" 2027-04-12T10:05:00Z 2029-09-12T10:00:00Z)" \
        dealer-service-duesseldorf installer "2029-09-12T10:00:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" install \
        "$(install_data outgoing "$NEWPACK_URN" 2029-09-12T10:30:00Z - fastened restorable firmware)" \
        dealer-service-duesseldorf installer "2029-09-12T10:30:00Z"
    issuer_event "$WORK_DIR/pack-2029.json" install \
        "$(install_data incoming "$BIKE_URN" 2029-09-12T10:30:00Z - fastened restorable firmware)" \
        dealer-service-duesseldorf installer "2029-09-12T10:30:00Z"

    # The old pack's own log closes its installation interval and carries
    # its provenance to the refurbisher.
    issuer_event "$WORK_DIR/pack-original.json" uninstall \
        "$(uninstall_data "$BIKE_URN" 2027-04-12T10:05:00Z 2029-09-12T10:00:00Z)" \
        dealer-service-duesseldorf installer "2029-09-12T10:00:00Z"
    issuer_event "$WORK_DIR/pack-original.json" custody.transfer \
        '{"from":"dealer-service-duesseldorf","to":"refurbisher-linz","counterparty_signed":true}' \
        dealer-service-duesseldorf custodian "2029-09-13T09:00:00Z"

    say "old pack: 412 cycles of H-2231 cells, removed for casing dent — harvested-part provenance IS value"
    say "new pack: EU-made (Voltaro), installed by a DE dealer into a JP-profile bike (both profiles survive)"
    say "independent repairer acted under EN 18239-style roles (right-to-repair echo)"

    b7_anchor="$(issuer_mint_pack "$WORK_DIR/pack-2029.json" "$WORK_DIR/b7-newpack.pack")"
    verify_and_expect "$WORK_DIR/b7-newpack.pack" "$b7_anchor" \
        "2029-09-12T11:00:00Z" 0 \
        "B7 new pack (post-swap) — issued, no flags"
    log_anchor_pack "$WORK_DIR/b7-newpack.pack" b7-newpack "$NEWPACK_URN"

    what "cross-jurisdiction install: both profiles survive the swap; the used-parts market keeps provenance the EN pipeline loses silently."

    # =====================================================================
    beat "B8" "Resale and auction (custody as ceremony; blind edges)"
    # =====================================================================
    what "custody transfers are signed ceremonies; the auction lens issues a dated, signed condition stamp; the buyer's household stays invisible."

    issuer_event "$WORK_DIR/e8-instance.json" custody.transfer \
        '{"from":"owner-2","to":"auction-house-vienna","counterparty_signed":true}' \
        auction-house-vienna custodian "2031-03-02T10:00:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" inspection.stamp \
        "$(stamp_data "$BIKE_URN" auction-house-vienna 2031-03-04T14:00:00Z)" \
        auction-house-vienna verifier "2031-03-04T14:30:00Z"
    issuer_event "$WORK_DIR/e8-instance.json" custody.transfer \
        '{"from":"auction-house-vienna","to":"buyer-vienna","counterparty_signed":true}' \
        auction-house-vienna custodian "2031-03-10T15:00:00Z"

    say "auction house checks the theft predicate against the flag subregister (E12) before listing"
    say "condition stamp: lens-scoped (auction lens != insurance lens), dated, signed"
    say "the buyer's household is invisible to Momiji — proof-of-binding != knowledge-of-parent (I12)"

    b8_anchor="$(issuer_mint_pack "$WORK_DIR/e8-instance.json" "$WORK_DIR/b8-auction.pack")"
    verify_and_expect "$WORK_DIR/b8-auction.pack" "$b8_anchor" \
        "2031-03-10T16:00:00Z" 0 \
        "B8 post-auction — the audit trail survives without a central watcher"
    log_anchor_pack "$WORK_DIR/b8-auction.pack" b8-auction "$BIKE_URN"

    what "enumeration resistance is a system property: the audit trail survives; the surveillance does not exist."

    # =====================================================================
    beat "B9" "A recall crosses the graph (predicate-based; dormant -> live)"
    # =====================================================================
    what "2030-05: Haichuan lot H-2231 recalled (thermal event) — the predicate is published; nobody enumerated the installed base."

    issuer_event "$WORK_DIR/cell-lot.json" recall.campaign \
        '{"campaign":"R-H2231-THERMAL","predicate":{"FactContains":{"path":"bom.lots","needle":"H-2231"}}}' \
        cn-samr regulator "2030-05-06T08:00:00Z"

    say "predicate: \"packs containing lot H-2231\" — each custodian evaluates locally against their own holdings"
    say "the Vienna bike's CURRENT pack (Voltaro, B7) is unaffected — traced through the swap edges"

    # The original pack's custodian (refurbisher -> powerwall) evaluates
    # the predicate against its own log: it DOES contain H-2231.
    issuer_event "$WORK_DIR/pack-original.json" recall.campaign \
        '{"campaign":"R-H2231-THERMAL","predicate":{"FactContains":{"path":"bom.lots","needle":"H-2231"}}}' \
        refurbisher-linz custodian "2030-05-07T09:00:00Z"

    b9_old_anchor="$(issuer_mint_pack "$WORK_DIR/pack-original.json" "$WORK_DIR/b9-oldpack.pack")"
    verify_and_expect "$WORK_DIR/b9-oldpack.pack" "$b9_old_anchor" \
        "2030-05-07T10:00:00Z" 2 \
        "B9 original pack — REACHED through its own log (expected FAIL: recall active)"
    log_anchor_pack "$WORK_DIR/b9-oldpack.pack" b9-oldpack "$PACK_URN"

    b9_new_anchor="$(issuer_mint_pack "$WORK_DIR/pack-2029.json" "$WORK_DIR/b9-newpack.pack")"
    verify_and_expect "$WORK_DIR/b9-newpack.pack" "$b9_new_anchor" \
        "2030-05-07T10:00:00Z" 1 \
        "B9 Vienna bike's current pack — unaffected by the recall (degraded only by freshness; safety clean)"
    log_anchor_pack "$WORK_DIR/b9-newpack.pack" b9-newpack "$NEWPACK_URN"

    what "the current pack's only degradation is freshness — its safety finding is clean; degradation is explicit, never silent."

    what "the recall reached the graph, not a list: computational, privacy-preserving — and the manufacturer receives aggregates."

    # =====================================================================
    beat "B-QUORUM" "Retroactive distrust is a quorum act (M-of-K, cross-jurisdiction)"
    # =====================================================================
    what "2030-06: evidence emerges that haichuan-cn misissued cell-lot certifications across 2027-2030 — the authority itself, not one lot. Retroactive distrust is authority-over-authority: no single regulator may do it; the graph demands a quorate M-of-K attestation."

    # The trust base: the live trust service (demo-live) or an
    # ephemeral instance; the beat narrates and skips when the real
    # ceremony binaries are absent (it never stages the cryptography).
    quorum_skip=""
    if [ ! -x "$QUORUM_CEREMONY_BIN" ]; then
        quorum_skip="quorum-ceremony binary missing ($QUORUM_CEREMONY_BIN; run: make deps-trust)"
    fi
    if [ -z "$quorum_skip" ] && [ -z "$TRUST_URL" ] && [ ! -x "$TRUST_BIN" ]; then
        quorum_skip="unidpp-trust binary missing ($TRUST_BIN; run: make deps-trust)"
    fi
    if [ -n "$quorum_skip" ]; then
        note "B-QUORUM narrated without running: $quorum_skip"
        say "retroactive distrust of an authority requires a quorate attestation —"
        say "one regulator's POST is refused (422 QuorumRequired); 2-of-3 members from"
        say "different jurisdictions combine threshold-Schnorr partials into ONE group"
        say "signature, the quorum node pins the group key, and the declaration lands."
        say "The full over-HTTP proof: unidpp-trust tests/quorum.rs."
    elif ! start_quorum_trust; then
        note "B-QUORUM narrated without running: the ephemeral trust service did not become healthy"
    else
        say "trust:    $QUORUM_URL — the standing authority for the quorum act"

        QUORUM_DIR="$WORK_DIR/quorum-ceremony"
        mkdir -p "$QUORUM_DIR"

        # -- 1. One regulator tries alone: the refusal is the policy. --
        printf '%s' '{"subject":{"kind":"node","id":"haichuan-cn"},"reason":{"token":"misissuance"},"declared_at":"2030-06-15T00:00:00Z","declared_by":"e8-retro-quorum","window":{"start":"2027-01-01T00:00:00Z","end":"2030-06-01T00:00:00Z"},"quorum":null}' \
            >"$QUORUM_DIR/no-attestation.json"
        quorum_code="$(quorum_post /revocations "$QUORUM_DIR/no-attestation.json" "$QUORUM_DIR/no-attestation.response.json")"
        check "B-QUORUM single regulator refused (422 — quorum attestation required)" 422 "$quorum_code"

        # -- 2. One member tries alone: the refusal is the mathematics. --
        show "quorum-ceremony declare --threshold 2 --signer reg-cn-samr  (one member alone)"
        if "$QUORUM_CEREMONY_BIN" declare \
            --quorum e8-retro-quorum --threshold 2 \
            --member reg-cn-samr --member reg-jp-meti --member reg-eu-espr \
            --signer reg-cn-samr \
            --subject-kind node --subject-id haichuan-cn \
            --reason misissuance \
            --window-start 2027-01-01T00:00:00Z --window-end 2030-06-01T00:00:00Z \
            --declared-at 2030-06-15T00:00:00Z \
            --out-dir "$QUORUM_DIR/below" >"$QUORUM_DIR/below.log" 2>&1; then
            check "B-QUORUM one-member ceremony refused by the cryptography" refused accepted
        else
            check "B-QUORUM one-member ceremony refused by the cryptography" refused refused
        fi
        say "          fewer than M partials cannot produce a group signature — no policy layer sees the request at all"

        # -- 3. The quorate ceremony: two members, two jurisdictions. --
        show "quorum-ceremony declare --threshold 2 --signer reg-cn-samr --signer reg-jp-meti  (CN+JP: 2-of-3)"
        if ! "$QUORUM_CEREMONY_BIN" declare \
            --quorum e8-retro-quorum --threshold 2 \
            --member reg-cn-samr --member reg-jp-meti --member reg-eu-espr \
            --signer reg-cn-samr --signer reg-jp-meti \
            --subject-kind node --subject-id haichuan-cn \
            --reason misissuance \
            --window-start 2027-01-01T00:00:00Z --window-end 2030-06-01T00:00:00Z \
            --declared-at 2030-06-15T00:00:00Z \
            --out-dir "$QUORUM_DIR/quorate" >"$QUORUM_DIR/quorate.log" 2>&1; then
            fail "the 2-of-3 quorum ceremony failed (see $QUORUM_DIR/quorate.log)"
        fi
        quorum_artifacts=0
        for quorum_file in quorum-node.json revocation.json ceremony.json; do
            [ -f "$QUORUM_DIR/quorate/$quorum_file" ] && quorum_artifacts=$((quorum_artifacts + 1))
        done
        check "B-QUORUM ceremony artifacts written (node pin, attestation, audit trail)" 3 "$quorum_artifacts"

        # -- 4. Pinning is load-bearing: an unpinned group key certifies
        #       nothing; then the quorum node pins it. --
        quorum_code="$(quorum_post /revocations "$QUORUM_DIR/quorate/revocation.json" "$QUORUM_DIR/unpinned.response.json")"
        check "B-QUORUM unpinned group key certifies nothing (422)" 422 "$quorum_code"
        quorum_code="$(quorum_post /nodes "$QUORUM_DIR/quorate/quorum-node.json" "$QUORUM_DIR/node.response.json")"
        check "B-QUORUM quorum node pinned (threshold group + group key registered)" 201 "$quorum_code"

        # -- 5. Quorate: the declaration lands. --
        quorum_code="$(quorum_post /revocations "$QUORUM_DIR/quorate/revocation.json" "$QUORUM_DIR/declared.response.json")"
        check "B-QUORUM quorate 2-of-3 declaration accepted (201)" 201 "$quorum_code"

        # -- 6. The pack: Tier A cannot see standing (its own finding
        #       says so); the verdict degrades through the overlay. --
        what "the lot's own pack still verifies on its own terms — but its issuing authority is now distrusted ab initio: the combined verdict a verifier must present is void."
        bq_anchor="$(issuer_mint_pack "$WORK_DIR/cell-lot.json" "$WORK_DIR/b-quorum-lot.pack")"
        verify_and_expect "$WORK_DIR/b-quorum-lot.pack" "$bq_anchor" \
            "2030-06-10T00:00:00Z" 2 \
            "B-QUORUM the lot's own pack, pre-act — the B9 campaign in its log already fails it (the quorum question is its authority, not this taint)"
        log_anchor_pack "$WORK_DIR/b-quorum-lot.pack" b-quorum-lot "$LOT_URN"

        show "GET $QUORUM_URL/revocations?at=2027-01-15T08:00:00Z&subject=node:haichuan-cn  (the lot's issuance moment)"
        check "B-QUORUM lot issuance (2027-01-15) inside the window: void ab initio" \
            "void-ab-initio" "$(quorum_standing "at=2027-01-15T08:00:00Z" standing_at_as_of)"
        check "B-QUORUM the pack verdict degrades: in-window verifications no longer stand" \
            "false" "$(quorum_standing "at=2027-01-15T08:00:00Z" verifications_at_stand)"
        check "B-QUORUM the quorum view names the form: threshold-group, quorate" \
            "threshold-group" "$(quorum_standing "at=2027-01-15T08:00:00Z" quorum.form)"

        show "GET $QUORUM_URL/revocations?at=2027-01-15T08:00:00Z&known_by=2030-06-14T23:59:59Z  (a diligent verifier, before the act was knowable)"
        check "B-QUORUM evidentiary cutoff: pre-declaration verifications stand" \
            "true" "$(quorum_standing "at=2027-01-15T08:00:00Z&known_by=2030-06-14T23:59:59Z" verifications_at_stand)"

        check "B-QUORUM before the window (2026-07): retroactivity does not leak past its start" \
            "valid" "$(quorum_standing "at=2026-07-01T00:00:00Z" standing_at_as_of)"

        stop_quorum_trust
    fi

    what "retroactive distrust took a quorum: two jurisdictions' regulators combined partials into one group signature — and the cutoff protects every verifier who acted before it was knowable."

    # =====================================================================
    beat "G-GRID" "The grid: one subject, two sovereignty segments, one spine"
    # =====================================================================
    what "Phase 1 of the build contract (REQUIREMENTS.md): sovereignty per-segment — an open EU segment and a SEALED CN segment, a commitment spine over both, and a verifier who proves the sealed segment without ever seeing it."

    show "unidpp grid"
    if ! "$UNIDPP" grid --dossier "$WORK_DIR/pack-0001-dossier.json" --frozen "$WORK_DIR/pack-0001-frozen.json" > "$WORK_DIR/ggrid.txt"; then
        fail "unidpp grid failed (see $WORK_DIR/ggrid.txt)"
    fi
    # SI-1: the frozen view — air-gapped ingest + verify +
    # re-execution in a separate process.
    if ! "$UNIDPP" frozen "$WORK_DIR/pack-0001-frozen.json" > "$WORK_DIR/frozen.txt"; then
        fail "unidpp frozen failed (see $WORK_DIR/frozen.txt)"
    fi
    check "G-GRID the frozen view verifies air-gapped (SI-1)" \
        "ok" "$(grep -c "offline frozen view" "$WORK_DIR/frozen.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID the frozen view re-execution matches the issuer render (SI-1)" \
        "ok" "$(grep -c "re-execution MATCHES" "$WORK_DIR/frozen.txt" | sed 's/1/ok/;s/0/failed/')"
    # XB-5: the offline verifier — a separate process, one file, the
    # verifier's own anchors, zero calls to foreign systems.
    if ! "$UNIDPP" dossier "$WORK_DIR/pack-0001-dossier.json" > "$WORK_DIR/dossier.txt"; then
        fail "unidpp dossier failed (see $WORK_DIR/dossier.txt)"
    fi
    check "G-GRID the offline dossier verdict: zero foreign API calls (XB-5)" \
        "ok" "$(grep -c "zero calls to foreign synchronous APIs" "$WORK_DIR/dossier.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID the offline verdict reproduces the coverage report" \
        "ok" "$(grep -c "cn-dynamic: attested-by-authority (governing policy cn-dynamic-bms v1)" "$WORK_DIR/dossier.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID the spine's log receipt verifies offline (CN-4)" \
        "ok" "$(grep -c "log receipt verified" "$WORK_DIR/dossier.txt" | sed 's/1/ok/;s/0/failed/')"
    ggrid_out="$(cat "$WORK_DIR/ggrid.txt")"
    check "G-GRID the sealed segment verifies from the spine alone"         "ok" "$(grep -c "sealed segment: existence + currency from the spine alone" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID the spine proves append-only growth"         "ok" "$(grep -c "append-only growth provable" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID forged segment state fails loudly"         "ok" "$(grep -c "forged segment state fails" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID the receiving profile decides (XB-4)"        "ok" "$(grep -c "acceptance: the receiving profile decides" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    # The sealed plaintext NEVER appears in the transcript.
    if grep -q "cycle_count=412" "$WORK_DIR/ggrid.txt"; then
        check "G-GRID the sealed contents never appear" leaked never
    else
        check "G-GRID the sealed contents never appear" never never
    fi
    # The cross-border moment (Phase 2): the CN battery case —
    # attestation offer, substitution, coverage-graded verdict.
    check "G-GRID S13 offers attestation, not data" \
        "ok" "$(grep -c "sealed policy offers ATTESTATION" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID substitution verifies under the verifier's own anchors" \
        "ok" "$(grep -c "attestation verifies under the verifier" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID the verdict is a coverage report object (verified-direct + attested)" \
        "ok" "$(grep -c "coverage report object" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID the recorded route replays the verdict byte-identically (SI-11)" \
        "ok" "$(grep -c "recorded route replays the verdict byte-identically" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID the ancestry renders the three-way report (SI-6)" \
        "ok" "$(grep -c "ancestry renders the three-way report" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    check "G-GRID retrieval withholds the sealed class WITH an offer (RT-4)" \
        "ok" "$(grep -c "sealed class withheld WITH coverage and an offer pointer" "$WORK_DIR/ggrid.txt" | sed 's/1/ok/;s/0/failed/')"
    say "18/18 in the grid verdict — the CN battery case incl. retrieval (Part 10): report, acceptance, offline dossier, frozen view, route, ancestry (XB-1..5, XB-8, SI-1/6/11, RT-4)"

    # =====================================================================
    beat "B10" "End of life (the material loop closes)"
    # =====================================================================
    what "E13 decompose = inverse transformation 1 -> N into material passports with mass balance; end-of-waste is a NEW passport issuance."

    issuer_create "local:recycler:scrap-steel/J-000842" - S0 steelworks-linz \
        "https://resolver.unidpp.org/r/scrap-steel-j000842" \
        "$SCRAP_URN" "$WORK_DIR/scrap-steel.json"
    issuer_create "local:recycler:pack-material/J-000842" - S0 recycler-linz \
        "https://resolver.unidpp.org/r/recycle-pack-j000842" \
        "$RECYCLE_URN" "$WORK_DIR/recycle-pack.json"

    issuer_event "$WORK_DIR/e8-instance.json" decompose \
        "$(decompose_data)" \
        recycler-linz recycler "2033-07-14T09:00:00Z"

    issuer_event "$WORK_DIR/scrap-steel.json" end-of-waste \
        '{"evidence_ref":"eow-cert-linz-2033-0742","outputs":[]}' \
        steelworks-linz "accredited actor" "2033-07-15T08:00:00Z"

    b10_scrap_anchor="$(issuer_mint_pack "$WORK_DIR/scrap-steel.json" "$WORK_DIR/b10-scrap.pack")"
    verify_and_expect "$WORK_DIR/b10-scrap.pack" "$b10_scrap_anchor" \
        "2033-07-15T09:00:00Z" 1 \
        "B10 end-of-waste scrap passport — DEGRADED by status (waste regime re-entry), not by trust: the moment scrap legally re-qualifies"
    log_anchor_pack "$WORK_DIR/b10-scrap.pack" b10-scrap "$SCRAP_URN"

    b10_bike_anchor="$(issuer_mint_pack "$WORK_DIR/e8-instance.json" "$WORK_DIR/b10-bike.pack")"
    verify_and_expect "$WORK_DIR/b10-bike.pack" "$b10_bike_anchor" \
        "2033-07-15T09:00:00Z" 1 \
        "B10 decomposed bike — transformed: degraded-with-reason under archival semantics" \
        "--max-age 0"
    log_anchor_pack "$WORK_DIR/b10-bike.pack" b10-bike "$BIKE_URN"

    mass_balance

    what "circularity became auditable arithmetic: in - out = loss; recycled-content claims compute from the graph."

    # =====================================================================
    hr "STORY COMPLETE — B1 through B10"
    # =====================================================================
    say "checks:   $CHECKS_OK/$CHECKS_TOTAL passed"
    say "artifacts: $WORK_DIR (passports, packs, registry responses)"
    if [ -n "$LOG_URL" ] && [ -d "$LOG_RECEIPTS" ]; then
        receipt_count=0
        for receipt_file in "$LOG_RECEIPTS"/*.receipt.json; do
            [ -e "$receipt_file" ] && receipt_count=$((receipt_count + 1))
        done
        say "log:      $receipt_count signed inclusion receipts in $LOG_RECEIPTS"
    fi
    if [ "$FAILED" -gt 0 ] || [ "$CHECKS_OK" != "$CHECKS_TOTAL" ]; then
        printf '\033[31mDEMO FAILED (%s failing checks)\033[0m\n' "$FAILED"
        exit 1
    fi
    printf '\033[32mDEMO PASSED — all verify outcomes matched the story.\033[0m\n'
}

# Mirror the console output into the transcript. Process substitution —
# not a pipeline — keeps `main` in THIS shell: the variables it sets
# (REGISTRY_PID) must survive so the EXIT trap can stop the services it
# started (a pipeline subshell would orphan them). The work dir must
# exist before tee opens the file.
mkdir -p "$WORK_DIR"
exec > >(tee "$TRANSCRIPT") 2>&1
main "$@"
exit $?
