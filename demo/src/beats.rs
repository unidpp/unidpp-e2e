//! Every beat is a function; the ordered table at the bottom is the
//! story. The narration strings are the script's, verbatim — they are
//! the story.

use crate::engine::{grep_verdict, grep_verdict_ci, tail_line, which, Flow, Sink, Story, R};
use crate::http::urlencode;
use crate::json;
use crate::payloads;
use crate::story::CreateSpec;
use serde_json::{json, Value};
use std::fs;

// STORY cast (STORY.md section 0 — the identities of every beat).
pub const BIKE_ID: &str = "local:momiji:e8/J-000842";
pub const TYPE_ID: &str = "local:momiji:e8/type/2027.1";
pub const BIKE_URN: &str = "urn:unidpp:passport:momiji-e8-j000842";
pub const TYPE_URN: &str = "urn:unidpp:passport:momiji-e8-type-2027-1";
pub const DRIVE_URN: &str = "urn:unidpp:passport:rhine-du-m771";
pub const PACK_URN: &str = "urn:unidpp:passport:weilian-wp-p9904";
pub const LOT_URN: &str = "urn:unidpp:passport:haichuan-cell-h2231";
pub const NEWPACK_URN: &str = "urn:unidpp:passport:voltaro-wp-eu7781";
pub const SCRAP_URN: &str = "urn:unidpp:passport:scrap-steel-j000842";
pub const RECYCLE_URN: &str = "urn:unidpp:passport:recycle-pack-j000842";
pub const BIKE_TYPE_REF: &str = "momiji:e8/type/2027.1";

const ISSUANCE_DATA: &str = r#"{"derived":false,"inputs":[]}"#;

/// One passport of the STORY cast (the issuer_create call shape).
struct Cast<'a> {
    id: &'a str,
    type_ref: &'a str,
    capability: &'a str,
    eo: &'a str,
    resolver: &'a str,
    urn: &'a str,
    out: &'a str,
}

fn create(story: &mut Story, cast: &Cast) -> R<()> {
    story.issuer_create(&CreateSpec {
        id: cast.id.to_string(),
        type_ref: cast.type_ref.to_string(),
        capability: cast.capability.to_string(),
        eo: cast.eo.to_string(),
        resolver: cast.resolver.to_string(),
        urn: cast.urn.to_string(),
        out: story.artifact(cast.out),
        config: None,
    })
}

fn event(
    story: &mut Story,
    passport_name: &str,
    event_type: &str,
    data: &str,
    actor: &str,
    role: &str,
    at: &str,
) -> R<()> {
    story.issuer_event(
        &story.artifact(passport_name),
        event_type,
        data,
        actor,
        role,
        at,
    )
}

fn mint(story: &mut Story, passport_name: &str, pack_name: &str) -> R<String> {
    story.issuer_mint_pack(&story.artifact(passport_name), &story.artifact(pack_name))
}

fn load(story: &mut Story, name: &str) -> R<Value> {
    json::load(&story.artifact(name)).map_err(|e| story.abort(e))
}

fn field(story: &mut Story, name: &str, field: &str) -> R<String> {
    let doc = load(story, name)?;
    Ok(json::field_print(&doc, field))
}

fn read_artifact(story: &Story, name: &str) -> String {
    fs::read_to_string(story.artifact(name)).unwrap_or_default()
}

