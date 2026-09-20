//! The harness engine: the running tally with the shell harness's exact
//! printer shapes (the ledger compares the counts, so the lines stay
//! byte-for-byte), the ANSI strip that makes the demo transcripts stable
//! pattern targets, the child-process redirect forms the shell used, and
//! the service lifecycle of the tenant-isolation leg.

use crate::http;
use crate::pattern::Pattern;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const BOLD: &str = "\u{1b}[1m";
const GREEN: &str = "\u{1b}[32m";
const RED: &str = "\u{1b}[31m";
const YELLOW: &str = "\u{1b}[33m";
const RESET: &str = "\u{1b}[0m";

/// The shell's curl had no `-m`, so the story's 30 s window is the
/// harness's too.
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Harness {
    /// The unidpp-e2e checkout (the crate compiles inside it, as the
    /// demo crate does — the same layout assumption the Makefile's deps
    /// targets bake in).
    pub root: PathBuf,
    /// The sibling-repos parent (`unidpp-cli`, `unidpp-registry`, …).
    pub family: PathBuf,
    pub test_work: PathBuf,
    pub unidpp: PathBuf,
    pub demo_bin: PathBuf,
    pub pass: u32,
    pub fail: u32,
    pub skipped: u32,
}

impl Harness {
    pub fn from_env() -> Harness {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest.parent().unwrap().to_path_buf();
        let family = root.parent().unwrap().to_path_buf();
        Harness {
            test_work: PathBuf::from(env_or(
                "UNIDPP_E2E_TEST_WORK",
                &root.join("build/test").display().to_string(),
            )),
            unidpp: PathBuf::from(env_or(
                "UNIDPP_BIN",
                &family
                    .join("unidpp-cli/target/release/unidpp")
                    .display()
                    .to_string(),
            )),
            demo_bin: PathBuf::from(env_or(
                "UNIDPP_DEMO_BIN",
                &root
                    .join("demo/target/release/unidpp-demo")
                    .display()
                    .to_string(),
            )),
            root,
            family,
            pass: 0,
            fail: 0,
            skipped: 0,
        }
    }

    /// The story's artifact dir — the same default the demo binary uses
    /// (the shell harness hardcoded `build/e2e`; honoring the one env
    /// contract on both the write and the read side keeps an overridden
    /// run coherent instead of silently asserting stale artifacts).
    pub fn e2e_work_dir(&self) -> PathBuf {
        PathBuf::from(env_or(
            "UNIDPP_E2E_WORK_DIR",
            &self.root.join("build/e2e").display().to_string(),
        ))
    }

    pub fn leg_header(&self, number: u8, label: &str) {
        line(&format!("\n{BOLD}== test {number} =={RESET}  {label}"));
    }

    /// A silent `pass += n` (the bench and hub legs credit their
    /// child's own checks wholesale, exactly as the shell did).
    pub fn credit(&mut self, checks: u32) {
        self.pass += checks;
    }

    pub fn count_ok(&mut self, desc: &str) {
        line(&format!("  {GREEN}[ok]{RESET}   {desc}"));
        self.pass += 1;
    }

    pub fn count_fail(&mut self, desc: &str) {
        line(&format!("  {RED}[FAIL]{RESET} {desc}"));
        self.fail += 1;
    }

    /// The shell's `assert <description> <expected> <actual>`.
    pub fn assert(&mut self, desc: &str, expected: &str, actual: &str) {
        if expected == actual {
            self.count_ok(desc);
        } else {
            self.count_fail(&format!(
                "{desc}: expected {}, got {}",
                shell_q(expected),
                shell_q(actual)
            ));
        }
    }

    /// The shell's `assert_grep <description> <text> <pattern>`: any
    /// matching line passes; none fails with the pattern named.
    pub fn assert_grep(&mut self, text: &str, pattern: &str, desc: &str) {
        if Pattern::new(pattern).matches_any_line(text) {
            self.count_ok(desc);
        } else {
            self.count_fail(&format!("{desc}: pattern not found: {pattern}"));
        }
    }

    /// The shell's `[SKIP]` line — a binary that could not be produced
    /// is a sibling mid-edit, never a failure of the leg itself.
    pub fn skip(&mut self, text: &str) {
        line(&format!("  {YELLOW}[SKIP]{RESET} {text}"));
        self.skipped += 1;
    }

    /// A colored `[FAIL]` line that carries its own diagnosis (the
    /// shell's one-off printf forms outside the assert pair).
    pub fn fail_line(&mut self, text: &str) {
        line(&format!("  {RED}[FAIL]{RESET} {text}"));
        self.fail += 1;
    }

    pub fn say(&self, text: &str) {
        line(text);
    }

