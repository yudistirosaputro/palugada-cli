//! `palugada web` — an optional, embedded, file-backed authoring console.
//!
//! A synchronous `tiny_http` server bound to loopback only. It serves a vanilla
//! UI (embedded via `include_str!`) and a small JSON API that reads/writes the
//! same config/profile/knowledge files the CLI uses. No async runtime, no DB,
//! no secrets exposed. The agent-consumption path stays the cold CLI; this runs
//! only while a human is editing.

use serde_json::json;
use std::path::PathBuf;

const INDEX_HTML: &str = include_str!("web/index.html");
const APP_JS: &str = include_str!("web/app.js");
const STYLE_CSS: &str = include_str!("web/style.css");

type Resp = tiny_http::Response<std::io::Cursor<Vec<u8>>>;

#[derive(Debug, PartialEq)]
pub enum Route {
    Index,
    AppJs,
    StyleCss,
    Overview,
    Projects,
    Profiles,
    Profile(String),
    Convention(String, String),
    Recipe(String, String),
    CreateProfile,
    AddConvention(String),
    AddRecipe(String),
    SetProjectProfile(String),
    Init,
    SkillMap(String),
    SetConventionBody(String, String),
    SetRecipeBody(String, String),
    ProjectConfig(String),
    SaveProjectConfig(String),
    VerifyCapability(String, String),
    ImportPreview(String),
    ImportCommit(String),
    ProjectRules(String),
    OverlayConventionBody(String, String),
    AddOverlayConvention(String),
    SetOverlayConventionBody(String, String),
    SetOverlayReviewMap(String),
    NotFound,
}

/// Map (method, path) to a route. Pure — unit-tested without a live server.
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
        ("GET", ["api", "profile", id, "convention", cid]) => {
            Route::Convention((*id).to_string(), (*cid).to_string())
        }
        ("GET", ["api", "profile", id, "recipe", rid]) => {
            Route::Recipe((*id).to_string(), (*rid).to_string())
        }
        ("POST", ["api", "profile"]) => Route::CreateProfile,
        ("POST", ["api", "profile", id, "convention"]) => Route::AddConvention((*id).to_string()),
        ("POST", ["api", "profile", id, "recipe"]) => Route::AddRecipe((*id).to_string()),
        ("POST", ["api", "profile", id, "import", "preview"]) => Route::ImportPreview((*id).to_string()),
        ("POST", ["api", "profile", id, "import", "commit"]) => Route::ImportCommit((*id).to_string()),
        ("POST", ["api", "project", name, "profile"]) => Route::SetProjectProfile((*name).to_string()),
        ("POST", ["api", "init"]) => Route::Init,
        ("GET", ["api", "project", name, "skillmap"]) => Route::SkillMap((*name).to_string()),
        ("POST", ["api", "profile", id, "convention", cid, "body"]) => {
            Route::SetConventionBody((*id).to_string(), (*cid).to_string())
        }
        ("POST", ["api", "profile", id, "recipe", rid, "body"]) => {
            Route::SetRecipeBody((*id).to_string(), (*rid).to_string())
        }
        ("GET", ["api", "project", name, "config"]) => Route::ProjectConfig((*name).to_string()),
        ("POST", ["api", "project", name, "config"]) => Route::SaveProjectConfig((*name).to_string()),
        ("POST", ["api", "project", name, "verify", cap]) => {
            Route::VerifyCapability((*name).to_string(), (*cap).to_string())
        }
        ("GET", ["api", "project", name, "rules"]) => Route::ProjectRules((*name).to_string()),
        ("GET", ["api", "project", name, "convention", id]) => {
            Route::OverlayConventionBody((*name).to_string(), (*id).to_string())
        }
        ("POST", ["api", "project", name, "convention"]) => {
            Route::AddOverlayConvention((*name).to_string())
        }
        ("POST", ["api", "project", name, "convention", id, "body"]) => {
            Route::SetOverlayConventionBody((*name).to_string(), (*id).to_string())
        }
        ("POST", ["api", "project", name, "review-map"]) => {
            Route::SetOverlayReviewMap((*name).to_string())
        }
        _ => Route::NotFound,
    }
}

/// Accept only loopback Host headers (defends against DNS-rebinding).
pub fn host_ok(host: &str) -> bool {
    let h = host.split(':').next().unwrap_or("");
    h == "localhost" || h == "127.0.0.1"
}

pub fn run(port: u16, open: bool) -> Result<(), String> {
    let addr = format!("127.0.0.1:{port}");
    let server = tiny_http::Server::http(&addr).map_err(|e| format!("bind {addr}: {e}"))?;
    let url = format!("http://{addr}");
    println!("palugada web → {url}   (Ctrl-C to stop)");
    if open {
        open_browser(&url);
    }
    for request in server.incoming_requests() {
        handle(request);
    }
    Ok(())
}

