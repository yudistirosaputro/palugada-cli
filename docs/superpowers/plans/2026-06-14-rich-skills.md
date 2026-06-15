# Rich skill generation (A2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** `palugada init` + new `palugada skills sync` generate a rich, profile-agnostic skill set (search-first standard, 4 flow skills, integration-gated tool skills) — references to palugada commands, never inlined knowledge.

**Architecture:** A pure `scaffold::skill_files(profile, kinds, agents) -> Vec<(String, String)>` builds (relative-path, body) pairs; `generate` (init) and `cmd_skills_sync` both write them through `write_file` (existing exists/force logic). Tool skills are gated by which integrations the project config declares.

**Reference spec:** `docs/superpowers/specs/2026-06-14-rich-skills-design.md` · **Models:** `/Users/septiandwisaputro/Documents/project/android/ttsecuritas/.claude/skills/{android,android-git}/SKILL.md`

**Test:** `cargo test` · **Build:** `cargo build`

---

## Task 1: `skill_files` builder + integration gating

**Files:** `src/scaffold.rs` (+ tests).

- [ ] **Step 1: failing tests** in `scaffold::tests` (create the module if absent):

```rust
    #[test]
    fn skill_files_gates_tool_skills_by_integration() {
        // only git_host present → git skill, but not docs/ci/design
        let only_git = skill_files("android-mvvm", &["git_host"], &["claude".into()]);
        let names: Vec<&str> = only_git.iter().map(|(p, _)| p.as_str()).collect();
        assert!(names.iter().any(|p| p.contains("palugada-search/SKILL.md")));
        assert!(names.iter().any(|p| p.contains("palugada-bugfix/SKILL.md")));
        assert!(names.iter().any(|p| p.contains("palugada-git/SKILL.md")));
        assert!(!names.iter().any(|p| p.contains("palugada-docs/SKILL.md")));
        assert!(!names.iter().any(|p| p.contains("palugada-ci/SKILL.md")));
        // all integrations → all tool skills
        let all = skill_files("android-mvvm", &["git_host", "wiki", "issue_tracker", "ci", "design"], &["claude".into()]);
        let an: Vec<&str> = all.iter().map(|(p, _)| p.as_str()).collect();
        for s in ["palugada-docs", "palugada-ci", "palugada-design"] {
            assert!(an.iter().any(|p| p.contains(s)), "missing {s}");
        }
    }

    #[test]
    fn skill_bodies_are_profile_agnostic_references() {
        let files = skill_files("android-mvvm", &["git_host"], &["claude".into()]);
        let search = files.iter().find(|(p, _)| p.contains("palugada-search")).unwrap();
        assert!(search.1.contains("palugada symbol"), "search skill must reference symbol");
        assert!(search.1.to_lowercase().contains("before") && search.1.contains("grep"), "search-first rule");
        // no skill hardcodes profile knowledge: bodies use `for --list`/`q --list`, not a convention dump
        let flow = files.iter().find(|(p, _)| p.contains("palugada-feature")).unwrap();
        assert!(flow.1.contains("palugada brief feature"));
        // codex single-guide path
        let guide = skill_files("android-mvvm", &["git_host"], &["codex".into()]);
        assert!(guide.iter().any(|(p, _)| p == "AGENTS.md"));
        assert!(!guide.iter().any(|(p, _)| p.contains(".claude/skills")));
    }
```

- [ ] **Step 2:** Implement `integration_kinds` + `skill_files` in `src/scaffold.rs`.

`integration_kinds` from a `ProjectConfig`:
```rust
/// Which integrations a project declares (drives which tool skills are generated).
pub fn integration_kinds(pc: &crate::config::ProjectConfig) -> Vec<&'static str> {
    let i = &pc.integrations;
    let mut k = Vec::new();
    if i.issue_tracker.is_some() { k.push("issue_tracker"); }
    if i.wiki.is_some() { k.push("wiki"); }
    if i.git_host.is_some() { k.push("git_host"); }
    if i.design.is_some() { k.push("design"); }
    if i.ci.is_some() { k.push("ci"); }
    if i.chat.is_some() { k.push("chat"); }
    k
}
```