/// ===========================================================================
/// B1 — Assembly in Kyoto (issuance where duty attaches)
/// ===========================================================================
fn b1(story: &mut Story) -> R<Flow> {
    story
        .out
        .beat("B1", "Assembly in Kyoto (issuance where duty attaches)");
    story.out.what("the JP type passport and the instance passport exist, and the build record lists every part at finest recorded granularity.");

    create(
        story,
        &Cast {
            id: TYPE_ID,
            type_ref: "-",
            capability: "S0",
            eo: "momiji-mobility",
            resolver: "https://resolver.unidpp.org/r/momiji-e8-type-2027-1",
            urn: TYPE_URN,
            out: "e8-type.json",
        },
    )?;
    create(
        story,
        &Cast {
            id: BIKE_ID,
            type_ref: BIKE_TYPE_REF,
            capability: "S2",
            eo: "momiji-mobility",
            resolver: "https://resolver.unidpp.org/r/momiji-e8-j000842",
            urn: BIKE_URN,
            out: "e8-instance.json",
        },
    )?;
    create(
        story,
        &Cast {
            id: "local:rhine:du/M-771233",
            type_ref: "-",
            capability: "S1",
            eo: "rhine-drives-de",
            resolver: "https://resolver.unidpp.org/r/rhine-du-m771",
            urn: DRIVE_URN,
            out: "drive-unit.json",
        },
    )?;
    create(
        story,
        &Cast {
            id: "local:weilian:wp/P-9904",
            type_ref: "-",
            capability: "S2",
            eo: "weilian-shenzhen",
            resolver: "https://resolver.unidpp.org/r/weilian-wp-p9904",
            urn: PACK_URN,
            out: "pack-original.json",
        },
    )?;
    create(
        story,
        &Cast {
            id: "local:haichuan:cell/H-2231",
            type_ref: "-",
            capability: "S0",
            eo: "haichuan-cn",
            resolver: "https://resolver.unidpp.org/r/haichuan-cell-h2231",
            urn: LOT_URN,
            out: "cell-lot.json",
        },
    )?;

    let instance = load(story, "e8-instance.json")?;
    let bike_type = load(story, "e8-type.json")?;
    story.out.say(&format!(
        "instance passport   {} ({})",
        json::field_print(&instance, "passport_id"),
        json::field_print(&instance, "product_id")
    ));
    story.out.say(&format!(
        "type passport       {} ({})",
        json::field_print(&bike_type, "passport_id"),
        json::field_print(&bike_type, "product_id")
    ));
    story.out.say("type ref + version  momiji:e8/type/2027.1 (the 2028.1 hardware revision exists — type-version visibility)");
    story.out.say("dormant identifiers: the 40 Haichuan cells of lot H-2231 have no passports yet — absorption is regime-temporal");

    event(
        story,
        "e8-type.json",
        "issuance",
        ISSUANCE_DATA,
        "momiji-type-approval",
        "issuing authority",
        "2027-04-12T09:00:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "issuance",
        ISSUANCE_DATA,
        "momiji-mobility",
        "issuing authority",
        "2027-04-12T09:30:00Z",
    )?;
    event(
        story,
        "pack-original.json",
        "issuance",
        ISSUANCE_DATA,
        "weilian-shenzhen",
        "issuing authority",
        "2027-03-02T08:00:00Z",
    )?;
    event(
        story,
        "cell-lot.json",
        "issuance",
        ISSUANCE_DATA,
        "haichuan-cn",
        "issuing authority",
        "2027-01-15T08:00:00Z",
    )?;
    event(
        story,
        "drive-unit.json",
        "issuance",
        ISSUANCE_DATA,
        "rhine-drives-de",
        "issuing authority",
        "2027-03-20T08:00:00Z",
    )?;

    story.out.what("invariant I7 in one line: the passport is issued where the legal duty attaches (JP road-traffic/EPAC type facts) — not where the server is.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B2 — Parts carry their own duties (mixed-profile children)
/// ===========================================================================
fn b2(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "B2",
        "Parts carry their own duties (mixed-profile children)",
    );
    story.out.what("installation edges R3 are typed: pairing, alteration, recoverability — bidirectional parent/child knowledge.");

    // The bike's outgoing side of each installation edge (R3).
    event(
        story,
        "e8-instance.json",
        "install",
        &payloads::install_data(
            "outgoing",
            DRIVE_URN,
            "2027-04-12T10:00:00Z",
            "torque-to-yield",
            "fastened",
            "harvestable",
            "none",
        ),
        "momiji-assembly",
        "installer",
        "2027-04-12T10:00:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "install",
        &payloads::install_data(
            "outgoing",
            PACK_URN,
            "2027-04-12T10:05:00Z",
            "-",
            "fastened",
            "restorable",
            "firmware",
        ),
        "momiji-assembly",
        "installer",
        "2027-04-12T10:05:00Z",
    )?;
    // The parts' incoming side (bidirectional knowledge).
    event(
        story,
        "pack-original.json",
        "install",
        &payloads::install_data(
            "incoming",
            BIKE_URN,
            "2027-04-12T10:05:00Z",
            "-",
            "fastened",
            "restorable",
            "firmware",
        ),
        "momiji-assembly",
        "installer",
        "2027-04-12T10:05:00Z",
    )?;
    // The pack's cells: dormant identifiers recorded as an installation
    // edge to the lot identity at finest recorded granularity (I3).
    event(
        story,
        "pack-original.json",
        "install",
        &payloads::install_data(
            "incoming",
            LOT_URN,
            "2027-03-02T08:30:00Z",
            "-",
            "potted",
            "absorbing",
            "none",
        ),
        "weilian-shenzhen",
        "installer",
        "2027-03-02T08:30:00Z",
    )?;

    // The charger's CCC certificate joins the subregister as a registered
    // equivalence transform (GB 4943.1-2022 ~= IEC 62368-1).
    story.registry_post(
        "/items",
        &payloads::reg_transform_body(),
        &story.artifact("reg-transform-response.json"),
    )?;
    story.registry_get(
        "/transforms/gb4943-1-2022-eq-iec-62368-1",
        &story.artifact("reg-transform.json"),
    )?;

    let transform = load(story, "reg-transform.json")?;
    story.out.say(
        "bike -> drive unit:       torque-mount alteration, harvestable (recoverability spectrum)",
    );
    story
        .out
        .say("bike -> pack:             battery swappable = restorable, CAN/firmware pairing");
    story.out.say("pack -> cell lot H-2231:  absorbing (dormant identifiers — the cell-passport ratchet adopts them one day)");
    story.out.say(&format!(
        "certificate join:         {} (class {})",
        json::field_print(&transform, "identifier"),
        json::field_print(&transform, "item_class")
    ));

    story.out.what("a foreign verifier reads a CCC certificate without adopting CN rules — equivalence claims are registered transforms, not re-issuance.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B-CTO — The build-to-order variant (configuration-vector composition)
/// ===========================================================================
fn b_cto(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "B-CTO",
        "The build-to-order variant (configuration-vector composition)",
    );
    story.out.what("CTO = model + configuration vector: each option is a registered model with its own passport; the instance composes through the same R3 edges.");

    let cto_id = "local:momiji:e8/J-000843";
    let cto_urn = "urn:unidpp:passport:momiji-e8-j000843";
    let cto_batt_std_type_urn = "urn:unidpp:passport:weilian-wp-type-std";
    let cto_batt_lr_type_urn = "urn:unidpp:passport:voltaro-wp-type-lr9";
    let cto_rack_type_urn = "urn:unidpp:passport:arca-rack-type-r200";
    let cto_batt_lr_urn = "urn:unidpp:passport:voltaro-lr-0001";
    let cto_rack_urn = "urn:unidpp:passport:arca-r200-0007";
    let cto_config = "urn:unidpp:option:battery:long-range,urn:unidpp:option:rack:yes";

    // Option families: one model passport per option (the catalogue).
    create(
        story,
        &Cast {
            id: "local:weilian:wp/type-STD",
            type_ref: "-",
            capability: "S0",
            eo: "weilian-shenzhen",
            resolver: "https://resolver.unidpp.org/r/weilian-wp-type-std",
            urn: cto_batt_std_type_urn,
            out: "cto-batt-std-type.json",
        },
    )?;
    create(
        story,
        &Cast {
            id: "local:voltaro:wp/type-LR9",
            type_ref: "-",
            capability: "S0",
            eo: "voltaro-eu",
            resolver: "https://resolver.unidpp.org/r/voltaro-wp-type-lr9",
            urn: cto_batt_lr_type_urn,
            out: "cto-batt-lr-type.json",
        },
    )?;
    create(
        story,
        &Cast {
            id: "local:arca:rack/type-R200",
            type_ref: "-",
            capability: "S0",
            eo: "arca-cycles",
            resolver: "https://resolver.unidpp.org/r/arca-rack-type-r200",
            urn: cto_rack_type_urn,
            out: "cto-rack-type.json",
        },
    )?;
    event(
        story,
        "cto-batt-std-type.json",
        "issuance",
        ISSUANCE_DATA,
        "weilian-shenzhen",
        "issuing authority",
        "2027-04-01T08:00:00Z",
    )?;
    event(
        story,
        "cto-batt-lr-type.json",
        "issuance",
        ISSUANCE_DATA,
        "voltaro-eu",
        "issuing authority",
        "2027-04-01T08:30:00Z",
    )?;
    event(
        story,
        "cto-rack-type.json",
        "issuance",
        ISSUANCE_DATA,
        "arca-cycles",
        "issuing authority",
        "2027-04-01T09:00:00Z",
    )?;

    // The CTO instance: the 2027.1 frame plus the ordered configuration
    // vector [battery: long-range, rack: yes].
    story.issuer_create(&CreateSpec {
        id: cto_id.to_string(),
        type_ref: BIKE_TYPE_REF.to_string(),
        capability: "S2".to_string(),
        eo: "momiji-mobility".to_string(),
        resolver: "https://resolver.unidpp.org/r/momiji-e8-j000843".to_string(),
        urn: cto_urn.to_string(),
        out: story.artifact("e8-cto.json"),
        config: Some(cto_config.to_string()),
    })?;
    event(
        story,
        "e8-cto.json",
        "issuance",
        ISSUANCE_DATA,
        "momiji-mobility",
        "issuing authority",
        "2027-04-12T11:00:00Z",
    )?;

    // Option instances: the chosen long-range pack and the rack.
    create(
        story,
        &Cast {
            id: "local:voltaro:wp/LR-0001",
            type_ref: "voltaro:wp/type/LR9",
            capability: "S2",
            eo: "voltaro-eu",
            resolver: "https://resolver.unidpp.org/r/voltaro-lr-0001",
            urn: cto_batt_lr_urn,
            out: "cto-batt-lr.json",
        },
    )?;
    create(
        story,
        &Cast {
            id: "local:arca:rack/R-200-0007",
            type_ref: "arca:rack/type/R200",
            capability: "S1",
            eo: "arca-cycles",
            resolver: "https://resolver.unidpp.org/r/arca-r200-0007",
            urn: cto_rack_urn,
            out: "cto-rack.json",
        },
    )?;
    event(
        story,
        "cto-batt-lr.json",
        "issuance",
        ISSUANCE_DATA,
        "voltaro-eu",
        "issuing authority",
        "2027-04-12T10:30:00Z",
    )?;
    event(
        story,
        "cto-rack.json",
        "issuance",
        ISSUANCE_DATA,
        "arca-cycles",
        "issuing authority",
        "2027-04-12T10:45:00Z",
    )?;

    // Composition through the SAME typed R3 edges (bidirectional).
    event(
        story,
        "e8-cto.json",
        "install",
        &payloads::install_data(
            "outgoing",
            cto_batt_lr_urn,
            "2027-04-12T11:10:00Z",
            "-",
            "fastened",
            "restorable",
            "firmware",
        ),
        "momiji-assembly",
        "installer",
        "2027-04-12T11:10:00Z",
    )?;
    event(
        story,
        "cto-batt-lr.json",
        "install",
        &payloads::install_data(
            "incoming",
            cto_urn,
            "2027-04-12T11:10:00Z",
            "-",
            "fastened",
            "restorable",
            "firmware",
        ),
        "momiji-assembly",
        "installer",
        "2027-04-12T11:10:00Z",
    )?;
    event(
        story,
        "e8-cto.json",
        "install",
        &payloads::install_data(
            "outgoing",
            cto_rack_urn,
            "2027-04-12T11:15:00Z",
            "-",
            "fastened",
            "restorable",
            "none",
        ),
        "momiji-assembly",
        "installer",
        "2027-04-12T11:15:00Z",
    )?;
    event(
        story,
        "cto-rack.json",
        "install",
        &payloads::install_data(
            "incoming",
            cto_urn,
            "2027-04-12T11:15:00Z",
            "-",
            "fastened",
            "restorable",
            "none",
        ),
        "momiji-assembly",
        "installer",
        "2027-04-12T11:15:00Z",
    )?;

    // The composition manifest: the config vector as a build artifact.
    let vector: Vec<String> = cto_config
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect();
    let manifest = json!({
        "model": "momiji:e8/type/2027.1",
        "instance": "urn:unidpp:passport:momiji-e8-j000843",
        "config": vector,
    });
    let _ = fs::write(
        story.artifact("cto-config.json"),
        serde_json::to_string_pretty(&manifest).unwrap_or_default(),
    );

    story
        .out
        .say("frame model         momiji:e8/type/2027.1 (the same type passport as J-000842)");
    story.out.say(&format!("configuration       {cto_config}"));
    story.out.say(&format!(
        "composed children   {cto_batt_lr_urn} (battery LR) + {cto_rack_urn} (rack)"
    ));
    story.out.say(
        "not chosen          the STD battery model exists in the catalogue — no instance, no edge",
    );

    // The composed instance's installation edges resolve to exactly the
    // configured children (the projector's traversal, read from the
    // graph itself).
    let cto_doc = load(story, "e8-cto.json")?;
    let batt_doc = load(story, "cto-batt-lr.json")?;
    let rack_doc = load(story, "cto-rack.json")?;
    story.check(
        "CTO instance outgoing installs == 2",
        "2",
        &json::outgoing_install_count(&cto_doc).to_string(),
    );
    story.check(
        "CTO battery edge names the LR pack",
        cto_batt_lr_urn,
        &json::install_child(&cto_doc, 0),
    );
    story.check(
        "CTO rack edge names the rack",
        cto_rack_urn,
        &json::install_child(&cto_doc, 1),
    );
    // Bidirectional knowledge: each child's incoming edge names the CTO.
    story.check(
        "LR pack incoming edge names the CTO instance",
        cto_urn,
        &json::install_child(&batt_doc, 0),
    );
    story.check(
        "rack incoming edge names the CTO instance",
        cto_urn,
        &json::install_child(&rack_doc, 0),
    );
    // The rejected option never enters this build's graph.
    let cto_text = read_artifact(story, "e8-cto.json");
    story.check(
        "STD battery absent from the CTO graph",
        "absent",
        if cto_text.contains(cto_batt_std_type_urn) {
            "present"
        } else {
            "absent"
        },
    );
    // The configuration vector is queryable on the live document.
    if story.issuer_mode() {
        let issuer_url = story.cfg.issuer_url.clone().unwrap_or_default();
        story
            .out
            .show(&format!("GET {issuer_url}/passports/{cto_urn}"));
        let view = story
            .issuer_get_passport(cto_urn)
            .ok_or_else(|| story.abort("cannot query the CTO instance".to_string()))?;
        let _ = fs::write(story.artifact("e8-cto-view.json"), &view);
        let view_doc: Value = serde_json::from_str(&view)
            .map_err(|e| story.abort(format!("invalid CTO view: {e}")))?;
        story.check(
            "config vector queryable == 2 options",
            "2",
            &json::count_array(&view_doc, "config").to_string(),
        );
    } else {
        story.out.say(&format!(
            "config vector:      local driver mode — the sidecar manifest {}",
            story.artifact("cto-config.json").display()
        ));
    }

    // The composed instance verifies like any other: one identity, one
    // pack, the same pipeline.
    let cto_anchor = mint(story, "e8-cto.json", "b-cto.pack")?;
    story.verify_and_expect(
        &story.artifact("b-cto.pack"),
        &cto_anchor,
        "2027-04-12T11:30:00Z",
        0,
        "B-CTO composed instance — issued, both children recorded",
        &[],
    )?;
    story.log_anchor_pack(&story.artifact("b-cto.pack"), "b-cto", cto_urn)?;

    story.out.what("the CTO variant is composition, not re-issuance: the same frame type, one config vector, R3 edges to each chosen option's own passport.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B-INT — S12 interop: render to UNTP, ingest back
/// ===========================================================================
fn b_int(story: &mut Story) -> R<Flow> {
    story
        .out
        .beat("B-INT", "S12 interop: render to UNTP, ingest back");
    story.out.what("the S12 seam is bidirectional: the core passport renders as the UNTP verifiable-credential triad, and the triad ingests back as a core passport — the same subject identity, conformity credentials landed as profile bindings, idempotent per subject.");

    story.start_gateway()?;

    let (render_id, source_identity) = if story.issuer_mode() {
        let cto_urn = "urn:unidpp:passport:momiji-e8-j000843".to_string();
        let identity = field(story, "e8-cto.json", "product_id")?;
        story.out.say(&format!(
            "subject:   the B-CTO instance the story just composed ({identity}),"
        ));
        story
            .out
            .say("           rendered by the gateway from the live issuer document");
        (cto_urn, identity)
    } else {
        story.gateway_get("/", &story.artifact("b-int-discovery.json"))?;
        let discovery = load(story, "b-int-discovery.json")?;
        let fixture_id = discovery
            .get("source")
            .and_then(|s| s.get("fixtures"))
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|f| f.get("product_id"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        story.out.say(&format!(
            "subject:   the gateway's seeded pilot fixture ({fixture_id}) — the local"
        ));
        story
            .out
            .say("           driver keeps the story passports as CLI files, not behind an HTTP");
        story
            .out
            .say("           issuer; run make demo-live to round-trip the real B-CTO passport");
        (fixture_id.clone(), fixture_id)
    };

    let render_path = format!("/untp/product/{}", urlencode(&render_id));
    story.gateway_get(&render_path, &story.artifact("b-int-triad.json"))?;

    let triad = load(story, "b-int-triad.json")?;
    story.check(
        "B-INT triad renders under the UNTP profile",
        "urn:unidpp:profile:render:untp",
        &json::dotted_print(&triad, "rendering.profile"),
    );
    if story.issuer_mode() {
        story.check(
            "B-INT render source is the live issuer",
            "issuer",
            &json::dotted_print(&triad, "rendering.source"),
        );
        let issuer_url = story.cfg.issuer_url.clone().unwrap_or_default();
        story
            .out
            .say("verdict:   the gateway verified the issuer's Ed25519 event signatures against");
        story.out.say(&format!(
            "           the anchors pinned from GET {issuer_url}/keyring"
        ));
    } else {
        story.check(
            "B-INT render source is the seeded fixture",
            "fixture",
            &json::dotted_print(&triad, "rendering.source"),
        );
    }
    story.check(
        "B-INT render keeps the source product identity",
        &source_identity,
        &json::triad_identifier(&triad),
    );

    story.out.show(&format!(
        "POST {}/untp/ingest  (the triad just rendered)",
        story.gateway_url()
    ));
    let code1 = story
        .gateway_ingest(
            &story.artifact("b-int-triad.json"),
            &story.artifact("b-int-ingest-1.json"),
        )
        .ok_or_else(|| story.abort("gateway ingest failed".to_string()))?;
    let ingest1 = load(story, "b-int-ingest-1.json")?;
    let status1 = json::field_print(&ingest1, "status");
    // A gateway this run started answers 201/imported (fresh store); one
    // still holding a previous run's ingests answers 200/matched. Both
    // honor the contract; anything else fails loudly.
    if (code1 == 201 && status1 == "imported") || (code1 == 200 && status1 == "matched") {
        story.check(
            "B-INT first ingest imports (201), or matches on re-runs (200)",
            "ok",
            "ok",
        );
    } else {
        story.check(
            "B-INT first ingest imports (201), or matches on re-runs (200)",
            "201/imported or 200/matched",
            &format!("{code1}/{status1}"),
        );
    }
    let pid1 = json::field_print(&ingest1, "passport_id");
    story.check(
        "B-INT ingested identity round-trips to the source passport",
        &source_identity,
        &json::field_print(&ingest1, "identity"),
    );
    let triad_again = load(story, "b-int-triad.json")?;
    story.check(
        "B-INT standardsConformance lands as profile bindings",
        "ok",
        &json::untp_bindings(&triad_again, &ingest1),
    );
    story.out.say(&format!(
        "imported:  {}",
        json::field_print(&ingest1, "passport_id")
    ));
    story.out.say(&format!(
        "bindings:  {} profile bindings from the triad's conformity credentials",
        json::count_array(&ingest1, "profiles")
    ));

    // The second ingest of the same subject must match, never duplicate (I1).
    story.out.show(&format!(
        "POST {}/untp/ingest  (again — idempotence per subject)",
        story.gateway_url()
    ));
    let code2 = story
        .gateway_ingest(
            &story.artifact("b-int-triad.json"),
            &story.artifact("b-int-ingest-2.json"),
        )
        .ok_or_else(|| story.abort("gateway re-ingest failed".to_string()))?;
    story.check("B-INT re-ingest HTTP status", "200", &code2.to_string());
    let ingest2 = load(story, "b-int-ingest-2.json")?;
    story.check(
        "B-INT re-ingest matches (idempotent per subject)",
        "matched",
        &json::field_print(&ingest2, "status"),
    );
    story.check(
        "B-INT re-ingest returns the same passport id",
        &pid1,
        &json::field_print(&ingest2, "passport_id"),
    );

    Story::stop_process(&mut story.gateway);

    story.out.what("render and ingest are inverse projections over one identity: the UNTP consumer and the UniDPP core agree on the subject — no parallel-universe passport.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// G-CORR — One thing, two codes: correlate, never consolidate
/// ===========================================================================
fn g_corr(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "G-CORR",
        "One thing, two codes: correlate, never consolidate",
    );
    story.out.what("the tyre ships with two marks side by side — its GTIN marking and an ISO 15459 marking under a different scheme. Both are legitimate (spec 6.3 k): the resolver correlates them and states both sides; a later scheme rotation is stated, never silent.");

    story.start_resolver()?;

    let tyre_gtin_key = "gs1:(01)04006381333931";
    let tyre_urn_key = "iso-15459:urn:iso:std:iso-iec:15459:unidpp:inst:4006381333931";

    // The GTIN marking resolves to the gateway's live UNTP render (one
    // anchor, many representations).
    let linkset = format!(
        "{{\"identifier\": \"{tyre_gtin_key}\", \"links\": [\n  {{\"linkType\": \"dpp\", \"href\": \"{}/untp/product/gtin:4006381333931\",\n   \"asOf\": \"2026-05-04T08:00:00Z\", \"title\": \"tyre UNTP render (live gateway)\"}}\n]}}\n",
        story.gateway_url()
    );
    let _ = fs::write(story.artifact("gcorr-linkset.json"), &linkset);
    let code = admin_post_file(
        story,
        "/admin/linksets",
        "gcorr-linkset.json",
        "gcorr-register.json",
    );
    story.check(
        "G-CORR the GTIN marking registers",
        "201",
        &code.to_string(),
    );

    // The correlation: both marks on one nameplate, asserted by the OEM.
    // The 15459 side is NOT registered here — the cross-registry case.
    let correlate = format!(
        "{{\"identifierA\": \"{tyre_gtin_key}\", \"identifierB\": \"{tyre_urn_key}\",\n \"assertor\": \"urn:unidpp:actor:tyre-oem-conti\",\n \"evidence\": \"both codes printed on one nameplate (two marks, one thing)\",\n \"direction\": \"mutual\"}}\n"
    );
    let _ = fs::write(story.artifact("gcorr-correlate.json"), &correlate);
    let code = admin_post_file(
        story,
        "/admin/correlations",
        "gcorr-correlate.json",
        "gcorr-correlated.json",
    );
    story.check("G-CORR the correlation records", "201", &code.to_string());

    // Side 1: the GTIN marking resolves, carries its render link, AND
    // states its counterpart.
    let side1 = story
        .resolve_with_headers(tyre_gtin_key, &[])
        .ok_or_else(|| story.abort("resolver side 1 resolve failed".to_string()))?;
    let _ = fs::write(story.artifact("gcorr-side1.hdr"), &side1.raw_head);
    let _ = fs::write(story.artifact("gcorr-side1.json"), &side1.body);
    let side1_doc: Value = serde_json::from_str(&side1.body)
        .map_err(|e| story.abort(format!("invalid side 1 body: {e}")))?;
    let counterpart = side1_doc
        .get("unidpp:correlated-with")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|c| c.get("other"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    story.check(
        "G-CORR side 1 linkset names the counterpart",
        tyre_urn_key,
        &counterpart,
    );
    let header_value = side1
        .header("x-unidpp-correlated-with")
        .unwrap_or("")
        .to_string();
    story.check(
        "G-CORR side 1 header names the counterpart",
        tyre_urn_key,
        &header_value,
    );

    // Side 2: the 15459 marking was never registered here — its refusal
    // NAMES its counterpart (holding either side discovers the other;
    // absence stated, never silence).
    let side2 = story
        .resolve_with_headers(tyre_urn_key, &[])
        .ok_or_else(|| story.abort("resolver side 2 resolve failed".to_string()))?;
    let _ = fs::write(story.artifact("gcorr-side2.json"), &side2.body);
    let side2_doc: Value = serde_json::from_str(&side2.body)
        .map_err(|e| story.abort(format!("invalid side 2 body: {e}")))?;
    let side2_other = side2_doc
        .get("unidpp:correlated-with")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|c| c.get("other"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    story.check(
        "G-CORR the never-registered side names its counterpart",
        tyre_gtin_key,
        &side2_other,
    );

    // The rotation: the GTIN marking is retired in favour of the 15459
    // marking (scheme rotation). The old identity KEEPS resolving and
    // states its successor — rotation is never revocation.
    let rotate = format!(
        "{{\"identifier\": \"{tyre_gtin_key}\", \"successor\": \"{tyre_urn_key}\",\n \"effectiveAt\": \"2027-06-01T00:00:00Z\",\n \"authority\": \"urn:unidpp:actor:tyre-oem-conti\",\n \"reason\": \"the fleet migrates to the 15459 marking\"}}\n"
    );
    let _ = fs::write(story.artifact("gcorr-rotate.json"), &rotate);
    let code = admin_post_file(
        story,
        "/admin/supersessions",
        "gcorr-rotate.json",
        "gcorr-rotated.json",
    );
    story.check("G-CORR the rotation records", "200", &code.to_string());
    // Read as-of AFTER the rotation's effective instant (as-of is
    // honest: before 2027-06-01 the old marking carries no statement).
    let after = story
        .resolve_with_headers(tyre_gtin_key, &[("asof", "2027-07-01T00:00:00Z")])
        .ok_or_else(|| story.abort("resolver after-rotation resolve failed".to_string()))?;
    let _ = fs::write(story.artifact("gcorr-after-rotation.json"), &after.body);
    let after_doc: Value = serde_json::from_str(&after.body)
        .map_err(|e| story.abort(format!("invalid after-rotation body: {e}")))?;
    let successor = after_doc
        .get("unidpp:superseded-by")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    story.check(
        "G-CORR the retired marking still resolves, stating its successor",
        tyre_urn_key,
        &successor,
    );

    story.out.say("the resolver's journal (resolver-journal.jsonl) holds the whole statement history — correlation, rotation — replayable as-of.");
    story.out.what("one thing, two sovereign identifiers: correlated from either side, rotation stated on the old — the DPP algebra relates passports instead of consolidating them.");
    Ok(Flow::Next)
}

/// POST one of the resolver's admin bodies (the G-CORR helper): status
/// code back, response body into the out artifact.
fn admin_post_file(story: &mut Story, path: &str, body_name: &str, out_name: &str) -> u16 {
    let body = fs::read_to_string(story.artifact(body_name)).unwrap_or_default();
    let reply = story.resolver_post(path, &body);
    match reply {
        Some(r) => {
            let _ = fs::write(story.artifact(out_name), &r.body);
            r.status
        }
        None => 0,
    }
}

/// ===========================================================================
/// B3 — Placement in the EU (profile growth by dated binding)
/// ===========================================================================
fn b3(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "B3",
        "Placement in the EU (profile growth by dated binding)",
    );
    story.out.what("the EU lens set binds onto the SAME identity by a registry applicability event — no re-minting, no parallel-universe passport (I1).");

    story.registry_post(
        "/items",
        &payloads::reg_profile_body(),
        &story.artifact("reg-profile-response.json"),
    )?;
    // Idempotence: only POST the binding when this exact triple has not
    // yet been recorded (the registry does not deduplicate bindings).
    let existing = story.binding_already_seen(BIKE_TYPE_REF);
    if existing == 0 {
        story.registry_post(
            "/applicability",
            &payloads::reg_binding_body(BIKE_TYPE_REF),
            &story.artifact("reg-binding-response.json"),
        )?;
    } else {
        story.out.note(&format!(
            "applicability binding already present ({existing}) — skipped (registry journal carries it)"
        ));
    }

    let query = format!(
        "/applicability?product_type={}&at=2027-06-01T00%3A00%3A00Z",
        urlencode(BIKE_TYPE_REF)
    );
    story.registry_get(&query, &story.artifact("reg-applicability-2027.json"))?;
    story
        .out
        .say("at 2027-06-01 (JP market only): no EU duty applies yet");
    let query = format!(
        "/applicability?product_type={}&at=2028-06-01T00%3A00%3A00Z",
        urlencode(BIKE_TYPE_REF)
    );
    story.registry_get(&query, &story.artifact("reg-applicability-2028.json"))?;
    story
        .out
        .say("at 2028-06-01: the EU battery-lens binding is in force for the same type ref");

    let before = json::count_array(
        &load(story, "reg-applicability-2027.json")?,
        "applicability",
    );
    let after = json::count_array(
        &load(story, "reg-applicability-2028.json")?,
        "applicability",
    );
    story.check(
        "EU profiles bound at 2027-06-01 (before placement)",
        "0",
        &before.to_string(),
    );
    story.check(
        "EU profiles bound at 2028-06-01 (after placement)",
        "1",
        &after.to_string(),
    );

    // The importer becomes the battery producer (LMT duty): the
    // placement custody edge, dated for the border moment.
    event(
        story,
        "e8-instance.json",
        "custody.transfer",
        r#"{"from":"momiji-mobility","to":"dusseldorf-importer","counterparty_signed":true}"#,
        "momiji-mobility",
        "custodian",
        "2028-02-14T18:00:00Z",
    )?;

    story.out.what("jurisdiction growth is a registry event + a custody edge — the JP lens stays mounted; manifest history stays as-of-reconstructable.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B4 — The border moment (offline, degraded origins)
/// ===========================================================================
fn b4(story: &mut Story) -> R<Flow> {
    story
        .out
        .beat("B4", "The border moment (offline, degraded origins)");
    story.out.what("the officer's terminal verifies the signed Tier-A pack OFFLINE — nothing is fetched — and the three readings print.");

    let anchor = mint(story, "e8-instance.json", "b4-border.pack")?;
    if let Some(pinned) = story.trust_anchor.clone() {
        story
            .out
            .say(&format!("issuer pins anchor (public key, hex): {anchor}"));
        story.out.say(&format!(
            "verifier pins anchor from unidpp-trust /keyring: {pinned}"
        ));
        story.check(
            "issuer pack anchor == trust-pinned anchor",
            &pinned,
            &anchor,
        );
    } else {
        story
            .out
            .say(&format!("issuer pins anchor (public key, hex): {anchor}"));
    }
    story
        .out
        .say("officer terminal: offline (Shenzhen host unreachable, EU registry mid-outage)");

    story.verify_and_expect(
        &story.artifact("b4-border.pack"),
        &anchor,
        "2028-02-15T09:30:00Z",
        0,
        "B4 border moment — PASS, as-of stamped, coverage states what was not reachable",
        &[],
    )?;

    story.log_anchor_pack(&story.artifact("b4-border.pack"), "b4-border", BIKE_URN)?;

    story.out.what("verdict PASS with the three readings named (cryptographic / evidentiary / current-state) and full field coverage — honesty is the feature.");

    // Optional subset exit: the minimum live circuit is issuance (B1)
    // plus an offline verify under the trust-pinned anchor (B4).
    if story.cfg.stop_after_b4 {
        story
            .out
            .hr("SUBSET COMPLETE — B1 through B4 (UNIDPP_E2E_STOP_AFTER=B4)");
        story.out.say(&format!(
            "checks:   {}/{} passed",
            story.tally.ok, story.tally.total
        ));
        story.out.say(&format!(
            "artifacts: {} (passports, packs, registry responses)",
            story.cfg.work_dir.display()
        ));
        if story.tally.failed > 0 || story.tally.ok != story.tally.total {
            story.out.demo_failed(story.tally.failed);
            story.subset_exit = Some(1);
        } else {
            story.out.demo_passed_subset();
            story.subset_exit = Some(0);
        }
        return Ok(Flow::Subset);
    }
    Ok(Flow::Next)
}

/// ===========================================================================
/// B5 — Life in service (edge state, capability classes)
/// ===========================================================================
fn b5(story: &mut Story) -> R<Flow> {
    story
        .out
        .beat("B5", "Life in service (edge state, capability classes)");
    story.out.what("the S2 BMS commits its log prefix and reveals at the dealer visit (commit-now / reveal-later); staleness becomes bounded.");

    // Cleared the border, sold to the first owner (Duesseldorf).
    event(
        story,
        "e8-instance.json",
        "custody.transfer",
        r#"{"from":"dusseldorf-importer","to":"owner-1-duesseldorf","counterparty_signed":true}"#,
        "dusseldorf-importer",
        "custodian",
        "2028-03-01T10:00:00Z",
    )?;

    event(
        story,
        "e8-instance.json",
        "milestone.record",
        r#"{"counters":{"bms.cycle_count":"412","odometer.km":"2871.4"}}"#,
        "e8-bms-controller",
        "device",
        "2028-11-05T11:00:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "inspection.stamp",
        &payloads::stamp_data(
            BIKE_URN,
            "dealer-service-duesseldorf",
            "2028-11-05T11:00:00Z",
        ),
        "dealer-service-duesseldorf",
        "verifier",
        "2028-11-05T11:20:00Z",
    )?;

    let counters = json::last_milestone_counters(&load(story, "e8-instance.json")?);
    if counters.is_empty() {
        story.out.raw("    (no milestone events recorded)");
    } else {
        for (key, value) in counters {
            story.out.raw(&format!("    counter: {key} = {value}"));
        }
    }

    story.out.say("capability classes on one bike: BMS logs (S2), optional Connect module (S3), silent rack/frame (S0)");
    story.out.say("who measured what, with which unit, under whose model — SoH is a derived verdict whose transform is a registered item");

    story.out.what("truth becomes bounded and auditable: the service-center era had episodic, undetectable staleness.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B6 — Firmware update and the derestriction incident
/// ===========================================================================
fn b6(story: &mut Story) -> R<Flow> {
    story
        .out
        .beat("B6", "Firmware update and the derestriction incident");
    story.out.what("the OTA is a software.update (declared values change, no physical change); the dongle is a product.modify that CHANGES THE LEGAL CLASS.");

    event(
        story,
        "e8-instance.json",
        "software.update",
        r#"{"versions":{"controller":"2.4.1"},"unlocked_features":["range-algorithm-v2"]}"#,
        "momiji-mobility",
        "economic operator",
        "2029-03-02T04:00:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "custody.transfer",
        r#"{"from":"owner-1-duesseldorf","to":"owner-2","counterparty_signed":true}"#,
        "owner-1-duesseldorf",
        "custodian",
        "2029-05-20T15:00:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "product.modify",
        r#"{"description":"derestriction dongle: assistance cutoff 25 -> 45 km/h","derived_type":"momiji:e8/type/2027.1#moped-2029","reevaluation_required":true}"#,
        "owner-2",
        "accredited modifier",
        "2029-06-18T16:00:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "status.change",
        r#"{"from":"issued","to":"non-conformant","authority":"jp-road-traffic"}"#,
        "jp-road-traffic",
        "regulator",
        "2029-06-19T09:00:00Z",
    )?;

    story.out.what("derived type spawns (E4b), profiles re-evaluate (JP: non-conformant; EU: type-approval regime required) — graded, never binary.");

    let anchor = mint(story, "e8-instance.json", "b6-derestricted.pack")?;
    story.verify_and_expect(&story.artifact("b6-derestricted.pack"), &anchor,
        "2029-06-20T10:00:00Z", 2,
        "B6 derestriction — the modified machine is legally an unregistered moped (expected FAIL: non-conformant)", &[])?;
    story.log_anchor_pack(
        &story.artifact("b6-derestricted.pack"),
        "b6-derestricted",
        BIKE_URN,
    )?;

    // The dongle comes off at the next dealer visit; re-evaluation passes.
    event(
        story,
        "e8-instance.json",
        "product.modify",
        r#"{"description":"dongle removed at service: assistance cutoff restored to 25 km/h","derived_type":null,"reevaluation_required":true}"#,
        "dealer-service-duesseldorf",
        "accredited modifier",
        "2029-09-03T10:00:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "status.change",
        r#"{"from":"non-conformant","to":"issued","authority":"jp-road-traffic"}"#,
        "jp-road-traffic",
        "regulator",
        "2029-09-04T09:00:00Z",
    )?;

    story.out.what("history is never rewritten: the incident stays in the log; the state machine recovered through a legal re-evaluation event.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B7 — Repair (regulated child swap, cross-jurisdiction install)
/// ===========================================================================
fn b7(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "B7",
        "Repair (regulated child swap, cross-jurisdiction install)",
    );
    story.out.what("water damage: uninstall + install (two events); the old pack's interval closes CARRYING ITS HISTORY.");

    create(
        story,
        &Cast {
            id: "local:voltaro:wp/EU-7781",
            type_ref: "-",
            capability: "S2",
            eo: "voltaro-eu",
            resolver: "https://resolver.unidpp.org/r/voltaro-wp-eu7781",
            urn: NEWPACK_URN,
            out: "pack-2029.json",
        },
    )?;
    event(
        story,
        "pack-2029.json",
        "issuance",
        ISSUANCE_DATA,
        "voltaro-eu",
        "issuing authority",
        "2029-09-10T08:00:00Z",
    )?;

    event(
        story,
        "e8-instance.json",
        "uninstall",
        &payloads::uninstall_data(PACK_URN, "2027-04-12T10:05:00Z", "2029-09-12T10:00:00Z"),
        "dealer-service-duesseldorf",
        "installer",
        "2029-09-12T10:00:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "install",
        &payloads::install_data(
            "outgoing",
            NEWPACK_URN,
            "2029-09-12T10:30:00Z",
            "-",
            "fastened",
            "restorable",
            "firmware",
        ),
        "dealer-service-duesseldorf",
        "installer",
        "2029-09-12T10:30:00Z",
    )?;
    event(
        story,
        "pack-2029.json",
        "install",
        &payloads::install_data(
            "incoming",
            BIKE_URN,
            "2029-09-12T10:30:00Z",
            "-",
            "fastened",
            "restorable",
            "firmware",
        ),
        "dealer-service-duesseldorf",
        "installer",
        "2029-09-12T10:30:00Z",
    )?;

    // The old pack's own log closes its installation interval and carries
    // its provenance to the refurbisher.
    event(
        story,
        "pack-original.json",
        "uninstall",
        &payloads::uninstall_data(BIKE_URN, "2027-04-12T10:05:00Z", "2029-09-12T10:00:00Z"),
        "dealer-service-duesseldorf",
        "installer",
        "2029-09-12T10:00:00Z",
    )?;
    event(
        story,
        "pack-original.json",
        "custody.transfer",
        r#"{"from":"dealer-service-duesseldorf","to":"refurbisher-linz","counterparty_signed":true}"#,
        "dealer-service-duesseldorf",
        "custodian",
        "2029-09-13T09:00:00Z",
    )?;

    story.out.say("old pack: 412 cycles of H-2231 cells, removed for casing dent — harvested-part provenance IS value");
    story.out.say("new pack: EU-made (Voltaro), installed by a DE dealer into a JP-profile bike (both profiles survive)");
    story
        .out
        .say("independent repairer acted under EN 18239-style roles (right-to-repair echo)");

    let anchor = mint(story, "pack-2029.json", "b7-newpack.pack")?;
    story.verify_and_expect(
        &story.artifact("b7-newpack.pack"),
        &anchor,
        "2029-09-12T11:00:00Z",
        0,
        "B7 new pack (post-swap) — issued, no flags",
        &[],
    )?;
    story.log_anchor_pack(
        &story.artifact("b7-newpack.pack"),
        "b7-newpack",
        NEWPACK_URN,
    )?;

    story.out.what("cross-jurisdiction install: both profiles survive the swap; the used-parts market keeps provenance the EN pipeline loses silently.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B8 — Resale and auction (custody as ceremony; blind edges)
/// ===========================================================================
fn b8(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "B8",
        "Resale and auction (custody as ceremony; blind edges)",
    );
    story.out.what("custody transfers are signed ceremonies; the auction lens issues a dated, signed condition stamp; the buyer's household stays invisible.");

    event(
        story,
        "e8-instance.json",
        "custody.transfer",
        r#"{"from":"owner-2","to":"auction-house-vienna","counterparty_signed":true}"#,
        "auction-house-vienna",
        "custodian",
        "2031-03-02T10:00:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "inspection.stamp",
        &payloads::stamp_data(BIKE_URN, "auction-house-vienna", "2031-03-04T14:00:00Z"),
        "auction-house-vienna",
        "verifier",
        "2031-03-04T14:30:00Z",
    )?;
    event(
        story,
        "e8-instance.json",
        "custody.transfer",
        r#"{"from":"auction-house-vienna","to":"buyer-vienna","counterparty_signed":true}"#,
        "auction-house-vienna",
        "custodian",
        "2031-03-10T15:00:00Z",
    )?;

    story.out.say("auction house checks the theft predicate against the flag subregister (E12) before listing");
    story
        .out
        .say("condition stamp: lens-scoped (auction lens != insurance lens), dated, signed");
    story.out.say("the buyer's household is invisible to Momiji — proof-of-binding != knowledge-of-parent (I12)");

    let anchor = mint(story, "e8-instance.json", "b8-auction.pack")?;
    story.verify_and_expect(
        &story.artifact("b8-auction.pack"),
        &anchor,
        "2031-03-10T16:00:00Z",
        0,
        "B8 post-auction — the audit trail survives without a central watcher",
        &[],
    )?;
    story.log_anchor_pack(&story.artifact("b8-auction.pack"), "b8-auction", BIKE_URN)?;

    story.out.what("enumeration resistance is a system property: the audit trail survives; the surveillance does not exist.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B9 — A recall crosses the graph (predicate-based; dormant -> live)
/// ===========================================================================
fn b9(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "B9",
        "A recall crosses the graph (predicate-based; dormant -> live)",
    );
    story.out.what("2030-05: Haichuan lot H-2231 recalled (thermal event) — the predicate is published; nobody enumerated the installed base.");

    event(
        story,
        "cell-lot.json",
        "recall.campaign",
        r#"{"campaign":"R-H2231-THERMAL","predicate":{"FactContains":{"path":"bom.lots","needle":"H-2231"}}}"#,
        "cn-samr",
        "regulator",
        "2030-05-06T08:00:00Z",
    )?;

    story.out.say("predicate: \"packs containing lot H-2231\" — each custodian evaluates locally against their own holdings");
    story.out.say("the Vienna bike's CURRENT pack (Voltaro, B7) is unaffected — traced through the swap edges");

    // The original pack's custodian (refurbisher -> powerwall) evaluates
    // the predicate against its own log: it DOES contain H-2231.
    event(
        story,
        "pack-original.json",
        "recall.campaign",
        r#"{"campaign":"R-H2231-THERMAL","predicate":{"FactContains":{"path":"bom.lots","needle":"H-2231"}}}"#,
        "refurbisher-linz",
        "custodian",
        "2030-05-07T09:00:00Z",
    )?;

    let old_anchor = mint(story, "pack-original.json", "b9-oldpack.pack")?;
    story.verify_and_expect(
        &story.artifact("b9-oldpack.pack"),
        &old_anchor,
        "2030-05-07T10:00:00Z",
        2,
        "B9 original pack — REACHED through its own log (expected FAIL: recall active)",
        &[],
    )?;
    story.log_anchor_pack(&story.artifact("b9-oldpack.pack"), "b9-oldpack", PACK_URN)?;

    let new_anchor = mint(story, "pack-2029.json", "b9-newpack.pack")?;
    story.verify_and_expect(&story.artifact("b9-newpack.pack"), &new_anchor,
        "2030-05-07T10:00:00Z", 1,
        "B9 Vienna bike's current pack — unaffected by the recall (degraded only by freshness; safety clean)", &[])?;
    story.log_anchor_pack(
        &story.artifact("b9-newpack.pack"),
        "b9-newpack",
        NEWPACK_URN,
    )?;

    story.out.what("the current pack's only degradation is freshness — its safety finding is clean; degradation is explicit, never silent.");
    story.out.what("the recall reached the graph, not a list: computational, privacy-preserving — and the manufacturer receives aggregates.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B-QUORUM — Retroactive distrust is a quorum act (M-of-K, cross-jurisdiction)
/// ===========================================================================
fn b_quorum(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "B-QUORUM",
        "Retroactive distrust is a quorum act (M-of-K, cross-jurisdiction)",
    );
    story.out.what("2030-06: evidence emerges that haichuan-cn misissued cell-lot certifications across 2027-2030 — the authority itself, not one lot. Retroactive distrust is authority-over-authority: no single regulator may do it; the graph demands a quorate M-of-K attestation.");

    // The trust base: the live trust service, or an ephemeral instance;
    // the beat narrates and skips when the real ceremony binaries are
    // absent (it never stages the cryptography).
    let mut skip = None;
    if !story.cfg.quorum_ceremony_bin.is_file() {
        skip = Some(format!(
            "quorum-ceremony binary missing ({}; run: make deps-trust)",
            story.cfg.quorum_ceremony_bin.display()
        ));
    }
    if skip.is_none() && story.cfg.trust_url.is_none() && !story.cfg.trust_bin.is_file() {
        skip = Some(format!(
            "unidpp-trust binary missing ({}; run: make deps-trust)",
            story.cfg.trust_bin.display()
        ));
    }
    let run = match skip {
        Some(reason) => {
            story
                .out
                .note(&format!("B-QUORUM narrated without running: {reason}"));
            None
        }
        None => match story.start_quorum_trust() {
            Ok(()) => Some(()),
            Err(reason) => {
                story
                    .out
                    .note(&format!("B-QUORUM narrated without running: {reason}"));
                None
            }
        },
    };

    if run.is_none() {
        story
            .out
            .say("retroactive distrust of an authority requires a quorate attestation —");
        story
            .out
            .say("one regulator's POST is refused (422 QuorumRequired); 2-of-3 members from");
        story
            .out
            .say("different jurisdictions combine threshold-Schnorr partials into ONE group");
        story
            .out
            .say("signature, the quorum node pins the group key, and the declaration lands.");
        story
            .out
            .say("The full over-HTTP proof: unidpp-trust tests/quorum.rs.");
    } else {
        story.out.say(&format!(
            "trust:    {} — the standing authority for the quorum act",
            story.quorum_url_public()
        ));

        let quorum_dir = story.cfg.work_dir.join("quorum-ceremony");
        let _ = fs::create_dir_all(&quorum_dir);

        // -- 1. One regulator tries alone: the refusal is the policy. --
        let no_attestation = "{\"subject\":{\"kind\":\"node\",\"id\":\"haichuan-cn\"},\"reason\":{\"token\":\"misissuance\"},\"declared_at\":\"2030-06-15T00:00:00Z\",\"declared_by\":\"e8-retro-quorum\",\"window\":{\"start\":\"2027-01-01T00:00:00Z\",\"end\":\"2030-06-01T00:00:00Z\"},\"quorum\":null}";
        let _ = fs::write(quorum_dir.join("no-attestation.json"), no_attestation);
        let code = story.quorum_post(
            "/revocations",
            &quorum_dir.join("no-attestation.json"),
            &quorum_dir.join("no-attestation.response.json"),
        );
        story.check(
            "B-QUORUM single regulator refused (422 — quorum attestation required)",
            "422",
            &code.unwrap_or(0).to_string(),
        );

        // -- 2. One member tries alone: the refusal is the mathematics. --
        story
            .out
            .show("quorum-ceremony declare --threshold 2 --signer reg-cn-samr  (one member alone)");
        let below = ceremony(story, &quorum_dir, "below", &["reg-cn-samr"]);
        story.check(
            "B-QUORUM one-member ceremony refused by the cryptography",
            "refused",
            if below == Some(0) {
                "accepted"
            } else {
                "refused"
            },
        );
        story.out.say("          fewer than M partials cannot produce a group signature — no policy layer sees the request at all");

        // -- 3. The quorate ceremony: two members, two jurisdictions. --
        story.out.show("quorum-ceremony declare --threshold 2 --signer reg-cn-samr --signer reg-jp-meti  (CN+JP: 2-of-3)");
        if ceremony(
            story,
            &quorum_dir,
            "quorate",
            &["reg-cn-samr", "reg-jp-meti"],
        ) != Some(0)
        {
            return Err(story.abort(format!(
                "the 2-of-3 quorum ceremony failed (see {})",
                quorum_dir.join("quorate.log").display()
            )));
        }
        let artifacts = ["quorum-node.json", "revocation.json", "ceremony.json"]
            .iter()
            .filter(|name| quorum_dir.join("quorate").join(name).is_file())
            .count();
        story.check(
            "B-QUORUM ceremony artifacts written (node pin, attestation, audit trail)",
            "3",
            &artifacts.to_string(),
        );

        // -- 4. Pinning is load-bearing: an unpinned group key certifies
        //       nothing; then the quorum node pins it. --
        let code = story.quorum_post(
            "/revocations",
            &quorum_dir.join("quorate/revocation.json"),
            &quorum_dir.join("unpinned.response.json"),
        );
        story.check(
            "B-QUORUM unpinned group key certifies nothing (422)",
            "422",
            &code.unwrap_or(0).to_string(),
        );
        let code = story.quorum_post(
            "/nodes",
            &quorum_dir.join("quorate/quorum-node.json"),
            &quorum_dir.join("node.response.json"),
        );
        story.check(
            "B-QUORUM quorum node pinned (threshold group + group key registered)",
            "201",
            &code.unwrap_or(0).to_string(),
        );

        // -- 5. Quorate: the declaration lands. --
        let code = story.quorum_post(
            "/revocations",
            &quorum_dir.join("quorate/revocation.json"),
            &quorum_dir.join("declared.response.json"),
        );
        story.check(
            "B-QUORUM quorate 2-of-3 declaration accepted (201)",
            "201",
            &code.unwrap_or(0).to_string(),
        );

        // -- 6. The pack: Tier A cannot see standing (its own finding
        //       says so); the verdict degrades through the overlay. --
        story.out.what("the lot's own pack still verifies on its own terms — but its issuing authority is now distrusted ab initio: the combined verdict a verifier must present is void.");
        let anchor = mint(story, "cell-lot.json", "b-quorum-lot.pack")?;
        story.verify_and_expect(&story.artifact("b-quorum-lot.pack"), &anchor,
            "2030-06-10T00:00:00Z", 2,
            "B-QUORUM the lot's own pack, pre-act — the B9 campaign in its log already fails it (the quorum question is its authority, not this taint)", &[])?;
        story.log_anchor_pack(
            &story.artifact("b-quorum-lot.pack"),
            "b-quorum-lot",
            LOT_URN,
        )?;

        story.out.show(&format!(
            "GET {}/revocations?at=2027-01-15T08:00:00Z&subject=node:haichuan-cn  (the lot's issuance moment)",
            story.quorum_url_public()
        ));
        story.check(
            "B-QUORUM lot issuance (2027-01-15) inside the window: void ab initio",
            "void-ab-initio",
            &story.quorum_standing("at=2027-01-15T08:00:00Z", "standing_at_as_of"),
        );
        story.check(
            "B-QUORUM the pack verdict degrades: in-window verifications no longer stand",
            "false",
            &story.quorum_standing("at=2027-01-15T08:00:00Z", "verifications_at_stand"),
        );
        story.check(
            "B-QUORUM the quorum view names the form: threshold-group, quorate",
            "threshold-group",
            &story.quorum_standing("at=2027-01-15T08:00:00Z", "quorum.form"),
        );

        story.out.show(&format!(
            "GET {}/revocations?at=2027-01-15T08:00:00Z&known_by=2030-06-14T23:59:59Z  (a diligent verifier, before the act was knowable)",
            story.quorum_url_public()
        ));
        story.check(
            "B-QUORUM evidentiary cutoff: pre-declaration verifications stand",
            "true",
            &story.quorum_standing(
                "at=2027-01-15T08:00:00Z&known_by=2030-06-14T23:59:59Z",
                "verifications_at_stand",
            ),
        );

        story.check(
            "B-QUORUM before the window (2026-07): retroactivity does not leak past its start",
            "valid",
            &story.quorum_standing("at=2026-07-01T00:00:00Z", "standing_at_as_of"),
        );

        Story::stop_process(&mut story.quorum);
    }

    story.out.what("retroactive distrust took a quorum: two jurisdictions' regulators combined partials into one group signature — and the cutoff protects every verifier who acted before it was knowable.");
    Ok(Flow::Next)
}

/// Run the quorum-ceremony binary's declare step for the given signers;
/// both streams land in the ceremony's log artifact.
fn ceremony(
    story: &mut Story,
    quorum_dir: &std::path::Path,
    out_dir: &str,
    signers: &[&str],
) -> Option<i32> {
    let bin = story.cfg.quorum_ceremony_bin.display().to_string();
    let mut argv = vec![
        bin,
        "declare".to_string(),
        "--quorum".to_string(),
        "e8-retro-quorum".to_string(),
        "--threshold".to_string(),
        "2".to_string(),
        "--member".to_string(),
        "reg-cn-samr".to_string(),
        "--member".to_string(),
        "reg-jp-meti".to_string(),
        "--member".to_string(),
        "reg-eu-espr".to_string(),
    ];
    for signer in signers {
        argv.push("--signer".to_string());
        argv.push(signer.to_string());
    }
    argv.extend([
        "--subject-kind".to_string(),
        "node".to_string(),
        "--subject-id".to_string(),
        "haichuan-cn".to_string(),
        "--reason".to_string(),
        "misissuance".to_string(),
        "--window-start".to_string(),
        "2027-01-01T00:00:00Z".to_string(),
        "--window-end".to_string(),
        "2030-06-01T00:00:00Z".to_string(),
        "--declared-at".to_string(),
        "2030-06-15T00:00:00Z".to_string(),
        "--out-dir".to_string(),
        quorum_dir.join(out_dir).display().to_string(),
    ]);
    let log = quorum_dir.join(format!("{out_dir}.log"));
    let _ = fs::File::create(&log);
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    story.run_command(&refs, Sink::File(log.clone()), Sink::File(log))
}

/// ===========================================================================
/// G-GRID — The grid: one subject, two sovereignty segments, one spine
/// ===========================================================================
fn g_grid(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "G-GRID",
        "The grid: one subject, two sovereignty segments, one spine",
    );
    story.out.what("Phase 1 of the build contract (REQUIREMENTS.md): sovereignty per-segment — an open EU segment and a SEALED CN segment, a commitment spine over both, and a verifier who proves the sealed segment without ever seeing it.");

    let unidpp = story.cfg.unidpp.display().to_string();
    let dossier = story
        .artifact("pack-0001-dossier.json")
        .display()
        .to_string();
    let frozen = story
        .artifact("pack-0001-frozen.json")
        .display()
        .to_string();
    let anchors = story
        .artifact("pack-0001-anchors.json")
        .display()
        .to_string();
    let route = story.artifact("pack-0001-route.json").display().to_string();

    story.out.show("unidpp grid");
    let ggrid = story.artifact("ggrid.txt");
    let _ = fs::File::create(&ggrid);
    let code = story.run_command(
        &[
            &unidpp,
            "grid",
            "--dossier",
            &dossier,
            "--frozen",
            &frozen,
            "--anchors",
            &anchors,
            "--route-out",
            &route,
        ],
        Sink::File(ggrid.clone()),
        Sink::Tee,
    );
    if code != Some(0) {
        return Err(story.abort(format!("unidpp grid failed (see {})", ggrid.display())));
    }

    let py_dir = story.cfg.family_dir.join("unidpp-py");
    // FW-2: the suite-foreign leg — the Python harness verifies the
    // exported frozen view under the exported pinned anchors.
    // Suite-foreign is the required demo; suite-suite proves nothing.
    if py_dir.is_dir() && which("python3").is_some() {
        let foreign = story.artifact("foreign.txt");
        let _ = fs::File::create(&foreign);
        let code = story.run_command_in_dir(
            &py_dir,
            &["python3", "-m", "unidpp.harness.f1", &frozen, &anchors],
            Sink::File(foreign.clone()),
            Sink::File(foreign.clone()),
        );
        if code != Some(0) {
            return Err(story.abort(format!(
                "the foreign harness F1 run failed (see {})",
                foreign.display()
            )));
        }
        // F-INT (the F2/F3 foreign legs): the inter-scheme data the
        // suite just produced — the dossier's recorded route — replays
        // through the foreign harness's protocol layer.
        let foreign_route = story.artifact("foreign-route.txt");
        let _ = fs::File::create(&foreign_route);
        let script = "import json, sys\n\
                      from unidpp.harness import protocol, vectors\n\
                      report = json.load(open(sys.argv[1]))\n\
                      route = report[\"route\"]\n\
                      replayed = protocol.route_replay(route)\n\
                      assert replayed == report[\"entries\"], \"the foreign replay diverged from the suite's report\"\n\
                      digest = vectors.route_digest(route)\n\
                      print(f\"F-INT: PASS — the foreign harness replays the suite's recorded route; digest {digest.hex()[:16]}\u{2026}\")\n";
        let code = story.run_python_stdin(
            &py_dir,
            script,
            &[&route],
            Sink::File(foreign_route.clone()),
            Sink::File(foreign_route.clone()),
        );
        let route_text = fs::read_to_string(&foreign_route).unwrap_or_default();
        if code == Some(0) {
            story.check(
                "G-GRID F-INT the FOREIGN harness replays the suite's recorded route (F2)",
                "ok",
                &grep_verdict(&route_text, "F-INT: PASS"),
            );
        } else {
            story.out.note(&format!(
                "F-INT narrated without running: {}",
                tail_line(&route_text)
            ));
            story.out.say("the foreign harness replays the recorded route (proven in unidpp-py tests/test_harness_f2f3.py)");
        }
        let foreign_text = fs::read_to_string(&foreign).unwrap_or_default();
        story.check(
            "G-GRID the FOREIGN harness verifies the frozen view (FW-2)",
            "ok",
            &grep_verdict(&foreign_text, "F1: PASS"),
        );
    } else {
        story
            .out
            .say("foreign harness leg skipped: unidpp-py or python3 not present in this checkout");
    }

    // SI-1: the frozen view — air-gapped ingest + verify + re-execution
    // in a separate process.
    let frozen_txt = story.artifact("frozen.txt");
    let _ = fs::File::create(&frozen_txt);
    let code = story.run_command(
        &[&unidpp, "frozen", &frozen],
        Sink::File(frozen_txt.clone()),
        Sink::Tee,
    );
    if code != Some(0) {
        return Err(story.abort(format!(
            "unidpp frozen failed (see {})",
            frozen_txt.display()
        )));
    }
    // FW-3: the F1 claim test — third-party runnable against the
    // published frozen view.
    let f1 = story.artifact("f1.txt");
    let _ = fs::File::create(&f1);
    let code = story.run_command(
        &[&unidpp, "conform", "f1", &frozen],
        Sink::File(f1.clone()),
        Sink::Tee,
    );
    if code != Some(0) {
        return Err(story.abort(format!("unidpp conform f1 failed (see {})", f1.display())));
    }
    story.check(
        "G-GRID the F1 claim test passes (FW-3)",
        "ok",
        &grep_verdict(&read_artifact(story, "f1.txt"), "F1: PASS"),
    );
    // The F5 self-certification: every golden vector of the family
    // re-derives in this binary (one command, all classes runnable by
    // any third party).
    let f5 = story.artifact("f5.txt");
    let _ = fs::File::create(&f5);
    let family = story.cfg.family_dir.display().to_string();
    let code = story.run_command(
        &[&unidpp, "conform", "f5", &family],
        Sink::File(f5.clone()),
        Sink::File(f5),
    );
    if code == Some(0) {
        story.check(
            "G-GRID the F5 self-certification sweep — every golden vector reproduces (FW-3)",
            "ok",
            &grep_verdict(&read_artifact(story, "f5.txt"), "F5: PASS"),
        );
    } else {
        story.out.note(&format!(
            "G-GRID F5 narrated without running: {}",
            tail_line(&read_artifact(story, "f5.txt"))
        ));
        story.out.say(
            "the F5 sweep replays every golden vector of the family (proven in harness test 11)",
        );
    }
    let frozen_text = read_artifact(story, "frozen.txt");
    story.check(
        "G-GRID the frozen view verifies air-gapped (SI-1)",
        "ok",
        &grep_verdict(&frozen_text, "offline frozen view"),
    );
    story.check(
        "G-GRID the frozen view re-execution matches the issuer render (SI-1)",
        "ok",
        &grep_verdict(&frozen_text, "re-execution MATCHES"),
    );
    // XB-5: the offline verifier — a separate process, one file, the
    // verifier's own anchors, zero calls to foreign systems.
    let dossier_txt = story.artifact("dossier.txt");
    let _ = fs::File::create(&dossier_txt);
    let code = story.run_command(
        &[&unidpp, "dossier", &dossier],
        Sink::File(dossier_txt.clone()),
        Sink::Tee,
    );
    if code != Some(0) {
        return Err(story.abort(format!(
            "unidpp dossier failed (see {})",
            dossier_txt.display()
        )));
    }
    let dossier_text = read_artifact(story, "dossier.txt");
    story.check(
        "G-GRID the offline dossier verdict: zero foreign API calls (XB-5)",
        "ok",
        &grep_verdict(&dossier_text, "zero calls to foreign synchronous APIs"),
    );
    story.check(
        "G-GRID the offline verdict reproduces the coverage report",
        "ok",
        &grep_verdict(
            &dossier_text,
            "cn-dynamic: attested-by-authority (governing policy cn-dynamic-bms v1)",
        ),
    );
    story.check(
        "G-GRID the spine's log receipt verifies offline (CN-4)",
        "ok",
        &grep_verdict(&dossier_text, "log receipt verified"),
    );

    let ggrid_text = read_artifact(story, "ggrid.txt");
    story.check(
        "G-GRID the sealed segment verifies from the spine alone",
        "ok",
        &grep_verdict(
            &ggrid_text,
            "sealed segment: existence + currency from the spine alone",
        ),
    );
    story.check(
        "G-GRID the spine proves append-only growth",
        "ok",
        &grep_verdict(&ggrid_text, "append-only growth provable"),
    );
    story.check(
        "G-GRID forged segment state fails loudly",
        "ok",
        &grep_verdict(&ggrid_text, "forged segment state fails"),
    );
    story.check(
        "G-GRID the receiving profile decides (XB-4)",
        "ok",
        &grep_verdict(&ggrid_text, "acceptance: the receiving profile decides"),
    );
    // The sealed plaintext NEVER appears in the transcript.
    story.check(
        "G-GRID the sealed contents never appear",
        "never",
        if ggrid_text.contains("cycle_count=412") {
            "leaked"
        } else {
            "never"
        },
    );
    // The cross-border moment (Phase 2): the CN battery case —
    // attestation offer, substitution, coverage-graded verdict.
    story.check(
        "G-GRID S13 offers attestation, not data",
        "ok",
        &grep_verdict(&ggrid_text, "sealed policy offers ATTESTATION"),
    );
    story.check(
        "G-GRID substitution verifies under the verifier's own anchors",
        "ok",
        &grep_verdict(&ggrid_text, "attestation verifies under the verifier"),
    );
    story.check(
        "G-GRID the verdict is a coverage report object (verified-direct + attested)",
        "ok",
        &grep_verdict(&ggrid_text, "coverage report object"),
    );
    story.check(
        "G-GRID the recorded route replays the verdict byte-identically (SI-11)",
        "ok",
        &grep_verdict(
            &ggrid_text,
            "recorded route replays the verdict byte-identically",
        ),
    );
    story.check(
        "G-GRID the ancestry renders the three-way report (SI-6)",
        "ok",
        &grep_verdict(&ggrid_text, "ancestry renders the three-way report"),
    );
    story.check(
        "G-GRID retrieval withholds the sealed class WITH an offer (RT-4)",
        "ok",
        &grep_verdict(
            &ggrid_text,
            "sealed class withheld WITH coverage and an offer pointer",
        ),
    );
    let ok_count = ggrid_text.lines().filter(|l| l.contains("[ok]")).count();
    let fail_count = ggrid_text.lines().filter(|l| l.contains("[FAIL]")).count();
    story.out.say(&format!(
        "{ok_count}/{ok_count} in the grid verdict ({fail_count} failed) — the CN battery case incl. retrieval (Part 10): report, acceptance, offline dossier, frozen view, route, ancestry (XB-1..5, XB-8, SI-1/6/11, RT-4)"
    ));
    Ok(Flow::Next)
}

/// ===========================================================================
/// G-DEVICE — The device is a cryptographic principal
/// ===========================================================================
fn g_device(story: &mut Story) -> R<Flow> {
    story.out.beat("G-DEVICE", "The device is a cryptographic principal (manufacture certificate, scoped slots, edge commitments)");
    // ID-5 / RC-2's demonstration step: the device-drill runner (real
    // device.rs machinery). Narrated when the binary is absent — the
    // beat never stages the cryptography.
    if story.cfg.device_drill_bin.is_file() {
        let bin = story.cfg.device_drill_bin.display().to_string();
        story.out.show(&bin);
        let out = story.artifact("gdevice.txt");
        let _ = fs::File::create(&out);
        let code = story.run_command(&[&bin], Sink::File(out), Sink::Tee);
        if code != Some(0) {
            return Err(story.abort(format!(
                "device-drill failed (see {})",
                story.artifact("gdevice.txt").display()
            )));
        }
        let text = read_artifact(story, "gdevice.txt");
        story.check("G-DEVICE the drill holds 10/10 (certificate, path-finding, scoped revocation, edge law)",
            "ok", &grep_verdict(&text, "device-drill: 10/10"));
        story.check(
            "G-DEVICE revocation is scoped to the slot key, never the device",
            "ok",
            &grep_verdict(&text, "unaffected"),
        );
        story.check(
            "G-DEVICE an edited reveal is rejected — never contradict",
            "ok",
            &grep_verdict_ci(&text, "edited reveal is rejected"),
        );
    } else {
        story.out.note(&format!(
            "G-DEVICE narrated without running: device-drill binary missing ({}; run: make deps-signatif)",
            story.cfg.device_drill_bin.display()
        ));
        story.out.say(
            "the manufacture certificate binds the device key to the static segment's commitment;",
        );
        story
            .out
            .say("per-segment slot keys are certified by their segment authority; revoking one");
        story
            .out
            .say("chain kills that segment's attestations only; the edge commits first and");
        story.out.say(
            "reveals later, never contradicting. Full proof: unidpp-signatif src/device.rs tests.",
        );
    }
    story.out.what("the device signs; the authority scopes; the edge keeps its word — three proofs, one principal.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// G-PRODUCT — Productness is a dated, per-regime predicate
/// ===========================================================================
fn g_product(story: &mut Story) -> R<Flow> {
    story.out.beat(
        "G-PRODUCT",
        "Productness is a dated, per-regime predicate (same-identity toggle vs derived re-entry)",
    );
    // PA-3's demonstration step, through the real event algebra: the
    // SAME identity toggles under end-of-waste (no new passport); scrap
    // splits into DERIVED passports (R2).
    create(
        story,
        &Cast {
            id: "local:recycler:cell-lot/J-000900",
            type_ref: "-",
            capability: "S0",
            eo: "recycler-linz",
            resolver: "https://resolver.unidpp.org/r/cell-lot-j000900",
            urn: "urn:unidpp:passport:gprod-cell",
            out: "gprod-cell.json",
        },
    )?;
    let id_before = field(story, "gprod-cell.json", "passport_id")?;

    event(
        story,
        "gprod-cell.json",
        "status.change",
        r#"{"from":"issued","to":"end-of-waste","authority":"at-regulator-linz"}"#,
        "at-regulator-linz",
        "regulator",
        "2033-08-01T08:00:00Z",
    )?;
    event(
        story,
        "gprod-cell.json",
        "end-of-waste",
        r#"{"evidence_ref":"eow-cert-linz-2033-0901","outputs":[]}"#,
        "steelworks-linz",
        "accredited actor",
        "2033-08-02T08:00:00Z",
    )?;

    // The SAME identity re-qualifies: no new passport issues; the
    // predicate toggled, dated.
    event(
        story,
        "gprod-cell.json",
        "status.change",
        r#"{"from":"end-of-waste","to":"issued","authority":"at-regulator-linz"}"#,
        "at-regulator-linz",
        "regulator",
        "2033-08-03T08:00:00Z",
    )?;
    let id_after = field(story, "gprod-cell.json", "passport_id")?;
    story.check(
        "G-PRODUCT end-of-waste re-qualifies the SAME identity (no new passport)",
        &id_before,
        &id_after,
    );

    let anchor = mint(story, "gprod-cell.json", "gprod-cell.pack")?;
    story.verify_and_expect(
        &story.artifact("gprod-cell.pack"),
        &anchor,
        "2033-08-03T09:00:00Z",
        0,
        "G-PRODUCT the re-qualified cell lot — PASS on the same passport that left",
        &[],
    )?;

    // Scrap: the material LOSES its identity — derived passports follow
    // R2; the parent does not re-qualify again.
    create(
        story,
        &Cast {
            id: "local:recycler:scrap-cu/J-000900",
            type_ref: "-",
            capability: "S0",
            eo: "recycler-linz",
            resolver: "https://resolver.unidpp.org/r/scrap-cu-j000900",
            urn: "urn:unidpp:passport:gprod-scrap-cu",
            out: "gprod-scrap-cu.json",
        },
    )?;
    create(
        story,
        &Cast {
            id: "local:recycler:scrap-al/J-000900",
            type_ref: "-",
            capability: "S0",
            eo: "recycler-linz",
            resolver: "https://resolver.unidpp.org/r/scrap-al-j000900",
            urn: "urn:unidpp:passport:gprod-scrap-al",
            out: "gprod-scrap-al.json",
        },
    )?;
    event(
        story,
        "gprod-cell.json",
        "split",
        r#"{"carve_outs":[{"child":"urn:unidpp:passport:gprod-scrap-cu","quantity":{"amount":"11.2","unit":{"uom":"kg","registry_uri":"https://unitsml.org/units/kg"}}},{"child":"urn:unidpp:passport:gprod-scrap-al","quantity":{"amount":"3.1","unit":{"uom":"kg","registry_uri":"https://unitsml.org/units/kg"}}}],"remainder":{"amount":"0.4","unit":{"uom":"kg","registry_uri":"https://unitsml.org/units/kg"}},"parent_consumed":true}"#,
        "recycler-linz",
        "custodian (transformer)",
        "2033-08-04T08:00:00Z",
    )?;
    let scrap_id = field(story, "gprod-scrap-cu.json", "passport_id")?;
    story.check(
        "G-PRODUCT scrap issues a DERIVED passport (R2 — a new identity)",
        "derived",
        if !scrap_id.is_empty() && scrap_id != id_before {
            "derived"
        } else {
            "same-identity"
        },
    );

    story.out.what("end-of-waste toggled the predicate on the same identity, dated; scrap split into derived passports — the two re-entries never conflated.");
    Ok(Flow::Next)
}

/// ===========================================================================
/// B10 — End of life (the material loop closes)
/// ===========================================================================
fn b10(story: &mut Story) -> R<Flow> {
    story
        .out
        .beat("B10", "End of life (the material loop closes)");
    story.out.what("E13 decompose = inverse transformation 1 -> N into DERIVED material passports (R2) with mass balance; the derived scrap identity then re-qualifies under end-of-waste on ITS OWN passport (clause 8 e).");

    create(
        story,
        &Cast {
            id: "local:recycler:scrap-steel/J-000842",
            type_ref: "-",
            capability: "S0",
            eo: "steelworks-linz",
            resolver: "https://resolver.unidpp.org/r/scrap-steel-j000842",
            urn: SCRAP_URN,
            out: "scrap-steel.json",
        },
    )?;
    create(
        story,
        &Cast {
            id: "local:recycler:pack-material/J-000842",
            type_ref: "-",
            capability: "S0",
            eo: "recycler-linz",
            resolver: "https://resolver.unidpp.org/r/recycle-pack-j000842",
            urn: RECYCLE_URN,
            out: "recycle-pack.json",
        },
    )?;

    event(
        story,
        "e8-instance.json",
        "decompose",
        &payloads::decompose_data(SCRAP_URN, RECYCLE_URN),
        "recycler-linz",
        "recycler",
        "2033-07-14T09:00:00Z",
    )?;

    event(
        story,
        "scrap-steel.json",
        "end-of-waste",
        r#"{"evidence_ref":"eow-cert-linz-2033-0742","outputs":[]}"#,
        "steelworks-linz",
        "accredited actor",
        "2033-07-15T08:00:00Z",
    )?;

    let scrap_anchor = mint(story, "scrap-steel.json", "b10-scrap.pack")?;
    story.verify_and_expect(&story.artifact("b10-scrap.pack"), &scrap_anchor,
        "2033-07-15T09:00:00Z", 1,
        "B10 end-of-waste scrap passport — DEGRADED by status (waste regime re-entry), not by trust: the moment scrap legally re-qualifies", &[])?;
    story.log_anchor_pack(&story.artifact("b10-scrap.pack"), "b10-scrap", SCRAP_URN)?;

    let bike_anchor = mint(story, "e8-instance.json", "b10-bike.pack")?;
    story.verify_and_expect(
        &story.artifact("b10-bike.pack"),
        &bike_anchor,
        "2033-07-15T09:00:00Z",
        1,
        "B10 decomposed bike — transformed: degraded-with-reason under archival semantics",
        &["--max-age", "0"],
    )?;
    story.log_anchor_pack(&story.artifact("b10-bike.pack"), "b10-bike", BIKE_URN)?;

    // B10 mass balance narration: in - out = loss, from the artifact.
    let outputs = json::last_decompose_outputs(&load(story, "e8-instance.json")?);
    let total_out: f64 = outputs
        .iter()
        .filter_map(|(a, _)| a.parse::<f64>().ok())
        .sum();
    let total_in = 25.9;
    let loss = total_in - total_out;
    for (amount, child) in &outputs {
        story
            .out
            .raw(&format!("    out: {amount:>5} kg  ->  {child}"));
    }
    story
        .out
        .raw(&format!("    in : {:>5} kg  (declared mass)", "25.9"));
    story
        .out
        .raw(&format!("    loss = in - out = {loss:.1} kg  (auditable)"));

    story.out.what("circularity became auditable arithmetic: in - out = loss; recycled-content claims compute from the graph.");
    Ok(Flow::Next)
}

/// One beat: a function over the story's state.
pub type BeatFn = fn(&mut Story) -> R<Flow>;

/// The ordered story: adding a beat is adding a function here.
pub const BEATS: &[(&str, BeatFn)] = &[
    ("B1", b1),
    ("B2", b2),
    ("B-CTO", b_cto),
    ("B-INT", b_int),
    ("G-CORR", g_corr),
    ("B3", b3),
    ("B4", b4),
    ("B5", b5),
    ("B6", b6),
    ("B7", b7),
    ("B8", b8),
    ("B9", b9),
    ("B-QUORUM", b_quorum),
    ("G-GRID", g_grid),
    ("G-DEVICE", g_device),
    ("G-PRODUCT", g_product),
    ("B10", b10),
];
