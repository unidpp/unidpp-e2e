//! The demonstration engine: the narration printer (every line mirrors
//! to the console and the transcript artifact), the check tally, the
//! command runner, and the service lifecycle (spawn a sibling release
//! binary, poll for readiness, stop by PID at exit).

use crate::http;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BOLD: &str = "\u{1b}[1m";
const RESET: &str = "\u{1b}[0m";
const RED: &str = "\u{1b}[31m";
const GREEN: &str = "\u{1b}[32m";
const CYAN: &str = "\u{1b}[36m";
const YELLOW: &str = "\u{1b}[33m";

/// A failed beat aborts the whole story (the script's `fail` exits 1;
/// here the runner catches the error, stops the services, and exits 1).
pub struct Abort;

pub type R<T> = Result<T, Abort>;

/// The B4 subset exit (UNIDPP_E2E_STOP_AFTER=B4) ends the story without
/// running the beats that follow it.
pub enum Flow {
    Next,
    Subset,
}

#[derive(Default)]
pub struct Tally {
    pub total: u32,
    pub ok: u32,
    pub failed: u32,
}

/// Every printed line lands on the console and in transcript.txt — the
/// shell script teed its whole output, and the transcript doubles as the
/// assertion target for the test harness.
pub struct Out {
    transcript: File,
    /// The hook's narration goes to stderr: its stdout carries return
    /// values only (the mint verb's anchor), exactly as the sourced hook
    /// kept its notes off the captured stdout.
    to_stderr: bool,
}

impl Out {
    pub fn new(transcript_path: &Path) -> std::io::Result<Out> {
        let transcript = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(transcript_path)?;
        Ok(Out {
            transcript,
            to_stderr: false,
        })
    }

    /// The hook's console: stderr (stdout stays reserved for return
    /// values — see the field comment).
    pub fn new_err(transcript_path: &Path) -> std::io::Result<Out> {
        let out = Out::new(transcript_path)?;
        Ok(Out {
            to_stderr: true,
            ..out
        })
    }

    fn line(&mut self, text: &str) {
        self.bytes(text.as_bytes());
        self.bytes(b"\n");
    }

    fn bytes(&mut self, bytes: &[u8]) {
        if self.to_stderr {
            let mut stderr = std::io::stderr();
            let _ = stderr.write_all(bytes);
            let _ = stderr.flush();
        } else {
            let mut stdout = std::io::stdout();
            let _ = stdout.write_all(bytes);
            let _ = stdout.flush();
        }
        let _ = self.transcript.write_all(bytes);
        let _ = self.transcript.flush();
    }

    pub fn raw(&mut self, text: &str) {
        self.line(text);
    }

    /// Child-process output passes through unmodified (verify verdicts,
    /// command stderr) — no added newline, exactly as the tee did.
    pub fn raw_bytes(&mut self, bytes: &[u8]) {
        self.bytes(bytes);
    }

    pub fn hr(&mut self, text: &str) {
        self.line(&format!("\n{BOLD}{text}{RESET}"));
    }

    pub fn beat(&mut self, id: &str, title: &str) {
        self.line(&format!(
            "\n{BOLD}======================================================================{RESET}"
        ));
        self.line(&format!("{BOLD}{id} — {title}{RESET}"));
        self.line(&format!(
            "{BOLD}----------------------------------------------------------------------{RESET}"
        ));
    }

    pub fn what(&mut self, text: &str) {
        self.line(&format!("{CYAN}    what just happened: {text}{RESET}"));
    }

    pub fn say(&mut self, text: &str) {
        self.line(&format!("    {text}"));
    }

    pub fn note(&mut self, text: &str) {
        self.line(&format!("{YELLOW}    [orchestrator] {text}{RESET}"));
    }

    pub fn show(&mut self, command: &str) {
        self.line(&format!("  {GREEN}$ {command}{RESET}"));
    }

    pub fn ok(&mut self, label: &str, expected: &str) {
        self.line(&format!("  {GREEN}[ok]{RESET}   {label} == {expected}"));
    }