`skill_files(profile, kinds, agents)` returns `Vec<(String, String)>` of repo-relative paths + bodies:
- For **claude** in `agents`: `CLAUDE.md` (thin pointer) + `.claude/skills/palugada-search/SKILL.md` + `.claude/skills/palugada-{bugfix,feature,refactor,review}/SKILL.md` + gated tool skills `.claude/skills/palugada-{git,docs,ci,design}/SKILL.md`.
- For **codex/gemini/cursor**: one guide file each (`AGENTS.md` / `GEMINI.md` / `.cursor/rules/palugada.mdc`) = the richer single-file guide (search-first + compact flow/connector command reference, gated). cursor body wrapped via `cursor_wrap`.
- Gating: git skill iff `kinds` contains `git_host`; docs iff `issue_tracker` or `wiki`; ci iff `ci` or `chat`; design iff `design`.

Template content (author the markdown; each is references-only, profile-agnostic). Required, asserted elements:
- **CLAUDE.md / guide**: 1-line "use `palugada symbol`/`fact` before grep"; a list of the generated `palugada-*` skills (claude) or a compact command table (codex/gemini); `palugada <cmd> --help`. Profile id MAY appear as a one-word note but NO convention text.
- **palugada-search**: frontmatter `name: palugada-search`, a TRIGGER description (find/locate symbol/function/where-defined, before grep/find/rg), `allowed-tools: Bash(palugada *), Grep, Glob, Read`. Body: discovery order `palugada symbol <name>` → `palugada symbol <name> --kind <k>` → `palugada fact <family>` → grep only as fallback (and report when the index missed). The hard rule "run palugada symbol/fact BEFORE any grep".
- **palugada-<flow>** (×4): frontmatter `name: palugada-<flow>` + task-specific trigger + `allowed-tools: Bash(palugada *), Read, Grep, Glob, Write, Edit`. Body steps: `palugada brief <flow> <target>` → `palugada for <task>` / `palugada q <topic>` (`--list` to discover) → act → (review flow: `palugada brief review <ref>`).
- **palugada-git**: triggers on git/branch/MR/PR/commit/conflict; `allowed-tools: Bash(palugada *), Bash(git *), Bash(gh *), Bash(glab *), Read, Write, Edit`. Body: `palugada git whoami`, `palugada pr recent <file>`, commit (`type(scope): …`) + branch (`type/TICKET-…`) conventions, push safety (`--force-with-lease`, never force on shared).
- **palugada-docs**: `palugada issue view <KEY>`, `palugada wiki page <ID>`, `palugada prd fetch/search/cat`.
- **palugada-ci**: `palugada ci status <JOB>`, `palugada notify <msg>`.
- **palugada-design**: `palugada design file <KEY>`.

- [ ] **Step 3:** `cargo test skill_files skill_bodies` → pass. **Step 4: commit** `feat(scaffold): rich profile-agnostic skill_files builder (gated tool skills)`.

---

## Task 2: `generate` uses `skill_files`

**Files:** `src/scaffold.rs`.

- [ ] **Step 1:** In `generate`, replace the "step 2 agent files" block (the per-agent `match` writing CLAUDE.md/SKILL/AGENTS/etc.) with: after writing the config skeleton, load it to get integrations, then write the skill set:

```rust
    // 2. agent files (rich, profile-agnostic, gated by integrations)
    let pc = crate::config::ProjectConfig::load_from(&repo_str)?;
    let kinds = integration_kinds(&pc);
    for (rel, body) in skill_files(&profile, &kinds, &agents) {
        write_file(&repo.join(&rel), &body, opts.force, &mut written, &mut skipped)?;
    }
```

Validate agent names up front (since `skill_files` would otherwise silently skip unknown ones): before the loop, return an error for any agent not in {claude,codex,gemini,cursor} (preserve current behavior). Remove the now-dead `agent_guide`/`agent_skill`/`FLOWS`/`SKILL_TEMPLATE`/`GUIDE_TEMPLATE` once unused (keep `config_skeleton`/`CONFIG_TEMPLATE`/`cursor_wrap`/`detect_profile`/`write_file`).

