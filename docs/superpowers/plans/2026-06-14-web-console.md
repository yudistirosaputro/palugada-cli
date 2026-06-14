# `palugada web` console Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or subagent-driven-development) to implement task-by-task. Steps use `- [ ]`.

**Goal:** An optional embedded `palugada web` console (vanilla-JS, file-backed) to browse config/profiles/knowledge and author profiles + conventions/recipes + generate agent skill files.

**Architecture:** `src/web.rs` runs a synchronous `tiny_http` server on `127.0.0.1`, serves embedded vanilla UI (`include_str!`), and routes a small JSON API to existing modules. New library helpers (`knowledge` accessors + writers, `scaffold::generate`) hold the logic so it's unit-tested and reused; the HTTP layer is thin.

**Tech Stack:** Rust, `tiny_http` 0.12 (sync, no tokio), `serde_json`/`serde_yaml`, vanilla HTML/CSS/JS embedded in the binary.

**Reference spec:** `docs/superpowers/specs/2026-06-14-web-console-design.md`

**Test:** `cargo test` · **Build:** `cargo build` · **Run:** `./target/debug/palugada web --port 7777`

---

## Task 1: knowledge data accessors

**Files:** Modify `src/knowledge.rs` (+ tests).

- [ ] **Step 1: failing tests** — add to `knowledge::tests`:

```rust
    #[test]
    fn conventions_accessor_reads_index() {
        let kn = tempfile::tempdir().unwrap();
        let c = kn.path().join("profiles").join("p").join("conventions");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("_index.json"),
            r#"{"topics":[{"id":"arch","title":"Arch","description":"d","tags":["x"],"sections":[{"id":"o","title":"Overview"}]}]}"#).unwrap();
        let v = conventions(kn.path(), "p").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "arch");
        assert_eq!(v[0].sections, vec!["Overview".to_string()]);
    }
```

- [ ] **Step 2: run → fail** (`cargo test conventions_accessor_reads_index`): `conventions` undefined.

- [ ] **Step 3: implement** — in `src/knowledge.rs` add public serializable DTOs + accessors (reusing `read_conv_index`/`read_recipe_index`):

```rust
#[derive(serde::Serialize)]
pub struct TopicMeta {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub sections: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct RecipeMeta {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
}

pub fn conventions(kn: &Path, profile: &str) -> Result<Vec<TopicMeta>, String> {
    Ok(read_conv_index(kn, profile)?
        .topics
        .into_iter()
        .map(|t| TopicMeta {
            id: t.id,
            title: t.title,
            description: t.description,
            tags: t.tags,
            sections: t.sections.into_iter().map(|s| s.title).collect(),
        })
        .collect())
}

pub fn recipes(kn: &Path, profile: &str) -> Result<Vec<RecipeMeta>, String> {
    Ok(read_recipe_index(kn, profile)?
        .recipes
        .into_iter()
        .map(|r| RecipeMeta { id: r.id, title: r.title, description: r.description, tags: r.tags })
        .collect())
}

/// Raw markdown of a convention / recipe file.
pub fn convention_md(kn: &Path, profile: &str, id: &str) -> Result<String, String> {
    let p = kn.join("profiles").join(profile).join("conventions").join(format!("{id}.md"));
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}
pub fn recipe_md(kn: &Path, profile: &str, id: &str) -> Result<String, String> {
    let p = kn.join("profiles").join(profile).join("recipes").join(format!("{id}.md"));
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}
```

- [ ] **Step 4: run → pass** (`cargo test`). **Step 5: commit** `feat(knowledge): typed data accessors for conventions/recipes`.

---

## Task 2: knowledge writers (`slug`, `add_convention`, `add_recipe`)

**Files:** Modify `src/knowledge.rs` (+ tests).

- [ ] **Step 1: failing tests**:

```rust
    #[test]
    fn slug_kebabs_titles() {
        assert_eq!(slug("Errors in Coroutines"), "errors-in-coroutines");
        assert_eq!(slug("Sealed UiState!"), "sealed-uistate");
    }

    #[test]
    fn add_convention_writes_md_and_index() {
        let kn = tempfile::tempdir().unwrap();
        let c = kn.path().join("profiles").join("p").join("conventions");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("_index.json"), r#"{"schema_version":"1.0","topics":[]}"#).unwrap();
        let spec = ConventionSpec {
            id: "errorhandling".into(), title: "Error Handling".into(),
            description: "Handle failures.".into(), tags: vec!["error".into()],
            sections: vec![SectionSpec { title: "Modeling Failures".into(), body: "Model errors explicitly.".into(), code: false }],
        };
        add_convention(kn.path(), "p", &spec).unwrap();
        let md = convention_md(kn.path(), "p", "errorhandling").unwrap();
        assert!(md.contains("## Modeling Failures {#modeling-failures}"));
        assert!(md.contains("id: errorhandling"));
        let topics = conventions(kn.path(), "p").unwrap();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].sections, vec!["Modeling Failures".to_string()]);
        // adding the same id again replaces, not duplicates
        add_convention(kn.path(), "p", &spec).unwrap();
        assert_eq!(conventions(kn.path(), "p").unwrap().len(), 1);
    }
```

