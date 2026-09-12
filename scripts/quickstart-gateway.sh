#!/usr/bin/env bash
# quickstart-gateway.sh — adoption path 3 of 6 (the CI-exercised
# third): AUGMENT-EXISTING via the FEDERATION GATEWAY.
#
# The adopter already has a passport system — an EN 18222 deployment,
# UNTP consumers, or both — and adopts UniDPP without touching it:
# `unidpp-gateway` runs as the translation edge in front of one
# `unidpp-issuer`, rendering the same core state through both
# foreign protocol bindings. Existing consumers keep consuming; the
# neutral core stays theirs.
#
# Steps: issuer standalone (no registry, no log, no trust) → gateway
# pointed at it → one passport + event issued → BOTH bindings serve
# the same identity (EN 18222 dppsByProductId and UNTP product) →
# gateway stopped, issuer still whole. Exit 0 = every check held.

set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$HERE/.." && pwd)"
FAMILY_DIR="$(cd "$ROOT_DIR/.." && pwd)"
WORK="${UNIDPP_QS_GATEWAY_WORK:-$ROOT_DIR/build/quickstart-gateway}"

ISSUER_BIN="${UNIDPP_ISSUER_BIN:-$FAMILY_DIR/unidpp-issuer/target/release/unidpp-issuer}"
GATEWAY_BIN="${UNIDPP_GATEWAY_BIN:-$FAMILY_DIR/unidpp-gateway/target/release/unidpp-gateway}"

ISSUER_BIND="${UNIDPP_QS_GATEWAY_ISSUER_BIND:-127.0.0.1:18521}"
GATEWAY_BIND="${UNIDPP_QS_GATEWAY_BIND:-127.0.0.1:18522}"
ISSUER_URL="http://$ISSUER_BIND"
GATEWAY_URL="http://$GATEWAY_BIND"

pass=0
fail=0
ok()  { printf '  \033[32m[ok]\033[0m   %s\n' "$1"; pass=$((pass + 1)); }
bad() { printf '  \033[31m[FAIL]\033[0m %s\n' "$1"; fail=$((fail + 1)); }

printf '\033[1m== adoption path 3 ==\033[0m  augment-existing: the gateway as translation edge\n'

for bin in "$ISSUER_BIN" "$GATEWAY_BIN"; do
    if [ ! -x "$bin" ]; then
        printf '  \033[33m[SKIP]\033[0m binary unavailable: %s\n' "$bin"
        exit 77
    fi
done

cleanup() {
    [ -n "${GATEWAY_PID:-}" ] && kill "$GATEWAY_PID" 2>/dev/null
    [ -n "${ISSUER_PID:-}" ] && kill "$ISSUER_PID" 2>/dev/null
    [ -n "${GATEWAY_PID:-}" ] && wait "$GATEWAY_PID" 2>/dev/null
    [ -n "${ISSUER_PID:-}" ] && wait "$ISSUER_PID" 2>/dev/null
    return 0
}
trap cleanup EXIT INT TERM

rm -rf "$WORK"
mkdir -p "$WORK"

# --- the upstream: one standalone issuer ---------------------------

UNIDPP_ISSUER_BIND="$ISSUER_BIND" \
UNIDPP_ISSUER_STATE_FILE="$WORK/issuer-journal.jsonl" \
    "$ISSUER_BIN" >"$WORK/issuer.log" 2>&1 &
ISSUER_PID=$!

# --- the adoption: the gateway in front of it ----------------------

UNIDPP_GATEWAY_BIND="$GATEWAY_BIND" \
UNIDPP_ISSUER_URL="$ISSUER_URL" \
    "$GATEWAY_BIN" >"$WORK/gateway.log" 2>&1 &
GATEWAY_PID=$!

issuer_ready=0
gateway_ready=0
for _ in $(seq 1 50); do
    [ "$issuer_ready" = 0 ] && curl -sf "$ISSUER_URL/healthz" >/dev/null 2>&1 && issuer_ready=1
    [ "$gateway_ready" = 0 ] && curl -sf "$GATEWAY_URL/healthz" >/dev/null 2>&1 && gateway_ready=1
    [ "$issuer_ready" = 1 ] && [ "$gateway_ready" = 1 ] && break
    sleep 0.2
done
[ "$issuer_ready" = 1 ] && ok "issuer healthy at $ISSUER_URL" || bad "issuer never became healthy"
[ "$gateway_ready" = 1 ] && ok "gateway healthy at $GATEWAY_URL (upstream: the issuer)" \
                        || { bad "gateway never became healthy"; tail -5 "$WORK/gateway.log"; }

# --- one passport through the upstream ------------------------------

