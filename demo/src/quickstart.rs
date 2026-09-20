//! The four quickstarts as scenarios of the same engine — one adoption
//! path per function, the scripts' check labels and counts the contract
//! (verify-only 8, publish-only 10, gateway 11, hub 10). Each path keeps
//! its own scratch dir, its own services and its own ports; a failed
//! check never aborts in place, and the closing summary decides the
//! exit code (the scripts' `[ "$fail" = 0 ]`). The SKIP (a missing
//! binary) exits 77, as the scripts do.

use crate::engine::{
    env_or, first_hex_run, head_lines, is_executable, tail_lines, Abort, Config, Out, Sink, Story,
    R,
};
use crate::http::{self, urlencode};
use crate::json;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The 1-second probe (the scripts' `curl -m 1` liveness checks).
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// One adoption path: a function over the story's state.
pub type Scenario = fn(&mut Story) -> R<()>;

/// The adoption paths in the scripts' order — adding a path is adding a
/// function here. The middle element is the path's work-dir override.
pub const SCENARIOS: &[(&str, &str, Scenario)] = &[
    ("verify-only", "UNIDPP_QS_VERIFY_WORK", verify_only),
    ("publish-only", "UNIDPP_QS_PUBLISH_WORK", publish_only),
    ("gateway", "UNIDPP_QS_GATEWAY_WORK", gateway),
    ("hub", "UNIDPP_QS_HUB_WORK", hub),
];

/// Run one adoption path; the return value is the process exit code.
pub fn run(name: Option<&str>) -> i32 {
    let names = || {
        SCENARIOS
            .iter()
            .map(|(n, _, _)| *n)
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let Some(name) = name else {
        eprintln!("unidpp-demo: quickstart needs a scenario ({})", names());
        return 2;
    };
    let Some((_, work_key, scenario)) = SCENARIOS.iter().find(|(n, _, _)| *n == name) else {
        eprintln!(
            "unidpp-demo: unknown quickstart scenario '{name}' ({})",
            names()
        );
        return 2;
    };

    let mut cfg = Config::from_env();
    cfg.work_dir = match env_or(work_key, "") {
        value if !value.is_empty() => PathBuf::from(value),
        _ => cfg.root.join("build").join(format!("quickstart-{name}")),
    };
    // The scripts start each run from an empty scratch dir.
    let _ = fs::remove_dir_all(&cfg.work_dir);
    if let Err(e) = fs::create_dir_all(&cfg.work_dir) {
        eprintln!("cannot create work dir {}: {e}", cfg.work_dir.display());
        return 1;
    }
    let out = match Out::new(&cfg.work_dir.join("transcript.txt")) {
        Ok(out) => out,
        Err(e) => {
            eprintln!("cannot open transcript: {e}");
            return 1;
        }
    };
    let mut story = Story::new(cfg, out);
    let result = scenario(&mut story);
    story.cleanup();
    match result {
        Ok(()) => 0,
        Err(_) => story.subset_exit.unwrap_or(1),
    }
}

/// The closing summary line; the runner exits 0 only when it is clean.
fn summary(story: &mut Story) {
    let (passed, failed) = (story.tally.ok, story.tally.failed);
    story.out.summary(passed, failed);
}

/// The scripts' SKIP: a missing binary, exit 77.
fn skip_exit_77(story: &mut Story, text: String) -> R<()> {
    story.out.skip(&text);
    story.subset_exit = Some(77);
    Err(Abort)
}

/// The scripts' sed diagnostic: the first 8 lines of a verify transcript.
fn show_head(story: &mut Story, path: &Path) {
    for line in head_lines(path, 8) {
        story.out.raw(&line);
    }
}

/// The shell's `$?` for a child that could not be executed at all.
fn code_text(code: Option<i32>) -> String {
    code.map(|c| c.to_string())
        .unwrap_or_else(|| "127".to_string())
}

/// ===========================================================================
/// adoption path 1 — verify-only: one binary, zero services
/// ===========================================================================
fn verify_only(story: &mut Story) -> R<()> {
    story.out.path_header(
        "== adoption path 1 ==",
        "verify-only: one binary, zero services",
    );
    if !is_executable(&story.cfg.unidpp) {
        return skip_exit_77(
            story,
            format!(
                "unidpp binary unavailable at {}",
                story.cfg.unidpp.display()
            ),
        );
    }
    let work = story.cfg.work_dir.clone();
    let unidpp = story.cfg.unidpp.display().to_string();
    let passport = work.join("passport.json");
    let pack = work.join("pack.hex");
    let passport_str = passport.display().to_string();
    let pack_str = pack.display().to_string();

    // steps 1-2: a specimen to verify (the CI stand-in for "a pack
    // arrives at the border"; a real adopter starts at step 3)
    let argv = [
        unidpp.as_str(),
        "create",
        "--id",
        "sgtin:4006381333931+21+QS001",
        "--type",
        "https://example.org/types/battery-pack",
        "--capability",
        "S1",
        "--out",
        passport_str.as_str(),
    ];
    let code = story.run_command(&argv, Sink::Null, Sink::Null);
    if code == Some(0) {
        story.pass_check("passport skeleton minted locally (specimen)");
    } else {
        story.fail_check("create failed");
    }

    let argv = [
        unidpp.as_str(),
        "event",
        "--passport",
        passport_str.as_str(),
        "--type",
        "custody.transfer",
        "--data",
        r#"{"from":"mfg","to":"border","counterparty_signed":true}"#,
        "--key",
        "border-specimen-seed",
    ];
    let code = story.run_command(&argv, Sink::Null, Sink::Null);
    if code == Some(0) {
        story.pass_check("one lifecycle event appended");
    } else {
        story.fail_check("event failed");
    }

    let pack_stdout = work.join("pack.stdout");
    let pack_stderr = work.join("pack.stderr");
    let argv = [
        unidpp.as_str(),
        "pack",
        "--passport",
        passport_str.as_str(),
        "--key",
        "border-specimen-pack-seed",
        "--out",
        pack_str.as_str(),
    ];
    let code = story.run_command(
        &argv,
        Sink::File(pack_stdout),
        Sink::File(pack_stderr.clone()),
    );
    if code == Some(0) {
        story.pass_check("Tier-A pack minted (specimen)");
    } else {
        story.fail_check("pack failed");
    }

    // The pack mint prints the derived public key on stderr — the anchor
    // a verifier pins. This is the trust-list entry in miniature.
    let stderr_text = fs::read_to_string(&pack_stderr).unwrap_or_default();
    let anchor = first_hex_run(&stderr_text, 130);
    match &anchor {
        Some(a) => story.pass_check(&format!(
            "anchor derived from the mint ({}…)",
            &a[..16.min(a.len())]
        )),
        None => story.fail_check("no anchor on the mint's stderr"),
    }
    let anchor = anchor.unwrap_or_default();

    // --- step 3: the adoption path proper — offline verification -------
    let verify_out = work.join("verify.stdout");
    let argv = [
        unidpp.as_str(),
        "verify",
        pack_str.as_str(),
        "--anchor",
        anchor.as_str(),
        "--max-age",
        "0",
    ];
    let code = story.run_command(
        &argv,
        Sink::File(verify_out.clone()),
        Sink::File(verify_out.clone()),
    );
    if code == Some(0) {
        story.pass_check("verdict: PASS, offline, no services");
    } else {
        story.fail_check(&format!("expected PASS, exit {}", code_text(code)));
        show_head(story, &verify_out);
    }

    let verdict_text = fs::read_to_string(&verify_out).unwrap_or_default();
    if verdict_text.to_ascii_lowercase().contains("pass") {
        story.pass_check("the verdict names its grade");
    } else {
        story.fail_check("verdict text missing");
    }

    // --- step 4: the terminal catches tampering on its own -------------
    // Flip a byte in the middle of the pack body (well away from the
    // signature slots at the end) — the damage a relabeller does. An
    // empty pack stays untouched (the script's indexer crashed on it).
    let original = fs::read_to_string(&pack).unwrap_or_default();
    let trimmed = original.trim().to_string();
    if !trimmed.is_empty() {
        let mid = trimmed.len() / 2;
        let flip = if trimmed.as_bytes()[mid] != b'0' {
            "0"
        } else {
            "1"
        };
        let _ = fs::write(
            &pack,
            format!("{}{flip}{}", &trimmed[..mid], &trimmed[mid + 1..]),
        );
    }

    let tampered_out = work.join("verify.tampered.stdout");
    let argv = [
        unidpp.as_str(),
        "verify",
        pack_str.as_str(),
        "--anchor",
        anchor.as_str(),
        "--max-age",
        "0",
    ];
    let code = story.run_command(
        &argv,
        Sink::File(tampered_out.clone()),
        Sink::File(tampered_out.clone()),
    );
    if code == Some(2) {
        story.pass_check("tampered pack: FAIL (exit 2), caught with zero services");
    } else {
        story.fail_check(&format!(
            "tampered pack returned exit {}, expected 2",
            code_text(code)
        ));
    }

    // --- step 5: no anchor, honestly degraded --------------------------
    let noanchor_out = work.join("verify.noanchor.stdout");
    let argv = [
        unidpp.as_str(),
        "verify",
        pack_str.as_str(),
        "--max-age",
        "0",
    ];
    let code = story.run_command(
        &argv,
        Sink::File(noanchor_out.clone()),
        Sink::File(noanchor_out),
    );
    match code {
        Some(1) | Some(2) => story.pass_check(&format!(
            "without an anchor the verdict degrades (exit {}), never silently passes",
            code_text(code)
        )),
        _ => story.fail_check(&format!(
            "no-anchor verify returned exit {}",
            code_text(code)
        )),
    }

    summary(story);
    Ok(())
}

/// ===========================================================================
/// adoption path 2 — publish-only: one issuer, nothing else
/// ===========================================================================
fn publish_only(story: &mut Story) -> R<()> {
    story.out.path_header(
        "== adoption path 2 ==",
        "publish-only: one issuer, nothing else",
    );
    if !is_executable(&story.cfg.issuer_bin) {
        let missing = story.cfg.issuer_bin.display().to_string();
        return skip_exit_77(story, format!("binary unavailable: {missing}"));
    }
    if !is_executable(&story.cfg.unidpp) {
        let missing = story.cfg.unidpp.display().to_string();
        return skip_exit_77(story, format!("binary unavailable: {missing}"));
    }
    let work = story.cfg.work_dir.clone();
    let issuer_bin = story.cfg.issuer_bin.clone();
    let bind = env_or("UNIDPP_QS_PUBLISH_BIND", "127.0.0.1:18511");
    let url = format!("http://{bind}");
    let (host, port) = Config::split_bind(&bind);

    // --- the whole deployment: one issuer, standalone ------------------
    let journal = work.join("issuer-journal.jsonl").display().to_string();
    let log = work.join("issuer.log");
    story.issuer = story.spawn_service_logged(
        &issuer_bin,
        &[
            ("UNIDPP_ISSUER_BIND", bind.as_str()),
            ("UNIDPP_ISSUER_STATE_FILE", journal.as_str()),
        ],
        &log,
        false,
    );
    if !story.wait_healthy_window(&host, port, 50, 200) {
        story.fail_check("issuer never became healthy");
        for line in tail_lines(&log, 5) {
            story.out.raw(&line);
        }
        return Err(Abort);
    }
    story.pass_check(&format!(
        "issuer healthy at {url} (standalone: no registry, no log, no trust)"
    ));

    // --- the anchor set a verifier pins --------------------------------
    let keyring_body = story
        .http_get(&url, "/keyring")
        .filter(|r| r.healthy())
        .map(|r| r.body);
    if let Some(body) = &keyring_body {
        let _ = fs::write(work.join("keyring.json"), body);
        story.pass_check("keyring published");
    } else {
        story.fail_check("no keyring");
    }
    let anchor_doc = keyring_body
        .as_deref()
        .and_then(|body| serde_json::from_str::<Value>(body).ok());
    let anchor = anchor_doc.as_ref().and_then(json::keyring_pack_anchor);
    match &anchor {
        Some(a) => story.pass_check(&format!(
            "pack anchor pinned from the keyring ({}…)",
            &a[..16.min(a.len())]
        )),
        None => {
            story.fail_check("could not read the pack anchor");
            // The script's diagnostic: the keyring, pretty-printed.
            if let Some(doc) = &anchor_doc {
                for line in json::py_dumps(doc).lines().take(20) {
                    story.out.raw(line);
                }
            }
        }
    }
    let anchor = anchor.unwrap_or_default();

    // --- publish: passport, event, pack — all through the API ----------
    let create_body = r#"{"identity":"gtin:4006381333931","type_ref":"https://example.org/types/battery-pack","capability":"S1"}"#;
    let created = story
        .http_post(&url, "/passports", create_body)
        .filter(|r| r.healthy())
        .map(|r| r.body);
    if let Some(body) = &created {
        let _ = fs::write(work.join("passport.json"), body);
        story.pass_check("passport issued through the API");
    } else {
        story.fail_check("passport creation failed");
    }
    let passport_doc = created
        .as_deref()
        .and_then(|body| serde_json::from_str::<Value>(body).ok());
    let passport_id = passport_doc
        .as_ref()
        .map(|doc| json::field_print(doc, "passport_id"))
        .unwrap_or_default();
    if passport_id.is_empty() {
        story.fail_check("no passport_id in the response");
    } else {
        story.pass_check(&format!("passport id: {passport_id}"));
    }

    let events_path = format!("/passports/{}/events", urlencode(&passport_id));
    let event_ok = story
        .http_post(
            &url,
            &events_path,
            r#"{"type":"custody.transfer","data":{"from":"mfg","to":"dist","counterparty_signed":true}}"#,
        )
        .map(|r| r.healthy())
        .unwrap_or(false);
    if event_ok {
        story.pass_check("event appended (server-signed)");
    } else {
        story.fail_check("event append failed");
    }

    let pack_response = story
        .http_post(
            &url,
            &format!("/passports/{}/pack", urlencode(&passport_id)),
            "{}",
        )
        .filter(|r| r.healthy())
        .map(|r| r.body);
    if let Some(body) = &pack_response {
        let _ = fs::write(work.join("pack-response.json"), body);
        story.pass_check("Tier-A pack minted through the API");
    } else {
        story.fail_check("pack mint failed");
    }
    let pack_hex = pack_response
        .as_deref()
        .and_then(|body| serde_json::from_str::<Value>(body).ok())
        .and_then(|doc| doc.get("pack").and_then(Value::as_str).map(str::to_string));
    if let Some(hex) = &pack_hex {
        let _ = fs::write(work.join("pack.hex"), hex);
        story.pass_check("pack bytes staged for the officer's terminal");
    } else {
        story.fail_check("pack staging failed");
    }

    // --- the service is gone; the pack still verifies -------------------
    Story::stop_process(&mut story.issuer);
    let still_up = http::get(&host, port, "/healthz", PROBE_TIMEOUT)
        .map(|r| r.healthy())
        .unwrap_or(false);
    if still_up {
        story.fail_check("issuer still up");
    } else {
        story.pass_check("issuer stopped");
    }

    let verify_out = work.join("verify.stdout");
    let unidpp = story.cfg.unidpp.display().to_string();
    let pack_path = work.join("pack.hex").display().to_string();
    let argv = [
        unidpp.as_str(),
        "verify",
        pack_path.as_str(),
        "--anchor",
        anchor.as_str(),
        "--max-age",
        "0",
    ];
    let code = story.run_command(
        &argv,
        Sink::File(verify_out.clone()),
        Sink::File(verify_out.clone()),
    );
    if code == Some(0) {
        story.pass_check("verdict: PASS, offline, after the issuer is gone");
    } else {
        story.fail_check(&format!("expected PASS, exit {}", code_text(code)));
        show_head(story, &verify_out);
    }

    summary(story);
    Ok(())
}

/// ===========================================================================
/// adoption path 3 — augment-existing: the gateway as translation edge
/// ===========================================================================
fn gateway(story: &mut Story) -> R<()> {
    story.out.path_header(
        "== adoption path 3 ==",
        "augment-existing: the gateway as translation edge",
    );
    if !is_executable(&story.cfg.issuer_bin) {
        let missing = story.cfg.issuer_bin.display().to_string();
        return skip_exit_77(story, format!("binary unavailable: {missing}"));
    }
    if !is_executable(&story.cfg.gateway_bin) {
        let missing = story.cfg.gateway_bin.display().to_string();
        return skip_exit_77(story, format!("binary unavailable: {missing}"));
    }
    let work = story.cfg.work_dir.clone();
    let issuer_bin = story.cfg.issuer_bin.clone();
    let gateway_bin = story.cfg.gateway_bin.clone();
    let issuer_bind = env_or("UNIDPP_QS_GATEWAY_ISSUER_BIND", "127.0.0.1:18521");
    let gateway_bind = env_or("UNIDPP_QS_GATEWAY_BIND", "127.0.0.1:18522");
    let issuer_url = format!("http://{issuer_bind}");
    let gateway_url = format!("http://{gateway_bind}");
    let (issuer_host, issuer_port) = Config::split_bind(&issuer_bind);
    let (gateway_host, gateway_port) = Config::split_bind(&gateway_bind);

    // --- the upstream: one standalone issuer ---------------------------
    let issuer_journal = work.join("issuer-journal.jsonl").display().to_string();
    let issuer_log = work.join("issuer.log");
    story.issuer = story.spawn_service_logged(
        &issuer_bin,
        &[
            ("UNIDPP_ISSUER_BIND", issuer_bind.as_str()),
            ("UNIDPP_ISSUER_STATE_FILE", issuer_journal.as_str()),
        ],
        &issuer_log,
        false,
    );

    // --- the adoption: the gateway in front of it ----------------------
    let gateway_log = work.join("gateway.log");
    story.gateway = story.spawn_service_logged(
        &gateway_bin,
        &[
            ("UNIDPP_GATEWAY_BIND", gateway_bind.as_str()),
            ("UNIDPP_ISSUER_URL", issuer_url.as_str()),
        ],
        &gateway_log,
        false,
    );

    let mut issuer_ready = story.wait_healthy_probe(&issuer_host, issuer_port);
    let mut gateway_ready = story.wait_healthy_probe(&gateway_host, gateway_port);
    for _ in 0..49 {
        if issuer_ready && gateway_ready {
            break;
        }
        if !issuer_ready {
            issuer_ready = story.wait_healthy_probe(&issuer_host, issuer_port);
        }
        if !gateway_ready {
            gateway_ready = story.wait_healthy_probe(&gateway_host, gateway_port);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if issuer_ready {
        story.pass_check(&format!("issuer healthy at {issuer_url}"));
    } else {
        story.fail_check("issuer never became healthy");
    }
    if gateway_ready {
        story.pass_check(&format!(
            "gateway healthy at {gateway_url} (upstream: the issuer)"
        ));
    } else {
        story.fail_check("gateway never became healthy");
        for line in tail_lines(&gateway_log, 5) {
            story.out.raw(&line);
        }
    }

    // --- one passport through the upstream ------------------------------
    let create_body = r#"{"identity":"gtin:4006381333931","type_ref":"https://example.org/types/battery-pack","capability":"S1"}"#;
    let created = story
        .http_post(&issuer_url, "/passports", create_body)
        .filter(|r| r.healthy())
        .map(|r| r.body);
    if let Some(body) = &created {
        let _ = fs::write(work.join("passport.json"), body);
        story.pass_check("passport issued at the upstream");
    } else {
        story.fail_check("passport creation failed");
    }
    let passport_doc = created
        .as_deref()
        .and_then(|body| serde_json::from_str::<Value>(body).ok());
    let passport_id = passport_doc
        .as_ref()
        .map(|doc| json::field_print(doc, "passport_id"))
        .unwrap_or_default();
    let product_id = passport_doc
        .as_ref()
        .map(|doc| json::field_print(doc, "product_id"))
        .unwrap_or_default();

    let events_path = format!("/passports/{}/events", urlencode(&passport_id));
    let event_ok = story
        .http_post(
            &issuer_url,
            &events_path,
            r#"{"type":"status.change","data":{"from":"issued","to":"suspended","authority":"qs-gateway-check"}}"#,
        )
        .map(|r| r.healthy())
        .unwrap_or(false);
    if event_ok {
        story.pass_check("event appended at the upstream");
    } else {
        story.fail_check("event append failed");
    }

    // --- the augment: both foreign bindings over one core state --------
    //
    // The pre-existing catalogue (the fixtures the gateway ships — the
    // stand-in for the DPPs the adopter already serves) answers through
    // BOTH protocol bindings, and the two renders name the same product.
    let en = story.http_get(&gateway_url, "/en18222/v1/dppsByProductId/4006381333931");
    if let Some(r) = &en {
        let _ = fs::write(work.join("en18222.json"), &r.body);
    }
    let en_code = en.as_ref().map(|r| r.status).unwrap_or(0);
    if en_code == 200 {
        story.pass_check("EN 18222 binding answered 200 for the catalogue GTIN");
    } else {
        story.fail_check(&format!("EN 18222 binding answered {en_code}"));
    }

    let untp = story.http_get(&gateway_url, "/untp/product/4006381333931");
    if let Some(r) = &untp {
        let _ = fs::write(work.join("untp.json"), &r.body);
    }
    let untp_code = untp.as_ref().map(|r| r.status).unwrap_or(0);
    if untp_code == 200 {
        story.pass_check("UNTP binding answered 200 for the same product");
    } else {
        story.fail_check(&format!("UNTP binding answered {untp_code}"));
    }

    // The UNTP form carries the GS1 AI-delimited form; the EN form the
    // bare key — both name the same product (the AD-3 parity).
    let parity = en
        .as_ref()
        .zip(untp.as_ref())
        .and_then(|(e, u)| {
            let en_doc = serde_json::from_str::<Value>(&e.body).ok()?;
            let untp_doc = serde_json::from_str::<Value>(&u.body).ok()?;
            Some((json::en18222_identity(&en_doc), untp_doc))
        })
        .map(|(en_identity, untp_doc)| {
            let untp_identity = json::untp_first_identifier(&untp_doc);
            let bare = if untp_identity.contains(')') {
                json::after_last_paren(&untp_identity).to_string()
            } else {
                untp_identity
            };
            bare == en_identity
        })
        .unwrap_or(false);
    if parity {
        story.pass_check("the two bindings name the SAME product identity (AD-3 parity)");
    } else {
        story.fail_check("the bindings diverged on identity");
    }

    // --- new issuance flows through the adopted core --------------------
    //
    // A passport issued at the upstream serves through the gateway's UNTP
    // binding under its passport id — the live path, no fixtures involved.
    let live = story.http_get(
        &gateway_url,
        &format!("/untp/product/{}", urlencode(&passport_id)),
    );
    if let Some(r) = &live {
        let _ = fs::write(work.join("untp-live.json"), &r.body);
    }
    let live_code = live.as_ref().map(|r| r.status).unwrap_or(0);
    if live_code == 200 {
        story.pass_check("the newly issued passport serves through the UNTP binding");
    } else {
        story.fail_check(&format!(
            "live passport through the gateway answered {live_code}"
        ));
    }

    // `01+<gtin>` and `(01)<gtin>` are the same GS1 identity in two
    // spellings; compare the bare keys.
    let carries = live
        .as_ref()
        .and_then(|r| serde_json::from_str::<Value>(&r.body).ok())
        .map(|doc| {
            let issued_bare = product_id.strip_prefix("01+").unwrap_or(&product_id);
            json::untp_identifier_values(&doc).iter().any(|value| {
                let bare = if value.starts_with('(') {
                    json::after_last_paren(value)
                } else {
                    value.as_str()
                };
                bare == issued_bare
            })
        })
        .unwrap_or(false);
    if carries {
        story.pass_check("the live render carries the identity the issuer issued");
    } else {
        story.fail_check("the live render lost the issued identity");
    }

    // --- the adoption is reversible: gateway gone, upstream whole -------
    Story::stop_process(&mut story.gateway);
    let still_up = http::get(&gateway_host, gateway_port, "/healthz", PROBE_TIMEOUT)
        .map(|r| r.healthy())
        .unwrap_or(false);
    if still_up {
        story.fail_check("gateway still up");
    } else {
        story.pass_check("gateway stopped");
    }

    let upstream = story
        .http_get(
            &issuer_url,
            &format!("/passports/{}", urlencode(&passport_id)),
        )
        .map(|r| r.healthy())
        .unwrap_or(false);
    if upstream {
        story.pass_check("upstream issuer unaffected by the gateway's removal");
    } else {
        story.fail_check("upstream issuer lost the passport");
    }

    summary(story);
    Ok(())
}

/// ===========================================================================
/// hub attachment — the stateless signed relay between willing pairs
/// ===========================================================================
fn hub(story: &mut Story) -> R<()> {
    story.out.path_header(
        "== hub attachment ==",
        "the stateless signed relay between willing pairs",
    );
    if !is_executable(&story.cfg.hub_bin) {
        return skip_exit_77(
            story,
            format!("binary unavailable: {}", story.cfg.hub_bin.display()),
        );
    }
    let work = story.cfg.work_dir.clone();
    let hub_bin = story.cfg.hub_bin.clone();
    let bind = env_or("UNIDPP_QS_HUB_BIND", "127.0.0.1:18581");
    let url = format!("http://{bind}");
    let (host, port) = Config::split_bind(&bind);
    let hub_log = work.join("hub.log");

    // --- the fixtures: both sides' published declarations ---------------
    let work_str = work.display().to_string();
    let argv = [
        "cargo",
        "run",
        "--release",
        "--example",
        "declarations",
        "--",
        work_str.as_str(),
    ];
    let code = story.run_command_in_dir(
        &story.cfg.family_dir.join("unidpp-hub"),
        &argv,
        Sink::Null,
        Sink::Null,
    );
    if code == Some(0) {
        story.pass_check("declaration fixtures generated (a willing pair, a declining pair)");
    } else {
        story.fail_check("the declarations example failed");
    }

    // The restart re-opens the log for append (the script's `>>`).
    let spawn_hub = |story: &mut Story, append: bool| {
        story.hub = story.spawn_service_logged(
            &hub_bin,
            &[
                ("UNIDPP_HUB_BIND", bind.as_str()),
                ("UNIDPP_HUB_ID", "hub-qs"),
                ("UNIDPP_HUB_SEED", "qs-seed"),
            ],
            &hub_log,
            append,
        );
    };
    spawn_hub(story, false);
    if !story.wait_healthy_window(&host, port, 50, 200) {
        story.fail_check("hub never became healthy");
        for line in tail_lines(&hub_log, 3) {
            story.out.raw(&line);
        }
        return Err(Abort);
    }
    story.pass_check(&format!("hub healthy at {url}"));

    // --- the willing pair relays ----------------------------------------
    let willing = read_json(&work.join("willing.json"));
    let relay_req = json!({
        "from": "eu-scheme",
        "to": "cn-scheme",
        "data_class": "*",
        "evidence_hex": "deadbeef01",
        "declarations": willing.clone(),
    });
    let relay_req_text = json::py_dumps(&relay_req);
    let _ = fs::write(work.join("relay-req.json"), &relay_req_text);
    let relayed = story.http_post(&url, "/relay", &relay_req_text);
    if let Some(r) = &relayed {
        let _ = fs::write(work.join("relay-resp.json"), &r.body);
    }
    let relay_code = relayed.as_ref().map(|r| r.status).unwrap_or(0);
    if relay_code == 200 {
        story.pass_check(&format!("the willing pair relays (HTTP {relay_code})"));
    } else {
        story.fail_check(&format!("the relay answered {relay_code}"));
    }

    let relay_resp = relayed
        .as_ref()
        .and_then(|r| serde_json::from_str::<Value>(&r.body).ok())
        .unwrap_or(Value::Null);
    let verify_req = json!({
        "evidence_hex": relay_resp.get("evidence_hex").unwrap_or(&Value::Null),
        "relay": relay_resp.get("relay").unwrap_or(&Value::Null),
    });
    let verify_req_text = json::py_dumps(&verify_req);
    let _ = fs::write(work.join("verify-req.json"), &verify_req_text);
    let verified = relay_verdict(story, &url, &verify_req_text);
    if verified == "True" {
        story.pass_check("the relay VERIFIES under the hub's keyring key");
    } else {
        story.fail_check(&format!("the relay did not verify ({verified})"));
    }

    // Tampered evidence under the same signature: caught.
    let mut tampered_req = verify_req.clone();
    tampered_req["evidence_hex"] = json!("deadbeef02");
    let tampered_text = json::py_dumps(&tampered_req);
    let _ = fs::write(work.join("verify-tampered.json"), &tampered_text);
    let caught = relay_verdict(story, &url, &tampered_text);
    if caught == "False" {
        story.pass_check("tampered evidence under the same signature: caught");
    } else {
        story.fail_check(&format!("tampering slipped ({caught})"));
    }

    // --- the declining pair: the WILL gap, never brokered around -------
    let declining = read_json(&work.join("declining.json"));
    let decline_req = json!({
        "from": "eu-scheme",
        "to": "jp-scheme",
        "data_class": "*",
        "evidence_hex": "00",
        "declarations": declining,
    });
    let decline_text = json::py_dumps(&decline_req);
    let _ = fs::write(work.join("decline-req.json"), &decline_text);
    let declined = story.http_post(&url, "/relay", &decline_text);
    if let Some(r) = &declined {
        let _ = fs::write(work.join("decline-resp.json"), &r.body);
    }
    let decline_code = declined.as_ref().map(|r| r.status).unwrap_or(0);
    if decline_code == 422 {
        story.pass_check("the declining pair is refused (HTTP 422)");
    } else {
        story.fail_check(&format!("the declining pair answered {decline_code}"));
    }
    let reason = declined
        .as_ref()
        .and_then(|r| serde_json::from_str::<Value>(&r.body).ok())
        .map(|doc| json::field_print(&doc, "error"))
        .unwrap_or_default();
    if reason.contains("WILL gap") {
        story.pass_check(
            "the refusal names the WILL gap — jp-scheme declined, the hub does not bridge",
        );
    } else {
        story.fail_check(&format!("the refusal lost the reason: {reason}"));
    }

    // --- the absent declaration: stated, never silence -----------------
    let absent_req = json!({
        "from": "eu-scheme",
        "to": "kr-scheme",
        "data_class": "*",
        "evidence_hex": "00",
        "declarations": willing,
    });
    let absent_text = json::py_dumps(&absent_req);
    let _ = fs::write(work.join("absent-req.json"), &absent_text);
    let absent = story.http_post(&url, "/relay", &absent_text);
    if let Some(r) = &absent {
        let _ = fs::write(work.join("absent-resp.json"), &r.body);
    }
    let absent_code = absent.as_ref().map(|r| r.status).unwrap_or(0);
    if absent_code == 422 {
        story.pass_check("the absent declaration is stated (HTTP 422)");
    } else {
        story.fail_check(&format!("the absent case answered {absent_code}"));
    }
    let reason = absent
        .as_ref()
        .and_then(|r| serde_json::from_str::<Value>(&r.body).ok())
        .map(|doc| json::field_print(&doc, "error"))
        .unwrap_or_default();
    if reason.contains("kr-scheme") {
        story.pass_check("the absence names the silent side");
    } else {
        story.fail_check(&format!("the absence lost the side: {reason}"));
    }

    // --- stateless: restart, the same relay verifies --------------------
    Story::stop_process(&mut story.hub);
    spawn_hub(story, true);
    if !story.wait_healthy_window(&host, port, 50, 200) {
        story.fail_check("the hub did not come back");
        return Err(Abort);
    }
    let verified = relay_verdict(story, &url, &verify_req_text);
    if verified == "True" {
        story.pass_check("after a restart the same relay still verifies — the hub held nothing");
    } else {
        story.fail_check(&format!("statelessness broke ({verified})"));
    }

    summary(story);
    Ok(())
}

/// POST /relay/verify and read `verified` back the Python way
/// (True / False / None — the scripts' `.get("verified")`).
fn relay_verdict(story: &Story, url: &str, body: &str) -> String {
    story
        .http_post(url, "/relay/verify", body)
        .and_then(|r| serde_json::from_str::<Value>(&r.body).ok())
        .map(|doc| json::py_print(doc.get("verified").unwrap_or(&Value::Null)))
        .unwrap_or_else(|| "None".to_string())
}

/// A fixture file as JSON (Null when missing or invalid — the request
/// then fails downstream, as the script's empty body did).
fn read_json(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}
