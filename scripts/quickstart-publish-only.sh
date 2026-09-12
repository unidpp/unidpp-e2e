#!/usr/bin/env bash
# quickstart-publish-only.sh — adoption path 2 of 6: PUBLISH-ONLY.
#
# The adopter issues passports and mints packs, and runs nothing
# else: no registry, no transparency log, no trust service, no
# gateway, no console. One `unidpp-issuer` deployment on loopback,
# its journal on disk; the packs it mints verify offline against
# the keyring it publishes.
#
# Steps: start the issuer standalone → healthz + keyring (the
# anchor set a verifier pins) → create a passport, append a signed
# event, mint the pack, all through the issuer's own API → stop
# the service → verify the minted pack OFFLINE with the officer's
# terminal against the pinned anchor. Exit 0 = every check held.

set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$HERE/.." && pwd)"
FAMILY_DIR="$(cd "$ROOT_DIR/.." && pwd)"
WORK="${UNIDPP_QS_PUBLISH_WORK:-$ROOT_DIR/build/quickstart-publish-only}"

ISSUER_BIN="${UNIDPP_ISSUER_BIN:-$FAMILY_DIR/unidpp-issuer/target/release/unidpp-issuer}"
UNIDPP="${UNIDPP_BIN:-$FAMILY_DIR/unidpp-cli/target/release/unidpp}"

BIND="${UNIDPP_QS_PUBLISH_BIND:-127.0.0.1:18511}"
URL="http://$BIND"

pass=0
fail=0
ok()  { printf '  \033[32m[ok]\033[0m   %s\n' "$1"; pass=$((pass + 1)); }
bad() { printf '  \033[31m[FAIL]\033[0m %s\n' "$1"; fail=$((fail + 1)); }

printf '\033[1m== adoption path 2 ==\033[0m  publish-only: one issuer, nothing else\n'

for bin in "$ISSUER_BIN" "$UNIDPP"; do
    if [ ! -x "$bin" ]; then
        printf '  \033[33m[SKIP]\033[0m binary unavailable: %s\n' "$bin"
        exit 77
    fi
done

cleanup() {
    [ -n "${ISSUER_PID:-}" ] && kill "$ISSUER_PID" 2>/dev/null
    [ -n "${ISSUER_PID:-}" ] && wait "$ISSUER_PID" 2>/dev/null
    return 0
}
trap cleanup EXIT INT TERM

rm -rf "$WORK"
mkdir -p "$WORK"

# --- the whole deployment: one issuer, standalone ------------------

UNIDPP_ISSUER_BIND="$BIND" \
UNIDPP_ISSUER_STATE_FILE="$WORK/issuer-journal.jsonl" \
    "$ISSUER_BIN" >"$WORK/issuer.log" 2>&1 &
ISSUER_PID=$!

ready=0
for _ in $(seq 1 50); do
    curl -sf "$URL/healthz" >/dev/null 2>&1 && { ready=1; break; }
    sleep 0.2
done
[ "$ready" = 1 ] && ok "issuer healthy at $URL (standalone: no registry, no log, no trust)" \
               || { bad "issuer never became healthy"; tail -5 "$WORK/issuer.log"; exit 1; }

# --- the anchor set a verifier pins --------------------------------

curl -sf "$URL/keyring" >"$WORK/keyring.json" \
    && ok "keyring published" \
    || bad "no keyring"

ANCHOR="$(python3 - "$WORK/keyring.json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1]))
roles = doc.get("roles", doc)
for role, keys in (roles.items() if isinstance(roles, dict) else []):
    if role == "pack":
        # the first pack key's public hex, whatever shape it takes
        if isinstance(keys, dict):
            for k in ("public", "public_hex", "anchor"):
                if isinstance(keys.get(k), str):
                    print(keys[k]); break
        elif isinstance(keys, list) and keys and isinstance(keys[0], dict):
            for k in ("public", "public_hex", "anchor"):
                if isinstance(keys[0].get(k), str):
                    print(keys[0][k]); break
        break
PY
)"
[ -n "$ANCHOR" ] && ok "pack anchor pinned from the keyring (${ANCHOR:0:16}…)" \
                || { bad "could not read the pack anchor"; python3 -m json.tool "$WORK/keyring.json" | head -20; }

# --- publish: passport, event, pack — all through the API ----------

ID_JSON="$(curl -sf -X POST "$URL/passports" \
    -H 'content-type: application/json' \
    -d '{"identity":"gtin:4006381333931","type_ref":"https://example.org/types/battery-pack","capability":"S1"}' \
    >"$WORK/passport.json" && echo yes)" \
    && ok "passport issued through the API" \
    || bad "passport creation failed"

PASSPORT_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["passport_id"])' "$WORK/passport.json" 2>/dev/null || true)"
[ -n "$PASSPORT_ID" ] && ok "passport id: $PASSPORT_ID" || bad "no passport_id in the response"

curl -sf -X POST "$URL/passports/$PASSPORT_ID/events" \
    -H 'content-type: application/json' \
    -d '{"type":"custody.transfer","data":{"from":"mfg","to":"dist","counterparty_signed":true}}' \
    >/dev/null \
    && ok "event appended (server-signed)" \
    || bad "event append failed"

curl -sf -X POST "$URL/passports/$PASSPORT_ID/pack" \
    -H 'content-type: application/json' -d '{}' >"$WORK/pack-response.json" \
    && ok "Tier-A pack minted through the API" \
    || bad "pack mint failed"

python3 -c 'import json,sys; open(sys.argv[2],"w").write(json.load(open(sys.argv[1]))["pack"])' \
    "$WORK/pack-response.json" "$WORK/pack.hex" \
    && ok "pack bytes staged for the officer's terminal" \
    || bad "pack staging failed"

# --- the service is gone; the pack still verifies -------------------

cleanup
ISSUER_PID=""
curl -sf -m 1 "$URL/healthz" >/dev/null 2>&1 \
    && bad "issuer still up" \
    || ok "issuer stopped"

"$UNIDPP" verify "$WORK/pack.hex" --anchor "$ANCHOR" --max-age 0 \
    >"$WORK/verify.stdout" 2>&1
code=$?
[ "$code" -eq 0 ] && ok "verdict: PASS, offline, after the issuer is gone" \
                 || { bad "expected PASS, exit $code"; sed -n '1,8p' "$WORK/verify.stdout"; }

printf '\n  summary: %d passed, %d failed\n' "$pass" "$fail"
[ "$fail" = 0 ]
