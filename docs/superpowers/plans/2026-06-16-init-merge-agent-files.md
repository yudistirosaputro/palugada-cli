# init/skills-sync merge into existing agent files — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `init`/`skills sync` append-or-replace a marker-delimited palugada section inside existing `CLAUDE.md`/`AGENTS.md`/`GEMINI.md` (instead of skipping them), and default `--agents` to auto-detecting the agents a repo already uses.

**Architecture:** A new marker-upsert writer handles the three user-owned root guides; palugada-owned files keep the write-if-missing/`--force` path. `detect_agents` resolves `--agents auto` from existing guide files. `GenerateOutcome` gains a `merged` bucket reported by both the CLI and the web.

**Tech Stack:** Rust (std::fs), existing `scaffold`/`main`/`web` modules.

Spec: `docs/superpowers/specs/2026-06-16-init-merge-agent-files-design.md`

---

## File structure

| File | Action |
|---|---|
| `src/scaffold.rs` | `upsert_marked_section`+`SectionWrite`, `write_agent_file`, `detect_agents`, `GenerateOutcome.merged`, generate/run wiring + tests |
| `src/main.rs` | `--agents` default `auto` (init + skills sync), detect wiring, skills-sync merge + reporting |
| `src/web.rs` | `init_op` returns `merged` |

---

## Task 1: scaffold marker upsert + detect_agents (pure, TDD)

**Files:**
- Modify: `src/scaffold.rs` (add helpers + tests)

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/scaffold.rs`:

```rust
#[test]
fn upsert_marked_section_creates_appends_replaces() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("AGENTS.md");

    // absent → Created, file is just the block
    assert!(matches!(upsert_marked_section(&p, "hello").unwrap(), SectionWrite::Created));
    let v1 = std::fs::read_to_string(&p).unwrap();
    assert!(v1.contains(MARK_START) && v1.contains("hello") && v1.contains(MARK_END));

    // identical re-run → Unchanged
    assert!(matches!(upsert_marked_section(&p, "hello").unwrap(), SectionWrite::Unchanged));

    // changed content → Merged, replaced in place (no duplicate markers)
    assert!(matches!(upsert_marked_section(&p, "world").unwrap(), SectionWrite::Merged));
    let v2 = std::fs::read_to_string(&p).unwrap();
    assert!(v2.contains("world") && !v2.contains("hello"));
    assert_eq!(v2.matches(MARK_START).count(), 1);

    // existing user file WITHOUT markers → append, preserve original
    let u = tmp.path().join("CLAUDE.md");
    std::fs::write(&u, "# My notes\nkeep me\n").unwrap();
    assert!(matches!(upsert_marked_section(&u, "palu").unwrap(), SectionWrite::Merged));
    let uv = std::fs::read_to_string(&u).unwrap();
    assert!(uv.starts_with("# My notes\nkeep me\n"));
    assert!(uv.contains(MARK_START) && uv.contains("palu"));
}

#[test]
fn detect_agents_reads_existing_guides() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();
    assert_eq!(detect_agents(r), vec!["claude".to_string()]); // empty → fallback
    std::fs::write(r.join("AGENTS.md"), "x").unwrap();
    assert_eq!(detect_agents(r), vec!["codex".to_string()]);
    std::fs::write(r.join("CLAUDE.md"), "x").unwrap();
    std::fs::write(r.join("GEMINI.md"), "x").unwrap();
    assert_eq!(detect_agents(r), vec!["claude".to_string(), "codex".to_string(), "gemini".to_string()]);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test scaffold::tests::upsert_marked_section scaffold::tests::detect_agents 2>&1 | tail -12`
Expected: FAIL — `cannot find ... upsert_marked_section` / `detect_agents`.

- [ ] **Step 3: Implement the helpers**

Add to `src/scaffold.rs` (e.g. just after `write_file`, ~line 164):

```rust
const MARK_START: &str = "<!-- palugada:start -->";
const MARK_END: &str = "<!-- palugada:end -->";

