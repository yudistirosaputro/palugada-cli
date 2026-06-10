//! `palugada exec <verb>` — the execution toolbelt.
//!
//! Verbs are DATA: `exec:` maps in the bound profile's `profile.yaml` and in
//! the repo's `.palugada/config.yaml` (project wins per-verb). The same verbs
//! (build/test/run/...) thus mean the right thing in every repo — android-cli
//! and gradle on Android, npm on web — and any AI CLI can branch on the exit
//! code or the `--json` outcome.

use crate::config::{ProjectConfig, VerbSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Deserialize, Default)]
struct ProfileExec {
    #[serde(default)]
    exec: BTreeMap<String, VerbSpec>,
}

/// Merge exec verbs: profile first, project `.palugada/config.yaml` overrides
/// per-verb. `kn`/`profile` may be absent — project-only verbs still work.
pub fn merged_verbs(
    kn: Option<&Path>,
    profile: &str,
    repo: &Path,
) -> Result<BTreeMap<String, (VerbSpec, &'static str)>, String> {
    let mut out: BTreeMap<String, (VerbSpec, &'static str)> = BTreeMap::new();
    if let Some(kn) = kn {
        if !profile.is_empty() {
            let pf_path = kn.join("profiles").join(profile).join("profile.yaml");
            if let Ok(raw) = fs::read_to_string(&pf_path) {
                let pf: ProfileExec = serde_yaml::from_str(&raw)
                    .map_err(|e| format!("parse {}: {e}", pf_path.display()))?;
                for (k, v) in pf.exec {
                    out.insert(k, (v, "profile"));
                }
            }
        }
    }
    let repo_str = repo.to_string_lossy();
    if let Ok(pc) = ProjectConfig::load_from(&repo_str) {
        for (k, v) in pc.exec {
            out.insert(k, (v, "project"));
        }
    }
    Ok(out)
}

/// Parse `k=v` CLI args into a substitution map. Keys are [a-z0-9_-].
pub fn parse_kv_args(args: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    for a in args {
        let (k, v) = a
            .split_once('=')
            .ok_or_else(|| format!("expected key=value, got '{a}'"))?;
        let ok = !k.is_empty()
            && k.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if !ok {
            return Err(format!("invalid placeholder key '{k}' — use [a-z0-9_-]"));
        }
        map.insert(k.to_string(), v.to_string());
    }
    Ok(map)
}

/// Substitute `{key}` placeholders. Every placeholder must have a value;
/// otherwise error listing exactly what to pass.
pub fn substitute(template: &str, args: &BTreeMap<String, String>) -> Result<String, String> {
    let re = regex::Regex::new(r"\{([a-z0-9_-]+)\}").unwrap();
    let mut missing: Vec<String> = Vec::new();
    let out = re
        .replace_all(template, |caps: &regex::Captures| {
            let key = &caps[1];
            match args.get(key) {
                Some(v) => v.clone(),
                None => {
                    missing.push(key.to_string());
                    String::new()
                }
            }
        })
        .to_string();
    if !missing.is_empty() {
        missing.sort();
        missing.dedup();
        return Err(format!(
            "command `{template}` needs value(s) for: {} — pass them as `palugada exec <verb> {}`",
            missing.join(", "),
            missing
                .iter()
                .map(|m| format!("{m}=<value>"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct ExecOutcome {
    pub verb: String,
    pub command: String,
    pub exit_code: i32,
    pub duration_ms: u128,
    pub tail: String,
}

pub struct ExecRequest<'a> {
    pub verb: &'a str,
    pub args: &'a BTreeMap<String, String>,
    /// JSON mode captures output (for `tail`); text mode streams it live.
    pub json: bool,
}

/// Run a verb's command(s) sequentially, stopping at the first failure.
/// Returns the outcome; the CALLER decides the process exit code.
pub fn run_verb(
    verbs: &BTreeMap<String, (VerbSpec, &'static str)>,
    repo: &Path,
    req: &ExecRequest,
) -> Result<ExecOutcome, String> {
    let (spec, _src) = verbs.get(req.verb).ok_or_else(|| {
        let have: Vec<&str> = verbs.keys().map(String::as_str).collect();
        format!(
            "no exec verb '{}' — available: {} (define it under `exec:` in .palugada/config.yaml or the profile)",
            req.verb,
            if have.is_empty() { "(none)".to_string() } else { have.join(", ") }
        )
    })?;
    let timeout = Duration::from_secs(spec.timeout_secs());
    let mut all_out = String::new();
    let mut ran: Vec<String> = Vec::new();
    let started = Instant::now();
    let mut exit_code = 0i32;
    for raw in spec.commands() {
        let cmd_str = substitute(&raw, req.args)?;
        if !req.json {
            eprintln!("$ {cmd_str}");
        }
        ran.push(cmd_str.clone());
        let code = run_one(&cmd_str, repo, timeout, req.json, &mut all_out)?;
        if code != 0 {
            exit_code = code;
            break;
        }
    }
    Ok(ExecOutcome {
        verb: req.verb.to_string(),
        command: ran.join(" && "),
        exit_code,
        duration_ms: started.elapsed().as_millis(),
        tail: tail_lines(&all_out, 40),
    })
}

/// Run one shell command with output captured into `out_buf` (used by doctor).
pub fn run_one_captured(
    cmd_str: &str,
    repo: &Path,
    timeout: Duration,
    out_buf: &mut String,
) -> Result<i32, String> {
    run_one(cmd_str, repo, timeout, true, out_buf)
}

fn run_one(
    cmd_str: &str,
    repo: &Path,
    timeout: Duration,
    capture: bool,
    out_buf: &mut String,
) -> Result<i32, String> {
    // timeout_secs == 0 means unlimited; cap at 24 h to avoid blocking forever.
    let timeout = if timeout.is_zero() { Duration::from_secs(60 * 60 * 24) } else { timeout };
    #[allow(unused_mut)]
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(cmd_str).current_dir(repo);
    // Put the child in its own process group so that killing the group on
    // timeout also reaps any grandchildren (e.g. backgrounded `sleep 30`).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        cmd.process_group(0);
    }
    if capture {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(|e| format!("spawn `{cmd_str}`: {e}"))?;
    let mut readers = Vec::new();
    if capture {
        if let Some(mut p) = child.stdout.take() {
            readers.push(std::thread::spawn(move || {
                let mut b = String::new();
                let _ = p.read_to_string(&mut b);
                b
            }));
        }
        if let Some(mut p) = child.stderr.take() {
            readers.push(std::thread::spawn(move || {
                let mut b = String::new();
                let _ = p.read_to_string(&mut b);
                b
            }));
        }
    }
    let start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| format!("wait `{cmd_str}`: {e}"))? {
            Some(s) => break Some(s),
            None if start.elapsed() >= timeout => {
                // Kill the whole process group so descendant processes that are
                // still holding the stdout/stderr pipes are also terminated.
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                #[cfg(not(unix))]
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    for r in readers {
        if let Ok(s) = r.join() {
            out_buf.push_str(&s);
        }
    }
    match status {
        Some(s) => Ok(s.code().unwrap_or(1)),
        None => {
            out_buf.push_str(&format!("\n(timed out after {}s — killed)\n", timeout.as_secs()));
            Ok(124)
        }
    }
}

/// Last `n` lines of `s`.
pub fn tail_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verbs(yaml: &str) -> BTreeMap<String, (VerbSpec, &'static str)> {
        let m: BTreeMap<String, VerbSpec> = serde_yaml::from_str(yaml).unwrap();
        m.into_iter().map(|(k, v)| (k, (v, "project"))).collect()
    }

    #[test]
    fn substitute_fills_and_reports_missing() {
        let mut args = BTreeMap::new();
        args.insert("apk".to_string(), "out/app.apk".to_string());
        assert_eq!(
            substitute("android run --apks={apk}", &args).unwrap(),
            "android run --apks=out/app.apk"
        );
        let err = substitute("run {apk} {device}", &BTreeMap::new()).unwrap_err();
        assert!(err.contains("apk") && err.contains("device"), "{err}");
    }

    #[test]
    fn parse_kv_args_validates_keys() {
        let ok = parse_kv_args(&["apk=a.apk".into(), "out-file=x.png".into()]).unwrap();
        assert_eq!(ok["apk"], "a.apk");
        assert!(parse_kv_args(&["noequals".into()]).is_err());
        assert!(parse_kv_args(&["BAD=1".into()]).is_err());
    }

    #[test]
    fn run_verb_captures_tail_and_exit_codes() {
        let repo = tempfile::tempdir().unwrap();
        let v = verbs("ok: \"echo hello\"\nfail: \"echo boom; exit 3\"\nseq: { cmd: [\"echo one\", \"exit 2\", \"echo never\"] }\n");
        let args = BTreeMap::new();
        let r = run_verb(&v, repo.path(), &ExecRequest { verb: "ok", args: &args, json: true }).unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.tail.contains("hello"));
        let r = run_verb(&v, repo.path(), &ExecRequest { verb: "fail", args: &args, json: true }).unwrap();
        assert_eq!(r.exit_code, 3);
        assert!(r.tail.contains("boom"));
        // list form stops at the first failure
        let r = run_verb(&v, repo.path(), &ExecRequest { verb: "seq", args: &args, json: true }).unwrap();
        assert_eq!(r.exit_code, 2);
        assert!(r.tail.contains("one") && !r.tail.contains("never"));
        assert_eq!(r.command, "echo one && exit 2");
        // unknown verb lists what exists
        let err = run_verb(&v, repo.path(), &ExecRequest { verb: "nope", args: &args, json: true }).unwrap_err();
        assert!(err.contains("fail, ok, seq"), "{err}");
    }

    #[test]
    fn run_verb_times_out_with_124() {
        let repo = tempfile::tempdir().unwrap();
        let v = verbs("sleepy: { cmd: \"sleep 5\", timeout_secs: 1 }\n");
        let args = BTreeMap::new();
        let r = run_verb(&v, repo.path(), &ExecRequest { verb: "sleepy", args: &args, json: true }).unwrap();
        assert_eq!(r.exit_code, 124);
        assert!(r.tail.contains("timed out"));
    }

    #[test]
    fn tail_lines_keeps_last_n() {
        let s = (1..=50).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let t = tail_lines(&s, 40);
        assert!(t.starts_with("11") && t.ends_with("50"));
    }

    /// Regression: duplicate placeholder keys in a template (`{x}` appears
    /// twice) must be reported exactly once — `missing.sort(); missing.dedup()`
    /// is required because `dedup` alone only removes adjacent duplicates.
    #[test]
    fn substitute_deduplicates_non_adjacent_missing_keys() {
        let err = substitute("run {x} {y} {x}", &BTreeMap::new()).unwrap_err();
        // Each key must appear exactly once in the key list.
        assert!(err.contains("x") && err.contains("y"), "missing key list: {err}");
        // The hint at the end must not contain a duplicate "x=<value>" entry.
        assert!(
            !err.contains("x=<value> y=<value> x"),
            "duplicate key in hint: {err}"
        );
        // Count occurrences of "x=" to confirm dedup worked (should be exactly 1).
        let count = err.matches("x=").count();
        assert_eq!(count, 1, "expected 'x=' once in error, got {count}: {err}");
    }

    /// Regression: when a command backgrounds children that hold the pipes
    /// (e.g. `echo started; sleep 30 & sleep 30`), killing only the `sh`
    /// process left the capture threads blocked on `read_to_string`.  Now we
    /// kill the whole process group so the timeout is actually honoured.
    #[cfg(unix)]
    #[test]
    fn orphan_descendants_do_not_block_timeout() {
        let repo = tempfile::tempdir().unwrap();
        // The shell forks two long-running sleeps into the background and
        // foreground; the whole group must be dead within the 1-second timeout.
        let v = verbs(
            "orphan: { cmd: \"echo started; sleep 30 & sleep 30\", timeout_secs: 1 }\n",
        );
        let args = BTreeMap::new();
        let start = std::time::Instant::now();
        let r = run_verb(
            &v,
            repo.path(),
            &ExecRequest { verb: "orphan", args: &args, json: true },
        )
        .unwrap();
        let elapsed_ms = start.elapsed().as_millis();
        assert_eq!(r.exit_code, 124, "expected timeout exit_code 124, got {}", r.exit_code);
        assert!(
            elapsed_ms < 5000,
            "orphan children blocked timeout: elapsed {}ms (expected < 5000ms)",
            elapsed_ms
        );
    }
}
