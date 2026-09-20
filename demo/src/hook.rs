//! The issuance driver (the issuer-hook port) as subcommands: `mode`
//! (the driver detection), `start` / `stop` (the service lifecycle),
//! and the three verbs every consumer of the hook calls — `create`,
//! `event`, `pack`. The verbs ARE the story's issuer methods, so the
//! hook subcommand and the demonstration cannot drift. Narration goes
//! to stderr; stdout carries return values only (the mint verb's
//! anchor), exactly as the sourced hook kept its notes off the captured
//! stdout.

use crate::engine::{is_executable, Abort, Config, Out, Story};
use crate::story::CreateSpec;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

/// Where a started issuer's PID waits for `hook stop` (the sourced hook
/// held ISSUER_PID in shell state; a subcommand needs the file).
const PID_FILE: &str = "/tmp/unidpp-issuer.pid";

/// The readiness window of the hook's maybe_start_issuer (50 × 0.2 s).
const START_POLLS: usize = 50;
const START_EVERY_MS: u64 = 200;

/// Run one hook verb; the return value is the process exit code.
pub fn run(args: &[String]) -> i32 {
    let Some(verb) = args.first().map(String::as_str) else {
        eprintln!("unidpp-demo: hook needs a verb (mode | start | stop | create | event | pack)");
        return 2;
    };
    if matches!(verb, "mode" | "start" | "stop") && args.len() > 1 {
        eprintln!("unidpp-demo: hook {verb} takes no arguments");
        return 2;
    }

    let cfg = Config::from_env();
    if let Err(e) = fs::create_dir_all(&cfg.work_dir) {
        eprintln!("cannot create work dir {}: {e}", cfg.work_dir.display());
        return 1;
    }
    let out = match Out::new_err(&cfg.work_dir.join("hook-transcript.txt")) {
        Ok(out) => out,
        Err(e) => {
            eprintln!("cannot open transcript: {e}");
            return 1;
        }
    };
    let mut story = Story::new(cfg, out);
    let result = match verb {
        "mode" => {
            println!("{}", if story.issuer_mode() { "issuer" } else { "cli" });
            Ok(())
        }
        "start" => driver_start(&mut story),
        "stop" => driver_stop(&mut story),
        "create" => verb_create(&mut story, &args[1..]),
        "event" => verb_event(&mut story, &args[1..]),
        "pack" => verb_pack(&mut story, &args[1..]),
        _ => {
            eprintln!("unidpp-demo: unknown hook verb '{verb}' (mode | start | stop | create | event | pack)");
            return 2;
        }
    };
    match result {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// maybe_start_issuer: in cli mode a no-op (the CLI driver needs no
/// service); in issuer mode an external issuer is used as found, and an
/// explicit UNIDPP_ISSUER_URL with nothing listening starts the binary.
fn driver_start(story: &mut Story) -> Result<(), Abort> {
    if !story.issuer_mode() {
        return Ok(());
    }
    let url = story.cfg.issuer_url.clone().unwrap_or_default();
    let (host, port) = Config::split_url(&url);
    if story.wait_healthy_probe(&host, port) {
        story.out.note(&format!("using external issuer at {url}"));
        return Ok(());
    }
    // The hook's two-step: the release binary, else the debug twin (a
    // deployment mid-build keeps working).
    let mut bin = story.cfg.issuer_bin.clone();
    if !is_executable(&bin) {
        let debug = debug_twin(&bin);
        if is_executable(&debug) {
            bin = debug;
        } else {
            return Err(story.abort(format!(
                "issuer binary missing: {} (UNIDPP_ISSUER_URL was set)",
                bin.display()
            )));
        }
    }
    let bind = story.cfg.issuer_bind.clone();
    story.out.note(&format!("starting unidpp-issuer on {bind}"));
    // The child is detached: this process exits after the readiness
    // poll, and the PID file is how `hook stop` finds it later.
    let child = Command::new(&bin)
        .env("UNIDPP_ISSUER_BIND", &bind)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let pid = match child {
        Ok(child) => child.id(),
        Err(e) => {
            return Err(story.abort(format!("could not start {}: {e}", bin.display())));
        }
    };
    let _ = fs::write(PID_FILE, pid.to_string());
    for _ in 0..START_POLLS {
        if story.wait_healthy_probe(&host, port) {
            story.out.say(&format!("unidpp-issuer healthy at {url}"));
            return Ok(());
        }
        sleep(Duration::from_millis(START_EVERY_MS));
    }
    // The trap killed the failed child; so does this.
    let _ = Command::new("kill").arg(pid.to_string()).status();
    let _ = fs::remove_file(PID_FILE);
    Err(story.abort(format!("issuer did not become healthy on {url}")))
}

/// stop_issuer: kill the recorded PID, then poll the port down (this
/// process is not the child's parent, so there is nothing to reap).
fn driver_stop(story: &mut Story) -> Result<(), Abort> {
    let Ok(pid_text) = fs::read_to_string(PID_FILE) else {
        return Ok(());
    };
    let Ok(pid) = pid_text.trim().parse::<u32>() else {
        let _ = fs::remove_file(PID_FILE);
        return Ok(());
    };
    let _ = Command::new("kill").arg(pid.to_string()).status();
    let url = story
        .cfg
        .issuer_url
        .clone()
        .unwrap_or_else(|| format!("http://{}", story.cfg.issuer_bind));
    let (host, port) = Config::split_url(&url);
    for _ in 0..25 {
        if !story.wait_healthy_probe(&host, port) {
            break;
        }
        sleep(Duration::from_millis(200));
    }
    let _ = fs::remove_file(PID_FILE);
    Ok(())
}

/// issuer_create <id> <type-ref|-> <capability> <eo> <resolver>
///               <passport-urn> <out-file> [config]
fn verb_create(story: &mut Story, a: &[String]) -> Result<(), Abort> {
    if a.len() < 7 {
        eprintln!(
            "usage: unidpp-demo hook create <id> <type-ref|-> <capability> <eo> <resolver> <passport-urn> <out-file> [config]"
        );
        return Err(Abort);
    }
    let config = a
        .get(7)
        .filter(|c| !c.is_empty() && c.as_str() != "-")
        .cloned();
    story.issuer_create(&CreateSpec {
        id: a[0].clone(),
        type_ref: a[1].clone(),
        capability: a[2].clone(),
        eo: a[3].clone(),
        resolver: a[4].clone(),
        urn: a[5].clone(),
        out: PathBuf::from(&a[6]),
        config,
    })
}

/// issuer_event <passport-file> <event-type> <data-json> [actor] [role] [at]
fn verb_event(story: &mut Story, a: &[String]) -> Result<(), Abort> {
    if a.len() < 3 {
        eprintln!(
            "usage: unidpp-demo hook event <passport-file> <event-type> <data-json> [actor] [role] [at]"
        );
        return Err(Abort);
    }
    let empty = String::new();
    let actor = a.get(3).unwrap_or(&empty);
    let role = a.get(4).unwrap_or(&empty);
    let at = a.get(5).unwrap_or(&empty);
    story.issuer_event(Path::new(&a[0]), &a[1], &a[2], actor, role, at)
}

/// issuer_mint_pack <passport-file> <out-file> — prints the signing
/// anchor (public key, hex) on stdout.
fn verb_pack(story: &mut Story, a: &[String]) -> Result<(), Abort> {
    if a.len() < 2 {
        eprintln!("usage: unidpp-demo hook pack <passport-file> <out-file>");
        return Err(Abort);
    }
    let anchor = story.issuer_mint_pack(Path::new(&a[0]), Path::new(&a[1]))?;
    println!("{anchor}");
    Ok(())
}

/// target/release/<bin> -> target/debug/<bin> (the hook's fallback).
fn debug_twin(bin: &Path) -> PathBuf {
    PathBuf::from(
        bin.to_string_lossy()
            .replace("/target/release/", "/target/debug/"),
    )
}
