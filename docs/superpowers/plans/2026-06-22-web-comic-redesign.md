# palugada web — Comic Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the `palugada web` console to the approved "Pop Workbench" comic identity (light-first + dark toggle), without changing any API or business logic.

**Architecture:** Static assets stay embedded in the binary and served by the synchronous `tiny_http` server in `src/web.rs`. The visual change lives in `style.css` (full rewrite) and `index.html`/`app.js` (structure + classes). Three OFL woff2 fonts are bundled and served from new static routes. The JSON API is untouched.

**Tech Stack:** Rust (`tiny_http`), vanilla JS (no build step), CSS custom properties, bundled Bangers + Comic Neue woff2.

## Global Constraints

- No framework, no build step; assets embedded via `include_str!`/`include_bytes!`.
- No new/changed JSON API endpoints or payloads. Only new static routes are the three `*.woff2`.
- Server is loopback-only; the `host_ok` check stays.
- Semantic colors carry meaning and must be preserved: conventions=green, recipes=gold, review_map=blue, engine=violet, warnings=amber, errors=red, success=green.
- Fonts load only from local routes — never an external host (offline-first).
- Reduced-motion respected; nav items and clickable rows keyboard-reachable; row actions revealed on hover **and** focus **and** touch.
- **Visual source of truth:** the approved mockup `scratchpad/palugada-redesign-mockup.html` (artifact version `v3-elegant-comic`). Its `<style>` block and DOM are the verbatim reference for `style.css` and the class structure.

---

### Task 1: Bundle and serve the three woff2 fonts

**Files:**
- Add: `src/web/bangers.woff2`, `src/web/comic400.woff2`, `src/web/comic700.woff2` (already copied in)
- Modify: `src/web.rs` (const embeds, `Route` enum, `route()`, `handle()`, response helper, route test)

**Interfaces:**
- Produces: GET `/bangers.woff2`, `/comic400.woff2`, `/comic700.woff2` → `200` `font/woff2` with `Cache-Control: public, max-age=31536000, immutable`. Unknown `*.woff2` → `404`.

- [ ] **Step 1: Add the binary embeds** near the existing `const STYLE_CSS` (after line 14):

```rust
const FONT_BANGERS: &[u8] = include_bytes!("web/bangers.woff2");
const FONT_COMIC_400: &[u8] = include_bytes!("web/comic400.woff2");
const FONT_COMIC_700: &[u8] = include_bytes!("web/comic700.woff2");
```

- [ ] **Step 2: Add a `Font` route variant** to the `Route` enum (after `StyleCss,`):

```rust
    Font(String),
```

- [ ] **Step 3: Match font paths** in `route()` — add this arm immediately after the `style.css` arm (before the `api` arms is fine; it must be before `_ => Route::NotFound`):

```rust
        ("GET", [name]) if name.ends_with(".woff2") => Route::Font((*name).to_string()),
```

- [ ] **Step 4: Serve the bytes** in `handle()` — add an arm after the `Route::StyleCss` arm:

```rust
        Route::Font(name) => {
            let bytes: &[u8] = match name.as_str() {
                "bangers.woff2" => FONT_BANGERS,
                "comic400.woff2" => FONT_COMIC_400,
                "comic700.woff2" => FONT_COMIC_700,
                _ => &[],
            };
            if bytes.is_empty() {
                let _ = request.respond(json_resp(404, err_json("not found")));
            } else {
                let _ = request.respond(font_asset(bytes));
            }
        }
```

- [ ] **Step 5: Add the binary response helper** next to `asset()` (after line 564):

```rust
fn font_asset(bytes: &[u8]) -> Resp {
    tiny_http::Response::from_data(bytes.to_vec())
        .with_status_code(200)
        .with_header(header("Content-Type", "font/woff2"))
        .with_header(header("Cache-Control", "public, max-age=31536000, immutable"))
}
```

- [ ] **Step 6: Add a route unit test** inside `mod tests` `route_parses_paths` (or as a new assertion):

```rust
        assert_eq!(route("GET", "/bangers.woff2"), Route::Font("bangers.woff2".into()));
        assert_eq!(route("GET", "/nope.woff2"), Route::Font("nope.woff2".into()));
```

- [ ] **Step 7: Build + test**

Run: `cargo test --lib web 2>&1 | tail -20 ; cargo build 2>&1 | tail -5`
Expected: route test passes; build succeeds (binary now embeds the fonts).