- [ ] **Step 2:** `cargo test && cargo build`. Smoke: `palugada init --repo /tmp/plg-rs --agents claude --force` writes `.claude/skills/palugada-search/SKILL.md` + flow skills + `palugada-git`/`-docs` (skeleton has git_host + issue_tracker/wiki); `CLAUDE.md` is short. Clean up the registry entry.

- [ ] **Step 3: commit** `feat(scaffold): init generates the rich gated skill set`.

---

## Task 3: `palugada skills sync`

**Files:** `src/main.rs`.

- [ ] **Step 1:** Add the command:

```rust
    /// (Re)generate this project's agent skill files: `skills sync`.
    Skills {
        #[command(subcommand)]
        action: SkillsCmd,
    },
```
```rust
#[derive(Subcommand)]
enum SkillsCmd {
    /// Write any missing skill files for the active project (`--force` to overwrite).
    Sync {
        /// Comma-separated agent targets (default: claude).
        #[arg(long, default_value = "claude")]
        agents: String,
        /// Overwrite existing skill files instead of skipping them.
        #[arg(long)]
        force: bool,
    },
}
```

Dispatch: `Commands::Skills { action } => cmd_skills_sync(action, project),`

- [ ] **Step 2:** Handler — resolve the project repo + profile + integrations, write the skill set additively:

```rust
fn cmd_skills_sync(action: SkillsCmd, project: Option<&str>) -> Result<(), String> {
    match action {
        SkillsCmd::Sync { agents, force } => {
            let global = GlobalConfig::load_or_default()?;
            let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
            let name = config::resolve_project_name(&global, project, &cwd)?;
            let entry = global.projects.registered.get(&name)
                .ok_or_else(|| format!("project '{name}' is not registered"))?;
            let pc = config::ProjectConfig::load_from(&entry.repo_path)?;
            let kinds = scaffold::integration_kinds(&pc);
            let agents: Vec<String> = agents.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let repo = std::path::Path::new(&entry.repo_path);
            let (mut wrote, mut skipped) = (0usize, 0usize);
            for (rel, body) in scaffold::skill_files(&pc.profile, &kinds, &agents) {
                let p = repo.join(&rel);
                if p.exists() && !force { skipped += 1; continue; }
                if let Some(parent) = p.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
                std::fs::write(&p, body).map_err(|e| format!("write {}: {e}", p.display()))?;
                wrote += 1;
            }
            println!("skills sync — project '{name}' (profile {}): wrote {wrote}, skipped {skipped} (exists)", pc.profile);
            if skipped > 0 && !force { println!("  use --force to overwrite existing skill files"); }
            Ok(())
        }
    }
}
```

- [ ] **Step 3:** `cargo test && cargo build`. Smoke: init a temp project, edit one skill file, `palugada skills sync --project <n>` → reports skipped (not clobbered); `--force` overwrites. Clean up.

- [ ] **Step 4: commit** `feat(skills): skills sync (expand-not-overwrite)`.

---

## Task 4: Docs + final verify

**Files:** `README.md`.

- [ ] **Step 1:** README — document the generated skill set (search-first + flows + gated tool skills, profile-agnostic references) and `palugada skills sync`; update the `init` "agent files" table.

- [ ] **Step 2:** `cargo test && cargo build --release`. Full smoke: init a temp Kotlin-ish repo → inspect `.claude/skills/` set; confirm `palugada-search` body has the before-grep rule and references `palugada symbol`. Clean up temp project + registry.

- [ ] **Step 3: commit** `docs: document the rich skill set + skills sync`.

---

## Self-review notes

- **Spec coverage:** §4 skill set + §5 per-agent → T1 `skill_files`; §6 mechanism → T2 (`generate`) + T3 (`skills sync`); §7 tests → T1 (gating + agnostic) + T3 smoke.
- **Type consistency:** `skill_files(&str, &[&str], &[String]) -> Vec<(String,String)>` used in T1 (def/tests), T2 (generate), T3 (sync). `integration_kinds(&ProjectConfig)` used in T2 + T3.
- **Expand-not-overwrite:** T3 skips existing unless `--force` (the spec's requirement); `init` keeps its `--force` semantics via `write_file`.
- **Out of scope:** A3 per-profile custom skills; managed-marker partial overwrite; inlining knowledge.
