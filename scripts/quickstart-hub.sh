#!/usr/bin/env bash
# quickstart-hub.sh — the hub attachment journey surface (the last
# host-journey step): a stateless signed relay between willing pairs
# divided by protocols.
#
# Steps: the declaration fixtures (a willing EU<->CN pair, a pair
# whose JP side declines at L0) → the hub service → the willing pair
# relays and the relay VERIFIES under the hub's keyring key → the
# declining pair is refused with both endpoints and the WILL gap
# named → the absent declaration is stated, never silence → the hub
# holds nothing after (stateless: restart it and the same relay
# verifies). Exit 0 = every check held.

set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$HERE/.." && pwd)"
FAMILY_DIR="$(cd "$ROOT_DIR/.." && pwd)"
WORK="${UNIDPP_QS_HUB_WORK:-$ROOT_DIR/build/quickstart-hub}"

HUB_BIN="${UNIDPP_HUB_BIN:-$FAMILY_DIR/unidpp-hub/target/release/unidpp-hub}"
BIND="${UNIDPP_QS_HUB_BIND:-127.0.0.1:18581}"
URL="http://$BIND"

pass=0
fail=0
ok()  { printf '  \033[32m[ok]\033[0m   %s\n' "$1"; pass=$((pass + 1)); }
bad() { printf '  \033[31m[FAIL]\033[0m %s\n' "$1"; fail=$((fail + 1)); }

printf '\033[1m== hub attachment ==\033[0m  the stateless signed relay between willing pairs\n'

if [ ! -x "$HUB_BIN" ]; then
    printf '  \033[33m[SKIP]\033[0m hub binary unavailable: %s\n' "$HUB_BIN"
    exit 77
fi

rm -rf "$WORK"; mkdir -p "$WORK"

# --- the fixtures: both sides' published declarations ---------------

( cd "$FAMILY_DIR/unidpp-hub" && cargo run --release --example declarations -- "$WORK" ) >/dev/null 2>&1 \
    && ok "declaration fixtures generated (a willing pair, a declining pair)" \
    || bad "the declarations example failed"

UNIDPP_HUB_BIND="$BIND" UNIDPP_HUB_ID="hub-qs" UNIDPP_HUB_SEED="qs-seed" \
    "$HUB_BIN" >"$WORK/hub.log" 2>&1 &
HUB_PID=$!
cleanup() { kill "$HUB_PID" 2>/dev/null; wait "$HUB_PID" 2>/dev/null; }
trap cleanup EXIT INT TERM

ready=0
for _ in $(seq 1 50); do
    curl -sf "$URL/healthz" >/dev/null 2>&1 && { ready=1; break; }
    sleep 0.2
done
[ "$ready" = 1 ] && ok "hub healthy at $URL" || { bad "hub never became healthy"; tail -3 "$WORK/hub.log"; exit 1; }

# --- the willing pair relays ----------------------------------------

python3 - "$WORK/willing.json" > "$WORK/relay-req.json" <<'PY'
import json, sys
decls = json.load(open(sys.argv[1]))
print(json.dumps({
    "from": "eu-scheme", "to": "cn-scheme", "data_class": "*",
    "evidence_hex": "deadbeef01", "declarations": decls}))
PY

code="$(curl -s -o "$WORK/relay-resp.json" -w '%{http_code}' -X POST "$URL/relay" \
    -H 'content-type: application/json' --data-binary @"$WORK/relay-req.json")"
[ "$code" = 200 ] && ok "the willing pair relays (HTTP $code)" || bad "the relay answered $code"

python3 - "$WORK/relay-resp.json" > "$WORK/verify-req.json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1]))
print(json.dumps({"evidence_hex": doc["evidence_hex"], "relay": doc["relay"]}))
PY
verified="$(curl -s -X POST "$URL/relay/verify" -H 'content-type: application/json' \
    --data-binary @"$WORK/verify-req.json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("verified"))')"
[ "$verified" = True ] && ok "the relay VERIFIES under the hub's keyring key" || bad "the relay did not verify ($verified)"

# Tampered evidence under the same signature: caught.
python3 - "$WORK/verify-req.json" > "$WORK/verify-tampered.json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1]))
doc["evidence_hex"] = "deadbeef02"
print(json.dumps(doc))
PY
caught="$(curl -s -X POST "$URL/relay/verify" -H 'content-type: application/json' \
    --data-binary @"$WORK/verify-tampered.json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("verified"))')"
[ "$caught" = False ] && ok "tampered evidence under the same signature: caught" || bad "tampering slipped ($caught)"

# --- the declining pair: the WILL gap, never brokered around -------

python3 - "$WORK/declining.json" > "$WORK/decline-req.json" <<'PY'
import json, sys
decls = json.load(open(sys.argv[1]))
print(json.dumps({
    "from": "eu-scheme", "to": "jp-scheme", "data_class": "*",
    "evidence_hex": "00", "declarations": decls}))
PY
code="$(curl -s -o "$WORK/decline-resp.json" -w '%{http_code}' -X POST "$URL/relay" \
    -H 'content-type: application/json' --data-binary @"$WORK/decline-req.json")"
reason="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("error",""))' "$WORK/decline-resp.json" 2>/dev/null)"
[ "$code" = 422 ] && ok "the declining pair is refused (HTTP 422)" || bad "the declining pair answered $code"
echo "$reason" | grep -q "WILL gap" \
    && ok "the refusal names the WILL gap — jp-scheme declined, the hub does not bridge" \
    || bad "the refusal lost the reason: $reason"

# --- the absent declaration: stated, never silence -----------------

python3 - "$WORK/willing.json" > "$WORK/absent-req.json" <<'PY'
import json, sys
decls = json.load(open(sys.argv[1]))
print(json.dumps({
    "from": "eu-scheme", "to": "kr-scheme", "data_class": "*",
    "evidence_hex": "00", "declarations": decls}))
PY
code="$(curl -s -o "$WORK/absent-resp.json" -w '%{http_code}' -X POST "$URL/relay" \
    -H 'content-type: application/json' --data-binary @"$WORK/absent-req.json")"
reason="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("error",""))' "$WORK/absent-resp.json" 2>/dev/null)"
[ "$code" = 422 ] && ok "the absent declaration is stated (HTTP 422)" || bad "the absent case answered $code"
echo "$reason" | grep -q "kr-scheme" \
    && ok "the absence names the silent side" \
    || bad "the absence lost the side: $reason"

# --- stateless: restart, the same relay verifies --------------------

cleanup
UNIDPP_HUB_BIND="$BIND" UNIDPP_HUB_ID="hub-qs" UNIDPP_HUB_SEED="qs-seed" \
    "$HUB_BIN" >>"$WORK/hub.log" 2>&1 &
HUB_PID=$!
ready=0
for _ in $(seq 1 50); do
    curl -sf "$URL/healthz" >/dev/null 2>&1 && { ready=1; break; }
    sleep 0.2
done
[ "$ready" = 1 ] || { bad "the hub did not come back"; exit 1; }
verified="$(curl -s -X POST "$URL/relay/verify" -H 'content-type: application/json' \
    --data-binary @"$WORK/verify-req.json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("verified"))')"
[ "$verified" = True ] && ok "after a restart the same relay still verifies — the hub held nothing" \
                     || bad "statelessness broke ($verified)"

printf '\n  summary: %d passed, %d failed\n' "$pass" "$fail"
[ "$fail" = 0 ]
