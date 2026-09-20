//! The LIVE preset of the `demo` command — demo-live.sh as engine
//! options, not a fork: the four sibling services started for real,
//! each with its JSONL journal under the live dir (wiped for fresh
//! sequencing), the fixed dev ceremony seeds that keep the pinned
//! anchor and the transcripts reproducible, and the story itself run
//! unchanged against the live URLs.

use crate::engine::{is_executable, Config, Story, R};
use std::fs;

/// The pinned-anchor ceremony, dev edition (fixed constants keep the
/// derived public keys — and therefore the transcripts — reproducible):
/// the issuer's event key doubles as the trust service's Ed25519
/// response key, and the issuer's pack-signing key IS the trust
/// service's pinned sign-ecdsa-p256 anchor. A seed drift fails loudly
/// in B4, which asserts the two anchors byte-identical.
const LIVE_ED25519_SEED: &str = "4c4956452d4556454e542d534545442d30313233343536373839616263646566";
const LIVE_P256_SEED: &str = "4c4956452d5041434b2d534545442d30313233343536373839616263646566";

/// Lay out the live dir, wipe the four journals (fresh sequencing per
/// run keeps receipt ids and applicability assertions deterministic),
/// and point the story's service URLs at what is about to start. Runs
/// before the transcript opens, so the artifacts land in <live>/e2e.
pub fn prepare(cfg: &mut Config) -> Result<(), String> {
    let dir = cfg.live_dir.clone();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create live dir {}: {e}", dir.display()))?;
    for journal in [
        "registry.journal.jsonl",
        "issuer.journal.jsonl",
        "trust.journal.jsonl",
        "log.journal.jsonl",
    ] {
        let _ = fs::remove_file(dir.join(journal));
    }
    cfg.work_dir = dir.join("e2e");
    cfg.registry_url_override = Some(format!("http://{}", cfg.registry_bind));
    cfg.issuer_url = Some(format!("http://{}", cfg.issuer_bind));
    cfg.trust_url = Some(format!("http://{}", cfg.trust_bind));
    cfg.log_url = Some(format!("http://{}", cfg.log_bind));
    Ok(())
}

