//! End-to-end tests that run the built `palugada` binary — the agent-facing
//! contract (output shape + exit codes that generated skills rely on) plus the
//! web console's CSRF gate. Each test uses an isolated `$HOME` and points
//! `PALUGADA_KNOWLEDGE` at the repo's bundled profiles.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;
use tempfile::TempDir;

fn knowledge_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("knowledge")
}

fn home() -> TempDir {
    tempfile::tempdir().unwrap()
}

/// A `palugada` invocation with an isolated HOME + bundled knowledge.
fn pal(h: &TempDir) -> Command {
    let mut c = Command::cargo_bin("palugada").unwrap();
    c.env("HOME", h.path())
        .env("USERPROFILE", h.path()) // Windows
        .env("PALUGADA_KNOWLEDGE", knowledge_dir());
    c
}

/// A scratch repo with the given files (relative path → contents).
fn repo_with(files: &[(&str, &str)]) -> TempDir {
    let d = tempfile::tempdir().unwrap();
    for (rel, body) in files {
        let p = d.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }
    d
}

#[test]
fn version_and_help() {
    let h = home();
    pal(&h).arg("--version").assert().success().stdout(predicate::str::contains("palugada"));
    pal(&h).arg("--help").assert().success();
}

#[test]
fn q_lists_hits_and_reports_misses() {
    let h = home();
    pal(&h).args(["q", "--list", "--profile", "rust-cli"]).assert().success();
    pal(&h)
        .args(["q", "architecture", "--profile", "rust-cli"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
    // A miss exits non-zero with a helpful message on stderr.
    pal(&h)
        .args(["q", "nope-not-a-topic", "--profile", "rust-cli"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no convention").or(predicate::str::contains("nope")));
}

#[test]
fn for_and_search_work() {
    let h = home();
    pal(&h).args(["for", "--list", "--profile", "rust-cli"]).assert().success();
    pal(&h).args(["s", "error", "--profile", "rust-cli"]).assert().success();
}

#[test]
fn symbol_without_an_index_hints_to_run_index() {
    let h = home();
    let repo = repo_with(&[("Cargo.toml", "")]);
    pal(&h)
        .args(["symbol", "Anything", "--repo"])
        .arg(repo.path())
        .assert()
        .success() // absent index is a note, not an error
        .stdout(predicate::str::contains("no index"));
}

#[test]
fn init_scaffolds_auto_indexes_and_symbol_works_immediately() {
    let h = home();
    let repo = repo_with(&[("Cargo.toml", ""), ("lib.rs", "pub fn hello_world() {}\n")]);
    pal(&h)
        .args(["init", "--repo"])
        .arg(repo.path())
        .args(["--agents", "claude"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Indexed"));
    assert!(repo.path().join(".palugada/index/symbols.json").exists(), "auto-index ran");
    // symbol resolves the repo via cwd and finds the freshly-indexed symbol.
    pal(&h)
        .current_dir(repo.path())
        .args(["symbol", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello_world"));
}

#[test]
fn init_no_index_skips_the_index() {
    let h = home();
    let repo = repo_with(&[("Cargo.toml", ""), ("a.rs", "pub fn x() {}\n")]);
    pal(&h)
        .args(["init", "--repo"])
        .arg(repo.path())
        .args(["--agents", "claude", "--no-index"])
        .assert()
        .success();
    assert!(!repo.path().join(".palugada/index").exists(), "--no-index skips indexing");
}

#[test]
fn init_rejects_a_nonexistent_profile() {
    let h = home();
    let repo = repo_with(&[("Cargo.toml", "")]);
    pal(&h)
        .args(["init", "--repo"])
        .arg(repo.path())
        .args(["--profile", "web-react", "--agents", "claude"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn brief_on_a_file_lists_in_file_symbols_and_json_has_the_wrapper() {
    let h = home();
    let repo = repo_with(&[("Cargo.toml", ""), ("lib.rs", "pub fn hello_world() {}\npub struct Widget;\n")]);
    pal(&h).args(["init", "--repo"]).arg(repo.path()).args(["--agents", "claude"]).assert().success();

    pal(&h)
        .current_dir(repo.path())
        .args(["brief", "bugfix", "lib.rs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("symbols defined in lib.rs"));

    pal(&h)
        .current_dir(repo.path())
        .args(["brief", "bugfix", "lib.rs", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"packs\"").and(predicate::str::contains("\"degraded\"")));
}

#[test]
fn brief_unknown_flow_errors_and_lists_flows() {
    let h = home();
    let repo = repo_with(&[("Cargo.toml", "")]);
    pal(&h).args(["init", "--repo"]).arg(repo.path()).args(["--agents", "claude", "--no-index"]).assert().success();
    pal(&h)
        .current_dir(repo.path())
        .args(["brief", "no-such-flow"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("flow 'no-such-flow' not defined"));
}

#[test]
fn exec_gates_repo_defined_verbs() {
    let h = home();
    let repo = repo_with(&[(
        ".palugada/config.yaml",
        "project: t\nprofile: rust-cli\nexec:\n  hello: \"echo PWNED-BY-REPO\"\n",
    )]);
    pal(&h).args(["config", "init"]).assert().success();
    pal(&h).args(["project", "add", "t"]).arg(repo.path()).assert().success();

    // Non-interactive (assert_cmd gives no tty) + no --yes → refuse.
    pal(&h)
        .args(["--project", "t", "exec", "hello", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("refusing").or(predicate::str::contains("not approved")));

    // --yes approves → the verb runs.
    pal(&h)
        .args(["--project", "t", "exec", "hello", "--json", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PWNED-BY-REPO"));
}

#[test]
fn doctor_on_a_fresh_project_exits_zero() {
    let h = home();
    let repo = repo_with(&[("Cargo.toml", "")]);
    pal(&h).args(["init", "--repo"]).arg(repo.path()).args(["--agents", "claude", "--no-index"]).assert().success();
    // No connectors configured → all SKIP → exit 0.
    pal(&h).current_dir(repo.path()).arg("doctor").assert().success();
}

// ── web console CSRF gate (WP2.2) ───────────────────────────────────────────

/// Kills the spawned server on drop so a failing assert never leaks it.
struct Server(std::process::Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Minimal HTTP/1.0 GET over raw TCP (avoids an HTTP-client dev-dep). Returns
/// (status_code, full_response_text).
fn http_get(port: u16, path: &str, headers: &[(&str, &str)]) -> (u16, String) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut req = format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n");
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    let status = resp
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    (status, resp)
}

#[test]
fn web_console_enforces_the_session_token() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command as PCommand, Stdio};

    let h = home();
    let child = PCommand::new(env!("CARGO_BIN_EXE_palugada"))
        .args(["web", "--port", "0"]) // OS-assigned port
        .env("HOME", h.path())
        .env("USERPROFILE", h.path())
        .env("PALUGADA_KNOWLEDGE", knowledge_dir())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut server = Server(child);

    // First stdout line: "palugada web → http://127.0.0.1:PORT   (Ctrl-C ...)".
    let mut line = String::new();
    BufReader::new(server.0.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let marker = "127.0.0.1:";
    let idx = line.find(marker).expect("bound-address line") + marker.len();
    let port: u16 = line[idx..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();

    // 1. Forged request with no token → 403 (the CSRF gate).
    let (status, _) = http_get(port, "/api/overview", &[]);
    assert_eq!(status, 403, "no-token API request must be refused");

    // 2. The served page carries a real token in the meta tag.
    let (idx_status, html) = http_get(port, "/", &[]);
    assert_eq!(idx_status, 200);
    let tk = "palugada-token\" content=\"";
    let ti = html.find(tk).expect("token meta tag") + tk.len();
    let token: String = html[ti..].chars().take_while(|&c| c != '"').collect();
    assert_eq!(token.len(), 64, "session token is 256-bit hex");

    // 3. Same-origin request WITH the token → 200.
    let (ok_status, _) =
        http_get(port, "/api/overview", &[("X-Palugada-Token", &token), ("Sec-Fetch-Site", "same-origin")]);
    assert_eq!(ok_status, 200, "authorized request must pass");

    // 4. Cross-site request even WITH the token → 403 (origin guard).
    let (xsite_status, _) = http_get(
        port,
        "/api/overview",
        &[("X-Palugada-Token", &token), ("Sec-Fetch-Site", "cross-site")],
    );
    assert_eq!(xsite_status, 403, "cross-site request must be refused");

    drop(server);
}