- [ ] **Step 2: run → fail.**

- [ ] **Step 3: implement** — add specs + writers to `src/knowledge.rs`:

```rust
#[derive(serde::Deserialize)]
pub struct SectionSpec {
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub code: bool,
}
#[derive(serde::Deserialize)]
pub struct ConventionSpec {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub sections: Vec<SectionSpec>,
}
#[derive(serde::Deserialize)]
pub struct RecipeSpec {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub body: String,
}

/// Kebab-case a heading: lowercase, runs of non-alphanumeric → single '-', trimmed.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn validate_doc_id(id: &str) -> Result<(), String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
        return Err(format!("invalid id '{id}' — use only [a-z0-9_-]"));
    }
    Ok(())
}

pub fn add_convention(kn: &Path, profile: &str, spec: &ConventionSpec) -> Result<(), String> {
    validate_doc_id(&spec.id)?;
    let dir = kn.join("profiles").join(profile).join("conventions");
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    // front-matter + body
    let mut fm = format!("---\nid: {}\ntitle: {}\ndescription: {}\nsections:\n",
        spec.id, yaml_scalar(&spec.title), yaml_scalar(&spec.description));
    let mut body = format!("# {}\n", spec.title);
    let mut sec_meta: Vec<serde_json::Value> = Vec::new();
    for s in &spec.sections {
        let sid = slug(&s.title);
        let tokens = s.body.len() / 4 + 8;
        fm.push_str(&format!("  - {{ id: {}, title: {}, tokens: {}, code: {} }}\n", sid, yaml_scalar(&s.title), tokens, s.code));
        body.push_str(&format!("\n## {} {{#{}}}\n{}\n", s.title, sid, s.body.trim()));
        sec_meta.push(serde_json::json!({ "id": sid, "title": s.title, "tokens": tokens }));
    }
    let tags_yaml = format!("[{}]", spec.tags.join(", "));
    fm.push_str(&format!("tags: {tags_yaml}\n---\n\n"));
    fs::write(dir.join(format!("{}.md", spec.id)), format!("{fm}{body}"))
        .map_err(|e| format!("write convention: {e}"))?;

    // update _index.json (Value-based: preserve other topics, replace same id)
    let entry = serde_json::json!({
        "id": spec.id, "title": spec.title, "file": format!("{}.md", spec.id),
        "description": spec.description, "tags": spec.tags, "sections": sec_meta,
    });
    upsert_index(&dir.join("_index.json"), "topics", &spec.id, entry)
}

pub fn add_recipe(kn: &Path, profile: &str, spec: &RecipeSpec) -> Result<(), String> {
    validate_doc_id(&spec.id)?;
    let dir = kn.join("profiles").join(profile).join("recipes");
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let md = format!("---\nid: {}\ntitle: {}\ndescription: {}\ntags: [{}]\n---\n\n# {}\n\n{}\n",
        spec.id, yaml_scalar(&spec.title), yaml_scalar(&spec.description), spec.tags.join(", "), spec.title, spec.body.trim());
    fs::write(dir.join(format!("{}.md", spec.id)), md).map_err(|e| format!("write recipe: {e}"))?;
    let entry = serde_json::json!({
        "id": spec.id, "title": spec.title, "description": spec.description,
        "file": format!("{}.md", spec.id), "tags": spec.tags,
    });
    upsert_index(&dir.join("_index.json"), "recipes", &spec.id, entry)
}

/// Quote a YAML scalar if it could be misparsed.
fn yaml_scalar(s: &str) -> String {
    if s.is_empty() || s.contains(['"', ':', '#', '\n']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("{:?}", s) // Rust debug = double-quoted, escapes quotes
    } else {
        s.to_string()
    }
}

/// Insert-or-replace an object (matched by `id`) in a JSON index file's array.
fn upsert_index(path: &Path, array_key: &str, id: &str, entry: serde_json::Value) -> Result<(), String> {
    let mut root: serde_json::Value = if path.exists() {
        serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("parse {}: {e}", path.display()))?
    } else {
        serde_json::json!({ "schema_version": "1.0", array_key: [] })
    };
    let arr = root.get_mut(array_key).and_then(|v| v.as_array_mut())
        .ok_or_else(|| format!("{} has no '{array_key}' array", path.display()))?;
    arr.retain(|e| e.get("id").and_then(|v| v.as_str()) != Some(id));
    arr.push(entry);
    let out = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    fs::write(path, out + "\n").map_err(|e| format!("write {}: {e}", path.display()))
}
```