/// Start the four services in the script's order, each announced before
/// and confirmed healthy after (the script's note + wait_healthy pair).
pub fn start_services(story: &mut Story) -> R<()> {
    let dir = story.cfg.live_dir.clone();
    let registry_bin = story.cfg.registry_bin.clone();
    let registry_bind = story.cfg.registry_bind.clone();
    let issuer_bin = story.cfg.issuer_bin.clone();
    let issuer_bind = story.cfg.issuer_bind.clone();
    let trust_bin = story.cfg.trust_bin.clone();
    let trust_bind = story.cfg.trust_bind.clone();
    let log_bin = story.cfg.log_bin.clone();
    let log_bind = story.cfg.log_bind.clone();

    // 1. registry (items, applicability, transforms)
    if !is_executable(&registry_bin) {
        return Err(story.abort(format!(
            "unidpp-registry binary missing: {} (run: make deps-live)",
            registry_bin.display()
        )));
    }
    let registry_journal = dir.join("registry.journal.jsonl").display().to_string();
    story.out.note(&format!(
        "starting unidpp-registry on {registry_bind} (journal: {registry_journal})"
    ));
    story.registry = story.spawn_service(
        &registry_bin,
        &[
            ("UNIDPP_REGISTRY_BIND", registry_bind.as_str()),
            ("UNIDPP_REGISTRY_STATE_FILE", registry_journal.as_str()),
        ],
    );
    wait_healthy(story, "unidpp-registry", &format!("http://{registry_bind}"))?;

    // 2. issuer (server-signed events, server-minted packs)
    if !is_executable(&issuer_bin) {
        return Err(story.abort(format!(
            "unidpp-issuer binary missing: {} (run: make deps-live)",
            issuer_bin.display()
        )));
    }
    let issuer_journal = dir.join("issuer.journal.jsonl").display().to_string();
    story.out.note(&format!(
        "starting unidpp-issuer on {issuer_bind} (journal: {issuer_journal})"
    ));
    story.issuer = story.spawn_service(
        &issuer_bin,
        &[
            ("UNIDPP_ISSUER_BIND", issuer_bind.as_str()),
            ("UNIDPP_ISSUER_STATE_FILE", issuer_journal.as_str()),
            ("UNIDPP_ISSUER_EVENT_SEED", LIVE_ED25519_SEED),
            ("UNIDPP_ISSUER_PACK_SEED", LIVE_P256_SEED),
        ],
    );
    wait_healthy(story, "unidpp-issuer", &format!("http://{issuer_bind}"))?;

    // 3. trust (verify anchors via GET /keyring)
    if !is_executable(&trust_bin) {
        return Err(story.abort(format!(
            "unidpp-trust binary missing: {} (run: make deps-live)",
            trust_bin.display()
        )));
    }
    let trust_journal = dir.join("trust.journal.jsonl").display().to_string();
    story.out.note(&format!(
        "starting unidpp-trust on {trust_bind} (journal: {trust_journal})"
    ));
    story.trust = story.spawn_service(
        &trust_bin,
        &[
            ("UNIDPP_TRUST_BIND", trust_bind.as_str()),
            ("UNIDPP_TRUST_STATE_FILE", trust_journal.as_str()),
            ("UNIDPP_TRUST_SIGN_SEED", LIVE_ED25519_SEED),
            ("UNIDPP_TRUST_SIGN_SEED_P256", LIVE_P256_SEED),
        ],
    );
    wait_healthy(story, "unidpp-trust", &format!("http://{trust_bind}"))?;

    // 4. log (transparency log: pack commitments -> receipts)
    if !is_executable(&log_bin) {
        return Err(story.abort(format!(
            "unidpp-log binary missing: {} (run: make deps-live)",
            log_bin.display()
        )));
    }
    let log_journal = dir.join("log.journal.jsonl").display().to_string();
    story.out.note(&format!(
        "starting unidpp-log on {log_bind} (journal: {log_journal})"
    ));
    story.log = story.spawn_service(
        &log_bin,
        &[
            ("UNIDPP_LOG_BIND", log_bind.as_str()),
            ("UNIDPP_LOG_STATE_FILE", log_journal.as_str()),
        ],
    );
    wait_healthy(story, "unidpp-log", &format!("http://{log_bind}"))?;
    Ok(())
}

/// The script's wait_healthy: 500 polls, 0.1 s apart, then the green
/// readiness line — or the loud failure.
fn wait_healthy(story: &mut Story, name: &str, url: &str) -> R<()> {
    let (host, port) = Config::split_url(url);
    if !story.wait_healthy_window(&host, port, 500, 100) {
        return Err(story.abort(format!("{name} did not become healthy on {url}")));
    }
    story.out.healthy(name, url);
    Ok(())
}

/// The script's banner between the four healthy lines and the story's
/// own preamble (the narration carries over verbatim).
pub fn banner(story: &mut Story) {
    story
        .out
        .hr("UniDPP live demo — four services up, the E8 story next");
    story
        .out
        .say(&format!("registry : http://{}", story.cfg.registry_bind));
    story
        .out
        .say(&format!("issuer   : http://{}", story.cfg.issuer_bind));
    story.out.say(&format!(
        "trust    : http://{} (verify anchor source: GET /keyring)",
        story.cfg.trust_bind
    ));
    story.out.say(&format!(
        "log      : http://{} (pack commitments -> signed receipts)",
        story.cfg.log_bind
    ));
    story
        .out
        .say("gateway  : started by demo.sh on 127.0.0.1:8398 (B-INT: UNTP render + ingest,");
    story
        .out
        .say("           issuer-upstream through the exported UNIDPP_ISSUER_URL)");
    story
        .out
        .say("anchor   : trust /keyring pins the issuer pack key (shared dev seed)");
    story.out.say(&format!(
        "journals : {}/*.journal.jsonl",
        story.cfg.live_dir.display()
    ));
    story.out.say("");
}
