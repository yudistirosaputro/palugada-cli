# Per-profile custom skills (A3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** A profile carries user-authored custom skills (`profiles/<id>/skills/<name>/SKILL.md`) that `init`/`skills sync` emit into a bound project, plus `palugada skills new` to scaffold one.

**Architecture:** `scaffold::custom_skill_files(kn, profile)` reads the profile's `skills/` dir into (rel-path, body) pairs; `generate` + `skills sync` append them (Claude only) to the standard A2 set through the same write path. `scaffold::new_custom_skill` scaffolds a starter.

**Reference spec:** `docs/superpowers/specs/2026-06-14-profile-custom-skills-design.md`

**Test:** `cargo test` · **Build:** `cargo build`

---

## Task 1: `custom_skill_files` + `new_custom_skill`

**Files:** `src/scaffold.rs` (+ tests).

- [ ] **Step 1: failing tests** in `scaffold::tests`:

```rust
    #[test]
    fn custom_skill_files_reads_profile_skills_dir() {
        let kn = tempfile::tempdir().unwrap();
        let d = kn.path().join("profiles").join("p").join("skills").join("mvi-state");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("SKILL.md"), "---\nname: mvi-state\n---\n# MVI\n").unwrap();
        let files = custom_skill_files(kn.path(), "p");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, ".claude/skills/mvi-state/SKILL.md");
        assert!(files[0].1.contains("name: mvi-state"));
        // no skills dir → empty
        assert!(custom_skill_files(kn.path(), "other").is_empty());
    }

    #[test]
    fn new_custom_skill_scaffolds_and_guards() {
        let kn = tempfile::tempdir().unwrap();
        let p = new_custom_skill(kn.path(), "p", "mvi-state").unwrap();
        assert!(p.exists());
        let body = fs::read_to_string(&p).unwrap();
        assert!(body.contains("name: mvi-state"));
        // refuses duplicate
        assert!(new_custom_skill(kn.path(), "p", "mvi-state").is_err());
        // rejects reserved prefix + invalid name
        assert!(new_custom_skill(kn.path(), "p", "palugada-foo").unwrap_err().contains("reserved"));
        assert!(new_custom_skill(kn.path(), "p", "Bad Name").is_err());
        // and it shows up via custom_skill_files
        assert!(custom_skill_files(kn.path(), "p").iter().any(|(rel, _)| rel.contains("mvi-state")));
    }
```

- [ ] **Step 2: implement** in `src/scaffold.rs`:

```rust
/// User-authored custom skills for a profile: `profiles/<profile>/skills/<name>/SKILL.md`
/// → (".claude/skills/<name>/SKILL.md", body) pairs (sorted; empty if none).
pub fn custom_skill_files(kn: &Path, profile: &str) -> Vec<(String, String)> {
    let dir = kn.join("profiles").join(profile).join("skills");
    let mut out: Vec<(String, String)> = Vec::new();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let skill = entry.path().join("SKILL.md");
        if let Ok(body) = fs::read_to_string(&skill) {
            out.push((format!(".claude/skills/{name}/SKILL.md"), body));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Scaffold a starter custom skill into `profiles/<profile>/skills/<name>/SKILL.md`.
pub fn new_custom_skill(kn: &Path, profile: &str, name: &str) -> Result<std::path::PathBuf, String> {
    let valid = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !valid {
        return Err(format!("invalid skill name '{name}' — use only [a-z0-9-_]"));
    }
    if name.starts_with("palugada-") {
        return Err(format!(
            "'{name}' uses the reserved 'palugada-' prefix (that namespace is the generated standard set)"
        ));
    }
    let dir = kn.join("profiles").join(profile).join("skills").join(name);
    let p = dir.join("SKILL.md");
    if p.exists() {
        return Err(format!("custom skill '{name}' already exists at {}", p.display()));
    }
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    fs::write(&p, custom_skill_template(name)).map_err(|e| format!("write {}: {e}", p.display()))?;
    Ok(p)
}

fn custom_skill_template(name: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: >\n  TRIGGER when … (describe when an agent should use this skill).\nallowed-tools: Bash(palugada *), Read, Grep, Glob, Write, Edit\n---\n\n# {name}\n\nDescribe the task here. Pull rules from the profile instead of inlining them:\n\n    palugada for <task>     # a recipe   (`palugada for --list`)\n    palugada q <topic>      # a convention (`palugada q --list`)\n\nLocate code with the `palugada-search` skill before grepping.\n"
    )
}
```

- [ ] **Step 3:** `cargo test custom_skill_files new_custom_skill` → pass. **Step 4: commit** `feat(scaffold): per-profile custom skills (read + scaffold)`.