- [ ] **Step 4: run → pass** (`cargo test`). **Step 5: commit** `feat(knowledge): add_convention/add_recipe writers + slug`.

---

## Task 3: `scaffold::generate` refactor

**Files:** Modify `src/scaffold.rs`, `src/main.rs`.

- [ ] **Step 1:** In `src/scaffold.rs`, extract the file-generation + registry work out of `run` into `pub fn generate(opts: &InitOptions) -> Result<GenerateOutcome, String>` returning:

```rust
pub struct GenerateOutcome {
    pub name: String,
    pub profile: String,
    pub auth: String,
    pub agents: Vec<String>,
    pub written: Vec<String>,
    pub skipped: Vec<String>,
    pub became_active: bool,
    pub config_path: String,
}
```

Move steps 1–3 of `run` (config skeleton, agent files, registry insert/save) into `generate`, populating and returning the outcome. Keep `run(opts)` as a thin wrapper that calls `generate(&opts)` and prints the same summary it prints today (step 4).

- [ ] **Step 2:** `cargo test && cargo build` — existing behavior unchanged (no test asserts on `init` stdout). Manually: `./target/debug/palugada init --repo /tmp/x --agents claude` still works.

- [ ] **Step 3: add a test** in `scaffold` (`#[cfg(test)]`): `generate` into a tempdir repo writes `CLAUDE.md` + 4 skill files and returns them in `written`. (Set `HOME` is not needed — `generate` calls `GlobalConfig::load_or_default`/`save`; to avoid touching real home, accept that the registry write targets `$HOME`; instead assert only on the repo files by checking the returned `written` contains the repo paths. If home writes are a concern, gate the registry write behind a flag — but keep v1 simple and just assert the repo files exist.)

Test:
```rust
    #[test]
    fn generate_writes_agent_files() {
        let repo = tempfile::tempdir().unwrap();
        let opts = InitOptions {
            repo: repo.path().to_string_lossy().to_string(),
            name: Some("demo".into()), profile: Some("android-mvvm".into()),
            auth: Some("default".into()), agents: vec!["claude".into()], force: true,
        };
        let out = generate(&opts).unwrap();
        assert!(repo.path().join("CLAUDE.md").exists());
        assert!(repo.path().join(".claude/skills/bugfix/SKILL.md").exists());
        assert!(out.written.iter().any(|w| w.ends_with("CLAUDE.md")));
    }
```

- [ ] **Step 4:** `cargo test` → pass. **Step 5: commit** `refactor(scaffold): split generate() from run()`.

---

## Task 4: tiny_http server skeleton + routing + command

**Files:** `Cargo.toml`; new `src/web.rs` + `src/web/{index.html,app.js,style.css}` (stubs); `src/main.rs`.

- [ ] **Step 1:** `cargo add tiny_http@0.12`.

- [ ] **Step 2: failing tests** — create `src/web.rs` with pure helpers + tests first:

```rust
    #[test]
    fn route_parses_paths() {
        assert!(matches!(route("GET", "/api/overview"), Route::Overview));
        assert!(matches!(route("GET", "/api/profile/android-mvvm"), Route::Profile(p) if p == "android-mvvm"));
        assert!(matches!(route("POST", "/api/profile/p/convention"), Route::AddConvention(p) if p == "p"));
        assert!(matches!(route("GET", "/nope"), Route::NotFound));
    }
    #[test]
    fn host_guard_allows_localhost_only() {
        assert!(host_ok("localhost:7777"));
        assert!(host_ok("127.0.0.1:7777"));
        assert!(!host_ok("evil.example.com"));
    }
```

- [ ] **Step 3: implement** `src/web.rs`: a `Route` enum, `route(method,&str)->Route`, `host_ok(&str)->bool`, embedded assets via `include_str!`, and `run(port, open)`:

