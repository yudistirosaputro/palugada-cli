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
    Init,
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
        ("POST", ["api", "init"]) => Route::Init,
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

/// JSON API dispatch. Read handlers here; write handlers added next.
fn api(route: Route, _body: &str) -> (u16, String) {
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
        _ => (501, err_json("not implemented yet")),
    }
}

fn read<F: FnOnce() -> Result<serde_json::Value, String>>(f: F) -> (u16, String) {
    match f() {
        Ok(v) => (200, v.to_string()),
        Err(e) => (500, err_json(&e)),
    }
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
        .map(|(name, e)| json!({ "name": name, "repo_path": e.repo_path, "active": *name == active }))
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
        assert_eq!(route("POST", "/api/init"), Route::Init);
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
