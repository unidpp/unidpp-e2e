#!/usr/bin/env bash
# issuer-hook.sh — the SINGLE, clearly-marked issuer integration point.
#
# TODO.impl/10-remaining-tasks-definitive.md, item `10-issuer-service.md`
# defines the unidpp-issuer contract this file integrates against. The
# contract in tree as of 2026-09-07:
#
#   POST /passports                      {identity, granularity?, capability,
#                                        type_ref|type, eo_id?, resolver_uri?,
#                                        passport_id?, valid_from?, valid_to?,
#                                        config?}
#                                        -> created_view {passport_id, ...}
#   GET  /passports/{id}                 full passport_view (CLI-compatible
#                                        document + config + log_head)
#   POST /passports/{id}/events          {type, data (JSON value), at?, sign?,
#                                        actor?, actor_role?}
#                                        -> {seq, event_type, status, ...}
#   POST /passports/{id}/pack            {budget?, encoding?, sign?}
#                                        -> {pack, anchor, bytes, ec, ...}
#   GET  /passports/{id}/verdict         full-pipeline verdict + coverage
#
# Today the issuer binary is mid-build (the parallel TODO #10 agent is
# still landing wire shapes), so the DEFAULT driver below performs the
# equivalent steps with the unidpp-cli (create | event | pack) against
# local passport JSON files. Activate the service mode explicitly:
#
#   make demo UNIDPP_ISSUER_URL=http://127.0.0.1:8096
#     # auto-starts the issuer if its binary exists, drives everything
#     # through its real HTTP API, and syncs the local mirror file after
#     # each call so the narration helpers (json_get, b5_counters,
#     # mass_balance) keep reading the same document.
#
# Nothing else in the orchestrator knows how passports are created,
# events appended, or packs minted — every beat calls the three verbs
# below (issuer_create / issuer_event / issuer_mint_pack), so swapping
# the CLI for the service is a change to this file only.
#
# Requires from the sourcing orchestrator: run_quiet, note, json_get,
# python3_count, WORK_DIR, UNIDPP, ROOT_DIR.

set -u

ISSUER_URL="${UNIDPP_ISSUER_URL:-}"
ISSUER_BIN="$ROOT_DIR/../unidpp-issuer/target/release/unidpp-issuer"
[ -x "$ISSUER_BIN" ] || ISSUER_BIN="$ROOT_DIR/../unidpp-issuer/target/debug/unidpp-issuer"
ISSUER_PID=""
ISSUER_BIND="${UNIDPP_ISSUER_BIND:-127.0.0.1:8096}"
ISSUER_ADMIN_TOKEN="${UNIDPP_ISSUER_ADMIN_TOKEN:-}"

# Activate the issuer driver ONLY with an explicit UNIDPP_ISSUER_URL.
# Auto-sniffing the binary is racy while TODO #10 is still landing: the
# debug binary can appear under our feet mid-run and flip behavior
# non-deterministically; an explicit URL keeps `make demo`
# deterministic today.
detect_issuer_mode() {
    if [ -n "$ISSUER_URL" ]; then
        printf issuer
        return 0
    fi
    printf cli
}

# Start the issuer if the driver needs one and we are responsible.
maybe_start_issuer() {
    [ "$(detect_issuer_mode)" = issuer ] || return 0
    if curl -sf "$ISSUER_URL/healthz" >/dev/null 2>&1; then
        note "using external issuer at $ISSUER_URL"
        return 0
    fi
    [ -x "$ISSUER_BIN" ] || fail "issuer binary missing: $ISSUER_BIN (UNIDPP_ISSUER_URL was set)"
    note "starting unidpp-issuer on $ISSUER_BIND"
    (
        unset UNIDPP_ISSUER_BIND
        UNIDPP_ISSUER_BIND="$ISSUER_BIND" "$ISSUER_BIN" >/dev/null 2>&1 &
        echo $!
    ) >/tmp/unidpp-issuer.pid.$$
    ISSUER_PID="$(cat /tmp/unidpp-issuer.pid.$$)"
    rm -f /tmp/unidpp-issuer.pid.$$
    issuer_ready=0
    for _ in $(seq 1 50); do
        if curl -sf "$ISSUER_URL/healthz" >/dev/null 2>&1; then
            issuer_ready=1
            break
        fi
        sleep 0.2
    done
    [ "$issuer_ready" = 1 ] || fail "issuer did not become healthy on $ISSUER_URL"
    say "unidpp-issuer healthy at $ISSUER_URL"
}

stop_issuer() {
    if [ -n "$ISSUER_PID" ]; then
        kill "$ISSUER_PID" 2>/dev/null
        wait "$ISSUER_PID" 2>/dev/null
        ISSUER_PID=""
    fi
}

