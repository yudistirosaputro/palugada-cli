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
    ConventionRaw(String, String),
    RecipeRaw(String, String),
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
    SetFlows(String),
    ProjectRules(String),
    ProjectDocs(String),
    ProjectDoc(String, String),
    OverlayConventionBody(String, String),
    AddOverlayConvention(String),
    SetOverlayConventionBody(String, String),
    SetOverlayReviewMap(String),
    Connectors,
    SaveConnector(String),
    VerifyConnector(String),
    ProjectConnectors(String),
    SaveProjectConnector(String, String),
    VerifyProjectConnector(String, String),
    AuthProfileSecrets(String),
    AuthProfiles,
    CreateAuthProfile,
    DeleteAuthProfile(String),
    ProfileConnectors(String),
    SaveProfileConnector(String, String),
    VerifyProfileConnector(String, String),
    Palette(String),
    PaletteProfile(String, String),
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
        ("GET", ["api", "connectors"]) => Route::Connectors,
        ("GET", ["api", "connectors", "project", name]) => {
            Route::ProjectConnectors((*name).to_string())
        }
        ("POST", ["api", "connectors", "project", name, cap]) => {
            Route::SaveProjectConnector((*name).to_string(), (*cap).to_string())
        }
        ("POST", ["api", "connectors", "project", name, cap, "verify"]) => {
            Route::VerifyProjectConnector((*name).to_string(), (*cap).to_string())
        }
        ("POST", ["api", "connectors", cap]) => Route::SaveConnector((*cap).to_string()),
        ("POST", ["api", "connectors", cap, "verify"]) => Route::VerifyConnector((*cap).to_string()),
        ("GET", ["api", "auth-profile", name, "secrets"]) => {
            Route::AuthProfileSecrets((*name).to_string())
        }
        ("GET", ["api", "auth-profiles"]) => Route::AuthProfiles,
        ("POST", ["api", "auth-profiles"]) => Route::CreateAuthProfile,
        ("DELETE", ["api", "auth-profiles", name]) => Route::DeleteAuthProfile((*name).to_string()),
        ("GET", ["api", "auth-profiles", name, "connectors"]) => {
            Route::ProfileConnectors((*name).to_string())
        }
        ("POST", ["api", "auth-profiles", name, "connectors", cap]) => {
            Route::SaveProfileConnector((*name).to_string(), (*cap).to_string())
        }
        ("POST", ["api", "auth-profiles", name, "connectors", cap, "verify"]) => {
            Route::VerifyProfileConnector((*name).to_string(), (*cap).to_string())
        }
        ("GET", ["api", "profile", id]) => Route::Profile((*id).to_string()),
        ("GET", ["api", "profile", id, "convention", cid]) => {
            Route::Convention((*id).to_string(), (*cid).to_string())
        }
        ("GET", ["api", "profile", id, "recipe", rid]) => {
            Route::Recipe((*id).to_string(), (*rid).to_string())
        }
        ("GET", ["api", "profile", id, "convention", cid, "raw"]) => {
            Route::ConventionRaw((*id).to_string(), (*cid).to_string())
        }
        ("GET", ["api", "profile", id, "recipe", rid, "raw"]) => {
            Route::RecipeRaw((*id).to_string(), (*rid).to_string())
        }
        ("GET", ["api", "profile", id, "palette"]) => Route::Palette((*id).to_string()),
        ("GET", ["api", "profile", id, "palette", other]) => {
            Route::PaletteProfile((*id).to_string(), (*other).to_string())
        }
        ("POST", ["api", "profile"]) => Route::CreateProfile,
        ("POST", ["api", "profile", id, "convention"]) => Route::AddConvention((*id).to_string()),
        ("POST", ["api", "profile", id, "recipe"]) => Route::AddRecipe((*id).to_string()),
        ("POST", ["api", "profile", id, "flows"]) => Route::SetFlows((*id).to_string()),
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
        ("GET", ["api", "project", name, "docs"]) => Route::ProjectDocs((*name).to_string()),
        ("GET", ["api", "project", name, "docs", doc]) => {
            Route::ProjectDoc((*name).to_string(), (*doc).to_string())
        }
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

