#!/usr/bin/env bash
# quickstart-verify-only.sh — adoption path 1 of 6: VERIFY-ONLY.
#
# The adopter holds one binary — `unidpp`, the officer's terminal —
# and no services at all. What arrives at the border is a pack file
# and a pinned anchor (from a jurisdiction trust list, a peer's
# keyring, or paper). The terminal unpacks, checks, and grades; it
# never phones home.
#
# This script is self-contained for CI: it mints a specimen pack with
# the same binary first (a real verify-only adopter starts at step 3
# with a pack someone else minted), then verifies it offline, then
# proves the terminal catches a tampered pack with no services to
# ask. Zero sockets are opened.
#
# Exit 0 = every check held. Any failure prints the diagnostic.

set -u
set -o pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$HERE/.." && pwd)"
FAMILY_DIR="$(cd "$ROOT_DIR/.." && pwd)"
WORK="${UNIDPP_QS_VERIFY_WORK:-$ROOT_DIR/build/quickstart-verify-only}"

UNIDPP="${UNIDPP_BIN:-$FAMILY_DIR/unidpp-cli/target/release/unidpp}"

pass=0
fail=0

ok()   { printf '  \033[32m[ok]\033[0m   %s\n' "$1"; pass=$((pass + 1)); }
bad()  { printf '  \033[31m[FAIL]\033[0m %s\n' "$1"; fail=$((fail + 1)); }

printf '\033[1m== adoption path 1 ==\033[0m  verify-only: one binary, zero services\n'

if [ ! -x "$UNIDPP" ]; then
    printf '  \033[33m[SKIP]\033[0m unidpp binary unavailable at %s\n' "$UNIDPP"
    exit 77
fi

rm -rf "$WORK"
mkdir -p "$WORK"

# --- steps 1-2: a specimen to verify (the CI stand-in for "a pack
# arrives at the border"; a real adopter starts at step 3) ----------

"$UNIDPP" create \
    --id 'sgtin:4006381333931+21+QS001' \
    --type 'https://example.org/types/battery-pack' \
    --capability S1 \
    --out "$WORK/passport.json" >/dev/null 2>&1 \
    && ok "passport skeleton minted locally (specimen)" \
    || bad "create failed"

"$UNIDPP" event \
    --passport "$WORK/passport.json" \
    --type 'custody.transfer' \
    --data '{"from":"mfg","to":"border","counterparty_signed":true}' \
    --key border-specimen-seed >/dev/null 2>&1 \
    && ok "one lifecycle event appended" \
    || bad "event failed"

"$UNIDPP" pack \
    --passport "$WORK/passport.json" \
    --key border-specimen-pack-seed \
    --out "$WORK/pack.hex" >"$WORK/pack.stdout" 2>"$WORK/pack.stderr" \
    && ok "Tier-A pack minted (specimen)" \
    || bad "pack failed"

# The pack mint prints the derived public key on stderr — the anchor
# a verifier pins. This is the trust-list entry in miniature.
ANCHOR="$(grep -oE '[0-9a-f]{130}' "$WORK/pack.stderr" | head -1 || true)"
[ -n "$ANCHOR" ] && ok "anchor derived from the mint (${ANCHOR:0:16}…)" \
                 || bad "no anchor on the mint's stderr"

# --- step 3: the adoption path proper — offline verification -------

"$UNIDPP" verify "$WORK/pack.hex" --anchor "$ANCHOR" --max-age 0 \
    >"$WORK/verify.stdout" 2>&1
code=$?
[ "$code" -eq 0 ] && ok "verdict: PASS, offline, no services" \
                 || { bad "expected PASS, exit $code"; sed -n '1,8p' "$WORK/verify.stdout"; }

grep -qi 'pass' "$WORK/verify.stdout" \
    && ok "the verdict names its grade" \
    || bad "verdict text missing"

# --- step 4: the terminal catches tampering on its own -------------

python3 - "$WORK/pack.hex" <<'PY'
import sys
p = sys.argv[1]
text = open(p).read().strip()
# Flip a byte in the middle of the pack body (well away from the
# signature slots at the end) — the damage a relabeller does.
mid = len(text) // 2
flip = '0' if text[mid] != '0' else '1'
open(p, 'w').write(text[:mid] + flip + text[mid + 1:])
PY

"$UNIDPP" verify "$WORK/pack.hex" --anchor "$ANCHOR" --max-age 0 \
    >"$WORK/verify.tampered.stdout" 2>&1
code=$?
[ "$code" -eq 2 ] && ok "tampered pack: FAIL (exit 2), caught with zero services" \
                 || bad "tampered pack returned exit $code, expected 2"

# --- step 5: no anchor, honestly degraded --------------------------

"$UNIDPP" verify "$WORK/pack.hex" --max-age 0 >"$WORK/verify.noanchor.stdout" 2>&1
code=$?
[ "$code" -eq 1 ] || [ "$code" -eq 2 ] \
    && ok "without an anchor the verdict degrades (exit $code), never silently passes" \
    || bad "no-anchor verify returned exit $code"

printf '\n  summary: %d passed, %d failed\n' "$pass" "$fail"
[ "$fail" = 0 ]
