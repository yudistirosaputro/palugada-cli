// palugada web console — vanilla JS, no framework, no build step.
"use strict";

const view = document.getElementById("view");
const toastEl = document.getElementById("toast");

// ── helpers ───────────────────────────────────────────────────────────────
async function api(path, method = "GET", body) {
  const opt = { method, headers: {} };
  if (body !== undefined) {
    opt.headers["Content-Type"] = "application/json";
    opt.body = JSON.stringify(body);
  }
  const res = await fetch(path, opt);
  const text = await res.text();
  let data;
  try { data = text ? JSON.parse(text) : {}; } catch { data = { raw: text }; }
  if (!res.ok) throw new Error(data.error || ("HTTP " + res.status));
  return data;
}
function toast(msg, isErr = false) {
  toastEl.textContent = msg;
  toastEl.className = "toast show" + (isErr ? " err" : "");
  setTimeout(() => { toastEl.className = "toast"; }, 3200);
}
function h(html) {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstChild;
}
function esc(s) {
  return (s == null ? "" : String(s)).replace(/[&<>"]/g,
    c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}
function splitCsv(s) {
  return (s || "").split(",").map(x => x.trim()).filter(Boolean);
}
function showBody(title, md) {
  let card = document.getElementById("bodyview");
  if (card) card.remove();
  card = h(`<div class="card" id="bodyview"></div>`);
  card.innerHTML =
    `<div class="row"><strong>${esc(title)}</strong><span class="spacer"></span>` +
    `<a class="link" id="bodyclose">close</a></div><pre>${esc(md)}</pre>`;
  view.insertBefore(card, view.children[1] || null);
  card.querySelector("#bodyclose").onclick = () => card.remove();
}

// ── nav ─────────────────────────────────────────────────────────────────--
const VIEWS = { overview: renderOverview, projects: renderProjects, profiles: renderProfiles, knowledge: renderKnowledge };
document.querySelectorAll(".nav-item").forEach(a => {
  a.onclick = () => setView(a.dataset.view);
});
function setView(name) {
  document.querySelectorAll(".nav-item").forEach(n => n.classList.toggle("active", n.dataset.view === name));
  (VIEWS[name] || renderOverview)();
}

// ── views ───────────────────────────────────────────────────────────────--
async function renderOverview() {
  view.innerHTML = "<h2>Overview</h2>";
  try {
    const o = await api("/api/overview");
    document.getElementById("kn-dir").textContent = o.knowledge_dir;
    view.appendChild(h(`<div class="card">
      <div><span class="muted">knowledge dir:</span> ${esc(o.knowledge_dir)}</div>
      <div><span class="muted">active project:</span> ${esc(o.active_project || "(none)")}</div>
      <div><span class="muted">default profile:</span> ${esc(o.default_profile || "(none)")}</div>
      <div><span class="muted">profiles:</span> ${o.profile_count} &nbsp;·&nbsp; <span class="muted">projects:</span> ${o.project_count}</div>
    </div>`));
  } catch (e) { toast(e.message, true); }
}

async function renderProjects() {
  view.innerHTML = "<h2>Projects</h2>";
  let d, profs;
  try {
    d = await api("/api/projects");
    profs = (await api("/api/profiles")).profiles;
  } catch (e) { toast(e.message, true); return; }
  if (!d.projects.length) {
    view.appendChild(h(`<p class="muted">No registered projects yet. Use <code>palugada init</code> or the Generate panel under a profile.</p>`));
  }
  d.projects.forEach(p => {
    const opts = profs.map(pr =>
      `<option value="${esc(pr.id)}"${pr.id === p.profile ? " selected" : ""}>${esc(pr.id)}</option>`).join("");
    const card = h(`<div class="card"><strong>${esc(p.name)}</strong>${p.active ? ' <span class="pill">active</span>' : ""}
      <div class="muted">${esc(p.repo_path)}</div>
      <div class="row" style="margin-top:6px"><label style="margin:0">profile</label>
        <select class="proj-profile" style="max-width:240px">${opts}</select></div></div>`);
    card.querySelector(".proj-profile").onchange = async (e) => {
      try {
        await api(`/api/project/${encodeURIComponent(p.name)}/profile`, "POST", { profile: e.target.value });
        toast(`${p.name} → ${e.target.value}`);
      } catch (err) {
        toast(err.message, true);
        renderProjects(); // revert the dropdown to the actual bound profile
      }
    };
    view.appendChild(card);
  });
}

async function renderProfiles() {
  view.innerHTML = "<h2>Profiles</h2>";
  const form = h(`<div class="card"><strong>New profile</strong>
    <label>id (a-z0-9-_)</label><input id="np-id" placeholder="web-react">
    <label>title</label><input id="np-title" placeholder="Web · React">
    <label>languages (comma-separated)</label><input id="np-langs" placeholder="typescript, tsx">
    <div class="row" style="margin-top:8px"><button id="np-create">Create profile</button></div></div>`);
  view.appendChild(form);
  form.querySelector("#np-create").onclick = async () => {
    const id = form.querySelector("#np-id").value.trim();
    if (!id) { toast("id required", true); return; }
    try {
      await api("/api/profile", "POST", {
        id,
        title: form.querySelector("#np-title").value.trim(),
        languages: splitCsv(form.querySelector("#np-langs").value),
      });
      toast("created profile: " + id);
      renderProfiles();
    } catch (e) { toast(e.message, true); }
  };
  const list = h(`<div></div>`);
  view.appendChild(list);
  try {
    const d = await api("/api/profiles");
    d.profiles.forEach(p => {
      const item = h(`<div class="card"><a class="link">${esc(p.id)}</a> <span class="muted">${esc(p.title)}</span></div>`);
      item.querySelector("a").onclick = () => renderProfileDetail(p.id);
      list.appendChild(item);
    });
  } catch (e) { toast(e.message, true); }
}

async function renderProfileDetail(id) {
  view.innerHTML = `<h2>${esc(id)}</h2><p><a class="link" id="back">← profiles</a></p>`;
  document.getElementById("back").onclick = renderProfiles;
  let d;
  try { d = await api("/api/profile/" + encodeURIComponent(id)); }
  catch (e) { toast(e.message, true); return; }

  const cv = h(`<div class="card"><h3>Conventions</h3></div>`);
  d.conventions.forEach(c => {
    const row = h(`<div class="row"><a class="link">${esc(c.id)}</a> <span class="muted">${esc(c.title)} · ${c.sections.length} sections</span></div>`);
    row.querySelector("a").onclick = async () => {
      try { const b = await api(`/api/profile/${id}/convention/${c.id}`); showBody(c.id, b.markdown); }
      catch (e) { toast(e.message, true); }
    };
    cv.appendChild(row);
  });
  cv.appendChild(addConventionForm(id));
  view.appendChild(cv);

  const rc = h(`<div class="card"><h3>Recipes</h3></div>`);
  d.recipes.forEach(r => {
    const row = h(`<div class="row"><a class="link">${esc(r.id)}</a> <span class="muted">${esc(r.title)}</span></div>`);
    row.querySelector("a").onclick = async () => {
      try { const b = await api(`/api/profile/${id}/recipe/${r.id}`); showBody(r.id, b.markdown); }
      catch (e) { toast(e.message, true); }
    };
    rc.appendChild(row);
  });
  rc.appendChild(addRecipeForm(id));
  view.appendChild(rc);

  view.appendChild(h(`<div class="card"><h3>Fact families</h3><div>${
    d.fact_families.map(f => `<span class="pill">${esc(f)}</span>`).join("") || '<span class="muted">none</span>'
  }</div><h3>Flows</h3><div>${
    Object.keys(d.flows).map(f => `<span class="pill">${esc(f)}</span>`).join("") || '<span class="muted">none</span>'
  }</div></div>`));

  view.appendChild(generateForm(id));
}

function addConventionForm(id) {
  const box = h(`<div style="margin-top:12px;border-top:1px solid #2b313c;padding-top:10px">
    <strong>+ Add convention</strong>
    <label>id</label><input class="ac-id" placeholder="errorhandling">
    <label>title</label><input class="ac-title" placeholder="Error Handling">
    <label>description</label><input class="ac-desc" placeholder="one-line summary">
    <label>tags (comma-separated)</label><input class="ac-tags" placeholder="error, coroutines">
    <div class="ac-sections"></div>
    <div class="row" style="margin-top:6px"><button class="ghost ac-addsec">+ section</button><span class="spacer"></span><button class="ac-save">Save convention</button></div>
    <div class="muted" style="margin-top:4px">Each section becomes a token-cheap chunk that <code>q</code>/<code>brief</code> pull on demand.</div>
  </div>`);
  const secs = box.querySelector(".ac-sections");
  const addSec = () => secs.appendChild(h(`<div class="section-row">
    <label>section title</label><input class="sec-title" placeholder="Modeling Failures">
    <label>section body (markdown)</label><textarea class="sec-body"></textarea>
    <label class="row" style="margin-top:4px"><input type="checkbox" class="sec-code" style="width:auto"> &nbsp;contains code</label>
  </div>`));
  box.querySelector(".ac-addsec").onclick = e => { e.preventDefault(); addSec(); };
  addSec();
  box.querySelector(".ac-save").onclick = async e => {
    e.preventDefault();
    const sections = [...secs.querySelectorAll(".section-row")].map(s => ({
      title: s.querySelector(".sec-title").value.trim(),
      body: s.querySelector(".sec-body").value,
      code: s.querySelector(".sec-code").checked,
    })).filter(s => s.title);
    const spec = {
      id: box.querySelector(".ac-id").value.trim(),
      title: box.querySelector(".ac-title").value.trim(),
      description: box.querySelector(".ac-desc").value.trim(),
      tags: splitCsv(box.querySelector(".ac-tags").value),
      sections,
    };
    if (!spec.id || !spec.title) { toast("id and title required", true); return; }
    try { await api(`/api/profile/${id}/convention`, "POST", spec); toast("saved convention " + spec.id); renderProfileDetail(id); }
    catch (err) { toast(err.message, true); }
  };
  return box;
}

function addRecipeForm(id) {
  const box = h(`<div style="margin-top:12px;border-top:1px solid #2b313c;padding-top:10px">
    <strong>+ Add recipe</strong>
    <label>id</label><input class="ar-id" placeholder="pagination">
    <label>title</label><input class="ar-title" placeholder="Add pagination">
    <label>description</label><input class="ar-desc" placeholder="one-line summary">
    <label>tags (comma-separated)</label><input class="ar-tags" placeholder="recipe, list">
    <label>body (markdown)</label><textarea class="ar-body" placeholder="## When to use this&#10;...&#10;## Steps&#10;1. ..."></textarea>
    <div class="row" style="margin-top:6px"><span class="spacer"></span><button class="ar-save">Save recipe</button></div>
  </div>`);
  box.querySelector(".ar-save").onclick = async e => {
    e.preventDefault();
    const spec = {
      id: box.querySelector(".ar-id").value.trim(),
      title: box.querySelector(".ar-title").value.trim(),
      description: box.querySelector(".ar-desc").value.trim(),
      tags: splitCsv(box.querySelector(".ar-tags").value),
      body: box.querySelector(".ar-body").value,
    };
    if (!spec.id || !spec.title) { toast("id and title required", true); return; }
    try { await api(`/api/profile/${id}/recipe`, "POST", spec); toast("saved recipe " + spec.id); renderProfileDetail(id); }
    catch (err) { toast(err.message, true); }
  };
  return box;
}

function generateForm(id) {
  const box = h(`<div class="card"><h3>Generate agent skills → project</h3>
    <label>repo path</label><input class="gen-repo" placeholder="/Users/me/dev/app">
    <label>agents</label>
    <div class="row">
      <label class="row"><input type="checkbox" class="gen-ag" value="claude" checked style="width:auto"> &nbsp;claude</label>
      <label class="row"><input type="checkbox" class="gen-ag" value="codex" style="width:auto"> &nbsp;codex</label>
      <label class="row"><input type="checkbox" class="gen-ag" value="gemini" style="width:auto"> &nbsp;gemini</label>
      <label class="row"><input type="checkbox" class="gen-ag" value="cursor" style="width:auto"> &nbsp;cursor</label>
    </div>
    <div class="row" style="margin-top:8px"><button class="gen-go">Generate</button></div>
    <div class="gen-out muted" style="margin-top:8px"></div></div>`);
  box.querySelector(".gen-go").onclick = async e => {
    e.preventDefault();
    const repo = box.querySelector(".gen-repo").value.trim();
    if (!repo) { toast("repo path required", true); return; }
    const agents = [...box.querySelectorAll(".gen-ag:checked")].map(c => c.value);
    try {
      const r = await api("/api/init", "POST", { repo, agents, profile: id });
      box.querySelector(".gen-out").innerHTML =
        `wrote ${r.written.length} files:<br>` + r.written.map(esc).join("<br>");
      toast("generated skills for " + id);
    } catch (err) { toast(err.message, true); }
  };
  return box;
}

async function renderKnowledge() {
  view.innerHTML = "<h2>Knowledge</h2>";
  let d;
  try { d = await api("/api/profiles"); } catch (e) { toast(e.message, true); return; }
  if (!d.profiles.length) { view.appendChild(h(`<p class="muted">No profiles.</p>`)); return; }
  const sel = h(`<div class="card"><label>profile</label><select id="kn-prof">${
    d.profiles.map(p => `<option value="${esc(p.id)}">${esc(p.id)} — ${esc(p.title)}</option>`).join("")
  }</select></div>`);
  view.appendChild(sel);
  const out = h(`<div></div>`);
  view.appendChild(out);
  const load = async () => {
    const id = document.getElementById("kn-prof").value;
    out.innerHTML = "";
    let pd;
    try { pd = await api("/api/profile/" + encodeURIComponent(id)); } catch (e) { toast(e.message, true); return; }
    const cl = h(`<div class="card"><h3>Conventions</h3></div>`);
    pd.conventions.forEach(c => {
      const r = h(`<div class="row"><a class="link">${esc(c.id)}</a> <span class="muted">${esc(c.title)}</span></div>`);
      r.querySelector("a").onclick = async () => { try { const b = await api(`/api/profile/${id}/convention/${c.id}`); showBody(c.id, b.markdown); } catch (e) { toast(e.message, true); } };
      cl.appendChild(r);
    });
    out.appendChild(cl);
    const rl = h(`<div class="card"><h3>Recipes</h3></div>`);
    pd.recipes.forEach(c => {
      const r = h(`<div class="row"><a class="link">${esc(c.id)}</a> <span class="muted">${esc(c.title)}</span></div>`);
      r.querySelector("a").onclick = async () => { try { const b = await api(`/api/profile/${id}/recipe/${c.id}`); showBody(c.id, b.markdown); } catch (e) { toast(e.message, true); } };
      rl.appendChild(r);
    });
    out.appendChild(rl);
  };
  document.getElementById("kn-prof").onchange = load;
  load();
}

// ── boot ──────────────────────────────────────────────────────────────────
setView("overview");