    pub fn fail_line(&mut self, label: &str, expected: &str, actual: &str) {
        self.line(&format!(
            "  {RED}[FAIL]{RESET} {label}: expected {expected}, got {actual}"
        ));
    }

    pub fn aborted(&mut self, message: &str) {
        self.line(&format!("\n{RED}E8 STORY ABORTED: {message}{RESET}"));
    }

    pub fn demo_failed(&mut self, failed: u32) {
        self.line(&format!(
            "{RED}DEMO FAILED ({failed} failing checks){RESET}"
        ));
    }

    pub fn demo_passed(&mut self) {
        self.line(&format!(
            "{GREEN}DEMO PASSED — all verify outcomes matched the story.{RESET}"
        ));
    }

    pub fn demo_passed_subset(&mut self) {
        self.line(&format!(
            "{GREEN}DEMO PASSED — B1-B4 subset (live smoke).{RESET}"
        ));
    }

    // -- The quickstarts' narration (the scripts' printf lines) ------------

    /// An adoption path's header: a bold `== ... ==` marker, then the
    /// path's plain-text name.
    pub fn path_header(&mut self, marker: &str, name: &str) {
        self.line(&format!("{BOLD}{marker}{RESET}  {name}"));
    }

    /// The scripts' ok(): the label alone, no expected/actual suffix.
    pub fn qs_ok(&mut self, label: &str) {
        self.line(&format!("  {GREEN}[ok]{RESET}   {label}"));
    }

    /// The scripts' bad(): the label carries its own diagnosis.
    pub fn qs_bad(&mut self, label: &str) {
        self.line(&format!("  {RED}[FAIL]{RESET} {label}"));
    }

    /// The scripts' [SKIP] line (a missing binary; the run exits 77).
    pub fn skip(&mut self, text: &str) {
        self.line(&format!("  {YELLOW}[SKIP]{RESET} {text}"));
    }

    /// The scripts' closing summary line.
    pub fn summary(&mut self, passed: u32, failed: u32) {
        self.line(&format!("\n  summary: {passed} passed, {failed} failed"));
    }

    /// The live preset's per-service readiness line (demo-live's
    /// wait_healthy print).
    pub fn healthy(&mut self, name: &str, url: &str) {
        self.line(&format!("    {GREEN}{name} healthy at {url}{RESET}"));
    }
}

pub struct Config {
    pub work_dir: PathBuf,
    pub family_dir: PathBuf,
    /// The unidpp-e2e checkout itself (the quickstarts' scratch dirs and
    /// the live dir are its build/ subdirectories).
    pub root: PathBuf,
    pub unidpp: PathBuf,
    pub registry_bin: PathBuf,
    pub registry_bind: String,
    pub registry_url_override: Option<String>,
    pub gateway_bin: PathBuf,
    pub gateway_bind: String,
    pub resolver_bin: PathBuf,
    pub resolver_bind: String,
    pub trust_bin: PathBuf,
    pub quorum_ceremony_bin: PathBuf,
    pub device_drill_bin: PathBuf,
    pub quorum_bind: String,
    pub issuer_bin: PathBuf,
    pub issuer_bind: String,
    pub issuer_url: Option<String>,
    pub issuer_admin_token: Option<String>,
    pub hub_bin: PathBuf,
    pub trust_bind: String,
    pub trust_url: Option<String>,
    pub log_bin: PathBuf,
    pub log_bind: String,
    pub log_url: Option<String>,
    pub live_dir: PathBuf,
    pub stop_after_b4: bool,
}

