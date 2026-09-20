//! The demonstration entry point: the preamble (environment, services,
//! topology), the ordered beat table, and the final tally. The story
//! itself lives in beats.rs; the engine in engine.rs.

mod beats;
mod engine;
mod http;
mod json;
mod payloads;
mod story;

use engine::{now_iso, which, Flow, Out, Story};
use std::fs;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let cfg = engine::Config::from_env();
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

    story.out.raw(&format!(
        "unidpp-demo: {} beats registered (the ordered table)",
        beats::BEATS.len()
    ));

    if !engine::is_executable(&story.cfg.unidpp) {
        return story.abort_exit(format!(
            "unidpp CLI missing: {} (run: make deps)",
            story.cfg.unidpp.display()
        ));
    }
    if which("python3").is_none() {
        return story.abort_exit("python3 is required".to_string());
    }

    story
        .out
        .hr("UniDPP end-to-end — the Momiji Mobility E8 (STORY.md beats B1-B10)");
    story.out.say(&format!("run:      {}", now_iso()));
    story.out.say("story:    UniDPP E8 exemplar (beats B1-B10)");
    story
        .out
        .say(&format!("artifacts: {}", story.cfg.work_dir.display()));

    if story.issuer_mode() {
        story.out.say(&format!(
            "issuance: unidpp-issuer service  at {}",
            story.cfg.issuer_url.clone().unwrap_or_default()
        ));
    } else {
        story
            .out
            .say("issuance: unidpp-cli (local driver) — set UNIDPP_ISSUER_URL or run");
        story
            .out
            .say("make demo-live to issue through the unidpp-issuer service");
    }

    if story.start_registry().is_err() {
        return story.abort_exit_quiet();
    }
    if story.pin_trust_anchor().is_err() {
        return story.abort_exit_quiet();
    }
    if story.probe_log().is_err() {
        return story.abort_exit_quiet();
    }

    // Live-service wiring: name every dependency up front when any live
    // URL is present, so the transcript states the topology.
    let any_live = story.cfg.issuer_url.is_some()
        || story.cfg.trust_url.is_some()
        || story.cfg.log_url.is_some();
    if any_live {
        let full =
            story.cfg.trust_url.is_some() && story.cfg.log_url.is_some() && story.issuer_mode();
        if full {
            story
                .out
                .say("topology: LIVE — four sibling services (started by scripts/demo-live.sh)");
        } else {
            story
                .out
                .say("topology: LIVE (partial — live service URLs detected)");
        }
        story.out.say(&format!(
            "  registry : {} (items, applicability, transforms)",
            story.registry_url()
        ));
        if story.issuer_mode() {
            story.out.say(&format!(
                "  issuer   : {} (server-signed events, server-minted packs)",
                story.cfg.issuer_url.clone().unwrap_or_default()
            ));
        }
        if let Some(trust) = &story.cfg.trust_url {
            story.out.say(&format!(
                "  trust    : {trust} (verify anchor source: GET /keyring)"
            ));
        }
        if let Some(log) = &story.cfg.log_url {
            story.out.say(&format!(
                "  log      : {log} (pack commitments -> signed receipts)"
            ));
        }
    }

    for (name, beat) in beats::BEATS {
        story.out.raw(&format!("== [{name}]"));
        match beat(&mut story) {
            Ok(Flow::Next) => {}
            Ok(Flow::Subset) => {
                story.cleanup();
                return story.subset_exit.unwrap_or(0);
            }
            Err(_) => {
                story.cleanup();
                return 1;
            }
        }
    }

    story.out.hr("STORY COMPLETE — B1 through B10");
    story.out.say(&format!(
        "checks:   {}/{} passed",
        story.tally.ok, story.tally.total
    ));
    story.out.say(&format!(
        "artifacts: {} (passports, packs, registry responses)",
        story.cfg.work_dir.display()
    ));
    if story.cfg.log_url.is_some() {
        let receipts = story.cfg.work_dir.join("log-receipts");
        let count = fs::read_dir(&receipts)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|e| e.file_name().to_string_lossy().ends_with(".receipt.json"))
                    .count()
            })
            .unwrap_or(0);
        story.out.say(&format!(
            "log:      {count} signed inclusion receipts in {}",
            receipts.display()
        ));
    }
    if story.tally.failed > 0 || story.tally.ok != story.tally.total {
        story.out.demo_failed(story.tally.failed);
        story.cleanup();
        return 1;
    }
    story.out.demo_passed();
    story.cleanup();
    0
}
