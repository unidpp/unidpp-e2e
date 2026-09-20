//! The twelve legs — one function per shell test, each check label and
//! count the contract (the ledger compares the summary). The demo legs
//! drive the unidpp-demo binary (the port of demo.sh and demo-live.sh);
//! the CLI, tenant, claims, and hub legs exercise the family directly;
//! the NF-1 bench keeps its script (its port is not this task's).

use crate::engine::{self, Harness};
use crate::http;
use crate::pattern::Pattern;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// The STORY beats the happy path demands by full label (the shell's
/// for-loop list, verbatim — the em-dashes are load-bearing).
const BEAT_LABELS: &[&str] = &[
    "B1 — Assembly in Kyoto",
    "B2 — Parts carry their own duties",
    "B3 — Placement in the EU",
    "B4 — The border moment",
    "B5 — Life in service",
    "B6 — Firmware update and the derestriction incident",
    "B7 — Repair",
    "B8 — Resale and auction",
    "B9 — A recall crosses the graph",
    "B10 — End of life",
];

/// The live leg's four sibling services, in the shell's order (the
/// registry the earlier legs already built is not re-checked here,
/// exactly as the shell did).
const LIVE_REPOS: &[&str] = &[
    "unidpp-issuer",
    "unidpp-trust",
    "unidpp-log",
    "unidpp-gateway",
];

/// grep -cF: how many lines contain the fixed string.
fn count_fixed(text: &str, needle: &str) -> usize {
    text.lines().filter(|line| line.contains(needle)).count()
}

/// The shell's `code_text`: a child that could not be executed at all
/// reads as the shell's 127.
fn code_text(code: Option<i32>) -> String {
    code.map(|c| c.to_string())
        .unwrap_or_else(|| "127".to_string())
}

// ---------------------------------------------------------------------------
// Test 1: happy path — the demo story (the shell ran demo.sh; the
// harness runs its port, the unidpp-demo binary)
// ---------------------------------------------------------------------------