- [ ] **Step 8: Commit**

```bash
git add src/web/bangers.woff2 src/web/comic400.woff2 src/web/comic700.woff2 src/web.rs
git commit -m "feat(web): bundle + serve Bangers/Comic Neue woff2 fonts"
```

---

### Task 2: Rewrite `style.css` to the Pop Workbench system

**Files:**
- Modify (full rewrite): `src/web/style.css`

**Interfaces:**
- Produces: CSS classes consumed by Task 3/4 — `topbar .brand .path-chip .theme-toggle .menu-btn`, `sidebar .nav-group-label .nav-item .nav-count`, `view-head .eyebrow h1 .subtitle .back-link`, `card .card-head .count .card-note`, `stat-grid .stat`, `kv-row .kk .vv`, `list .lrow .id-chip .ttl .meta .actions .ghost-btn`, `pill .sticker(.active/.new/.dot)`, `flow .flow-head .flow-name .flow-cmd .steps .step .num .step-tag(.convention/.recipe/.review_map/.engine) .arg .hint`, `table-scroll table.rules .origin(-profile/-project/-overridden)`, `btn(.secondary) .field .form-row .empty`, plus `[data-theme="dark"]` overrides and `.screen/.screen.on`.

- [ ] **Step 1: Copy the mockup's CSS verbatim.** Take everything inside the mockup `<style>` block from the comment `/* =====... Pop Workbench ... */` through the end (i.e., **excluding** the injected `@font-face` data-URI block and **excluding** the mockup-only `.mock-note` rule). This is the entire token system + component CSS.

