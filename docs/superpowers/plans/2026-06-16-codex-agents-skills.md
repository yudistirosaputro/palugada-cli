# Codex skill parity (.agents/skills) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Emit palugada's granular skill set to `.agents/skills/` for the `codex` agent (matching Claude's `.claude/skills/`), plus custom skills, keeping `AGENTS.md`/`GEMINI.md` as rich guides.

**Architecture:** Factor a base-dir-parameterized `standard_skill_set`; reuse for claude (`.claude/skills`) and codex (`.agents/skills`). Same `SKILL.md` bodies (codex needs only name+description; ignores `allowed-tools`).

Spec: `docs/superpowers/specs/2026-06-16-codex-agents-skills-design.md`

---

## Task 1: scaffold — `standard_skill_set` + refactor `skill_files` + `custom_skill_files(base)`

**Files:** `src/scaffold.rs`

- [ ] **Step 1: Add `standard_skill_set` and refactor `skill_files`**

Replace the body of `skill_files` (the `match agent` block's `claude`/`codex` arms) and add the helper. New `skill_files`:

```rust
pub fn skill_files(profile: &str, kinds: &[&str], agents: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for agent in agents {
        match agent.as_str() {
            "claude" => {
                out.push(("CLAUDE.md".into(), claude_pointer(profile, kinds)));
                out.extend(standard_skill_set(kinds, ".claude/skills"));
            }
            "codex" => {
                out.push(("AGENTS.md".into(), single_guide(profile, kinds)));
                out.extend(standard_skill_set(kinds, ".agents/skills"));
            }
            "gemini" => out.push(("GEMINI.md".into(), single_guide(profile, kinds))),
            "cursor" => {
                out.push((".cursor/rules/palugada.mdc".into(), cursor_wrap(&single_guide(profile, kinds))))
            }
            _ => {}
        }
    }
    out
}

/// The granular skill set under `base` (e.g. ".claude/skills" or ".agents/skills"):
/// search + one per flow + tool skills gated by configured integration kinds.
fn standard_skill_set(kinds: &[&str], base: &str) -> Vec<(String, String)> {
    let has = |k: &str| kinds.contains(&k);
    let path = |name: &str| format!("{base}/{name}/SKILL.md");
    let mut out = vec![(path("palugada-search"), SKILL_SEARCH.to_string())];
    for &(flow, title, trig, verb) in FLOW_SKILLS {
        out.push((path(&format!("palugada-{flow}")), skill_flow(flow, title, trig, verb)));
    }
    if has("git_host") {
        out.push((path("palugada-git"), SKILL_GIT.to_string()));
    }
    if has("issue_tracker") || has("wiki") {
        out.push((path("palugada-docs"), SKILL_DOCS.to_string()));
    }
    if has("ci") || has("chat") {
        out.push((path("palugada-ci"), SKILL_CI.to_string()));
    }
    if has("design") {
        out.push((path("palugada-design"), SKILL_DESIGN.to_string()));
    }
    out
}
```

Then delete the now-unused `skill_path` function:

```rust
fn skill_path(name: &str) -> String {
    format!(".claude/skills/{name}/SKILL.md")
}
```

- [ ] **Step 2: Parameterize `custom_skill_files` with a base dir**

Change the signature + the pushed path:

```rust
pub fn custom_skill_files(kn: &Path, profile: &str, base: &str) -> Vec<(String, String)> {
```
and inside the loop:
```rust
            out.push((format!("{base}/{name}/SKILL.md"), body));
```

- [ ] **Step 3: Update the existing custom-skill test + add codex coverage**

In `mod tests`, update `custom_skill_files_reads_profile_skills_dir` calls to pass a base and assert the path. Replace its `custom_skill_files(kn.path(), "p")` / `custom_skill_files(kn.path(), "other")` calls with `custom_skill_files(kn.path(), "p", ".claude/skills")` / `custom_skill_files(kn.path(), "other", ".claude/skills")`, and ensure the path assertion checks `.claude/skills/mvi-state/SKILL.md`. Then add a new test:

```rust
#[test]
fn skill_files_emits_codex_agents_skills() {
    let f = skill_files("p", &["git_host"], &["codex".to_string()]);
    let has = |p: &str| f.iter().any(|(rel, _)| rel == p);
    assert!(has("AGENTS.md"));
    assert!(has(".agents/skills/palugada-search/SKILL.md"));
    assert!(has(".agents/skills/palugada-bugfix/SKILL.md"));
    assert!(has(".agents/skills/palugada-git/SKILL.md")); // gated by git_host
    // claude still uses .claude/skills
    let c = skill_files("p", &[], &["claude".to_string()]);
    assert!(c.iter().any(|(rel, _)| rel == ".claude/skills/palugada-search/SKILL.md"));
    // codex with no integrations → no tool skill
    let n = skill_files("p", &[], &["codex".to_string()]);
    assert!(!n.iter().any(|(rel, _)| rel == ".agents/skills/palugada-git/SKILL.md"));
}

#[test]
fn custom_skill_files_honors_base() {
    let kn = tempfile::tempdir().unwrap();
    let d = kn.path().join("profiles").join("p").join("skills").join("mvi-state");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("SKILL.md"), "---\nname: mvi-state\n---\nbody").unwrap();
    let f = custom_skill_files(kn.path(), "p", ".agents/skills");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].0, ".agents/skills/mvi-state/SKILL.md");
}
```

- [ ] **Step 4: Fix the other `custom_skill_files` callers so it compiles**

`generate()` and `src/main.rs` skills sync call `custom_skill_files(&kn, &profile)` — Task 2 updates them. For now, update `generate()`'s call (Task 2 does the codex addition); the build will fail until Task 2 — run the scaffold unit tests which don't exercise those call sites:

Run: `cargo test scaffold:: 2>&1 | tail -12`
Expected: the scaffold module compiles for tests? (No — `generate` is in the same module and won't compile with the old call.) Therefore do Task 2 before building. Mark this step done once Task 2's edits are in, then run the combined build/test in Task 2 Step 4.

- [ ] **Step 5: Commit (after Task 2 compiles)** — combined with Task 2.

---

## Task 2: `generate()` + `skills sync` custom skills for codex + `single_guide` line

**Files:** `src/scaffold.rs`, `src/main.rs`

- [ ] **Step 1: `generate()` — emit custom skills for claude + codex**

Replace the existing claude-only custom-skill block in `generate()`:

```rust
    if agents.iter().any(|a| a == "claude") {
        if let Ok(kn) = crate::knowledge::knowledge_dir(&global) {
            for (rel, body) in custom_skill_files(&kn, &profile) {
                write_file(&repo.join(&rel), &body, opts.force, &mut written, &mut skipped)?;
            }
        }
    }
```

with:

```rust
    if let Ok(kn) = crate::knowledge::knowledge_dir(&global) {
        let mut custom: Vec<(String, String)> = Vec::new();
        if agents.iter().any(|a| a == "claude") {
            custom.extend(custom_skill_files(&kn, &profile, ".claude/skills"));
        }
        if agents.iter().any(|a| a == "codex") {
            custom.extend(custom_skill_files(&kn, &profile, ".agents/skills"));
        }
        for (rel, body) in custom {
            write_file(&repo.join(&rel), &body, opts.force, &mut written, &mut skipped)?;
        }
    }
```

- [ ] **Step 2: `single_guide` — add the skills pointer line**

In `single_guide`, change the final pushed line from:

```rust
    s.push_str(&format!("Bound profile `{profile}` — switch with `palugada profile use <id>`. `palugada <cmd> --help` for anything.\n"));
```

to:

```rust
    s.push_str("On-demand granular skills are generated under `.agents/skills/` for agents that load them (Codex); otherwise this file is the full guide.\n");
    s.push_str(&format!("Bound profile `{profile}` — switch with `palugada profile use <id>`. `palugada <cmd> --help` for anything.\n"));
```

- [ ] **Step 3: `skills sync` (`src/main.rs`) — custom skills for claude + codex**

Replace:

```rust
            let mut files = scaffold::skill_files(&pc.profile, &kinds, &agents);
            if agents.iter().any(|a| a == "claude") {
                let kn = knowledge::knowledge_dir(&global)?;
                files.extend(scaffold::custom_skill_files(&kn, &pc.profile));
            }
```

with:

```rust
            let mut files = scaffold::skill_files(&pc.profile, &kinds, &agents);
            let kn = knowledge::knowledge_dir(&global)?;
            if agents.iter().any(|a| a == "claude") {
                files.extend(scaffold::custom_skill_files(&kn, &pc.profile, ".claude/skills"));
            }
            if agents.iter().any(|a| a == "codex") {
                files.extend(scaffold::custom_skill_files(&kn, &pc.profile, ".agents/skills"));
            }
```

- [ ] **Step 4: Build + full scaffold tests**

Run: `cargo build 2>&1 | tail -3 && cargo test scaffold:: 2>&1 | tail -12`
Expected: build OK; all scaffold tests pass (incl. the two new ones).

- [ ] **Step 5: Commit**

```bash
git add src/scaffold.rs src/main.rs
git commit -m "feat(scaffold): emit granular skills to .agents/skills for codex

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Verify + e2e + finish + release

- [ ] **Step 1: Full test suite**

Run: `cargo test 2>&1 | tail -5`
Expected: all pass.

- [ ] **Step 2: Manual e2e — codex gets `.agents/skills/`**

```bash
TMP=$(mktemp -d)
./target/debug/palugada init --repo "$TMP" --agents codex --name codex-demo 2>&1 | grep -iE "agents:|wrote|merged" | head
echo "--- files ---"; find "$TMP" -name SKILL.md -o -name AGENTS.md | sed "s#$TMP/##" | sort
rm -rf "$TMP"
# remove codex-demo from ~/.palugada.yaml registry afterward (hand-edit/script)
```
Expected: `agents: codex`; files include `AGENTS.md` + `.agents/skills/palugada-{search,bugfix,feature,refactor,review}/SKILL.md`. No `.claude/skills`.

- [ ] **Step 3: Cleanup the demo registry entry + confirm dev tree clean**

Remove `codex-demo` from `~/.palugada.yaml`; `git status --porcelain` (expect empty).

- [ ] **Step 4: Finish — merge to main + push**

Verify tests pass on merged main; push; delete branch.

- [ ] **Step 5: Release 0.1.3**

Bump `Cargo.toml` + `npm/palugada-cli/package.json` to `0.1.3`; `cargo check`; commit on main; push; `git tag v0.1.3 && git push origin v0.1.3`; watch the run; verify `npm view palugada-cli version` → `0.1.3`.

---

## Self-Review

**Spec coverage:** `standard_skill_set` base param (T1), codex `.agents/skills` in `skill_files` (T1), `custom_skill_files(base)` (T1), generate() codex custom (T2), skills-sync codex custom (T2), single_guide line (T2), tests (T1), e2e (T3), release (T3). ✓
**Placeholder scan:** none.
**Type consistency:** `standard_skill_set(kinds, base)` and `custom_skill_files(kn, profile, base)` signatures match all call sites (skill_files, generate, skills sync, tests). `skill_path` removed (only caller was the refactored claude arm).
