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
use std::process::{Command, Stdio};

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
// ---------------------------------------------------------------------------

/// The NF-1 bench, as a leg (the port of scripts/bench-nf1.sh):
/// the three numbers, measured and reported. (1) Tier-A offline
/// verification latency (p95 < 50 ms). (3) Roll-up verification over
/// a deep graph without full traversal (the gate is structural).
/// (2) Served profile views p95 < 300 ms at reference scale — opt-in
/// via UNIDPP_BENCH_VIEWS=1, because CI runners are not the reference
/// machine class; a skipped run credits like the script's exit 0.
pub fn nf1_bench(h: &mut Harness) {
    h.say("\x1b[1m== NF-1 ==\x1b[0m  the three numbers");
    let work = std::env::var("UNIDPP_BENCH_WORK").unwrap_or_else(|_| {
        h.root
            .join("build/bench-nf1")
            .to_string_lossy()
            .into_owned()
    });
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::create_dir_all(&work);

    let cli = h.family.join("unidpp-cli");
    let core = h.family.join("unidpp-core");

    // 1. Tier-A offline verification (the officer's terminal's own
    //    pipeline).
    let verify_bench = cli.join("target/release/examples/nf1_verify_bench");
    if h.run_null(&[
        "cargo",
        "build",
        "--release",
        "--example",
        "nf1_verify_bench",
        "--manifest-path",
    ]) == Some(0)
        || verify_bench.is_file()
    {
        let _ = h.run_inherit(&[verify_bench.to_string_lossy().as_ref(), "64", "20"]);
    } else {
        h.say("  \x1b[33m[SKIP]\x1b[0m nf1_verify_bench could not build");
    }

    // 3. Roll-ups without full traversal.
    let rollup_bench = core.join("target/release/examples/nf1_rollup_bench");
    if h.run_null(&[
        "cargo",
        "build",
        "--release",
        "--example",
        "nf1_rollup_bench",
        "--manifest-path",
    ]) == Some(0)
        || rollup_bench.is_file()
    {
        let _ = h.run_inherit(&[rollup_bench.to_string_lossy().as_ref()]);
    } else {
        h.say("  \x1b[33m[SKIP]\x1b[0m nf1_rollup_bench could not build");
    }

    // 2. Served profile views (opt-in: the reference-class number).
    if std::env::var("UNIDPP_BENCH_VIEWS")
        .unwrap_or_default()
        .is_empty()
    {
        h.say(
            "  served-views p95: SKIPPED (set UNIDPP_BENCH_VIEWS=1 to measure; \
CI runners are not the reference machine class)",
        );
        h.credit(2);
        return;
    }
    let unidpp = h.family.join("unidpp-cli/target/release/unidpp");
    let projector_bin = h
        .family
        .join("unidpp-projector/target/release/unidpp-projector");
    if !unidpp.is_file() || !projector_bin.is_file() {
        h.say("  \x1b[33m[SKIP]\x1b[0m views bench: a dependent binary is unavailable");
        h.credit(2);
        return;
    }
    let population: usize = std::env::var("UNIDPP_BENCH_VIEWS_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(512);
    let requests: usize = std::env::var("UNIDPP_BENCH_VIEWS_REQUESTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let passports_dir = std::path::PathBuf::from(&work).join("passports");
    let _ = std::fs::create_dir_all(&passports_dir);
    for i in 0..population {
        let id = format!("sgtin:4006381333931+21+B{i:07}");
        let passport_id = format!("urn:unidpp:passport:nf1v-{i:07}");
        let out = passports_dir.join(format!("p{i:07}.json"));
        if h.run_null(&[
            unidpp.to_string_lossy().as_ref(),
            "create",
            "--id",
            &id,
            "--type",
            "https://example.org/types/battery-pack",
            "--capability",
            "S1",
            "--passport-id",
            &passport_id,
            "--out",
            out.to_string_lossy().as_ref(),
        ]) != Some(0)
        {
            h.fail_line(&format!("passport {i} could not be minted"));
            return;
        }
    }
    let bind =
        std::env::var("UNIDPP_BENCH_PROJECTOR_BIND").unwrap_or_else(|_| "127.0.0.1:18551".into());
    let port: u16 = bind
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(18551);
    let log = std::fs::File::create(std::path::PathBuf::from(&work).join("projector.log"))
        .expect("bench log");
    let mut child = Command::new(&projector_bin)
        .env_clear()
        .env("UNIDPP_PROJECTOR_BIND", &bind)
        .env("UNIDPP_PROJECTOR_PASSPORTS_DIR", &passports_dir)
        .stdout(log.try_clone().expect("log handle"))
        .stderr(log)
        .spawn()
        .expect("projector spawns");
    let mut ready = false;
    for _ in 0..50 {
        if crate::http::get(
            "127.0.0.1",
            port,
            "/healthz",
            std::time::Duration::from_secs(2),
        )
        .map(|r| r.status == 200)
        .unwrap_or(false)
        {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    if !ready {
        let _ = child.kill();
        h.fail_line("projector never became healthy");
        return;
    }
    h.say(&format!(
        "  served profile views ({population} passports, {requests} requests, fixture profile):"
    ));
    let profile = "urn:unidpp:profile:eu-battery-packs";
    let view = |pid: &str| -> Option<u128> {
        let path = format!("/view?passport={pid}&profile={profile}&actor=consumer");
        let start = std::time::Instant::now();
        let reply = crate::http::get("127.0.0.1", port, &path, std::time::Duration::from_secs(5))?;
        let _ = reply.body;
        if reply.status != 200 {
            return None;
        }
        Some(start.elapsed().as_micros())
    };
    // Warm: first requests build the view caches.
    for i in 0..8 {
        let pid = format!("urn:unidpp:passport:nf1v-{:07}", i % population);
        if view(&pid).is_none() {
            let _ = child.kill();
            h.fail_line(&format!("warm-up {pid} did not answer"));
            return;
        }
    }
    let mut samples: Vec<u128> = Vec::with_capacity(requests);
    for i in 0..requests {
        let pid = format!("urn:unidpp:passport:nf1v-{:07}", (i * 37) % population);
        match view(&pid) {
            Some(us) => samples.push(us),
            None => {
                let _ = child.kill();
                h.fail_line(&format!("{pid} did not answer"));
                return;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    samples.sort_unstable();
    let p50 = samples[samples.len() / 2];
    let p95 = samples[(samples.len() - 1) * 95 / 100];
    h.say(&format!("  p50  {p50:>9} µs"));
    h.say(&format!("  p95  {p95:>9} µs"));
    let holds = p95 < 300_000;
    h.say(&format!(
        "  bar  p95 < 300 ms — {}",
        if holds { "HOLDS" } else { "EXCEEDED" }
    ));
    if holds {
        h.credit(2);
    } else {
        h.fail_line("the served-views bar was exceeded");
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

// ---------------------------------------------------------------------------
// Test 13: the family contracts — the five cross-repo drift checks
// that only bite when run in-family, run together (TODO 250). Each
// skips loudly when its subject checkout is absent; green credits
// one check per subject found; drift fails the family CI.
// ---------------------------------------------------------------------------

pub fn family_contracts(h: &mut Harness) {
    let mut credits = 0u32;

    // 1. The environment contract (unidpp-config): render names ==
    //    every service's x-unidpp-env-keys. Its in-repo test skips
    //    without sibling checkouts; here the siblings are present.
    let config = h.family.join("unidpp-config");
    if config.is_dir() {
        let code = h.run_null(&[
            "cargo",
            "test",
            "--manifest-path",
            &format!("{}/Cargo.toml", config.display()),
            "the_services_env_keys_match_the_rendered_contract",
        ]);
        match code {
            Some(0) => {
                h.say("  [ok]   the env contract holds (config render == every service's x-unidpp-env-keys)");
                credits += 1;
            }
            _ => h.fail_line(
                "the env contract drifted (unidpp-config render vs the services' contracts)",
            ),
        }
    } else {
        h.say("  [SKIP] env contract: no unidpp-config checkout");
    }

    // 2. The docs API reference: every page matches its service's
    //    committed golden.
    let docs = h.family.join("unidpp-docs");
    if docs.is_dir() {
        let code = Command::new("node")
            .arg("tools/check-api-reference.mjs")
            .current_dir(&docs)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if code {
            h.say("  [ok]   the docs API reference matches the committed goldens");
            credits += 1;
        } else {
            h.fail_line("a docs API page drifted from its contract (npm run gen:api)");
        }
    } else {
        h.say("  [SKIP] docs reference: no unidpp-docs checkout");
    }

    // 3. The website's grounded facts: regenerating them changes
    //    nothing (modulo the generation date).
    let site = h.family.join("unidpp.github.io");
    if site.is_dir() {
        let out = Command::new("node")
            .arg("scripts/gen-facts.mjs")
            .current_dir(&site)
            .output();
        match out {
            Ok(o) if o.status.success() => {
                h.say("  [ok]   the website's facts regenerate cleanly");
                credits += 1;
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                let stdout = String::from_utf8_lossy(&o.stdout);
                h.fail_line(&format!(
                    "the website's facts could not regenerate: {}{}",
                    stdout.trim(),
                    stderr.trim()
                ));
            }
            Err(e) => {
                h.fail_line(&format!("the facts generator could not run: {e}"));
            }
        }
    } else {
        h.say("  [SKIP] website facts: no unidpp.github.io checkout");
    }

    // 4. The vendored vector corpora: byte-identical to the originals.
    if Path::new(&h.family.join("unidpp-ts/test-vectors")).is_dir() {
        let code = Command::new("node")
            .arg("scripts/vectors-cross-check.mjs")
            .current_dir(&h.root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if code {
            h.say("  [ok]   the TS/RB vector corpora are byte-identical to the originals");
            credits += 1;
        } else {
            h.fail_line("a vendored vector corpus drifted from its original");
        }
    } else {
        h.say("  [SKIP] vector corpora: no vendored corpus");
    }

    // 5. The papers: every rendered PDF was built against the
    //    current specification head.
    let papers = h.family.join("unidpp-papers");
    if papers.is_dir() {
        let code = Command::new("python3")
            .arg("build-papers-pdf.py")
            .arg("--check-fresh")
            .current_dir(&papers)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if code {
            h.say("  [ok]   the papers are rendered against the current spec head");
            credits += 1;
        } else {
            h.fail_line("a paper predates the current specification head (--check-fresh)");
        }
    } else {
        h.say("  [SKIP] papers freshness: no unidpp-papers checkout");
    }

    // 6. The committed manifest schema: `unidpp-config schema`'s
    //    output must equal the docs' committed file (the export is
    //    regenerated by hand; a model change silently strands it).
    if config.is_dir() {
        let docs_schema = h
            .family
            .join("unidpp-docs/public/operator-manifest.schema.json");
        if docs_schema.is_file() {
            let tmp =
                std::env::temp_dir().join(format!("unidpp-schema-{}.json", std::process::id()));
            let code = Command::new("cargo")
                .args(["run", "--quiet", "--manifest-path"])
                .arg(format!("{}/Cargo.toml", config.display()))
                .arg("--")
                .arg("schema")
                .stdout(std::fs::File::create(&tmp).expect("schema capture file"))
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            let fresh = std::fs::read_to_string(&tmp).unwrap_or_default();
            let _ = std::fs::remove_file(&tmp);
            let committed = std::fs::read_to_string(&docs_schema).unwrap_or_default();
            if code && !fresh.is_empty() && fresh.trim() == committed.trim() {
                h.say("  [ok]   the committed manifest schema equals unidpp-config's export");
                credits += 1;
            } else if code {
                h.fail_line("the committed operator-manifest.schema.json drifted from unidpp-config's model (regenerate: cargo run -- schema)");
            } else {
                h.fail_line("unidpp-config schema could not run");
            }
        } else {
            h.say("  [SKIP] schema freshness: no unidpp-docs checkout");
        }
    }

    // 7. rustfmt across every Rust repository of the family: the
    //    repos' own `fmt --check` gates only bite their next push;
    //    here the drift is named in the family CI, repo by repo.
    {
        let repos = [
            "unidpp-cli",
            "unidpp-registry",
            "unidpp-resolver",
            "unidpp-trust",
            "unidpp-log",
            "unidpp-issuer",
            "unidpp-projector",
            "unidpp-gateway",
            "unidpp-archive",
            "unidpp-hub",
            "unidpp-console",
            "unidpp-config",
            "unidpp-core",
            "unidpp-signatif",
        ];
        let mut drifted: Vec<&str> = Vec::new();
        let mut checked = 0usize;
        for repo in repos {
            let dir = h.family.join(repo);
            if !dir.is_dir() {
                continue;
            }
            checked += 1;
            let clean = Command::new("cargo")
                .arg("fmt")
                .arg("--check")
                .current_dir(&dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if !clean {
                drifted.push(repo);
            }
        }
        if drifted.is_empty() {
            h.say(&format!(
                "  [ok]   rustfmt clean across {checked} repositories"
            ));
            credits += 1;
        } else {
            h.fail_line(&format!(
                "rustfmt drift in: {} (cargo fmt and commit)",
                drifted.join(", ")
            ));
        }
    }

    if credits > 0 {
        h.credit(credits);
    }
}
