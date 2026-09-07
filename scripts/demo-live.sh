#!/usr/bin/env bash
# demo-live.sh — the LIVE-service demo.
#
# `make demo` walks the story against the registry + the CLI driver.
# This script starts ALL FOUR sibling services for real —
#
#   unidpp-registry  127.0.0.1:8098   items, applicability, transforms
#   unidpp-issuer    127.0.0.1:8096   server-signed events, server-minted packs
#   unidpp-trust     127.0.0.1:8092   verify anchors (GET /keyring)
#   unidpp-log       127.0.0.1:8194   transparency log (POST /commitments)
#                                    (not :8092 — that port is trust's, and
#                                     also the log's own default; they must
#                                     not collide)
#
# — each with its JSONL journal under build/live/, waits for every
# /healthz, then hands the whole B1–B10 story to scripts/demo.sh with
# the live URLs exported:
#
#   UNIDPP_ISSUER_URL  issuance goes through the issuer's HTTP API
#   UNIDPP_TRUST_URL   verify anchors are pinned from GET /keyring —
#                      the pinned P-256 key IS the issuer's pack
#                      signer: both services derive it from the same
#                      ceremony seed below (env-key mode). Production
#                      pins issuer keys through jurisdiction trust
#                      lists; the shared dev seed plays that role.
#                      demo.sh B4 asserts the two anchors are
#                      byte-identical, so seed drift fails loudly.
#   UNIDPP_LOG_URL     every minted pack's commitment is anchored in
#                      the transparency log; the signed inclusion
#                      receipts land in build/live/e2e/log-receipts/
#
# Journals are wiped at start (fresh sequencing per run): the log's
# receipt ids and the registry's applicability assertions stay
# deterministic across re-runs.

set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FAMILY_DIR="$(cd "$ROOT_DIR/.." && pwd)"
LIVE_DIR="${UNIDPP_E2E_LIVE_DIR:-$ROOT_DIR/build/live}"

REGISTRY_BIN="${UNIDPP_REGISTRY_BIN:-$FAMILY_DIR/unidpp-registry/target/release/unidpp-registry}"
ISSUER_BIN="$FAMILY_DIR/unidpp-issuer/target/release/unidpp-issuer"
TRUST_BIN="$FAMILY_DIR/unidpp-trust/target/release/unidpp-trust"
LOG_BIN="$FAMILY_DIR/unidpp-log/target/release/unidpp-log"

REGISTRY_BIND="${UNIDPP_REGISTRY_BIND:-127.0.0.1:8098}"
ISSUER_BIND="${UNIDPP_ISSUER_BIND:-127.0.0.1:8096}"
TRUST_BIND="${UNIDPP_TRUST_BIND:-127.0.0.1:8092}"
LOG_BIND="${UNIDPP_LOG_BIND:-127.0.0.1:8194}"

# The pinned-anchor ceremony, dev edition (fixed constants keep the
# derived public keys — and therefore the transcripts — reproducible):
#   LIVE_ED25519_SEED  the issuer's event key + the trust service's
#                      Ed25519 response key;
#   LIVE_P256_SEED     the issuer's pack-signing key AND the trust
#                      service's pinned sign-ecdsa-p256 anchor.
LIVE_ED25519_SEED="4c4956452d4556454e542d534545442d30313233343536373839616263646566"
LIVE_P256_SEED="4c4956452d5041434b2d534545442d30313233343536373839616263646566"

PIDS=""

fail() {
    printf '\033[31mdemo-live: %s\033[0m\n' "$1" >&2
    exit 1
}

note() { # orchestration notes (not story narration)
    printf '\033[33m    [demo-live] %s\033[0m\n' "$*"
}

cleanup() {
    for cleanup_pid in $PIDS; do
        kill "$cleanup_pid" 2>/dev/null
    done
    for cleanup_pid in $PIDS; do
        wait "$cleanup_pid" 2>/dev/null
    done
}
trap cleanup EXIT INT TERM

wait_healthy() { # wait_healthy <name> <url>
    wh_name="$1"
    wh_url="$2"
    wh_try=0
    while [ "$wh_try" -lt 100 ]; do
        if curl -sf "$wh_url/healthz" >/dev/null 2>&1; then
            printf '    \033[32m%s healthy at %s\033[0m\n' "$wh_name" "$wh_url"
            return 0
        fi
        wh_try=$((wh_try + 1))
        sleep 0.1
    done
    fail "$wh_name did not become healthy on $wh_url"
}