fn handle(mut request: tiny_http::Request) {
    let host = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Host"))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();
    if !host_ok(&host) {
        let _ = request.respond(text(403, "forbidden host"));
        return;
    }
    let method = request.method().as_str().to_string();
    let path = request.url().split('?').next().unwrap_or("/").to_string();
    match route(&method, &path) {
        Route::Index => {
            let _ = request.respond(asset(INDEX_HTML, "text/html; charset=utf-8"));
        }
        Route::AppJs => {
            let _ = request.respond(asset(APP_JS, "application/javascript; charset=utf-8"));
        }
        Route::StyleCss => {
            let _ = request.respond(asset(STYLE_CSS, "text/css; charset=utf-8"));
        }
        Route::NotFound => {
            let _ = request.respond(json_resp(404, err_json("not found")));
        }
        other => {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let (status, payload) = api(other, &body);
            let _ = request.respond(json_resp(status, payload));
        }
    }
}

/// JSON API dispatch — read and write handlers.
fn api(route: Route, body: &str) -> (u16, String) {
    match route {
        Route::Overview => read(overview_json),
        Route::Projects => read(projects_json),
        Route::Profiles => read(profiles_json),
        Route::Profile(id) => read(|| profile_json(&id)),
        Route::Convention(id, cid) => read(|| {
            let kn = knowledge_dir()?;
            Ok(json!({ "markdown": crate::knowledge::convention_md(&kn, &id, &cid)? }))
        }),
        Route::Recipe(id, rid) => read(|| {
            let kn = knowledge_dir()?;
            Ok(json!({ "markdown": crate::knowledge::recipe_md(&kn, &id, &rid)? }))
        }),
        Route::CreateProfile => write_op(|| create_profile(body)),
        Route::AddConvention(id) => write_op(|| {
            let kn = knowledge_dir()?;
            let spec: crate::knowledge::ConventionSpec =
                serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::knowledge::add_convention(&kn, &id, &spec)?;
            Ok(json!({ "ok": true, "id": spec.id }))
        }),
        Route::AddRecipe(id) => write_op(|| {
            let kn = knowledge_dir()?;
            let spec: crate::knowledge::RecipeSpec =
                serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::knowledge::add_recipe(&kn, &id, &spec)?;
            Ok(json!({ "ok": true, "id": spec.id }))
        }),
        Route::ImportPreview(id) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Req {
                markdown: String,
            }
            let kn = knowledge_dir()?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            let existing: std::collections::BTreeSet<String> =
                crate::knowledge::conventions(&kn, &id)?.into_iter().map(|c| c.id).collect();
            let candidates: Vec<serde_json::Value> =
                crate::knowledge::split_markdown_conventions(&req.markdown)
                    .into_iter()
                    .map(|d| {
                        json!({
                            "id": d.id, "title": d.title, "sections": d.sections,
                            "body": d.body, "exists": existing.contains(&d.id),
                        })
                    })
                    .collect();
            let warnings: Vec<String> = if candidates.is_empty() {
                vec!["no headings found — add a `# Heading` per topic".to_string()]
            } else {
                vec![]
            };
            Ok(json!({ "candidates": candidates, "warnings": warnings }))
        }),
        Route::ImportCommit(id) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Piece {
                #[serde(default)]
                id: String,
                #[serde(default)]
                title: String,
                #[serde(default)]
                description: String,
                #[serde(default)]
                tags: Vec<String>,
                #[serde(default)]
                body: String,
            }
            #[derive(serde::Deserialize)]
            struct Req {
                pieces: Vec<Piece>,
            }
            let kn = knowledge_dir()?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            if req.pieces.is_empty() {
                return Err("no pieces selected".to_string());
            }
            // Validate ALL before writing (write none on any invalid).
            for p in &req.pieces {
                if p.title.trim().is_empty() {
                    return Err(format!("piece '{}' needs a title", p.id));
                }
                if !crate::knowledge::valid_doc_id(p.id.trim()) {
                    return Err(format!("invalid id '{}' — use only [a-z0-9_-]", p.id));
                }
            }
            let dir = kn.join("profiles").join(&id).join("conventions");
            let (mut created, mut updated) = (0u32, 0u32);
            let mut ids: Vec<String> = Vec::new();
            for p in &req.pieces {
                let raw = format!(
                    "---\nid: {}\ntitle: {}\ndescription: {}\ntags: [{}]\n---\n\n# {}\n{}",
                    p.id.trim(),
                    p.title.trim(),
                    p.description,
                    p.tags.join(", "),
                    p.title.trim(),
                    p.body
                );
                let (cid, replaced) = crate::knowledge::add_convention_from_markdown(&dir, &raw)?;
                if replaced {
                    updated += 1;
                } else {
                    created += 1;
                }
                ids.push(cid);
            }
            Ok(json!({ "created": created, "updated": updated, "ids": ids }))
        }),
        Route::SetProjectProfile(name) => write_op(|| set_project_profile(&name, body)),
        Route::Init => write_op(|| init_op(body)),
        Route::SkillMap(name) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            Ok(jv(&crate::skillmap::skillmap(&global, &name)?))
        }),
        Route::SetConventionBody(id, cid) => write_op(|| set_doc_body(&id, "convention", &cid, body)),
        Route::SetRecipeBody(id, rid) => write_op(|| set_doc_body(&id, "recipe", &rid, body)),
        Route::ProjectConfig(name) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            crate::credentials::project_config_json(&global, &name)
        }),
        Route::SaveProjectConfig(name) => write_op(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            crate::credentials::save_project_config(&global, &name, body)
        }),
        Route::VerifyCapability(name, cap) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            crate::credentials::verify_capability(&global, &name, &cap)
        }),
        Route::ProjectRules(name) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            Ok(jv(&crate::effective::effective_rules(&global, &name)?))
        }),
        Route::OverlayConventionBody(name, id) => read(|| {
            let repo = project_repo(&name)?;
            let markdown =
                crate::knowledge::convention_md_in(&crate::effective::overlay_dir(&repo), &id)?;
            Ok(json!({ "markdown": markdown }))
        }),
        Route::AddOverlayConvention(name) => write_op(|| {
            let repo = project_repo(&name)?;
            let spec: crate::knowledge::ConventionSpec =
                serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::knowledge::add_convention_in(&crate::effective::overlay_dir(&repo), &spec)?;
            Ok(json!({ "ok": true, "id": spec.id }))
        }),
        Route::SetOverlayConventionBody(name, id) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Req {
                markdown: String,
            }
            let repo = project_repo(&name)?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::knowledge::set_convention_body_in(
                &crate::effective::overlay_dir(&repo),
                &id,
                &req.markdown,
            )?;
            Ok(json!({ "ok": true, "id": id }))
        }),
        Route::SetOverlayReviewMap(name) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Req {
                #[serde(default)]
                review_map: std::collections::BTreeMap<String, Vec<String>>,
            }
            let repo = project_repo(&name)?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::config::set_review_map(&repo, req.review_map)?;
            Ok(json!({ "ok": true }))
        }),
        _ => (501, err_json("not implemented yet")),
    }
}