/// Placeholder in `index.html` replaced with the per-session token at serve time.
const TOKEN_PLACEHOLDER: &str = "__PALUGADA_TOKEN__";

/// Mint a 256-bit random hex session token from the OS CSPRNG. It is injected
/// into the served `index.html` and required on every `/api/*` request. A
/// cross-origin web page cannot read the served HTML (the browser blocks it),
/// so it cannot learn the token and therefore cannot forge API calls — this is
/// the primary CSRF defense that the loopback + Host guard alone does NOT give.
fn new_session_token() -> Result<String, String> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).map_err(|e| format!("OS RNG unavailable: {e}"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compare the request's token against the session token without an early-exit
/// timing signal. `got` is `None` when the header is absent.
pub fn token_ok(expected: &str, got: Option<&str>) -> bool {
    match got {
        Some(g) if g.len() == expected.len() && !expected.is_empty() => {
            let mut diff = 0u8;
            for (a, b) in expected.bytes().zip(g.bytes()) {
                diff |= a ^ b;
            }
            diff == 0
        }
        _ => false,
    }
}

/// Reject cross-site requests. `Sec-Fetch-Site` (set by modern browsers, not
/// settable by page JS) must be `same-origin`/`none` when present; otherwise
/// `Origin`, when present, must be a loopback origin. When neither header is
/// present (non-browser clients) the request is allowed here and gated solely
/// by the session token.
pub fn origin_ok(sec_fetch_site: Option<&str>, origin: Option<&str>) -> bool {
    if let Some(sfs) = sec_fetch_site {
        return matches!(sfs, "same-origin" | "none");
    }
    if let Some(o) = origin {
        return origin_is_loopback(o);
    }
    true
}

fn origin_is_loopback(origin: &str) -> bool {
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"));
    match rest {
        Some(r) => {
            let host = r.split(['/', ':']).next().unwrap_or("");
            host == "localhost" || host == "127.0.0.1"
        }
        None => false,
    }
}

/// Read a request header by name (case-insensitive), owned copy.
fn req_header(request: &tiny_http::Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.to_string().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_string())
}

pub fn run(port: u16, open: bool) -> Result<(), String> {
    let addr = format!("127.0.0.1:{port}");
    let server = tiny_http::Server::http(&addr).map_err(|e| format!("bind {addr}: {e}"))?;
    let token = new_session_token()?;
    // Report the ACTUAL bound address — with `--port 0` the OS picks the port,
    // and tests/tools read it from this line.
    let bound = server.server_addr().to_ip().map(|a| a.to_string()).unwrap_or(addr);
    let url = format!("http://{bound}");
    println!("palugada web → {url}   (Ctrl-C to stop)");
    if open {
        open_browser(&url);
    }
    for request in server.incoming_requests() {
        handle(request, &token);
    }
    Ok(())
}

fn handle(mut request: tiny_http::Request, token: &str) {
    let host = req_header(&request, "Host").unwrap_or_default();
    if !host_ok(&host) {
        let _ = request.respond(text(403, "forbidden host"));
        return;
    }
    let method = request.method().as_str().to_string();
    let path = request.url().split('?').next().unwrap_or("/").to_string();
    match route(&method, &path) {
        Route::Index => {
            // Inject the per-session token so app.js can echo it back on every
            // API call. A cross-origin page cannot read this response, so the
            // token stays secret from a CSRF attacker.
            let html = INDEX_HTML.replace(TOKEN_PLACEHOLDER, token);
            let _ = request.respond(body(200, "text/html; charset=utf-8", html));
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
            // CSRF defense on every /api/* route: reject cross-site requests and
            // require the per-session token. Either check failing → 403, before
            // any handler (which may read/verify credentials) runs.
            let sfs = req_header(&request, "Sec-Fetch-Site");
            let origin = req_header(&request, "Origin");
            if !origin_ok(sfs.as_deref(), origin.as_deref()) {
                let _ = request.respond(json_resp(403, err_json("cross-site request refused")));
                return;
            }
            if !token_ok(token, req_header(&request, "X-Palugada-Token").as_deref()) {
                let _ =
                    request.respond(json_resp(403, err_json("missing or invalid session token")));
                return;
            }
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
            let md = crate::inherit::resolve_convention_raw(&kn, &id, &cid)?
                .ok_or_else(|| format!("no convention '{cid}' in profile '{id}' or its parents"))?;
            Ok(json!({ "markdown": md }))
        }),
        Route::Recipe(id, rid) => read(|| {
            let kn = knowledge_dir()?;
            let md = crate::inherit::resolve_recipe_raw(&kn, &id, &rid)?
                .ok_or_else(|| format!("no recipe '{rid}' in profile '{id}' or its parents"))?;
            Ok(json!({ "markdown": md }))
        }),
        // The LOCAL (un-merged) body of the profile's own file — what `editDoc`
        // prefills, so Save writes back the child's verbatim body and never
        // freezes the merged inheritance into a whole-body copy.
        Route::ConventionRaw(id, cid) => read(|| {
            let kn = knowledge_dir()?;
            Ok(json!({ "markdown": crate::knowledge::convention_md(&kn, &id, &cid)? }))
        }),
        Route::RecipeRaw(id, rid) => read(|| {
            let kn = knowledge_dir()?;
            Ok(json!({ "markdown": crate::knowledge::recipe_md(&kn, &id, &rid)? }))
        }),
        Route::Palette(id) => read(|| {
            let kn = knowledge_dir()?;
            serde_json::to_value(crate::palette::palette(&kn, &id)?).map_err(|e| e.to_string())
        }),
        Route::PaletteProfile(_id, other) => read(|| {
            let kn = knowledge_dir()?;
            Ok(json!({ "sections": crate::palette::profile_sections(&kn, &other)? }))
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
        Route::SetFlows(id) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Req {
                #[serde(default)]
                flows: std::collections::BTreeMap<String, Vec<String>>,
            }
            let kn = knowledge_dir()?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::profile::set_flows(&kn, &id, &req.flows)?;
            Ok(json!({ "ok": true, "flows": req.flows.len() }))
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
        Route::ProjectDocs(name) => read(|| {
            let dir = docs_dir(&name)?;
            let docs: Vec<serde_json::Value> = crate::personal::list(&dir)?
                .iter()
                .map(|n| {
                    let (title, source, fetched_at) = crate::personal::doc_summary(&dir, n);
                    json!({ "name": n, "title": title, "source": source, "fetched_at": fetched_at })
                })
                .collect();
            Ok(json!({ "docs": docs }))
        }),
        Route::ProjectDoc(name, doc) => read(|| {
            let dir = docs_dir(&name)?;
            let doc = crate::http::decode_segment(&doc);
            Ok(json!({ "name": doc, "body": crate::personal::cat(&dir, &doc)? }))
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
        Route::Connectors => read(|| crate::credentials::global_view("default")),
        Route::SaveConnector(cap) => write_op(|| crate::credentials::apply_global("default", &cap, body)),
        Route::VerifyConnector(cap) => read(|| crate::credentials::global_verify("default", &cap, body)),
        Route::ProjectConnectors(name) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            crate::credentials::project_connectors_view(&global, &name)
        }),
        Route::SaveProjectConnector(name, cap) => write_op(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            crate::credentials::apply_project_connector(&global, &name, &cap, body)
        }),
        Route::VerifyProjectConnector(name, cap) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            crate::credentials::project_verify(&global, &name, &cap, body)
        }),
        Route::AuthProfileSecrets(name) => read(|| {
            let name = crate::http::decode_segment(&name);
            crate::credentials::auth_profile_secrets(&name)
        }),
        Route::AuthProfiles => read(crate::credentials::list_auth_profiles_view),
        Route::CreateAuthProfile => write_op(|| crate::credentials::create_auth_profile(body)),
        Route::DeleteAuthProfile(name) => write_op(|| {
            let name = crate::http::decode_segment(&name);
            crate::credentials::delete_auth_profile(&name)
        }),
        Route::ProfileConnectors(name) => read(|| {
            let name = crate::http::decode_segment(&name);
            crate::credentials::global_view(&name)
        }),
        Route::SaveProfileConnector(name, cap) => write_op(|| {
            let name = crate::http::decode_segment(&name);
            crate::credentials::apply_global(&name, &cap, body)
        }),
        Route::VerifyProfileConnector(name, cap) => read(|| {
            let name = crate::http::decode_segment(&name);
            crate::credentials::global_verify(&name, &cap, body)
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
/// The per-project fetched-docs cache dir for a registered project (web side).
fn docs_dir(name: &str) -> Result<std::path::PathBuf, String> {
    Ok(crate::config::expand_home(&project_repo(name)?).join(".palugada").join("docs"))
}

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
        #[serde(default)]
        extends: Option<String>,
    }
    let np: NewProfile = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let kn = knowledge_dir()?;
    let extends = np.extends.as_deref().filter(|s| !s.is_empty());
    crate::profile::scaffold_new(&kn, &np.id, extends)?;
    // Apply the chosen title / languages over the generated profile.yaml.
    if !np.title.is_empty() || !np.languages.is_empty() {
        let pf = kn.join("profiles").join(&np.id).join("profile.yaml");
        let mut raw = std::fs::read_to_string(&pf).map_err(|e| e.to_string())?;
        if !np.title.is_empty() {
            // Flat profiles scaffold `title: "{id} profile"`; an extends-child is
            // copy-seeded from the parent, so its title line carries the PARENT's
            // title. A literal replace would no-op there and silently drop the
            // chosen title — so replace the whole `title:` line (like languages).
            let title = format!("title: \"{}\"", np.title.replace('"', "'"));
            if let Some(start) = raw.find("\ntitle:") {
                let line_start = start + 1;
                let line_end = raw[line_start..].find('\n').map(|i| line_start + i).unwrap_or(raw.len());
                raw.replace_range(line_start..line_end, &title);
            }
        }
        if !np.languages.is_empty() {
            // Flat profiles scaffold `languages: []`; an extends-child copies the
            // parent's `languages: [...]`. Replace whichever line is present.
            let langs = format!("languages: [{}]", np.languages.join(", "));
            if let Some(start) = raw.find("\nlanguages:") {
                let line_start = start + 1;
                let line_end = raw[line_start..].find('\n').map(|i| line_start + i).unwrap_or(raw.len());
                raw.replace_range(line_start..line_end, &langs);
            }
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
        // generate() never indexes; the web console stays non-blocking.
        no_index: true,
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
    let chain = crate::inherit::resolve_chain(&kn, id)?;
    Ok(json!({
        "id": id,
        "extends": crate::inherit::read_extends(&kn, id),
        "chain": chain,
        "conventions": jv(&crate::inherit::merged_conventions_provenance(&kn, id)?),
        "recipes": jv(&crate::inherit::merged_recipes_provenance(&kn, id)?),
        "fact_families": crate::indexer::fact_families(&kn, id).unwrap_or_default(),
        "flows": jv(&flows(&kn, id).unwrap_or_default()),
    }))
}

/// The profile's flow → step-list map, read from `profile.yaml`.
fn flows(kn: &std::path::Path, profile: &str) -> Result<std::collections::BTreeMap<String, Vec<String>>, String> {
    Ok(crate::manifest::ProfileManifest::load(kn, profile)?.flows)
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
        // fonts are no longer bundled — .woff2 requests fall through to NotFound
        assert_eq!(route("GET", "/bangers.woff2"), Route::NotFound);
        assert_eq!(route("GET", "/api/profile/android-mvvm"), Route::Profile("android-mvvm".into()));
        assert_eq!(
            route("GET", "/api/profile/p/convention/arch"),
            Route::Convention("p".into(), "arch".into())
        );
        assert_eq!(
            route("GET", "/api/profile/p/convention/arch/raw"),
            Route::ConventionRaw("p".into(), "arch".into())
        );
        assert_eq!(
            route("GET", "/api/profile/p/recipe/feature/raw"),
            Route::RecipeRaw("p".into(), "feature".into())
        );
        assert_eq!(route("GET", "/api/profile/p/palette"), Route::Palette("p".into()));
        assert_eq!(
            route("GET", "/api/profile/p/palette/other"),
            Route::PaletteProfile("p".into(), "other".into())
        );
        assert_eq!(route("POST", "/api/profile"), Route::CreateProfile);
        assert_eq!(route("POST", "/api/profile/p/convention"), Route::AddConvention("p".into()));
        assert_eq!(route("POST", "/api/profile/p/flows"), Route::SetFlows("p".into()));
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
        assert_eq!(route("GET", "/api/connectors"), Route::Connectors);
        assert_eq!(route("POST", "/api/connectors/git_host"), Route::SaveConnector("git_host".into()));
        assert_eq!(
            route("POST", "/api/connectors/git_host/verify"),
            Route::VerifyConnector("git_host".into())
        );
        assert_eq!(
            route("GET", "/api/connectors/project/app"),
            Route::ProjectConnectors("app".into())
        );
        assert_eq!(
            route("POST", "/api/connectors/project/app/wiki"),
            Route::SaveProjectConnector("app".into(), "wiki".into())
        );
        assert_eq!(
            route("POST", "/api/connectors/project/app/wiki/verify"),
            Route::VerifyProjectConnector("app".into(), "wiki".into())
        );
        assert_eq!(
            route("GET", "/api/auth-profile/default/secrets"),
            Route::AuthProfileSecrets("default".into())
        );
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

    #[test]
    fn session_token_is_64_hex_chars_and_fresh_each_call() {
        let a = new_session_token().unwrap();
        let b = new_session_token().unwrap();
        assert_eq!(a.len(), 64, "256 bits = 64 hex chars");
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "each session gets a distinct token");
    }

    #[test]
    fn token_guard_accepts_exact_and_rejects_wrong_or_missing() {
        let tok = "deadbeef";
        assert!(token_ok(tok, Some("deadbeef")));
        assert!(!token_ok(tok, Some("deadbeff")), "one byte off → reject");
        assert!(!token_ok(tok, Some("deadbe")), "wrong length → reject");
        assert!(!token_ok(tok, None), "missing header → reject");
        assert!(!token_ok("", Some("")), "empty session token never matches");
    }

    #[test]
    fn origin_guard_rejects_cross_site_requests() {
        // Modern browsers: Sec-Fetch-Site is authoritative.
        assert!(origin_ok(Some("same-origin"), None));
        assert!(origin_ok(Some("none"), None));
        assert!(!origin_ok(Some("cross-site"), None), "a random web page is cross-site");
        assert!(!origin_ok(Some("same-site"), None), "another local port is not same-origin");
        // Fallback to Origin when Sec-Fetch-Site is absent.
        assert!(origin_ok(None, Some("http://127.0.0.1:7777")));
        assert!(origin_ok(None, Some("http://localhost:7777")));
        assert!(!origin_ok(None, Some("https://attacker.example")));
        // Neither header (curl / non-browser) → allowed here; token still gates.
        assert!(origin_ok(None, None));
    }

    #[test]
    fn index_serves_token_in_place_of_placeholder() {
        // The shipped index.html carries the placeholder the server substitutes.
        assert!(INDEX_HTML.contains(TOKEN_PLACEHOLDER));
        let served = INDEX_HTML.replace(TOKEN_PLACEHOLDER, "abc123");
        assert!(served.contains("content=\"abc123\""));
        assert!(!served.contains(TOKEN_PLACEHOLDER));
    }

    #[test]
    fn profile_json_exposes_extends_chain_and_provenance() {
        let kn = tempfile::tempdir().unwrap();
        // base + child(extends base), child overrides a section
        for (p, ext) in [("base", None), ("kid", Some("base"))] {
            let d = kn.path().join("profiles").join(p);
            std::fs::create_dir_all(d.join("conventions")).unwrap();
            std::fs::create_dir_all(d.join("recipes")).unwrap();
            let mut y = format!("id: {p}\nfact_families:\n  - {{ id: symbol, symbol: true }}\n");
            if let Some(e) = ext { y.push_str(&format!("extends: {e}\n")); }
            std::fs::write(d.join("profile.yaml"), y).unwrap();
            std::fs::write(d.join("extractors.yaml"), "families:\n  - id: symbol\n    regex: 'x'\n").unwrap();
            std::fs::write(d.join("recipes/_index.json"), r#"{"recipes":[]}"#).unwrap();
        }
        crate::knowledge::add_convention_in(&kn.path().join("profiles/base/conventions"),
            &crate::knowledge::ConventionSpec { id: "arch".into(), title: "Arch".into(), description: "d".into(), tags: vec![],
                sections: vec![crate::knowledge::SectionSpec { title: "Layers".into(), body: "L".into(), code: false }] }).unwrap();
        std::fs::write(kn.path().join("profiles/kid/conventions/_index.json"), r#"{"topics":[]}"#).unwrap();

        std::env::set_var("PALUGADA_KNOWLEDGE", kn.path());
        let v = profile_json("kid").unwrap();
        std::env::remove_var("PALUGADA_KNOWLEDGE");

        assert_eq!(v["extends"], "base");
        assert_eq!(v["chain"][0], "kid");
        let arch = v["conventions"].as_array().unwrap().iter().find(|c| c["id"] == "arch").unwrap();
        assert_eq!(arch["origin"], "inherited");
        assert_eq!(arch["from"], "base");
    }

    /// editDoc must prefill from the child's LOCAL body, not the merged view.
    /// `convention_md` (what the `/raw` route returns) shows ONLY the child's own
    /// section; `resolve_convention_raw` (what viewDoc/renderDoc shows) merges in
    /// the parent's. Pinning this prevents Save from freezing inheritance into a
    /// whole-body copy of the merged view.
    #[test]
    fn raw_convention_body_is_local_not_merged() {
        let kn = tempfile::tempdir().unwrap();
        // base defines `arch` with a `Layers` section; child overrides `arch`
        // with only a `DataFlow` section.
        for (p, ext) in [("base", None), ("child", Some("base"))] {
            let d = kn.path().join("profiles").join(p);
            std::fs::create_dir_all(d.join("conventions")).unwrap();
            std::fs::create_dir_all(d.join("recipes")).unwrap();
            let mut y = format!("id: {p}\nfact_families:\n  - {{ id: symbol, symbol: true }}\n");
            if let Some(e) = ext { y.push_str(&format!("extends: {e}\n")); }
            std::fs::write(d.join("profile.yaml"), y).unwrap();
        }
        crate::knowledge::add_convention_in(&kn.path().join("profiles/base/conventions"),
            &crate::knowledge::ConventionSpec { id: "arch".into(), title: "Arch".into(), description: "d".into(), tags: vec![],
                sections: vec![crate::knowledge::SectionSpec { title: "Layers".into(), body: "L".into(), code: false }] }).unwrap();
        crate::knowledge::add_convention_in(&kn.path().join("profiles/child/conventions"),
            &crate::knowledge::ConventionSpec { id: "arch".into(), title: "Arch".into(), description: "d".into(), tags: vec![],
                sections: vec![crate::knowledge::SectionSpec { title: "DataFlow".into(), body: "F".into(), code: false }] }).unwrap();

        // RAW (what edit prefills): child's own body only — no parent's Layers.
        let raw = crate::knowledge::convention_md(kn.path(), "child", "arch").unwrap();
        assert!(raw.contains("DataFlow"), "raw should contain the child's section");
        assert!(!raw.contains("Layers"), "raw must NOT contain the parent's section");

        // MERGED (what viewDoc shows): both the child's and the parent's sections.
        let merged = crate::inherit::resolve_convention_raw(kn.path(), "child", "arch").unwrap().unwrap();
        assert!(merged.contains("DataFlow"), "merged should contain the child's section");
        assert!(merged.contains("Layers"), "merged should contain the parent's section");
    }
}