/// `${VAR:-default}` — the subcommands' own overrides use it too.
pub fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_opt(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

impl Config {
    /// The crate compiles inside the repo (demo/), and every sibling
    /// path the story needs is relative to that checkout — the same
    /// layout assumption the Makefile's deps targets bake in.
    pub fn from_env() -> Config {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest.parent().unwrap().to_path_buf();
        let family = root.parent().unwrap().to_path_buf();
        Config {
            work_dir: PathBuf::from(env_or(
                "UNIDPP_E2E_WORK_DIR",
                &root.join("build/e2e").display().to_string(),
            )),
            family_dir: family.clone(),
            root: root.clone(),
            unidpp: family_bin(&family, "UNIDPP_BIN", "unidpp-cli", "unidpp"),
            registry_bin: family_bin(
                &family,
                "UNIDPP_REGISTRY_BIN",
                "unidpp-registry",
                "unidpp-registry",
            ),
            registry_bind: env_or("UNIDPP_REGISTRY_BIND", "127.0.0.1:8098"),
            registry_url_override: env_opt("UNIDPP_REGISTRY_URL"),
            gateway_bin: family_bin(
                &family,
                "UNIDPP_GATEWAY_BIN",
                "unidpp-gateway",
                "unidpp-gateway",
            ),
            gateway_bind: env_or("UNIDPP_GATEWAY_BIND", "127.0.0.1:8398"),
            resolver_bin: family_bin(
                &family,
                "UNIDPP_RESOLVER_BIN",
                "unidpp-resolver",
                "unidpp-resolver",
            ),
            resolver_bind: env_or("UNIDPP_RESOLVER_BIND", "127.0.0.1:8095"),
            trust_bin: family_bin(&family, "UNIDPP_TRUST_BIN", "unidpp-trust", "unidpp-trust"),
            quorum_ceremony_bin: family_bin(
                &family,
                "UNIDPP_QUORUM_CEREMONY_BIN",
                "unidpp-trust",
                "quorum-ceremony",
            ),
            device_drill_bin: family_bin(
                &family,
                "UNIDPP_DEVICE_DRILL_BIN",
                "unidpp-signatif",
                "device-drill",
            ),
            quorum_bind: env_or("UNIDPP_QUORUM_BIND", "127.0.0.1:8097"),
            issuer_bin: family_bin(
                &family,
                "UNIDPP_ISSUER_BIN",
                "unidpp-issuer",
                "unidpp-issuer",
            ),
            issuer_bind: env_or("UNIDPP_ISSUER_BIND", "127.0.0.1:8096"),
            issuer_url: env_opt("UNIDPP_ISSUER_URL"),
            issuer_admin_token: env_opt("UNIDPP_ISSUER_ADMIN_TOKEN"),
            hub_bin: family_bin(&family, "UNIDPP_HUB_BIN", "unidpp-hub", "unidpp-hub"),
            trust_bind: env_or("UNIDPP_TRUST_BIND", "127.0.0.1:8092"),
            trust_url: env_opt("UNIDPP_TRUST_URL"),
            log_bin: family_bin(&family, "UNIDPP_LOG_BIN", "unidpp-log", "unidpp-log"),
            log_bind: env_or("UNIDPP_LOG_BIND", "127.0.0.1:8194"),
            log_url: env_opt("UNIDPP_LOG_URL"),
            live_dir: PathBuf::from(env_or(
                "UNIDPP_E2E_LIVE_DIR",
                &root.join("build/live").display().to_string(),
            )),
            stop_after_b4: env_opt("UNIDPP_E2E_STOP_AFTER").as_deref() == Some("B4"),
        }
    }

    /// (host, port) of a "host:port" bind string.
    pub fn split_bind(bind: &str) -> (String, u16) {
        let (host, port) = bind.rsplit_once(':').expect("bind is host:port");
        (
            host.to_string(),
            port.parse().expect("bind port is a number"),
        )
    }

    /// (host, port) of an "http://host:port" base URL.
    pub fn split_url(url: &str) -> (String, u16) {
        let bare = url.strip_prefix("http://").unwrap_or(url);
        let bare = bare.strip_suffix('/').unwrap_or(bare);
        Self::split_bind(bare)
    }
}

fn family_bin(family: &Path, env_key: &str, repo: &str, bin: &str) -> PathBuf {
    PathBuf::from(env_or(
        env_key,
        &family
            .join(repo)
            .join("target/release")
            .join(bin)
            .display()
            .to_string(),
    ))
}

/// Where a child command's stream goes: discarded, teed through the
/// transcript, or captured into an artifact file.
pub enum Sink {
    Null,
    Tee,
    File(PathBuf),
}

pub struct Story {
    pub cfg: Config,
    pub out: Out,
    pub tally: Tally,
    pub registry: Option<Child>,
    pub resolver: Option<Child>,
    pub gateway: Option<Child>,
    pub quorum: Option<Child>,
    /// The live preset's and the quickstarts' services (the demo story
    /// itself leaves them unset).
    pub issuer: Option<Child>,
    pub trust: Option<Child>,
    pub log: Option<Child>,
    pub hub: Option<Child>,
    pub quorum_url_override: Option<String>,
    pub trust_anchor: Option<String>,
    pub trust_key_id: String,
    pub trust_mode: String,
    pub log_id: Option<String>,
    pub subset_exit: Option<i32>,
}

impl Story {
    pub fn new(cfg: Config, out: Out) -> Story {
        Story {
            cfg,
            out,
            tally: Tally::default(),
            registry: None,
            resolver: None,
            gateway: None,
            quorum: None,
            issuer: None,
            trust: None,
            log: None,
            hub: None,
            quorum_url_override: None,
            trust_anchor: None,
            trust_key_id: String::new(),
            trust_mode: String::new(),
            log_id: None,
            subset_exit: None,
        }
    }

    pub fn artifact(&self, name: &str) -> PathBuf {
        self.cfg.work_dir.join(name)
    }

    pub fn issuer_mode(&self) -> bool {
        self.cfg.issuer_url.is_some()
    }

    /// The script's check semantics: [ok]/[FAIL], a running tally, and a
    /// non-zero exit when any check failed — a FAIL never aborts in
    /// place (the verdict mismatch paths abort separately).
    pub fn check(&mut self, label: &str, expected: &str, actual: &str) {
        self.tally.total += 1;
        if expected == actual {
            self.tally.ok += 1;
            self.out.ok(label, expected);
        } else {
            self.tally.failed += 1;
            self.out.fail_line(label, expected, actual);
        }
    }

    /// The script's fail(): print the abort banner and unwind the story.
    pub fn abort(&mut self, message: String) -> Abort {
        self.out.aborted(&message);
        Abort
    }

    /// The quickstarts' check pair: the scripts' ok()/bad() — the label
    /// alone carries the diagnosis, and the tally decides the exit.
    pub fn pass_check(&mut self, label: &str) {
        self.tally.total += 1;
        self.tally.ok += 1;
        self.out.qs_ok(label);
    }

    pub fn fail_check(&mut self, label: &str) {
        self.tally.total += 1;
        self.tally.failed += 1;
        self.out.qs_bad(label);
    }

    /// Print the abort banner, stop the services, and report exit 1.
    pub fn abort_exit(&mut self, message: String) -> i32 {
        self.out.aborted(&message);
        self.cleanup();
        1
    }

    /// The banner was already printed where the beat failed; stop the
    /// services and report exit 1.
    pub fn abort_exit_quiet(&mut self) -> i32 {
        self.cleanup();
        1
    }

    /// The script's run_quiet: show the command, silence its stdout,
    /// let its stderr through, and abort when it fails.
    pub fn run_quiet(&mut self, argv: &[&str]) -> R<()> {
        self.out.show(&argv.join(" "));
        let code = self.run_command(argv, Sink::Null, Sink::Tee);
        match code {
            Some(0) => Ok(()),
            _ => Err(self.abort(format!("command failed: {}", argv.join(" ")))),
        }
    }

    /// Run a child command and return its exit code (None when the
    /// binary could not be executed — the shell's 127).
    pub fn run_command(&mut self, argv: &[&str], stdout: Sink, stderr: Sink) -> Option<i32> {
        self.run_command_inner(None, argv, stdout, stderr)
    }

    /// Run a child command in a working directory (the foreign harness
    /// legs run from the unidpp-py checkout).
    pub fn run_command_in_dir(
        &mut self,
        dir: &Path,
        argv: &[&str],
        stdout: Sink,
        stderr: Sink,
    ) -> Option<i32> {
        self.run_command_inner(Some(dir), argv, stdout, stderr)
    }

    fn run_command_inner(
        &mut self,
        dir: Option<&Path>,
        argv: &[&str],
        stdout: Sink,
        stderr: Sink,
    ) -> Option<i32> {
        let mut command = Command::new(argv[0]);
        command.args(&argv[1..]);
        if let Some(dir) = dir {
            command.current_dir(dir);
        }
        command.stdout(stdio_for(&stdout));
        command.stderr(stdio_for(&stderr));
        let output = command.output().ok()?;
        let code = output.status.code();
        self.deliver(&stdout, &output.stdout, false);
        self.deliver(&stderr, &output.stderr, true);
        code
    }

    /// Feed a python program on stdin (the script's `python3 -` legs).
    pub fn run_python_stdin(
        &mut self,
        dir: &Path,
        script: &str,
        args: &[&str],
        stdout: Sink,
        stderr: Sink,
    ) -> Option<i32> {
        let mut command = Command::new("python3");
        command.arg("-").args(args).current_dir(dir);
        command.stdout(stdio_for(&stdout));
        command.stderr(stdio_for(&stderr));
        command.stdin(Stdio::piped());
        let mut child = command.spawn().ok()?;
        {
            let stdin = child.stdin.as_mut().expect("stdin is piped");
            let _ = stdin.write_all(script.as_bytes());
        }
        let output = child.wait_with_output().ok()?;
        let code = output.status.code();
        self.deliver(&stdout, &output.stdout, false);
        self.deliver(&stderr, &output.stderr, true);
        code
    }

    /// A captured stream reaches its sink: Null drops it, Tee prints it,
    /// File writes it — stdout truncates (the shell's `>`), stderr
    /// appends so a `> file 2>&1` pair shares one artifact.
    fn deliver(&mut self, sink: &Sink, bytes: &[u8], append: bool) {
        match sink {
            Sink::Null => {}
            Sink::Tee => self.out.raw_bytes(bytes),
            Sink::File(path) => {
                if append {
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                        let _ = file.write_all(bytes);
                    }
                } else {
                    let _ = fs::write(path, bytes);
                }
            }
        }
    }

    /// Spawn a sibling release binary with the service's own env (both
    /// streams discarded — the script redirected them to /dev/null).
    pub fn spawn_service(&mut self, bin: &Path, envs: &[(&str, &str)]) -> Option<Child> {
        Command::new(bin)
            .envs(envs.iter().copied())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()
    }

    /// Spawn with both streams landing in a log file (the quickstarts
    /// redirect each service to its own $WORK/*.log; `append` re-opens
    /// for append — the hub restart's `>>`).
    pub fn spawn_service_logged(
        &mut self,
        bin: &Path,
        envs: &[(&str, &str)],
        log: &Path,
        append: bool,
    ) -> Option<Child> {
        let file = if append {
            OpenOptions::new().create(true).append(true).open(log)
        } else {
            File::create(log)
        }
        .ok()?;
        Command::new(bin)
            .envs(envs.iter().copied())
            .stdout(Stdio::from(file.try_clone().ok()?))
            .stderr(Stdio::from(file))
            .spawn()
            .ok()
    }

    /// The script's readiness window: 250 polls, 0.2 s apart.
    pub fn wait_healthy(&self, host: &str, port: u16) -> bool {
        self.wait_healthy_window(host, port, 250, 200)
    }

    /// A readiness window at the caller's cadence (the quickstart
    /// scripts poll 50 times, 0.2 s apart; demo-live 500 times, 0.1 s).
    pub fn wait_healthy_window(&self, host: &str, port: u16, polls: usize, every_ms: u64) -> bool {
        for _ in 0..polls {
            if self.wait_healthy_probe(host, port) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(every_ms));
        }
        false
    }

    /// One healthz probe (the already-running checks poll once).
    pub fn wait_healthy_probe(&self, host: &str, port: u16) -> bool {
        http::get(host, port, "/healthz", Duration::from_secs(2)).is_some_and(|r| r.healthy())
    }

    /// Stop a service by PID (SIGTERM through /bin/kill — std's kill is
    /// SIGKILL, and the journals deserve a clean shutdown), then reap it.
    pub fn stop_process(child: &mut Option<Child>) {
        if let Some(process) = child {
            let pid = process.id();
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = process.wait();
        }
        *child = None;
    }

    /// The EXIT trap's order: gateway, resolver, registry, quorum — then
    /// the live preset's and the quickstarts' services (issuer, trust,
    /// log, hub). The shell scripts kill all their PIDs and then wait
    /// for all of them; stop-by-PID in sequence has the same effect
    /// (SIGTERM, then reap), no service depending on another's shutdown.
    pub fn cleanup(&mut self) {
        Self::stop_process(&mut self.gateway);
        Self::stop_process(&mut self.resolver);
        Self::stop_process(&mut self.registry);
        Self::stop_process(&mut self.quorum);
        Self::stop_process(&mut self.issuer);
        Self::stop_process(&mut self.trust);
        Self::stop_process(&mut self.log);
        Self::stop_process(&mut self.hub);
    }
}