---

## Task 2: emit custom skills from `generate` + `skills sync`

**Files:** `src/scaffold.rs`, `src/main.rs`.

- [ ] **Step 1:** In `scaffold::generate`, move the `GlobalConfig::load_or_default()` up so it's available before writing skills (it's already loaded for the registry — load it once near the top as `let mut global = …`). After the standard `skill_files` loop, append custom skills for Claude:

```rust
    if agents.iter().any(|a| a == "claude") {
        if let Ok(kn) = crate::knowledge::knowledge_dir(&global) {
            for (rel, body) in custom_skill_files(&kn, &profile) {
                write_file(&repo.join(&rel), &body, opts.force, &mut written, &mut skipped)?;
            }
        }
    }
```

(Reuse the single `global` for both this and the registry step; don't load it twice.)

- [ ] **Step 2:** In `cmd_skills_sync` (`src/main.rs`), after building the standard list, append custom skills (Claude only) before writing:

```rust
            let kn = knowledge::knowledge_dir(&global)?;
            // ... existing: agents, repo ...
            let mut files = scaffold::skill_files(&pc.profile, &kinds, &agents);
            if agents.iter().any(|a| a == "claude") {
                files.extend(scaffold::custom_skill_files(&kn, &pc.profile));
            }
            for (rel, body) in files { /* existing exists/force write loop */ }
```

- [ ] **Step 3:** `cargo test && cargo build`. Smoke: scaffold a custom skill into the android-mvvm profile, `init` a temp project on it, confirm the custom skill lands in `.claude/skills/`; then **remove the custom skill from the bundled profile** (don't commit a stray test skill).

- [ ] **Step 4: commit** `feat(scaffold): init + skills sync emit per-profile custom skills`.

---

## Task 3: `palugada skills new <name>`

**Files:** `src/main.rs`.

- [ ] **Step 1:** Add to `enum SkillsCmd`:

```rust
    /// Scaffold a custom skill in a profile: `skills new <name> [--profile <id>]`.
    New {
        name: String,
        /// Profile to add the skill to (default: the active project's profile).
        #[arg(long)]
        profile: Option<String>,
    },
```

- [ ] **Step 2:** Rename `cmd_skills_sync` → `cmd_skills` (it now handles both subcommands); update the dispatch `Commands::Skills { action } => cmd_skills(action, project)`. Add the `New` arm:

```rust
        SkillsCmd::New { name, profile } => {
            let global = GlobalConfig::load_or_default()?;
            let kn = knowledge::knowledge_dir(&global)?;
            let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
            let p = scaffold::new_custom_skill(&kn, &prof, &name)?;
            println!("created custom skill '{name}' for profile '{prof}':");
            println!("  {}", p.display());
            println!("edit it, then `palugada skills sync` into a project bound to '{prof}'.");
            Ok(())
        }
```

- [ ] **Step 3:** `cargo test && cargo build`. Smoke: `palugada skills new demo-skill --profile android-mvvm` writes the file; a second call errors (exists); `palugada skills new palugada-x` errors (reserved). Remove the scaffolded `demo-skill` dir afterward (don't commit it).

- [ ] **Step 4: commit** `feat(skills): skills new <name> scaffolder for per-profile custom skills`.

---

## Task 4: Docs + final verify

**Files:** `README.md`.

- [ ] **Step 1:** README — document per-profile custom skills (`profiles/<id>/skills/<name>/SKILL.md`, emitted alongside the standard set, Claude-only) and `palugada skills new <name>`.

- [ ] **Step 2:** `cargo test && cargo build --release`. Confirm no bundled test skills were left in `knowledge/profiles/android-mvvm/skills/` (`git status` clean except intended files).

- [ ] **Step 3: commit** `docs: document per-profile custom skills + skills new`.

---

## Self-review notes

- **Spec coverage:** §3 storage/emission → T1 (`custom_skill_files`) + T2; §3 scaffolder → T1 (`new_custom_skill`) + T3 (`skills new`); §4 collision → `new_custom_skill` reserved-prefix guard; §5 tests → T1.
- **Type consistency:** `custom_skill_files(&Path,&str)->Vec<(String,String)>` used in T1/T2; `new_custom_skill(&Path,&str,&str)->Result<PathBuf,String>` used in T1/T3. `cmd_skills` (renamed) handles `Sync` + `New`.
- **Additive/non-fatal:** `generate` skips custom skills if the knowledge dir can't be resolved (init must still work); `skills sync` requires kn (it's a knowledge op).
- **Out of scope:** web-console authoring of custom skills; codex/gemini custom skills.