```rust
use std::path::Path;

const INDEX_HTML: &str = include_str!("web/index.html");
const APP_JS: &str = include_str!("web/app.js");
const STYLE_CSS: &str = include_str!("web/style.css");

pub enum Route {
    Index, AppJs, StyleCss,
    Overview, Projects, Profiles,
    Profile(String),
    Convention(String, String), Recipe(String, String),
    CreateProfile, AddConvention(String), AddRecipe(String), Init,
    NotFound,
}

pub fn route(method: &str, path: &str) -> Route {
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (method, parts.as_slice()) {
        ("GET", [""]) | ("GET", ["index.html"]) => Route::Index,
        ("GET", ["app.js"]) => Route::AppJs,
        ("GET", ["style.css"]) => Route::StyleCss,
        ("GET", ["api", "overview"]) => Route::Overview,
        ("GET", ["api", "projects"]) => Route::Projects,
        ("GET", ["api", "profiles"]) => Route::Profiles,
        ("GET", ["api", "profile", id]) => Route::Profile((*id).to_string()),
        ("GET", ["api", "profile", id, "convention", cid]) => Route::Convention((*id).to_string(), (*cid).to_string()),
        ("GET", ["api", "profile", id, "recipe", rid]) => Route::Recipe((*id).to_string(), (*rid).to_string()),
        ("POST", ["api", "profile"]) => Route::CreateProfile,
        ("POST", ["api", "profile", id, "convention"]) => Route::AddConvention((*id).to_string()),
        ("POST", ["api", "profile", id, "recipe"]) => Route::AddRecipe((*id).to_string()),
        ("POST", ["api", "init"]) => Route::Init,
        _ => Route::NotFound,
    }
}

pub fn host_ok(host: &str) -> bool {
    let h = host.split(':').next().unwrap_or("");
    h == "localhost" || h == "127.0.0.1"
}
```

`run(port, open)` (skeleton serving only assets + 404 JSON for API; real handlers in Tasks 5–6):

```rust
pub fn run(port: u16, open: bool) -> Result<(), String> {
    let addr = format!("127.0.0.1:{port}");
    let server = tiny_http::Server::http(&addr).map_err(|e| format!("bind {addr}: {e}"))?;
    let url = format!("http://{addr}");
    println!("palugada web → {url}  (Ctrl-C to stop)");
    if open { let _ = open_browser(&url); }
    for request in server.incoming_requests() {
        let host = request.headers().iter().find(|h| h.field.equiv("Host")).map(|h| h.value.as_str().to_string()).unwrap_or_default();
        if !host_ok(&host) {
            let _ = request.respond(text(403, "forbidden host"));
            continue;
        }
        let method = request.method().as_str().to_string();
        let path = request.url().split('?').next().unwrap_or("/").to_string();
        let resp = handle(&method, &path, request);
        if let Err(e) = resp { eprintln!("web error: {e}"); }
    }
    Ok(())
}
```

with helpers `html(...)`, `js(...)`, `css(...)`, `json(status, &str)`, `text(status,&str)`, `open_browser(url)` (best-effort `Command::new(open|xdg-open|cmd)`), and a `handle(method,path,request)` that for Task 4 serves Index/AppJs/StyleCss and returns `json(404, "{\"error\":\"not found\"}")` for everything else. (Read/write handlers land in Tasks 5–6.)

Create stub `src/web/index.html` (`<!DOCTYPE html><html>…<div id=app>palugada web</div><script src=/app.js></script>`), empty-ish `app.js` (`console.log('palugada web')`), minimal `style.css`. Real UI in Task 7.

- [ ] **Step 4:** `src/main.rs`: `mod web;`, add `Web { #[arg(long, default_value_t=7777)] port: u16, #[arg(long)] open: bool }` to `Commands`, dispatch `Commands::Web { port, open } => web::run(port, open)`.

- [ ] **Step 5:** `cargo test` (route/host tests pass) + `cargo build`. Smoke: run `palugada web --port 7799 &`, `curl -s localhost:7799/ | head`, kill it.

- [ ] **Step 6: commit** `feat(web): tiny_http server skeleton + routing + Web command`.

---

## Task 5: Read API handlers

**Files:** Modify `src/web.rs`.

- [ ] **Step 1:** Implement the `GET /api/*` arms in `handle`, building JSON with `serde_json` from existing readers:
  - Overview: `{knowledge_dir, active_project, default_profile, profile_count}` from `GlobalConfig` + `knowledge::knowledge_dir` + `profile::list`.
  - Projects: from `global.projects` (name, repo_path, active flag).
  - Profiles: `profile::list` → `[{id,title}]`.
  - Profile(id): `{conventions: knowledge::conventions, recipes: knowledge::recipes, fact_families: indexer::fact_families, flows: <parse profile.yaml flows>}`. Add a small `flows(kn,id)` reader in `web.rs` (deserialize `{flows: BTreeMap<String,Vec<String>>}` from profile.yaml).
  - Convention/Recipe body: `knowledge::convention_md` / `recipe_md` → `{markdown}`.
  Each wraps errors as `json(500, {"error":...})`.