pub fn happy_path(h: &mut Harness) {
    let e2e = h.e2e_work_dir();
    // The demo leg starts from a clean artifact dir (the shell's
    // `rm -rf build/e2e`).
    let _ = fs::remove_dir_all(&e2e);

    let demo = h.demo_bin.display().to_string();
    let code = h.run_split(
        &[&demo, "demo"],
        &h.test_work.join("demo.stdout"),
        &h.test_work.join("demo.stderr"),
    );
    if code != Some(0) {
        h.say("  \u{1b}[31m[FAIL]\u{1b}[0m unidpp-demo exited non-zero; tail of stderr:");
        for line in engine::tail_lines(&h.test_work.join("demo.stderr"), 25) {
            h.say(&line);
        }
        h.fail += 1;
        return;
    }

    let transcript = e2e.join("transcript.txt");
    if !transcript.is_file() {
        h.say("  [FAIL] no transcript");
        h.fail += 1;
        return;
    }

    // Strip ANSI escape sequences for stable pattern matching; the
    // transcript doubles as the human narration AND the assertion
    // target.
    let plain = engine::strip_ansi(&engine::read(&transcript));
    let plain_path = h.test_work.join("transcript.plain.txt");
    let _ = fs::write(&plain_path, &plain);

    let beats = Pattern::new("^B[0-9]+ — ").count_lines(&plain);
    h.assert("all ten STORY beats labelled", "10", &beats.to_string());
    // The CTO composition beat rides between B2 and B3.
    h.assert(
        "CTO composition beat labelled",
        "1",
        &count_fixed(&plain, "B-CTO — The build-to-order variant").to_string(),
    );
    h.assert(
        "CTO composed instance shows its two children",
        "1",
        &count_fixed(&plain, "CTO instance outgoing installs == 2 == 2").to_string(),
    );

    // The S12 interop beat rides between B-CTO and B3: the gateway
    // renders the passport as the UNTP triad and ingests it back. The
    // identity-match check line only prints in its [ok] form on a real
    // round trip (the [FAIL] form carries "expected", not "==").
    h.assert(
        "S12 interop beat labelled",
        "1",
        &count_fixed(&plain, "B-INT — S12 interop").to_string(),
    );
    h.assert_grep(
        &plain,
        "B-INT ingested identity round-trips to the source passport == ",
        "B-INT identity round-trip check",
    );
    h.assert_grep(
        &plain,
        ".idempotent per subject. == matched",
        "B-INT re-ingest matched",
    );

    // The grid beat rides between B-QUORUM and B10 (Phase 1 of the
    // build contract): the sealed segment proven from the spine alone.
    h.assert(
        "grid beat labelled",
        "1",
        &count_fixed(&plain, "G-GRID — The grid").to_string(),
    );
    h.assert_grep(
        &plain,
        "G-GRID the sealed segment verifies from the spine alone == ok",
        "G-GRID sealed segment verified from the spine",
    );
    // The sealed contents leaking into the transcript IS the failure;
    // a fabricated assert pair reports it as one.
    if plain.contains("cycle_count=412") {
        h.assert("G-GRID sealed contents never leak", "never", "leaked");
    } else {
        h.assert("G-GRID sealed contents never leak", "never", "never");
    }
    // Phase 2: the CN battery case (S13 offer, substitution, grading).
    h.assert_grep(
        &plain,
        "G-GRID S13 offers attestation, not data == ok",
        "G-GRID S13 attestation offer",
    );
    h.assert_grep(
        &plain,
        "G-GRID substitution verifies under the verifier's own anchors == ok",
        "G-GRID sovereign substitution",
    );
    h.assert_grep(
        &plain,
        "G-GRID the verdict is a coverage report object .verified-direct . attested. == ok",
        "G-GRID coverage-graded verdict",
    );

    // The quorum beat rides between B9 and B10: retroactive distrust
    // of an authority as a quorate M-of-K act (2-of-3 jurisdictions).
    h.assert(
        "quorum beat labelled",
        "1",
        &count_fixed(&plain, "B-QUORUM — Retroactive distrust").to_string(),
    );
    // The outcome checks hold only when the beat RAN (the trust and
    // quorum-ceremony binaries present); a narrated skip is honest and
    // counts nothing.
    if plain.contains("B-QUORUM narrated without running") {
        h.say("  \u{1b}[33m[skip]\u{1b}[0m quorum beat narrated only (trust binaries absent)");
    } else {
        h.assert_grep(
            &plain,
            "B-QUORUM single regulator refused .422 — quorum attestation required. == 422",
            "B-QUORUM single regulator refused",
        );
        h.assert_grep(
            &plain,
            "B-QUORUM quorate 2-of-3 declaration accepted .201. == 201",
            "B-QUORUM quorate declaration accepted",
        );
        h.assert_grep(
            &plain,
            "B-QUORUM the pack verdict degrades: in-window verifications no longer stand == false",
            "B-QUORUM verdict degrades through the standing overlay",
        );
    }

    for label in BEAT_LABELS {
        let number = label.split(' ').next().unwrap_or(label);
        let candidate = plain
            .lines()
            .find(|line| line.contains(number) && line.contains(label));
        match candidate {
            Some(line) if line.starts_with(label) => {
                h.count_ok(&format!("beat label present: {label}"));
            }
            _ => h.count_fail(&format!("beat label missing: {label}")),
        }
    }

    // Story-expected verify outcomes. The "PASS" assertion matches
    // either the rendered verdict line or the `check "verify verdict
    // ..." == <code>` helper line (both end in PASS / == 0 / == 2
    // respectively).
    h.assert_grep(
        &plain,
        "B4 border moment.*(PASS|== 0)",
        "B4 border-moment PASS",
    );
    h.assert_grep(
        &plain,
        "B6 derestriction.*(FAIL|== 2)",
        "B6 derestriction FAIL",
    );
    h.assert_grep(&plain, "B7 new pack .post-swap..*== 0", "B7 new pack PASS");
    h.assert_grep(&plain, "B8 post-auction.*== 0", "B8 post-auction PASS");
    h.assert_grep(&plain, "B9 original pack.*(FAIL|== 2)", "B9 old pack FAIL");
    h.assert_grep(&plain, "B9 Vienna bike.*unaffected", "B9 new pack degraded");
    h.assert_grep(
        &plain,
        "B10 end-of-waste.*(DEGRADED|== 1)",
        "B10 scrap EoW degraded",
    );
    h.assert_grep(&plain, "B10 decomposed bike.*degraded", "B10 bike degraded");
    h.assert_grep(&plain, "DEMO PASSED", "closing line");

    h.say(&format!("  artifacts under: {}/", e2e.display()));
}