# POST a JSON body to the issuer service. -f returns non-zero on 4xx so
# failures surface here, not as silent 4xx-response bodies. The bearer
# header is only sent when UNIDPP_ISSUER_ADMIN_TOKEN is set (dev mode is
# open).
api_post_json() { # api_post_json <url> <json-body>
    api_post_json_url="$1"
    api_post_json_body="$2"
    api_post_json_curl=(
        curl -sSf -X POST "$api_post_json_url"
        -H 'content-type: application/json'
        --data "$api_post_json_body"
    )
    if [ -n "$ISSUER_ADMIN_TOKEN" ]; then
        api_post_json_curl+=(-H "authorization: Bearer $ISSUER_ADMIN_TOKEN")
    fi
    "${api_post_json_curl[@]}"
}

# GET the full passport view (CLI-compatible document + manifest).
api_get_passport() {
    api_get_passport_curl=(
        curl -sSf "$ISSUER_URL/passports/$1"
    )
    if [ -n "$ISSUER_ADMIN_TOKEN" ]; then
        api_get_passport_curl+=(-H "authorization: Bearer $ISSUER_ADMIN_TOKEN")
    fi
    "${api_get_passport_curl[@]}"
}

# Compose the create-passport JSON body from the orchestrator's CLI
# arguments. The issuer's API uses `identity` (CLI uses --id); we
# always send identity + capability; the rest is optional.
issuer_create_body() {
    python3 - "$@" <<'PYEOF'
import json, sys
identity, type_ref, capability, eo_id, resolver_uri, passport_id = sys.argv[1:]
body = {
    "identity": identity,
    "capability": capability,
}
if type_ref != "-":
    body["type_ref"] = type_ref
if eo_id != "-":
    body["eo_id"] = eo_id
if resolver_uri != "-":
    body["resolver_uri"] = resolver_uri
if passport_id != "-":
    body["passport_id"] = passport_id
print(json.dumps(body))
PYEOF
}

# Compose an append-event JSON body. `data` is a JSON value (not a
# string); we wrap the bare payload in the same externally-tagged form
# the issuer's payload_from_data accepts.
issuer_event_body() {
    python3 - "$@" <<'PYEOF'
import json, sys
type_token, data_json, actor, role, at = (sys.argv[1:] + [""] * 5)[:5]
payload = json.loads(data_json)
event_map = {
    "issuance": "Issuance",
    "custody.transfer": "CustodyTransfer",
    "split": "Split",
    "combine": "Combine",
    "end-of-waste": "EndOfWaste",
    "decompose": "Decompose",
    "install": "Install",
    "uninstall": "Uninstall",
    "part.replace": "PartReplace",
    "consumable.replace": "ConsumableReplace",
    "repair.perform": "RepairPerform",
    "product.modify": "ProductModify",
    "software.update": "SoftwareUpdate",
    "refurbish.remanufacture": "RefurbishRemanufacture",
    "recall.campaign": "RecallCampaign",
    "correction": "Correction",
    "status.change": "StatusChange",
    "flag.security": "FlagSecurity",
    "inspection.stamp": "InspectionStamp",
    "milestone.record": "MilestoneRecord",
}
wrapped = {event_map[type_token]: payload} if type_token in event_map else payload
body = {"type": type_token, "data": wrapped}
if actor:
    body["actor"] = actor
if role:
    body["actor_role"] = role
if at:
    body["at"] = at
print(json.dumps(body))
PYEOF
}

# POST /passports equivalent: mint a passport document.
#
#   issuer_create <id> <type-ref|-> <capability> <eo> <resolver>
#                 <passport-urn> <out-file>
issuer_create() {
    issuer_create_id="$1"
    issuer_create_type="$2"
    issuer_create_cap="$3"
    issuer_create_eo="$4"
    issuer_create_resolver="$5"
    issuer_create_urn="$6"
    issuer_create_out="$7"

    if [ "$(detect_issuer_mode)" = issuer ]; then
        issuer_create_body_json="$(issuer_create_body \
            "$issuer_create_id" "$issuer_create_type" "$issuer_create_cap" \
            "$issuer_create_eo" "$issuer_create_resolver" "$issuer_create_urn")"
        note "issuer service mode: POST /passports $issuer_create_id"
        if ! api_post_json "$ISSUER_URL/passports" "$issuer_create_body_json" \
            >"$issuer_create_out" 2>/dev/null; then
            # 409 (already issued) against a stateful issuer journal:
            # the identity is never re-minted (I1) — re-use the document.
            note "already issued server-side — re-using the existing document"
            api_get_passport "$issuer_create_urn" >"$issuer_create_out" \
                || fail "issuer has no passport $issuer_create_urn and refused the create"
        fi
        # Sync the local mirror so narration helpers (json_get, b5_counters,
        # mass_balance) read the same document.
        api_get_passport "$issuer_create_urn" >"$issuer_create_out.mirror"
        mv "$issuer_create_out.mirror" "$issuer_create_out"
    else
        issuer_create_cmd=("$UNIDPP" create --id "$issuer_create_id"
            --capability "$issuer_create_cap" --eo "$issuer_create_eo"
            --passport-id "$issuer_create_urn" --out "$issuer_create_out")
        [ "$issuer_create_type" != "-" ] &&
            issuer_create_cmd+=(--type "$issuer_create_type")
        [ "$issuer_create_resolver" != "-" ] &&
            issuer_create_cmd+=(--resolver "$issuer_create_resolver")
        run_quiet "${issuer_create_cmd[@]}"
    fi
}