fn read<F: FnOnce() -> Result<serde_json::Value, String>>(f: F) -> (u16, String) {
    match f() {
        Ok(v) => (200, v.to_string()),
        Err(e) => (500, err_json(&e)),
    }
}

/// Like `read` but maps errors to 400 (client/data error) for write endpoints.
fn write_op<F: FnOnce() -> Result<serde_json::Value, String>>(f: F) -> (u16, String) {
    match f() {
        Ok(v) => (200, v.to_string()),
        Err(e) => (400, err_json(&e)),
    }
}

/// Resolve a (URL-encoded) project name to its repo path via the registry.
fn project_repo(name: &str) -> Result<String, String> {
    let global = crate::config::GlobalConfig::load_or_default()?;
    let name = crate::http::decode_segment(name);
    Ok(global
        .projects
        .registered
        .get(&name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?
        .repo_path
        .clone())
}

fn create_profile(body: &str) -> Result<serde_json::Value, String> {
    #[derive(serde::Deserialize)]
    struct NewProfile {
        id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        languages: Vec<String>,
    }
    let np: NewProfile = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let kn = knowledge_dir()?;
    crate::profile::scaffold_new(&kn, &np.id)?;
    // Apply the chosen title / languages over the scaffold's defaults.
    if !np.title.is_empty() || !np.languages.is_empty() {
        let pf = kn.join("profiles").join(&np.id).join("profile.yaml");
        let mut raw = std::fs::read_to_string(&pf).map_err(|e| e.to_string())?;
        if !np.title.is_empty() {
            raw = raw.replace(
                &format!("title: \"{} profile\"", np.id),
                &format!("title: \"{}\"", np.title.replace('"', "'")),
            );
        }
        if !np.languages.is_empty() {
            raw = raw.replace("languages: []", &format!("languages: [{}]", np.languages.join(", ")));
        }
        std::fs::write(&pf, raw).map_err(|e| e.to_string())?;
    }
    Ok(json!({ "ok": true, "id": np.id }))
}

fn set_project_profile(name: &str, body: &str) -> Result<serde_json::Value, String> {
    #[derive(serde::Deserialize)]
    struct Req {
        profile: String,
    }
    let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    // The project name arrives URL-encoded in the path (project names may contain
    // spaces etc.); decode before matching the registry's plain keys.
    let name = crate::http::decode_segment(name);
    let global = crate::config::GlobalConfig::load_or_default()?;
    let kn = crate::knowledge::knowledge_dir(&global)?;
    let profs = crate::profile::list(&kn)?;
    if !profs.iter().any(|(p, _)| *p == req.profile) {
        return Err(format!(
            "unknown profile '{}' (available: {})",
            req.profile,
            profs.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    let entry = global
        .projects
        .registered
        .get(&name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    crate::config::set_profile(&entry.repo_path, &req.profile)?;
    Ok(json!({ "ok": true, "name": name, "profile": req.profile }))
}

/// Overwrite a convention/recipe markdown body verbatim (edit from the web).
fn set_doc_body(profile: &str, kind: &str, id: &str, body: &str) -> Result<serde_json::Value, String> {
    #[derive(serde::Deserialize)]
    struct Req {
        markdown: String,
    }
    let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let kn = knowledge_dir()?;
    match kind {
        "convention" => crate::knowledge::set_convention_body(&kn, profile, id, &req.markdown)?,
        "recipe" => crate::knowledge::set_recipe_body(&kn, profile, id, &req.markdown)?,
        other => return Err(format!("unknown doc kind '{other}'")),
    }
    Ok(json!({ "ok": true, "id": id }))
}

fn init_op(body: &str) -> Result<serde_json::Value, String> {
    #[derive(serde::Deserialize)]
    struct InitReq {
        repo: String,
        #[serde(default)]
        agents: Vec<String>,
        #[serde(default)]
        profile: Option<String>,
        #[serde(default)]
        name: Option<String>,
    }
    let req: InitReq = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let opts = crate::scaffold::InitOptions {
        repo: req.repo,
        name: req.name,
        profile: req.profile,
        auth: None,
        agents: req.agents,
        force: false,
    };
    let out = crate::scaffold::generate(&opts)?;
    Ok(json!({
        "ok": true, "name": out.name, "profile": out.profile,
        "written": out.written, "merged": out.merged, "skipped": out.skipped,
    }))
}

/// Serialize a value to `serde_json::Value` (null on failure — handlers control errors).
fn jv<T: serde::Serialize>(t: &T) -> serde_json::Value {
    serde_json::to_value(t).unwrap_or(serde_json::Value::Null)
}

fn overview_json() -> Result<serde_json::Value, String> {
    let global = crate::config::GlobalConfig::load_or_default()?;
    let kn = crate::knowledge::knowledge_dir(&global)?;
    let profs = crate::profile::list(&kn)?;
    Ok(json!({
        "knowledge_dir": kn.display().to_string(),
        "active_project": global.projects.active,
        "default_profile": global.defaults.profile,
        "profile_count": profs.len(),
        "project_count": global.projects.registered.len(),
    }))
}

fn projects_json() -> Result<serde_json::Value, String> {
    let global = crate::config::GlobalConfig::load_or_default()?;
    let active = global.projects.active.clone();
    let list: Vec<serde_json::Value> = global
        .projects
        .registered
        .iter()
        .map(|(name, e)| {
            let profile = crate::config::ProjectConfig::load_from(&e.repo_path)
                .map(|c| c.profile)
                .unwrap_or_default();
            json!({ "name": name, "repo_path": e.repo_path, "active": *name == active, "profile": profile })
        })
        .collect();
    Ok(json!({ "active": active, "projects": list }))
}

fn profiles_json() -> Result<serde_json::Value, String> {
    let kn = knowledge_dir()?;
    let list: Vec<serde_json::Value> = crate::profile::list(&kn)?
        .into_iter()
        .map(|(id, title)| json!({ "id": id, "title": title }))
        .collect();
    Ok(json!({ "profiles": list }))
}

fn profile_json(id: &str) -> Result<serde_json::Value, String> {
    let kn = knowledge_dir()?;
    Ok(json!({
        "id": id,
        "conventions": jv(&crate::knowledge::conventions(&kn, id)?),
        "recipes": jv(&crate::knowledge::recipes(&kn, id)?),
        "fact_families": crate::indexer::fact_families(&kn, id).unwrap_or_default(),
        "flows": jv(&flows(&kn, id).unwrap_or_default()),
    }))
}

/// The profile's flow → step-list map, read from `profile.yaml`.
fn flows(kn: &std::path::Path, profile: &str) -> Result<std::collections::BTreeMap<String, Vec<String>>, String> {
    #[derive(serde::Deserialize, Default)]
    struct F {
        #[serde(default)]
        flows: std::collections::BTreeMap<String, Vec<String>>,
    }
    let p = kn.join("profiles").join(profile).join("profile.yaml");
    let raw = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let f: F = serde_yaml::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(f.flows)
}

// ── response helpers ──────────────────────────────────────────────────────

fn header(k: &str, v: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("valid header")
}

fn body(status: u16, content_type: &str, s: String) -> Resp {
    tiny_http::Response::from_string(s)
        .with_status_code(status)
        .with_header(header("Content-Type", content_type))
}

fn asset(s: &str, content_type: &str) -> Resp {
    body(200, content_type, s.to_string())
}

fn json_resp(status: u16, s: String) -> Resp {
    body(status, "application/json", s)
}

fn text(status: u16, s: &str) -> Resp {
    body(status, "text/plain; charset=utf-8", s.to_string())
}

/// A JSON `{"error": msg}` payload, properly escaped.
fn err_json(msg: &str) -> String {
    serde_json::json!({ "error": msg }).to_string()
}

fn open_browser(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    if std::process::Command::new(cmd).arg(url).spawn().is_err() {
        eprintln!("(could not auto-open browser — visit {url})");
    }
}

/// Resolve the knowledge dir for API handlers (shared by read/write).
fn knowledge_dir() -> Result<PathBuf, String> {
    let global = crate::config::GlobalConfig::load_or_default()?;
    crate::knowledge::knowledge_dir(&global)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_parses_paths() {
        assert_eq!(route("GET", "/api/overview"), Route::Overview);
        assert_eq!(route("GET", "/"), Route::Index);
        assert_eq!(route("GET", "/app.js"), Route::AppJs);
        assert_eq!(route("GET", "/api/profile/android-mvvm"), Route::Profile("android-mvvm".into()));
        assert_eq!(
            route("GET", "/api/profile/p/convention/arch"),
            Route::Convention("p".into(), "arch".into())
        );
        assert_eq!(route("POST", "/api/profile"), Route::CreateProfile);
        assert_eq!(route("POST", "/api/profile/p/convention"), Route::AddConvention("p".into()));
        assert_eq!(route("POST", "/api/profile/p/import/preview"), Route::ImportPreview("p".into()));
        assert_eq!(route("POST", "/api/profile/p/import/commit"), Route::ImportCommit("p".into()));
        assert_eq!(route("POST", "/api/init"), Route::Init);
        assert_eq!(route("POST", "/api/project/x/profile"), Route::SetProjectProfile("x".into()));
        assert_eq!(route("GET", "/api/project/app/skillmap"), Route::SkillMap("app".into()));
        assert_eq!(
            route("POST", "/api/profile/p/convention/c/body"),
            Route::SetConventionBody("p".into(), "c".into())
        );
        assert_eq!(
            route("POST", "/api/profile/p/recipe/r/body"),
            Route::SetRecipeBody("p".into(), "r".into())
        );
        assert_eq!(route("GET", "/api/project/app/config"), Route::ProjectConfig("app".into()));
        assert_eq!(route("POST", "/api/project/app/config"), Route::SaveProjectConfig("app".into()));
        assert_eq!(
            route("POST", "/api/project/app/verify/git_host"),
            Route::VerifyCapability("app".into(), "git_host".into())
        );
        assert_eq!(route("GET", "/api/project/app/rules"), Route::ProjectRules("app".into()));
        assert_eq!(
            route("GET", "/api/project/app/convention/ours"),
            Route::OverlayConventionBody("app".into(), "ours".into())
        );
        assert_eq!(
            route("POST", "/api/project/app/convention"),
            Route::AddOverlayConvention("app".into())
        );
        assert_eq!(
            route("POST", "/api/project/app/convention/architecture/body"),
            Route::SetOverlayConventionBody("app".into(), "architecture".into())
        );
        assert_eq!(route("POST", "/api/project/app/review-map"), Route::SetOverlayReviewMap("app".into()));
        assert_eq!(route("GET", "/nope"), Route::NotFound);
    }

    #[test]
    fn host_guard_allows_localhost_only() {
        assert!(host_ok("localhost:7777"));
        assert!(host_ok("127.0.0.1:7777"));
        assert!(host_ok("localhost"));
        assert!(!host_ok("evil.example.com"));
        assert!(!host_ok(""));
    }
}