// ---------------------------------------------------------------------------
// Test 2: tamper detection — flipping a byte in a signed pack must FAIL
// ---------------------------------------------------------------------------

pub fn tamper_detection(h: &mut Harness) {
    let pack = h.e2e_work_dir().join("b4-border.pack");
    if !pack.is_file() {
        h.say(&format!(
            "  [FAIL] missing pack {} (run the happy path first)",
            pack.display()
        ));
        h.fail += 1;
        return;
    }

    // Read the anchor from the plain transcript: the colored file has
    // ANSI escape sequences that would be captured into the hex (the
    // shell's grep -oE | awk '{print $NF}').
    let plain = engine::read(&h.test_work.join("transcript.plain.txt"));
    let anchor = plain
        .lines()
        .find_map(|line| {
            let marker = "anchor (public key, hex): ";
            let at = line.find(marker)?;
            let hex: String = line[at + marker.len()..]
                .chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .collect();
            (!hex.is_empty()).then_some(hex)
        })
        .unwrap_or_default();
    if anchor.is_empty() {
        h.say("  [FAIL] could not read issuer anchor from transcript");
        h.fail += 1;
        return;
    }

    // `unidpp verify` accepts hex text in the positional arg or a path
    // to a hex text file. Build a tampered hex file by flipping one
    // hex digit in the middle — the head and tail remain identical.
    let original = engine::read(&pack).trim_end_matches('\n').to_string();
    let chars: Vec<char> = original.chars().collect();
    let mid = chars.len() / 2;
    let flipped = if chars.get(mid) == Some(&'0') {
        '1'
    } else {
        '0'
    };
    let tampered: String = chars[..mid]
        .iter()
        .chain(std::iter::once(&flipped))
        .chain(chars[mid + 1..].iter())
        .collect();
    let bytes_file = h.test_work.join("pack.bytes");
    let _ = fs::write(&bytes_file, &tampered);

    let unidpp = h.unidpp.display().to_string();
    let bytes = bytes_file.display().to_string();
    let code = h.run_null(&[
        &unidpp,
        "verify",
        &bytes,
        "--anchor",
        &anchor,
        "--as-of",
        "2028-02-15T09:30:00Z",
    ]);
    match code {
        None => {
            h.say(&format!("  [FAIL] unidpp CLI not found at {unidpp}"));
            h.fail += 1;
        }
        Some(ve) => h.assert("tampered pack yields FAIL (exit 2)", "2", &ve.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Test 3: missing anchor — degrades, never passes
// ---------------------------------------------------------------------------

pub fn missing_anchor(h: &mut Harness) {
    let pack = h.e2e_work_dir().join("b4-border.pack");
    if !pack.is_file() {
        h.say(&format!("  [FAIL] missing pack {}", pack.display()));
        h.fail += 1;
        return;
    }

    let unidpp = h.unidpp.display().to_string();
    let pack_str = pack.display().to_string();
    let code = h.run_null(&[
        &unidpp,
        "verify",
        &pack_str,
        "--as-of",
        "2028-02-15T09:30:00Z",
    ]);
    let ve = code.unwrap_or(127);
    h.assert("no anchor -> degraded (exit 1)", "1", &ve.to_string());
}

// ---------------------------------------------------------------------------
// Test 4: registry dated binding
// ---------------------------------------------------------------------------

pub fn registry_binding(h: &mut Harness) {
    let e2e = h.e2e_work_dir();
    let json_2027 = e2e.join("reg-applicability-2027.json");
    let json_2028 = e2e.join("reg-applicability-2028.json");
    if !json_2027.is_file() || !json_2028.is_file() {
        h.say("  [FAIL] registry responses missing (run the happy path first)");
        h.fail += 1;
        return;
    }

    let count = |path: &Path| -> String {
        serde_json::from_str::<Value>(&engine::read(path))
            .ok()
            .and_then(|doc| {
                doc.get("applicability")
                    .and_then(Value::as_array)
                    .map(Vec::len)
            })
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unreadable".to_string())
    };
    h.assert(
        "EU profiles bound before 2028-02-01",
        "0",
        &count(&json_2027),
    );
    h.assert(
        "EU profiles bound after 2028-02-01",
        "1",
        &count(&json_2028),
    );
}

// ---------------------------------------------------------------------------
// Test 5: live services — B1+B4 subset over registry+issuer+trust+log
// (the shell ran demo-live.sh; the harness runs its port, the demo
// binary's LIVE preset)
// ---------------------------------------------------------------------------

pub fn live_services(h: &mut Harness) {
    // Build any missing service binary; SKIP (not FAIL) when one cannot
    // be produced — the sibling repos are developed in parallel and can
    // be mid-edit.
    for repo in LIVE_REPOS {
        let bin = h.family.join(repo).join("target/release").join(repo);
        if engine::is_executable(&bin) {
            continue;
        }
        h.say(&format!("  ....... building missing {repo}"));
        let manifest = h.family.join(repo).join("Cargo.toml").display().to_string();
        let _ = h.run_null(&["cargo", "build", "--release", "--manifest-path", &manifest]);
    }
    for repo in LIVE_REPOS {
        let bin = h.family.join(repo).join("target/release").join(repo);
        if !engine::is_executable(&bin) {
            h.skip(&format!(
                "live test: {repo} binary unavailable (build failed or repo absent)"
            ));
            return;
        }
    }

    let live_work = h.test_work.join("live");
    let _ = fs::remove_dir_all(&live_work);
    let demo = h.demo_bin.display().to_string();
    let live = live_work.display().to_string();
    let code = h.run_split_env(
        &[&demo, "demo", "live"],
        &[
            ("UNIDPP_E2E_LIVE_DIR", live.as_str()),
            ("UNIDPP_E2E_STOP_AFTER", "B4"),
        ],
        &h.test_work.join("demo-live.stdout"),
        &h.test_work.join("demo-live.stderr"),
    );
    if code != Some(0) {
        h.say("  \u{1b}[31m[FAIL]\u{1b}[0m unidpp-demo (live) exited non-zero; tail of stderr:");
        for line in engine::tail_lines(&h.test_work.join("demo-live.stderr"), 25) {
            h.say(&line);
        }
        h.fail += 1;
        return;
    }

    let transcript = live_work.join("e2e/transcript.txt");
    if !transcript.is_file() {
        h.say("  [FAIL] no live transcript");
        h.fail += 1;
        return;
    }
    let plain = engine::strip_ansi(&engine::read(&transcript));
    let _ = fs::write(h.test_work.join("transcript-live.plain.txt"), &plain);

    h.assert_grep(&plain, "topology: LIVE", "live topology header present");
    h.assert_grep(&plain, "registry : http", "topology lists the registry");
    h.assert_grep(
        &plain,
        "issuer   : http.*server-minted packs",
        "topology lists the issuer",
    );
    h.assert_grep(
        &plain,
        "trust    : http.*verify anchor source",
        "topology lists the trust service",
    );
    h.assert_grep(
        &plain,
        "log      : http.*signed receipts",
        "topology lists the log",
    );
    h.assert_grep(
        &plain,
        "anchor pinned from .*/keyring",
        "trust anchor pinned from /keyring",
    );
    h.assert_grep(
        &plain,
        r"\[ok\].*issuer pack anchor == trust-pinned anchor",
        "issuer and trust anchors identical",
    );
    h.assert_grep(
        &plain,
        "B4 border moment.*(PASS|== 0)",
        "B4 verify PASS under the live anchor",
    );
    h.assert_grep(
        &plain,
        "log: +receipt [0-9]+ ",
        "log receipts narrated with ids",
    );
    h.assert_grep(&plain, "B-INT — S12 interop", "B-INT beat in the live run");
    h.assert_grep(
        &plain,
        "B-INT render source is the live issuer == issuer",
        "B-INT renders from the live issuer",
    );
    h.assert_grep(
        &plain,
        "B-INT ingested identity round-trips to the source passport == ",
        "B-INT identity round-trip in the live run",
    );
    h.assert_grep(&plain, "DEMO PASSED", "live subset completed");

    // Independent receipt check, outside the orchestrator: the stored
    // B4 receipt's commitment must equal the sha256 of the pack file.
    let receipt = live_work.join("e2e/log-receipts/b4-border.receipt.json");
    let pack = live_work.join("e2e/b4-border.pack");
    if receipt.is_file() && pack.is_file() {
        let want = Sha256::digest(fs::read(&pack).unwrap_or_default());
        let want = want.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let got = serde_json::from_str::<Value>(&engine::read(&receipt))
            .ok()
            .and_then(|doc| {
                doc.get("commitment")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_default();
        h.assert("b4 log receipt commitment == sha256(pack)", &want, &got);
    } else {
        h.say("  \u{1b}[31m[FAIL]\u{1b}[0m missing b4 receipt or pack artifact");
        h.fail += 1;
    }
}

// ---------------------------------------------------------------------------
// Tests 6-8: the adoption-path quickstarts (FW-6) — each scenario is
// self-asserting; the harness only records its exit. Exit 77 = SKIP (a
// binary the path needs is unavailable — a sibling repo mid-edit, never
// a failure of the path itself).
// ---------------------------------------------------------------------------

fn run_quickstart(h: &mut Harness, number: u8, scenario: &str) {
    let out = h.test_work.join(format!("quickstart-{number}.stdout"));
    let demo = h.demo_bin.display().to_string();
    let code = h.run_merged(&[&demo, "quickstart", scenario], &out);
    engine::cat(&out);
    match code {
        Some(0) => {}
        Some(77) => h.skip(quickstart_label(number)),
        _ => h.fail_line(&format!(
            "{} exited {}",
            quickstart_label(number),
            code_text(code)
        )),
    }
}

fn quickstart_label(number: u8) -> &'static str {
    match number {
        6 => "adoption path: verify-only (one binary, zero services)",
        7 => "adoption path: publish-only (one issuer, nothing else)",
        8 => "adoption path: augment-existing (the gateway translation edge)",
        _ => unreachable!("the quickstart legs are numbered 6 through 8"),
    }
}

pub fn quickstart_verify_only(h: &mut Harness) {
    run_quickstart(h, 6, "verify-only");
}

pub fn quickstart_publish_only(h: &mut Harness) {
    run_quickstart(h, 7, "publish-only");
}

pub fn quickstart_gateway(h: &mut Harness) {
    run_quickstart(h, 8, "gateway");
}

// ---------------------------------------------------------------------------
// Test 9: SV-7 at the service level — two registries as two tenants;
// credentials do not cross, journals do not mix.
// ---------------------------------------------------------------------------

pub fn tenant_isolation(h: &mut Harness) {
    let registry_bin = h
        .family
        .join("unidpp-registry/target/release/unidpp-registry");
    if !engine::is_executable(&registry_bin) {
        h.skip("registry binary unavailable");
        return;
    }

    let iso_work = h.test_work.join("isolation");
    let _ = fs::remove_dir_all(&iso_work);
    let _ = fs::create_dir_all(&iso_work);

    let bin = registry_bin.display().to_string();
    let tenant_a_state = iso_work.join("tenant-a.json").display().to_string();
    let tenant_b_state = iso_work.join("tenant-b.json").display().to_string();
    let mut tenant_a = engine::spawn_logged(
        Path::new(&bin),
        &[
            ("UNIDPP_REGISTRY_BIND", "127.0.0.1:18541"),
            ("UNIDPP_REGISTRY_STATE_FILE", &tenant_a_state),
            ("UNIDPP_REGISTRY_ADMIN_TOKEN", "tenant-a-token"),
        ],
        &iso_work.join("tenant-a.log"),
    );
    let mut tenant_b = engine::spawn_logged(
        Path::new(&bin),
        &[
            ("UNIDPP_REGISTRY_BIND", "127.0.0.1:18542"),
            ("UNIDPP_REGISTRY_STATE_FILE", &tenant_b_state),
            ("UNIDPP_REGISTRY_ADMIN_TOKEN", "tenant-b-token"),
        ],
        &iso_work.join("tenant-b.log"),
    );

    let ready =
        engine::wait_healthy("127.0.0.1", 18541) && engine::wait_healthy("127.0.0.1", 18542);
    if !ready {
        h.say("  \u{1b}[31m[FAIL]\u{1b}[0m the two tenant registries never became healthy");
        h.fail += 1;
        engine::stop_process(&mut tenant_a);
        engine::stop_process(&mut tenant_b);
        return;
    }
    h.assert("both tenant registries healthy", "1", "1");

    let body_a = r#"{"register_id":"tenant-a","item_id":"shared-name","version":"1","definition":"tenant A item","class":"data-element"}"#;
    let body_b = r#"{"register_id":"tenant-b","item_id":"shared-name","version":"1","definition":"tenant B item","class":"data-element"}"#;

    let post = |port: u16, token: &str, body: &str, out: &PathBuf| -> u16 {
        let reply = http::post_bearer(
            "127.0.0.1",
            port,
            "/items",
            body,
            engine::HTTP_TIMEOUT,
            token,
        );
        if let Some(reply) = &reply {
            let _ = fs::write(out, &reply.body);
        }
        reply.map(|r| r.status).unwrap_or(0)
    };

    // A's own credential writes at A.
    let code_a = post(
        18541,
        "tenant-a-token",
        body_a,
        &iso_work.join("a-own.json"),
    );
    h.assert(
        "tenant A writes at its own registry",
        "201",
        &code_a.to_string(),
    );

    // A's credential is REFUSED at B (the cross-tenant probe).
    let code_cross = post(
        18542,
        "tenant-a-token",
        body_a,
        &iso_work.join("a-at-b.json"),
    );
    h.assert(
        "tenant A's credential refused at tenant B (401)",
        "401",
        &code_cross.to_string(),
    );

    // B's own write succeeds at B — the refusal was the credential, not
    // the item name.
    let code_b = post(
        18542,
        "tenant-b-token",
        body_b,
        &iso_work.join("b-own.json"),
    );
    h.assert(
        "tenant B writes the same item id at its own registry",
        "201",
        &code_b.to_string(),
    );

    // The journals do not mix: each store holds its own tenant's item.
    let register_of = |port: u16| -> String {
        http::get(
            "127.0.0.1",
            port,
            "/items/shared-name",
            engine::HTTP_TIMEOUT,
        )
        .and_then(|reply| serde_json::from_str::<Value>(&reply.body).ok())
        .and_then(|doc| {
            doc.get("register")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "absent".to_string())
    };
    h.assert(
        "tenant B's journal holds B's item only",
        "tenant-b",
        &register_of(18542),
    );
    h.assert(
        "tenant A's journal holds A's item only",
        "tenant-a",
        &register_of(18541),
    );

    engine::stop_process(&mut tenant_a);
    engine::stop_process(&mut tenant_b);
}

// ---------------------------------------------------------------------------
// Test 10: NF-1 — the performance requirement's gated numbers (the
// bench script stays a script; its port is not this task's).
// ---------------------------------------------------------------------------

pub fn nf1_bench(h: &mut Harness) {
    let bench = h.root.join("scripts/bench-nf1.sh");
    let bench = bench.display().to_string();
    if h.run_inherit(&[&bench]) == Some(0) {
        h.credit(2);
    } else {
        h.fail_line("the NF-1 bench gated out");
    }
}

// ---------------------------------------------------------------------------
// Test 11: the class claim tests — every federation class through one
// command, against the family's own fixtures.
// ---------------------------------------------------------------------------

pub fn class_claims(h: &mut Harness) {
    let signatif_fixtures = h.family.join("unidpp-signatif/fixtures/canonical");
    let semantics_fixtures = h
        .family
        .join("unidpp-core/crates/semantics/fixtures/canonical");
    if !engine::is_executable(&h.unidpp) || !signatif_fixtures.is_dir() {
        h.skip("claim material unavailable");
        return;
    }

    let work = h.test_work.join("claims");
    let _ = fs::create_dir_all(&work);

    // Split the frozen-view fixture into the two documents the f1/f4
    // claims take as arguments (the shell's python heredoc).
    let frozen = engine::read(&signatif_fixtures.join("frozen-view.json"));
    if let Ok(doc) = serde_json::from_str::<Value>(&frozen) {
        let _ = fs::write(
            work.join("view.json"),
            serde_json::to_string(&doc["view"]).unwrap_or_default(),
        );
        let _ = fs::write(
            work.join("anchors.json"),
            serde_json::to_string(&doc["anchors"]).unwrap_or_default(),
        );
    }

    let unidpp = h.unidpp.display().to_string();
    let view = work.join("view.json").display().to_string();
    let anchors = work.join("anchors.json").display().to_string();
    // (class, arguments) — the f5 claim's `..` names the family dir
    // relative to the e2e checkout the conform runs execute from.
    let specs: [(&str, Vec<String>); 4] = [
        ("f1", vec![view.clone(), anchors.clone()]),
        (
            "f2",
            vec![signatif_fixtures
                .join("s13-signed-exchange.json")
                .display()
                .to_string()],
        ),
        (
            "f3",
            vec![semantics_fixtures
                .join("mapping-chain.json")
                .display()
                .to_string()],
        ),
        ("f5", vec!["..".to_string()]),
    ];
    for (class, args) in specs {
        let mut argv = vec![unidpp.clone(), "conform".to_string(), class.to_string()];
        argv.extend(args);
        let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
        let out = work.join(format!("{class}.txt"));
        if h.run_merged_in(&h.root, &argv, &out) == Some(0) {
            h.assert(&format!("conform {class} claim holds"), "pass", "pass");
        } else {
            h.fail_line(&format!("conform {class}:"));
            for line in engine::tail_lines(&out, 6) {
                h.say(&line);
            }
        }
    }
    // f4 takes three paths — run it explicitly outside the
    // two-argument loop.
    let out = work.join("f4-full.txt");
    if h.run_merged_in(
        &h.root,
        &[&unidpp, "conform", "f4", &view, &view, &anchors],
        &out,
    ) == Some(0)
    {
        h.assert("conform f4 claim holds (three-path form)", "pass", "pass");
    } else {
        h.fail_line("conform f4 (three-path):");
        for line in engine::tail_lines(&out, 6) {
            h.say(&line);
        }
    }
}

// ---------------------------------------------------------------------------
// Test 12: hub attachment — the stateless signed relay (SI-3).
// ---------------------------------------------------------------------------

pub fn hub(h: &mut Harness) {
    let hub_bin = h.family.join("unidpp-hub/target/release/unidpp-hub");
    if !engine::is_executable(&hub_bin) {
        let manifest = h.family.join("unidpp-hub/Cargo.toml").display().to_string();
        let _ = h.run_null(&["cargo", "build", "--release", "--manifest-path", &manifest]);
    }
    if !engine::is_executable(&hub_bin) {
        h.skip("hub binary unavailable");
        return;
    }
    // The hub quickstart is self-asserting (10 checks); the harness
    // credits them wholesale, as the shell did for the script.
    let demo = h.demo_bin.display().to_string();
    if h.run_inherit(&[&demo, "quickstart", "hub"]) == Some(0) {
        h.credit(10);
    } else {
        h.fail_line("the hub quickstart gated out");
    }
}

/// The demo binary is the harness's own product, not a sibling's: build
/// it when missing and stop loudly when unproducible (a skip here would
/// hide a break of ours — the shell never could skip, its demo was a
/// checked-in script).
pub fn ensure_demo_binary(h: &Harness) -> bool {
    if engine::is_executable(&h.demo_bin) {
        return true;
    }
    let manifest = h.root.join("demo/Cargo.toml").display().to_string();
    let _ = h.run_null(&["cargo", "build", "--release", "--manifest-path", &manifest]);
    engine::is_executable(&h.demo_bin)
}