# POST /passports/{id}/events equivalent: append a typed event.
#
#   issuer_event <passport-file> <event-type> <data-json> [actor] [role] [at]
issuer_event() {
    issuer_event_file="$1"
    issuer_event_type="$2"
    issuer_event_data="$3"
    issuer_event_actor="${4:-}"
    issuer_event_role="${5:-}"
    issuer_event_at="${6:-}"

    if [ "$(detect_issuer_mode)" = issuer ]; then
        issuer_event_urn="$(json_get "$issuer_event_file" passport_id)"
        issuer_event_body_json="$(issuer_event_body \
            "$issuer_event_type" "$issuer_event_data" \
            "$issuer_event_actor" "$issuer_event_role" "$issuer_event_at")"
        note "issuer service mode: POST /passports/$issuer_event_urn/events ($issuer_event_type)"
        api_post_json "$ISSUER_URL/passports/$issuer_event_urn/events" \
            "$issuer_event_body_json" >"$WORK_DIR/issuer-event-$$.json"
        # Re-sync the document so the next beat's narration is fresh.
        api_get_passport "$issuer_event_urn" >"$issuer_event_file"
    else
        issuer_event_cmd=("$UNIDPP" event --passport "$issuer_event_file"
            --type "$issuer_event_type" --data "$issuer_event_data")
        [ -n "$issuer_event_actor" ] && issuer_event_cmd+=(--actor "$issuer_event_actor")
        [ -n "$issuer_event_role" ] && issuer_event_cmd+=(--actor-role "$issuer_event_role")
        [ -n "$issuer_event_at" ] && issuer_event_cmd+=(--at "$issuer_event_at")
        run_quiet "${issuer_event_cmd[@]}"
    fi
}

# POST /passports/{id}/pack equivalent: mint the offline Tier-A pack.
# Prints the signing anchor (public key, hex) on stdout; writes the
# encoded pack to <out-file>.
issuer_mint_pack() {
    issuer_pack_file="$1"
    issuer_pack_out="$2"

    if [ "$(detect_issuer_mode)" = issuer ]; then
        issuer_pack_urn="$(json_get "$issuer_pack_file" passport_id)"
        # The note goes to stderr: this function's stdout is the anchor
        # the caller captures.
        note "issuer service mode: POST /passports/$issuer_pack_urn/pack" 1>&2
        api_post_json "$ISSUER_URL/passports/$issuer_pack_urn/pack" '{}' \
            >"$WORK_DIR/issuer-pack-$$.json"
        jq_pack "$WORK_DIR/issuer-pack-$$.json" pack >"$issuer_pack_out"
        jq_pack "$WORK_DIR/issuer-pack-$$.json" anchor
        rm -f "$WORK_DIR/issuer-pack-$$.json"
    else
        issuer_pack_log="$WORK_DIR/pack.stderr.$$"
        issuer_pack_cmd=("$UNIDPP" pack --passport "$issuer_pack_file"
            --key e8-demo-pack-seed --out "$issuer_pack_out")
        show "${issuer_pack_cmd[*]}" 1>&2
        if ! "${issuer_pack_cmd[@]}" >/dev/null 2>"$issuer_pack_log"; then
            fail "pack minting failed for $issuer_pack_file"
        fi
        sed -n 's/.*anchor (public key, hex): //p' "$issuer_pack_log"
        rm -f "$issuer_pack_log"
    fi
}

# Tiny field getter that does not depend on jq.
jq_pack() { # jq_pack <file> <field>
    python3 - "$1" "$2" <<'PYEOF'
import json, sys
with open(sys.argv[1]) as fh:
    doc = json.load(fh)
val = doc[sys.argv[2]]
if isinstance(val, (dict, list)):
    print(json.dumps(val))
else:
    print(val)
PYEOF
}