curl -sf -X POST "$ISSUER_URL/passports" \
    -H 'content-type: application/json' \
    -d '{"identity":"gtin:4006381333931","type_ref":"https://example.org/types/battery-pack","capability":"S1"}' \
    >"$WORK/passport.json" \
    && ok "passport issued at the upstream" \
    || bad "passport creation failed"

PASSPORT_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["passport_id"])' "$WORK/passport.json" 2>/dev/null || true)"
PRODUCT_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["product_id"])' "$WORK/passport.json" 2>/dev/null || true)"
# The issuer keys GS1 identities as `01+<gtin>`; the bindings want
# the bare GTIN.
GTIN="${PRODUCT_ID#01+}"
GTIN="${GTIN#gtin:}"

curl -sf -X POST "$ISSUER_URL/passports/$PASSPORT_ID/events" \
    -H 'content-type: application/json' \
    -d '{"type":"status.change","data":{"from":"issued","to":"suspended","authority":"qs-gateway-check"}}' \
    >/dev/null \
    && ok "event appended at the upstream" \
    || bad "event append failed"

# --- the augment: both foreign bindings over one core state --------
#
# The pre-existing catalogue (the fixtures the gateway ships — the
# stand-in for the DPPs the adopter already serves) answers through
# BOTH protocol bindings, and the two renders name the same product.

en_code="$(curl -s -o "$WORK/en18222.json" -w '%{http_code}' \
    "$GATEWAY_URL/en18222/v1/dppsByProductId/4006381333931")"
[ "$en_code" = 200 ] && ok "EN 18222 binding answered 200 for the catalogue GTIN" \
                   || bad "EN 18222 binding answered $en_code"

untp_code="$(curl -s -o "$WORK/untp.json" -w '%{http_code}' \
    "$GATEWAY_URL/untp/product/4006381333931")"
[ "$untp_code" = 200 ] && ok "UNTP binding answered 200 for the same product" \
                   || bad "UNTP binding answered $untp_code"

python3 - "$WORK/en18222.json" "$WORK/untp.json" <<'PY'
import json, sys
en = json.load(open(sys.argv[1]))
untp = json.load(open(sys.argv[2]))
en_identity = en["uniqueProductIdentifier"]
untp_identity = untp["passport"]["productIdentifiers"][0]["value"]
# The UNTP form carries the GS1 AI-delimited form; the EN form the
# bare key — both name the same product (the AD-3 parity).
bare = untp_identity.rsplit(")", 1)[1] if ")" in untp_identity else untp_identity
assert bare == en_identity, f"identity divergence: {untp_identity} vs {en_identity}"
print("parity")
PY
[ $? = 0 ] && ok "the two bindings name the SAME product identity (AD-3 parity)" \
           || bad "the bindings diverged on identity"

# --- new issuance flows through the adopted core --------------------
#
# A passport issued at the upstream serves through the gateway's UNTP
# binding under its passport id — the live path, no fixtures involved.

live_code="$(curl -s -o "$WORK/untp-live.json" -w '%{http_code}' \
    "$GATEWAY_URL/untp/product/$PASSPORT_ID")"
[ "$live_code" = 200 ] && ok "the newly issued passport serves through the UNTP binding" \
                   || bad "live passport through the gateway answered $live_code"

python3 - "$WORK/untp-live.json" "$PRODUCT_ID" <<'PY'
import json, sys
untp = json.load(open(sys.argv[1]))
issued = sys.argv[2]
values = [v["value"] for v in untp["passport"]["productIdentifiers"]]
# `01+<gtin>` and `(01)<gtin>` are the same GS1 identity in two
# spellings; compare the bare keys.
bare_keys = [v.rsplit(")", 1)[1] if v.startswith("(") else v for v in values]
issued_bare = issued[3:] if issued.startswith("01+") else issued
assert issued_bare in bare_keys, \
    f"the UNTP render does not carry the issued identity {issued}: {values}"
print("live")
PY
[ $? = 0 ] && ok "the live render carries the identity the issuer issued" \
           || bad "the live render lost the issued identity"

# --- the adoption is reversible: gateway gone, upstream whole -------

kill "$GATEWAY_PID" 2>/dev/null
wait "$GATEWAY_PID" 2>/dev/null
GATEWAY_PID=""

curl -sf -m 1 "$GATEWAY_URL/healthz" >/dev/null 2>&1 \
    && bad "gateway still up" \
    || ok "gateway stopped"

curl -sf "$ISSUER_URL/passports/$PASSPORT_ID" >/dev/null \
    && ok "upstream issuer unaffected by the gateway's removal" \
    || bad "upstream issuer lost the passport"

printf '\n  summary: %d passed, %d failed\n' "$pass" "$fail"
[ "$fail" = 0 ]