fn stdio_for(sink: &Sink) -> Stdio {
    match sink {
        Sink::Null => Stdio::null(),
        Sink::Tee | Sink::File(_) => Stdio::piped(),
    }
}

/// The script's `grep -c needle | sed 1->ok, 0->failed` idiom: one
/// matching line is the pass verdict, none is the failure, and any
/// other count reports itself (as the sed did, unhelpfully).
pub fn grep_verdict(text: &str, needle: &str) -> String {
    let count = text.lines().filter(|l| l.contains(needle)).count();
    match count {
        0 => "failed".to_string(),
        1 => "ok".to_string(),
        n => n.to_string(),
    }
}

pub fn grep_verdict_ci(text: &str, needle: &str) -> String {
    let lower = needle.to_ascii_lowercase();
    let count = text
        .lines()
        .filter(|l| l.to_ascii_lowercase().contains(&lower))
        .count();
    match count {
        0 => "failed".to_string(),
        1 => "ok".to_string(),
        n => n.to_string(),
    }
}

pub fn tail_line(text: &str) -> String {
    text.lines().last().unwrap_or("").to_string()
}

/// The first n lines of a file (the scripts' `sed -n '1,8p'` diagnostic).
pub fn head_lines(path: &Path, n: usize) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .take(n)
        .map(str::to_string)
        .collect()
}

/// The last n lines of a file (the scripts' `tail -5` diagnostic).
pub fn tail_lines(path: &Path, n: usize) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].iter().map(|l| (*l).to_string()).collect()
}

/// The scripts' `grep -oE '[0-9a-f]{130}' | head -1`: the first run of
/// at least `len` lowercase hex characters, cut at `len` (the pack
/// mint's derived anchor on stderr).
pub fn first_hex_run(text: &str, len: usize) -> Option<String> {
    let bytes = text.as_bytes();
    let is_hex = |b: u8| matches!(b, b'0'..=b'9' | b'a'..=b'f');
    let mut run = 0;
    for (i, b) in bytes.iter().enumerate() {
        if is_hex(*b) {
            run += 1;
            if run == len {
                let start = i + 1 - len;
                return Some(text[start..start + len].to_string());
            }
        } else {
            run = 0;
        }
    }
    None
}

/// `date -u +%Y-%m-%dT%H:%M:%SZ` without a time crate: civil-from-days
/// over the Unix epoch (Howard Hinnant's algorithm).
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after the epoch")
        .as_secs();
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// `command -v` over the PATH entries.
pub fn which(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

/// The script's `[ -x ]` (a file carrying an execute bit).
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}