#[derive(Debug, PartialEq)]
enum SectionWrite {
    Created,
    Merged,
    Unchanged,
}

fn marked_block(content: &str) -> String {
    format!("{MARK_START}\n{}\n{MARK_END}\n", content.trim_end())
}

/// Insert-or-replace a palugada marker block in `path`, preserving any other
/// content. Creates the file (and parents) if absent.
fn upsert_marked_section(path: &Path, content: &str) -> Result<SectionWrite, String> {
    let block = marked_block(content);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        fs::write(path, &block).map_err(|e| format!("write {}: {e}", path.display()))?;
        return Ok(SectionWrite::Created);
    }
    let existing = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let updated = match (existing.find(MARK_START), existing.find(MARK_END)) {
        (Some(s), Some(e)) if e > s => {
            let end = e + MARK_END.len();
            format!("{}{}{}", &existing[..s], block.trim_end(), &existing[end..])
        }
        _ => {
            let sep = if existing.ends_with('\n') { "\n" } else { "\n\n" };
            format!("{existing}{sep}{block}")
        }
    };
    if updated == existing {
        return Ok(SectionWrite::Unchanged);
    }
    fs::write(path, &updated).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(SectionWrite::Merged)
}

/// Agents a repo already uses, by which guide files exist; `["claude"]` if none.
pub fn detect_agents(repo: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if repo.join("CLAUDE.md").exists() {
        out.push("claude".to_string());
    }
    if repo.join("AGENTS.md").exists() {
        out.push("codex".to_string());
    }
    if repo.join("GEMINI.md").exists() {
        out.push("gemini".to_string());
    }
    if repo.join(".cursor").exists() {
        out.push("cursor".to_string());
    }
    if out.is_empty() {
        out.push("claude".to_string());
    }
    out
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test scaffold::tests::upsert_marked_section scaffold::tests::detect_agents 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/scaffold.rs
git commit -m "feat(scaffold): marker-section upsert + agent auto-detect (pure, tested)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: route guide files through merge in `generate()`

**Files:**
- Modify: `src/scaffold.rs` (`write_agent_file`, `GenerateOutcome.merged`, `generate()`, `run()`)

- [ ] **Step 1: Add `write_agent_file`**

Add to `src/scaffold.rs` after `upsert_marked_section`:

```rust
/// Write one generated agent file. The three user-owned root guides
/// (CLAUDE.md/AGENTS.md/GEMINI.md) upsert a marker block (preserving user
/// content); every other path uses the write-if-missing/`--force` path.
pub fn write_agent_file(
    path: &Path,
    content: &str,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
    merged: &mut Vec<String>,
) -> Result<(), String> {
    let base = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if matches!(base, "CLAUDE.md" | "AGENTS.md" | "GEMINI.md") {
        match upsert_marked_section(path, content)? {
            SectionWrite::Created => written.push(path.display().to_string()),
            SectionWrite::Merged => merged.push(path.display().to_string()),
            SectionWrite::Unchanged => skipped.push(path.display().to_string()),
        }
        Ok(())
    } else {
        write_file(path, content, force, written, skipped)
    }
}
```

- [ ] **Step 2: Add `merged` to `GenerateOutcome`**

In `struct GenerateOutcome` (after `pub skipped: Vec<String>,`):

```rust
    pub merged: Vec<String>,
```

- [ ] **Step 3: Wire `generate()`**

In `generate()`: replace the agents fallback

```rust
    let agents = if opts.agents.is_empty() {
        vec!["claude".to_string()]
    } else {
        opts.agents.clone()
    };
```

with auto-detection:

```rust
    let auto = opts.agents.is_empty() || (opts.agents.len() == 1 && opts.agents[0] == "auto");
    let agents = if auto { detect_agents(&repo) } else { opts.agents.clone() };
```

Add `let mut merged: Vec<String> = Vec::new();` next to `written`/`skipped`.

Change the agent-files loop to use `write_agent_file`:

```rust
    for (rel, body) in skill_files(&profile, &kinds, &agents) {
        write_agent_file(&repo.join(&rel), &body, opts.force, &mut written, &mut skipped, &mut merged)?;
    }
```

(Leave the config-skeleton write and the `custom_skill_files` loop on `write_file`.)

Add `merged` to the returned `GenerateOutcome { ... }`.

- [ ] **Step 4: Report merged in `run()`**

In `run()`, after the `for w in &out.written { ... }` loop, add:

```rust
    for m in &out.merged {
        println!("  merged   {m}  (palugada section)");
    }
```

- [ ] **Step 5: Build + run existing scaffold tests**

Run: `cargo test scaffold:: 2>&1 | tail -8 && cargo build 2>&1 | tail -3`
Expected: scaffold tests PASS; build OK (any callers of `GenerateOutcome` that don't yet set `merged` are updated in Task 3 — if `cargo build` errors on `web.rs`/`main.rs` missing `merged`, that's expected and fixed in Task 3; this step's gate is the scaffold unit tests passing).

- [ ] **Step 6: Commit**

```bash
git add src/scaffold.rs
git commit -m "feat(scaffold): merge guide files + auto-detect agents in generate()

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: CLI defaults + skills-sync merge + web outcome

**Files:**
- Modify: `src/main.rs` (init/skills-sync `--agents` default + detect + sync merge)
- Modify: `src/web.rs` (`init_op` returns `merged`)

- [ ] **Step 1: Default `--agents` to `auto`**

In `src/main.rs`, change both `#[arg(long, default_value = "claude")] agents: String,` occurrences (the `Init` command ~line 55 and the `Skills`/`Sync` command ~line 262) to:

```rust
        #[arg(long, default_value = "auto")]
        agents: String,
```

- [ ] **Step 2: `cmd_init` passes `auto` through**

Confirm `cmd_init`'s parse keeps `auto` as a single element (the existing split/trim/filter yields `["auto"]`, which `generate()` now detects from). No code change needed; `generate()` handles `["auto"]`.

- [ ] **Step 3: `skills sync` — auto-detect + merge writer**

In `cmd_skills`'s `SkillsCmd::Sync` arm (`src/main.rs`), replace the agents parse:

```rust
            let agents: Vec<String> = agents
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
```

with:

```rust
            let agents: Vec<String> = if agents.trim() == "auto" {
                scaffold::detect_agents(std::path::Path::new(&entry.repo_path))
            } else {
                agents.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
            };
```

Then replace the write loop + summary:

```rust
            let (mut wrote, mut skipped) = (0usize, 0usize);
            for (rel, body) in files {
                let p = repo.join(&rel);
                if p.exists() && !force {
                    skipped += 1;
                    continue;
                }
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
                }
                std::fs::write(&p, body).map_err(|e| format!("write {}: {e}", p.display()))?;
                println!("  wrote    {}", p.display());
                wrote += 1;
            }
            println!("skills sync — '{name}' (profile {}): wrote {wrote}, skipped {skipped} (exist)", pc.profile);
            if skipped > 0 && !force {
                println!("  use --force to overwrite existing skill files");
            }
```

with:

```rust
            let (mut written, mut skipped, mut merged) = (Vec::new(), Vec::new(), Vec::new());
            for (rel, body) in files {
                scaffold::write_agent_file(&repo.join(&rel), &body, force, &mut written, &mut skipped, &mut merged)?;
            }
            for w in &written {
                println!("  wrote    {w}");
            }
            for m in &merged {
                println!("  merged   {m}  (palugada section)");
            }
            for s in &skipped {
                println!("  skipped  {s}  (exists)");
            }
            println!(
                "skills sync — '{name}' (profile {}): {} wrote, {} merged, {} skipped",
                pc.profile, written.len(), merged.len(), skipped.len()
            );
            if !skipped.is_empty() && !force {
                println!("  use --force to overwrite existing palugada-owned files");
            }
```

- [ ] **Step 4: Web `init_op` returns `merged`**

In `src/web.rs` `init_op`, change the success payload to include `merged`:

```rust
    Ok(json!({
        "ok": true, "name": out.name, "profile": out.profile,
        "written": out.written, "merged": out.merged, "skipped": out.skipped,
    }))
```

- [ ] **Step 5: Build + full tests**

Run: `cargo build 2>&1 | tail -3 && cargo test 2>&1 | tail -6`
Expected: build OK; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/web.rs
git commit -m "feat(cli): --agents auto default + skills-sync merge + web merged outcome

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: End-to-end verification

**Files:** none.

- [ ] **Step 1: Full test suite**

Run: `cargo test 2>&1 | tail -6`
Expected: all pass.

- [ ] **Step 2: Manual e2e — merge into existing AGENTS.md + auto-detect**

```bash
TMP=$(mktemp -d)
printf '# My agents file\nkeep this line\n' > "$TMP/AGENTS.md"
./target/debug/palugada init "$TMP" --name tmp-demo 2>&1 | sed -n '1,20p'
echo "--- AGENTS.md after ---"
cat "$TMP/AGENTS.md"
```
Expected: output shows agents auto-resolved to `codex` and a `merged ... AGENTS.md (palugada section)` line; the file keeps `# My agents file / keep this line` and gains a `<!-- palugada:start -->…<!-- palugada:end -->` block. No `CLAUDE.md` was created.

- [ ] **Step 3: Idempotency — re-run merges cleanly**

```bash
./target/debug/palugada init "$TMP" --name tmp-demo 2>&1 | grep -E "merged|skipped" | head
grep -c 'palugada:start' "$TMP/AGENTS.md"   # expect: 1 (no duplicate block)
```
Expected: re-run reports the guide as skipped/unchanged; exactly one marker block.

- [ ] **Step 4: Clean up the demo + unregister**

```bash
rm -rf "$TMP"
```
(The demo registered `tmp-demo` in `~/.palugada.yaml`; remove that entry if you want — it points at a now-deleted temp dir and is harmless, but: `palugada` has no unregister command yet, so optionally hand-edit `~/.palugada.yaml` to drop `tmp-demo`.)

- [ ] **Step 5: Confirm repo tree clean**

Run: `git status --porcelain`
Expected: empty (no stray edits to the dev repo).

---

## Self-Review

**Spec coverage:**
- Marker upsert for the three guides → Task 1 + 2. ✓
- Palugada-owned files stay write-if-missing → `write_agent_file` else-branch (Task 2). ✓
- `detect_agents` + `--agents auto` default → Task 1 (fn), Task 2 (generate), Task 3 (CLI + skills sync). ✓
- `GenerateOutcome.merged` + CLI/web reporting → Task 2 (struct/run), Task 3 (skills sync + web). ✓
- `--force` unchanged for palugada-owned files → `write_agent_file` else-branch. ✓
- Tests (upsert 4 cases, detect_agents 3 cases) + manual merge/idempotency → Tasks 1, 4. ✓

**Placeholder scan:** No TBD/TODO; every code step shows the code.

**Type consistency:** `write_agent_file(path, content, force, &mut Vec, &mut Vec, &mut Vec)` signature identical in Task 2 (def), generate() call (Task 2), and skills sync call (Task 3). `SectionWrite::{Created,Merged,Unchanged}` produced by `upsert_marked_section` and matched in `write_agent_file`. `GenerateOutcome.merged` added in Task 2 and read in `run()` (Task 2) + web `init_op` (Task 3). `detect_agents(&Path) -> Vec<String>` matches all call sites.