    pub fn summary(&self) {
        line(&format!(
            "\n{BOLD}== summary =={RESET}  {} passed, {} failed, {} skipped",
            self.pass, self.fail, self.skipped
        ));
    }

    // -- child processes (the shell's redirect forms) --------------------

    /// `> out 2> err` — the demo legs keep the two streams apart so a
    /// failure can show stderr's tail.
    pub fn run_split(&self, argv: &[&str], out: &Path, err: &Path) -> Option<i32> {
        let output = command(argv).output().ok()?;
        let _ = fs::write(out, &output.stdout);
        let _ = fs::write(err, &output.stderr);
        output.status.code()
    }

    /// Like `run_split`, with extra env (the live leg pins the demo's
    /// live dir and its B4 stop).
    pub fn run_split_env(
        &self,
        argv: &[&str],
        envs: &[(&str, &str)],
        out: &Path,
        err: &Path,
    ) -> Option<i32> {
        let mut child = command(argv);
        child.envs(envs.iter().copied());
        let output = child.output().ok()?;
        let _ = fs::write(out, &output.stdout);
        let _ = fs::write(err, &output.stderr);
        output.status.code()
    }

    /// `> file 2>&1` — the quickstart legs keep the child's whole
    /// console in one artifact and then cat it.
    pub fn run_merged(&self, argv: &[&str], out: &Path) -> Option<i32> {
        let output = command(argv).output().ok()?;
        if let Ok(mut file) = File::create(out) {
            let _ = file.write_all(&output.stdout);
            let _ = file.write_all(&output.stderr);
        }
        output.status.code()
    }

    /// The conform runs execute from the e2e checkout (the f5 claim's
    /// `..` argument names the family dir relative to it).
    pub fn run_merged_in(&self, cwd: &Path, argv: &[&str], out: &Path) -> Option<i32> {
        let mut child = command(argv);
        child.current_dir(cwd);
        let output = child.output().ok()?;
        if let Ok(mut file) = File::create(out) {
            let _ = file.write_all(&output.stdout);
            let _ = file.write_all(&output.stderr);
        }
        output.status.code()
    }

    /// No redirects — the child shares the harness console (the bench
    /// and the hub quickstart narrate their own checks inline).
    pub fn run_inherit(&self, argv: &[&str]) -> Option<i32> {
        command(argv).status().ok().and_then(|status| status.code())
    }

    /// `>/dev/null 2>&1` — only the exit code matters.
    pub fn run_null(&self, argv: &[&str]) -> Option<i32> {
        command(argv)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()
            .and_then(|status| status.code())
    }
}

fn command(argv: &[&str]) -> Command {
    let mut child = Command::new(argv[0]);
    child.args(&argv[1..]);
    child
}

/// `${VAR:-default}` over strings, as every family crate reads it.
pub fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn line(text: &str) {
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.write_all(b"\n");
    let _ = stdout.flush();
}

/// The shell's `cat` — bytes to the console, no interpretation.
pub fn cat(path: &Path) {
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(&fs::read(path).unwrap_or_default());
    let _ = stdout.flush();
}

pub fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// The last `n` lines of a file (the shell's `tail -n` diagnostics).
pub fn tail_lines(path: &Path, n: usize) -> Vec<String> {
    let text = read(path);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].iter().map(|l| (*l).to_string()).collect()
}

/// The shell's `[ -x ]` (a file carrying an execute bit).
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// The sed line that made the transcripts greppable: strip ANSI escape
/// sequences (`ESC [ <digits/semicolons> <letter>`) — the colored file
/// would otherwise leak control bytes into the assertion targets.
pub fn strip_ansi(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'[') {
            let mut j = i + 2;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b';') {
                j += 1;
            }
            if j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                i = j + 1;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// bash's `%q`: bare for the plain tokens this harness compares, quoted
/// for anything else.
fn shell_q(s: &str) -> String {
    let plain = !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@%+=:,./_-".contains(&b));
    if plain {
        s.to_string()
    } else {
        format!("{s:?}")
    }
}

/// The shell's `trap`-guarded service: spawn a sibling release binary
/// with its own env and both streams in a log file.
pub fn spawn_logged(bin: &Path, envs: &[(&str, &str)], log: &Path) -> Option<Child> {
    let file = File::create(log).ok()?;
    Command::new(bin)
        .envs(envs.iter().copied())
        .stdout(Stdio::from(file.try_clone().ok()?))
        .stderr(Stdio::from(file))
        .spawn()
        .ok()
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

/// The shell's readiness window: 250 polls, 0.2 s apart.
pub fn wait_healthy(host: &str, port: u16) -> bool {
    for _ in 0..250 {
        if http::get(host, port, "/healthz", Duration::from_secs(2)).is_some_and(|r| r.healthy()) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}