- [ ] **Step 2: Prepend the local `@font-face` block** (replaces the mockup's base64 version — points at the routes from Task 1):

```css
@font-face { font-family: "Bangers"; src: url("/bangers.woff2") format("woff2"); font-weight: 400; font-style: normal; font-display: swap; }
@font-face { font-family: "Comic Neue"; src: url("/comic400.woff2") format("woff2"); font-weight: 400; font-style: normal; font-display: swap; }
@font-face { font-family: "Comic Neue"; src: url("/comic700.woff2") format("woff2"); font-weight: 700; font-style: normal; font-display: swap; }
```

- [ ] **Step 3: Add the toast rule** (the mockup has no toast; the real app does). Append, themed to match:

```css
.toast { position: fixed; right: 16px; bottom: 16px; z-index: 50; padding: 10px 14px;
  border: var(--bw) solid var(--ink); border-radius: var(--r); background: var(--conv-bg);
  color: var(--ink); font-weight: 700; box-shadow: var(--shadow); opacity: 0;
  transform: translateY(6px); transition: opacity .2s, transform .2s; pointer-events: none; max-width: 380px; }
.toast.show { opacity: 1; transform: none; }
.toast.err { background: var(--err-bg); }
```

- [ ] **Step 4: Add section-row / candidate / import helper classes** used by the add-convention/import forms (mockup doesn't show them; keep them styled consistently):

```css
.section-row, .candidate, .cd-int { border: var(--bw) dashed var(--ink); border-radius: var(--r-sm);
  padding: var(--s3); margin: var(--s3) 0; background: var(--surface-2); }
.im-text, .im-preview { width: 100%; }
.warn-pill { color: var(--warn); font-weight: 700; font-size: 12px; margin-left: 6px; }
.ok-pill { color: var(--ok); font-weight: 700; font-size: 12px; margin-left: 6px; }
.warn { color: var(--warn); }
.card.warn { border-color: var(--warn); box-shadow: 3px 3px 0 var(--warn); }
.spacer { flex: 1; }
a.link { color: var(--accent); cursor: pointer; text-decoration: none; font-weight: 700; }
a.link:hover { text-decoration: underline; }
pre { background: var(--surface-2); border: var(--bw) solid var(--ink); border-radius: var(--r);
  padding: var(--s3); overflow: auto; white-space: pre-wrap; font-family: var(--font-mono); }
```

- [ ] **Step 5: Verify** the file parses and has no `data:font` references and no `.mock-note`:

Run: `grep -c "data:font" src/web/style.css ; grep -c "mock-note" src/web/style.css ; grep -c "@font-face" src/web/style.css`
Expected: `0`, `0`, `3`.

- [ ] **Step 6: Commit**

```bash
git add src/web/style.css
git commit -m "feat(web): Pop Workbench stylesheet (comic, light+dark)"
```

---

### Task 3: Update `index.html` chrome

**Files:**
- Modify: `src/web/index.html`

**Interfaces:**
- Consumes: classes from Task 2.
- Produces: `#themeBtn` `#themeLabel` `#menuBtn` `#sidebar` for `app.js` (Task 4) to wire.

- [ ] **Step 1: Replace the body markup** with the new chrome (keep `<head>` link to `/style.css`; add a tiny pre-paint theme script so dark mode doesn't flash):

```html
<body>
  <script>try{var t=localStorage.getItem("palugada-theme");if(t)document.documentElement.setAttribute("data-theme",t);}catch(e){}</script>
  <header class="topbar">
    <button class="menu-btn" id="menuBtn" aria-label="Menu"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><path d="M4 6h16M4 12h16M4 18h16"/></svg></button>
    <div class="brand"><span class="mark">🛠</span><span class="name">palugada</span></div>
    <span class="path-chip"><span class="lbl">knowledge</span><span id="kn-dir">…</span></span>
    <span class="grow"></span>
    <button class="theme-toggle" id="themeBtn" aria-label="Toggle light/dark">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>
      <span id="themeLabel">Dark</span>
    </button>
  </header>
  <div class="shell">
    <nav class="sidebar" id="sidebar">
      <div class="nav-group-label">Console</div>
      <a class="nav-item active" data-view="overview"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><rect x="3" y="3" width="7" height="9" rx="1.5"/><rect x="14" y="3" width="7" height="5" rx="1.5"/><rect x="14" y="12" width="7" height="9" rx="1.5"/><rect x="3" y="16" width="7" height="5" rx="1.5"/></svg>Overview</a>
      <a class="nav-item" data-view="projects"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>Projects</a>
      <a class="nav-item" data-view="profiles"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><path d="M4 5h16M4 12h16M4 19h10"/></svg>Profiles</a>
      <a class="nav-item" data-view="knowledge"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><path d="M4 5a2 2 0 0 1 2-2h12v18H6a2 2 0 0 1-2-2z"/><path d="M8 7h7M8 11h7"/></svg>Knowledge</a>
    </nav>
    <main class="view" id="view">Loading…</main>
  </div>
  <div id="toast" class="toast"></div>
  <script src="/app.js"></script>
</body>
```

- [ ] **Step 2: Commit**

```bash
git add src/web/index.html
git commit -m "feat(web): comic topbar + sidebar chrome, theme toggle, mobile menu"
```

---

### Task 4: Update `app.js` rendering to the new structure

**Files:**
- Modify: `src/web/app.js`

**Interfaces:**
- Consumes: classes from Task 2; `#themeBtn #themeLabel #menuBtn #sidebar #kn-dir` from Task 3.
- Produces: no exports; same API calls as today.

This task only changes the HTML strings each render function emits and adds chrome wiring. Logic, endpoints, and event handlers stay. Apply per-area:

- [ ] **Step 1: Chrome wiring (boot).** At the bottom near `setView("overview")`, add theme-toggle + mobile-drawer handlers:

```js
// theme toggle (persisted; pre-paint script in index.html applies it on load)
(function () {
  const btn = document.getElementById("themeBtn");
  const label = document.getElementById("themeLabel");
  const sync = () => { const d = document.documentElement.getAttribute("data-theme") === "dark"; label.textContent = d ? "Light" : "Dark"; };
  btn.onclick = () => {
    const d = document.documentElement.getAttribute("data-theme") === "dark";
    const next = d ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", next);
    try { localStorage.setItem("palugada-theme", next); } catch (e) {}
    sync();
  };
  sync();
  const sidebar = document.getElementById("sidebar");
  const menu = document.getElementById("menuBtn");
  if (menu) menu.onclick = () => sidebar.classList.toggle("open");
})();
```

- [ ] **Step 2: View-head helper.** Add a helper and use it at the top of each `render*` instead of bare `<h2>`:

```js
function viewHead(eyebrow, title, subtitle) {
  return `<div class="view-head"><div class="eyebrow">${esc(eyebrow)}</div>` +
    `<h1>${esc(title)}</h1>` + (subtitle ? `<p class="subtitle">${subtitle}</p>` : "") + `</div>`;
}
```
Replace `view.innerHTML = "<h2>Overview</h2>"` → `view.innerHTML = viewHead("Workspace", "Overview", "Your engineering know-how, indexed once and served token-cheap.")`; and likewise for Projects ("Repos"/"Projects"), Profiles ("Knowledge"/"Profiles"), Knowledge ("Browse"/"Knowledge"). For detail screens use a `.back-link` (see Step 6) then `viewHead`.

- [ ] **Step 3: Overview card** — wrap the key/values in `.kv-row`s and keep `#kn-dir` populated. Replace the overview card template:

```js
view.appendChild(h(`<div class="card"><div class="card-head"><h3>Workspace</h3></div>
  <div class="kv-row"><span class="kk">Knowledge dir</span><span class="vv mono">${esc(o.knowledge_dir)}</span></div>
  <div class="kv-row"><span class="kk">Active project</span><span class="vv">${o.active_project ? esc(o.active_project)+' <span class="sticker active dot">active</span>' : '<span class="muted">(none)</span>'}</span></div>
  <div class="kv-row"><span class="kk">Default profile</span><span class="vv">${o.default_profile ? '<span class="id-chip">'+esc(o.default_profile)+'</span>' : '<span class="muted">(none)</span>'}</span></div>
  <div class="kv-row"><span class="kk">Counts</span><span class="vv">${o.profile_count} profiles · ${o.project_count} projects</span></div></div>`));
```
Keep the existing `document.getElementById("kn-dir").textContent = o.knowledge_dir;` line.

- [ ] **Step 4: Card headers.** Everywhere a card starts with `<h3>X</h3>`, wrap as `<div class="card-head"><h3>X</h3></div>` (Conventions, Recipes, Fact families, Flows, Effective Rules, Credentials, Generate, Import). Where a count or an add-button belongs on that header row, use `<div class="card-head"><h3>X</h3><span class="count">N</span><span class="grow"></span>…</div>`.

- [ ] **Step 5: List rows (conventions, recipes, profiles, knowledge lists).** Replace the `.row`-with-link pattern with `.lrow` + `id-chip` + actions. Example for conventions in `renderProfileDetail`:

```js
d.conventions.forEach(c => {
  const row = h(`<div class="lrow"><span class="id-chip">${esc(c.id)}</span><span class="ttl">${esc(c.title)}</span><span class="meta">· ${c.sections.length} sections</span><span class="actions"><a class="link r-view">View</a></span></div>`);
  row.querySelector(".r-view").onclick = async () => {
    try { const b = await api(`/api/profile/${id}/convention/${c.id}`); showBody(c.id, b.markdown, row); }
    catch (e) { toast(e.message, true); }
  };
  cv.appendChild(row);
});
```
Apply the same shape to recipes, the profiles list (`renderProfiles`), and both lists in `renderKnowledge`. Keep each existing `onclick` body.

- [ ] **Step 6: Back link.** Replace `<p><a class="link" id="back">← projects</a></p>` (in `renderProjectDetail`/`renderProfileDetail`) with:

```js
`<a class="link back-link" id="back"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M15 18l-6-6 6-6"/></svg>${kind}</a>` // kind = "Projects" | "Profiles"
```

- [ ] **Step 7: Numbered flows (`stepRow`/skillCard flow render).** The profile-detail Flows card (`flowsCard`) renders editable chips — keep its edit UI but restyle chips with the existing `.step-chip` class (already in CSS via Task 4 fallback) OR leave functional. For the **read-only** skill-map flow render in `stepRow`, emit numbered steps. Replace `stepRow` to wrap each step with a number and `.step-tag`:

```js
function stepRow(profile, st, n) {
  const num = `<span class="num">${n}</span>`;
  if (st.kind === "engine")
    return h(`<div class="step">${num}<span class="step-tag engine">engine</span><span class="arg">${esc(st.token)}</span><span class="hint">— ${esc(st.label)}</span></div>`);
  if (st.kind === "review_map") {
    const rows = (st.expand || []).map(e => `<div class="hint" style="margin-left:40px">${esc(e.family)} → ${e.conventions.map(esc).join(", ")}</div>`).join("");
    return h(`<div class="step">${num}<span class="step-tag review_map">review_map</span><span class="hint">by changed file kind</span>${rows}</div>`);
  }
  const missing = st.exists === false;
  const row = h(`<div class="step">${num}<span class="step-tag ${esc(st.kind)}">${esc(st.kind)}</span><span class="arg">${esc(st.id)}</span>${
    missing ? ' <span class="warn-pill">⚠ missing</span>' : ' <a class="link doc-view">view</a> <a class="link doc-edit">edit</a>'}</div>`);
  if (!missing) {
    row.querySelector(".doc-view").onclick = () => viewDoc(profile, st.kind, st.id, row);
    row.querySelector(".doc-edit").onclick = () => editDoc(profile, st.kind, st.id, row);
  }
  return row;
}
```
Update its caller in `skillCard` to pass an incrementing index: `(s.steps||[]).forEach((st,i)=>steps.appendChild(stepRow(profile, st, i+1)));` and wrap the steps container as `<div class="flow"><div class="steps">…</div></div>` if not already.

- [ ] **Step 8: Rules table scroll wrapper.** In `rulesCard`, wrap each `<table class="rules">…</table>` build in a `.table-scroll` div: after creating `ctab`/`rtab`, append `const cwrap = h('<div class="table-scroll"></div>'); cwrap.appendChild(ctab); card.appendChild(cwrap);` (same for `rtab`). Origin badge classes (`origin origin-${origin}`) already match the CSS.

- [ ] **Step 9: Stickers for status pills.** Replace `<span class="pill">active</span>` (project active) with `<span class="sticker active dot">active</span>`, and the profiles "default" marker likewise with `<span class="sticker new">default</span>` where applicable. Fact-family `pill`s stay `.pill`.

- [ ] **Step 10: Build the bundle is N/A (no build).** Verify no syntax errors by loading in the browser during Task 5; for a static check run:

Run: `node --check src/web/app.js`
Expected: no output (valid JS).

- [ ] **Step 11: Commit**

```bash
git add src/web/app.js
git commit -m "feat(web): render console in Pop Workbench structure (heads, rows, numbered flows, stickers, theme toggle)"
```

---

### Task 5: Verify end-to-end and finish the branch

**Files:** none (verification)

- [ ] **Step 1: Build**

Run: `cargo build 2>&1 | tail -5`
Expected: success.

- [ ] **Step 2: Run the console** on a test port in the background:

Run: `cargo run -- web --port 7799 &` then open `http://127.0.0.1:7799`.

- [ ] **Step 3: Visual checklist** (light + dark):
  - Topbar shows Bangers wordmark; theme toggle flips and **persists** across reload (localStorage).
  - Fonts load from `/bangers.woff2` etc. (DevTools Network: 200, `font/woff2`, no external host).
  - Overview: stat-less workspace card with `.kv-row`s; `#kn-dir` populated.
  - Profiles → a profile: Conventions/Recipes as `.lrow` with `id-chip`; Fact families as `.pill`s; Flows show **numbered** steps with colored `.step-tag`s.
  - A project detail: Effective Rules table scrolls inside its card on narrow width; origin badges colored.
  - Resize < 880px: sidebar hides, `menu-btn` opens the drawer; no horizontal page scroll.
  - All semantic colors read correctly (conventions green, recipes gold, review blue, engine violet).

- [ ] **Step 4: Stop the server** (`kill %1` or Ctrl-C).

- [ ] **Step 5: Run the full test suite**

Run: `cargo test 2>&1 | tail -15`
Expected: all pass.

- [ ] **Step 6: Finish the branch** — use the `superpowers:finishing-a-development-branch` flow: merge `feat/web-comic-redesign` into `main` and push (per user workflow preference).

---

## Self-Review

- **Spec coverage:** style.css rewrite (T2) ✓; index.html chrome + theme persistence (T3) ✓; app.js all views + numbered flows + stickers + table-scroll + theme/drawer (T4) ✓; web.rs font embed/route/test (T1) ✓; bundled woff2 (T1) ✓; semantic colors preserved (T2, carried from mockup) ✓; a11y/reduced-motion (T2 CSS, carried) ✓; verification (T5) ✓.
- **Placeholder scan:** Rust steps carry full code; CSS sourced verbatim from the existing mockup file (concrete, not a placeholder) with the exact `@font-face`/toast/helper additions inlined; app.js steps give the exact replacement snippets per function.
- **Type/name consistency:** route variant `Font(String)` and helper `font_asset` used consistently in T1; class names in T2 match those emitted in T3/T4 (`lrow`, `id-chip`, `step`/`num`/`step-tag`, `table-scroll`, `sticker`, `card-head`, `view-head`/`eyebrow`).