- [ ] **Step 2:** `cargo build`; smoke: `palugada web` then `curl -s localhost:PORT/api/profile/android-mvvm | python3 -m json.tool | head` shows the 4 conventions + 2 recipes.

- [ ] **Step 3: commit** `feat(web): read API (overview/projects/profiles/profile/bodies)`.

---

## Task 6: Write API handlers

**Files:** Modify `src/web.rs`.

- [ ] **Step 1:** Implement POST arms (read the request body to a `String`, parse with `serde_json`):
  - CreateProfile: `{id,title,languages}` → `profile::scaffold_new(&kn, &id)` (title/languages applied by a follow-up small write, or v1 just scaffolds with id and notes title editing later). Return `{ok:true}` or `{error}`.
  - AddConvention(id): parse `knowledge::ConventionSpec` → `knowledge::add_convention(&kn,&id,&spec)`.
  - AddRecipe(id): parse `knowledge::RecipeSpec` → `knowledge::add_recipe`.
  - Init: `{repo,agents,profile?,name?}` → build `scaffold::InitOptions` → `scaffold::generate` → return outcome JSON.

- [ ] **Step 2:** `cargo build`; smoke with curl: POST a convention to a throwaway profile created via `profile new`, then `curl .../api/profile/<id>` shows it; `palugada profile validate <id>` passes.

- [ ] **Step 3: commit** `feat(web): write API (create profile, add convention/recipe, init)`.

---

## Task 7: UI assets (sidebar console)

**Files:** Replace `src/web/index.html`, `src/web/app.js`, `src/web/style.css`.

- [ ] **Step 1:** `index.html` — `<!DOCTYPE>` doc: header bar, left `<nav>` with Overview/Projects/Profiles/Knowledge, a `<main id="view">`, `<script src="/app.js">`. `style.css` — dark, readable, sidebar layout (A), cards, forms, buttons (no framework).

- [ ] **Step 2:** `app.js` — vanilla, segmented by view:
  - `api(path, method, body)` fetch helper (JSON).
  - Router on nav clicks → `renderOverview/Projects/Profiles`.
  - `renderProfiles()`: list profiles; "+ New profile" form (POST `/api/profile`); click a profile → `renderProfile(id)`.
  - `renderProfile(id)`: show conventions/recipes/fact_families/flows; "+ Add convention" form with dynamic **section rows** (title/body/code) → POST; "+ Add recipe" form → POST; "Generate agent skills" form (repo path + agent checkboxes) → POST `/api/init`, show written files.
  - Click a convention/recipe → fetch body, show in a `<pre>`.
  - Show success/error toasts from API responses.

- [ ] **Step 3:** `cargo build`; **manual verification (real app):** `palugada web --open`; in the browser: create a profile `demo`, add a convention with two sections, confirm it appears; run `palugada profile validate demo` in a terminal → passes; generate skills into `/tmp/demoapp` and confirm `CLAUDE.md` written.

- [ ] **Step 4: commit** `feat(web): vanilla-JS console UI (browse + author + generate)`.

---

## Task 8: Docs + final verification

**Files:** `README.md`.

- [ ] **Step 1:** README — add a `palugada web` row to the commands table and a short "## Web console" section (run `palugada web`, opens `http://127.0.0.1:7777`, author profiles/knowledge, generate skills; localhost-only; writes the same files the CLI reads).

- [ ] **Step 2:** `cargo test && cargo build --release && ./target/release/palugada web --help`.

- [ ] **Step 3: commit** `docs: document palugada web console`.

---

## Self-review notes

- **Spec coverage:** §4.1 server→T4; §4.2 assets→T4/T7; §4.3 API→T4(route)+T5(read)+T6(write); §4.4 helpers→T1/T2/T3; §4.5 split authoring→T2(sections)+T7(section rows); §5 dep→T4; §6 tests→T1/T2/T3/T4 (host/route/writers/generate).
- **Type consistency:** `ConventionSpec`/`RecipeSpec`/`SectionSpec` defined in `knowledge.rs` (T2) and parsed in `web.rs` (T6); `conventions`/`recipes`/`convention_md`/`recipe_md` (T1) consumed in T5; `scaffold::generate`/`GenerateOutcome` (T3) consumed in T6; `route`/`Route`/`host_ok` (T4) used in T5/T6 handlers.
- **No secrets:** the API never reads `secrets.yaml`; overview exposes only non-secret config.
- **Out of scope:** config/secrets editing, connectors/prd/index in UI, search — later slices.