mkdir -p "$LIVE_DIR"
# Fresh sequencing per run (these journals are this script's own
# build/live/ artifacts — never a sibling repo's state).
rm -f "$LIVE_DIR/registry.journal.jsonl" "$LIVE_DIR/issuer.journal.jsonl" \
    "$LIVE_DIR/trust.journal.jsonl" "$LIVE_DIR/log.journal.jsonl"

# --- 1. registry (items, applicability, transforms) -----------------------
[ -x "$REGISTRY_BIN" ] || fail "unidpp-registry binary missing: $REGISTRY_BIN (run: make deps-live)"
note "starting unidpp-registry on $REGISTRY_BIND (journal: $LIVE_DIR/registry.journal.jsonl)"
UNIDPP_REGISTRY_BIND="$REGISTRY_BIND" \
UNIDPP_REGISTRY_STATE_FILE="$LIVE_DIR/registry.journal.jsonl" \
    "$REGISTRY_BIN" >/dev/null 2>&1 &
PIDS="$PIDS $!"
wait_healthy unidpp-registry "http://$REGISTRY_BIND"

# --- 2. issuer (server-signed events, server-minted packs) ----------------
[ -x "$ISSUER_BIN" ] || fail "unidpp-issuer binary missing: $ISSUER_BIN (run: make deps-live)"
note "starting unidpp-issuer on $ISSUER_BIND (journal: $LIVE_DIR/issuer.journal.jsonl)"
UNIDPP_ISSUER_BIND="$ISSUER_BIND" \
UNIDPP_ISSUER_STATE_FILE="$LIVE_DIR/issuer.journal.jsonl" \
UNIDPP_ISSUER_EVENT_SEED="$LIVE_ED25519_SEED" \
UNIDPP_ISSUER_PACK_SEED="$LIVE_P256_SEED" \
    "$ISSUER_BIN" >/dev/null 2>&1 &
PIDS="$PIDS $!"
wait_healthy unidpp-issuer "http://$ISSUER_BIND"

# --- 3. trust (verify anchors via GET /keyring) ---------------------------
[ -x "$TRUST_BIN" ] || fail "unidpp-trust binary missing: $TRUST_BIN (run: make deps-live)"
note "starting unidpp-trust on $TRUST_BIND (journal: $LIVE_DIR/trust.journal.jsonl)"
UNIDPP_TRUST_BIND="$TRUST_BIND" \
UNIDPP_TRUST_STATE_FILE="$LIVE_DIR/trust.journal.jsonl" \
UNIDPP_TRUST_SIGN_SEED="$LIVE_ED25519_SEED" \
UNIDPP_TRUST_SIGN_SEED_P256="$LIVE_P256_SEED" \
    "$TRUST_BIN" >/dev/null 2>&1 &
PIDS="$PIDS $!"
wait_healthy unidpp-trust "http://$TRUST_BIND"

# --- 4. log (transparency log: pack commitments -> receipts) --------------
[ -x "$LOG_BIN" ] || fail "unidpp-log binary missing: $LOG_BIN (run: make deps-live)"
note "starting unidpp-log on $LOG_BIND (journal: $LIVE_DIR/log.journal.jsonl)"
UNIDPP_LOG_BIND="$LOG_BIND" \
UNIDPP_LOG_STATE_FILE="$LIVE_DIR/log.journal.jsonl" \
    "$LOG_BIN" >/dev/null 2>&1 &
PIDS="$PIDS $!"
wait_healthy unidpp-log "http://$LOG_BIND"

# --- hand the story to the orchestrator ------------------------------------
printf '\n\033[1mUniDPP live demo — four services up, the E8 story next\033[0m\n'
printf '    registry : http://%s\n' "$REGISTRY_BIND"
printf '    issuer   : http://%s\n' "$ISSUER_BIND"
printf '    trust    : http://%s (verify anchor source: GET /keyring)\n' "$TRUST_BIND"
printf '    log      : http://%s (pack commitments -> signed receipts)\n' "$LOG_BIND"
printf '    anchor   : trust /keyring pins the issuer pack key (shared dev seed)\n'
printf '    journals : %s/*.journal.jsonl\n' "$LIVE_DIR"
printf '\n'

export UNIDPP_REGISTRY_URL="http://$REGISTRY_BIND"
export UNIDPP_ISSUER_URL="http://$ISSUER_BIND"
export UNIDPP_TRUST_URL="http://$TRUST_BIND"
export UNIDPP_LOG_URL="http://$LOG_BIND"
export UNIDPP_E2E_WORK_DIR="$LIVE_DIR/e2e"

"$SCRIPT_DIR/demo.sh" "$@"
demo_status=$?
exit "$demo_status"
