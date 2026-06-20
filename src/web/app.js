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
    const card = h(`<div class="card"><a class="link projlink"><strong>${esc(p.name)}</strong></a>${p.active ? ' <span class="pill">active</span>' : ""}
      <div class="muted">${esc(p.repo_path)}</div>
      <div class="row" style="margin-top:6px"><label style="margin:0">profile</label>
        <select class="proj-profile" style="max-width:240px">${opts}</select></div></div>`);
    card.querySelector(".projlink").onclick = () => renderProjectDetail(p.name);
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

async function renderProjectDetail(name) {
  view.innerHTML = `<h2>${esc(name)}</h2><p><a class="link" id="back">← projects</a></p>`;
  document.getElementById("back").onclick = renderProjects;
  let m;
  try { m = await api("/api/project/" + encodeURIComponent(name) + "/skillmap"); }
  catch (e) { toast(e.message, true); return; }
  view.appendChild(h(`<div class="card"><span class="muted">profile:</span> <strong>${esc(m.profile)}</strong></div>`));
  try {
    const cfg = await api("/api/project/" + encodeURIComponent(name) + "/config");
    view.appendChild(credentialsCard(name, cfg));
  } catch (e) { toast(e.message, true); }
  if (m.warnings && m.warnings.length) {
    view.appendChild(h(`<div class="card warn"><strong>⚠ warnings</strong>${
      m.warnings.map(w => `<div class="muted">• ${esc(w)}</div>`).join("")
    }</div>`));
  }
  m.skills.forEach(s => view.appendChild(skillCard(m.profile, s)));
  try {
    const rules = await api("/api/project/" + encodeURIComponent(name) + "/rules");
    view.appendChild(rulesCard(name, rules));
  } catch (e) { toast(e.message, true); }
}

const ORIGIN_LABEL = { profile: "profile", project: "project", overridden: "overridden" };

function rulesCard(name, data) {
  const card = h(`<div class="card"><h3>Effective Rules</h3>
    <div class="muted">Edits here touch THIS project's overlay in its repo (<code>.palugada/</code>),
      committed with the project — not the shared profile <strong>${esc(data.profile)}</strong>.</div></div>`);
  if (data.warnings && data.warnings.length) {
    card.appendChild(h(`<div class="warn">${data.warnings.map(w => `⚠ ${esc(w)}`).join("<br>")}</div>`));
  }

  // Conventions table
  const ctab = h(`<table class="rules"><thead><tr><th>origin</th><th>id</th><th>title</th><th></th></tr></thead><tbody></tbody></table>`);
  const ctbody = ctab.querySelector("tbody");
  data.conventions.forEach(c => {
    const editable = c.origin === "project" || c.origin === "overridden";
    const row = h(`<tr><td><span class="origin origin-${esc(c.origin)}">${esc(ORIGIN_LABEL[c.origin] || c.origin)}</span></td>
      <td><code>${esc(c.id)}</code></td><td>${esc(c.title)}</td>
      <td>${editable ? '<a class="link r-edit">edit</a>' : '<a class="link r-view">view</a>'}</td></tr>`);
    if (editable) row.querySelector(".r-edit").onclick = () => editOverlayConvention(name, c.id);
    else row.querySelector(".r-view").onclick = () => viewDoc(data.profile, "convention", c.id);
    ctbody.appendChild(row);
  });
  card.appendChild(ctab);

  const addBtn = h(`<div class="row" style="margin-top:6px"><button class="ghost r-add">+ Add project rule</button></div>`);
  card.appendChild(addBtn);
  addBtn.querySelector(".r-add").onclick = () => {
    if (card.querySelector(".overlay-add")) return;
    card.insertBefore(addOverlayConventionForm(name), addBtn);
  };

  // review_map table
  card.appendChild(h(`<h4 style="margin:12px 0 4px">review_map</h4>`));
  const rtab = h(`<table class="rules"><thead><tr><th>origin</th><th>family</th><th>conventions</th></tr></thead><tbody></tbody></table>`);
  const rtbody = rtab.querySelector("tbody");
  data.review_map.forEach(e => {
    rtbody.appendChild(h(`<tr><td><span class="origin origin-${esc(e.origin)}">${esc(e.origin)}</span></td>
      <td><code>${esc(e.family)}</code></td><td>${e.conventions.map(esc).join(", ")}</td></tr>`));
  });
  card.appendChild(rtab);
  const override = {};
  data.review_map.filter(e => e.origin === "project").forEach(e => { override[e.family] = e.conventions; });
  const rmBtn = h(`<div class="row" style="margin-top:6px"><button class="ghost r-rm">Edit review_map override</button></div>`);
  card.appendChild(rmBtn);
  rmBtn.querySelector(".r-rm").onclick = () => editReviewMap(name, override);
  return card;
}

async function editOverlayConvention(name, id) {
  let b;
  try { b = await api(`/api/project/${encodeURIComponent(name)}/convention/${encodeURIComponent(id)}`); }
  catch (e) { toast(e.message, true); return; }
  let card = document.getElementById("bodyview");
  if (card) card.remove();
  card = h(`<div class="card" id="bodyview">
    <div class="row"><strong>edit overlay convention: ${esc(id)}</strong><span class="spacer"></span><a class="link" id="ed-close">close</a></div>
    <div class="muted">Edits this project's overlay in <code>.palugada/conventions/${esc(id)}.md</code> (committed with the repo). Whole-file verbatim.</div>
    <textarea id="ed-body" style="min-height:320px;width:100%"></textarea>
    <div class="row" style="margin-top:6px"><span class="spacer"></span><button id="ed-save">Save</button></div></div>`);
  card.querySelector("#ed-body").value = b.markdown;
  view.insertBefore(card, view.children[1] || null);
  card.querySelector("#ed-close").onclick = () => card.remove();
  card.querySelector("#ed-save").onclick = async () => {
    const markdown = card.querySelector("#ed-body").value;
    try {
      await api(`/api/project/${encodeURIComponent(name)}/convention/${encodeURIComponent(id)}/body`, "POST", { markdown });
      toast(`saved overlay convention ${id}`);
      renderProjectDetail(name);
    } catch (e) { toast(e.message, true); }
  };
}

function addOverlayConventionForm(name) {
  const box = h(`<div class="overlay-add" style="margin-top:10px;border-top:1px solid #2b313c;padding-top:10px">
    <strong>+ Add project rule</strong>
    <label>id</label><input class="oa-id" placeholder="our-extra-rule">
    <label>title</label><input class="oa-title" placeholder="Our Extra Rule">
    <label>description</label><input class="oa-desc" placeholder="one-line summary">
    <label>tags (comma-separated)</label><input class="oa-tags" placeholder="kt, viewmodel">
    <div class="oa-sections"></div>
    <div class="row" style="margin-top:6px"><button class="ghost oa-addsec">+ section</button><span class="spacer"></span><button class="oa-save">Save project rule</button></div>
    <div class="muted" style="margin-top:4px">Written to this project's <code>.palugada/conventions/</code>; overrides the profile if the id matches.</div>
  </div>`);
  const secs = box.querySelector(".oa-sections");
  const addSec = () => secs.appendChild(h(`<div class="section-row">
    <label>section title</label><input class="sec-title" placeholder="Rule">
    <label>section body (markdown)</label><textarea class="sec-body"></textarea>
    <label class="row" style="margin-top:4px"><input type="checkbox" class="sec-code" style="width:auto"> &nbsp;contains code</label>
  </div>`));
  box.querySelector(".oa-addsec").onclick = e => { e.preventDefault(); addSec(); };
  addSec();
  box.querySelector(".oa-save").onclick = async e => {
    e.preventDefault();
    const sections = [...secs.querySelectorAll(".section-row")].map(s => ({
      title: s.querySelector(".sec-title").value.trim(),
      body: s.querySelector(".sec-body").value,
      code: s.querySelector(".sec-code").checked,
    })).filter(s => s.title);
    const spec = {
      id: box.querySelector(".oa-id").value.trim(),
      title: box.querySelector(".oa-title").value.trim(),
      description: box.querySelector(".oa-desc").value.trim(),
      tags: splitCsv(box.querySelector(".oa-tags").value),
      sections,
    };
    if (!spec.id || !spec.title) { toast("id and title required", true); return; }
    try { await api(`/api/project/${encodeURIComponent(name)}/convention`, "POST", spec); toast("saved project rule " + spec.id); renderProjectDetail(name); }
    catch (err) { toast(err.message, true); }
  };
  return box;
}

function editReviewMap(name, current) {
  let card = document.getElementById("bodyview");
  if (card) card.remove();
  card = h(`<div class="card" id="bodyview">
    <div class="row"><strong>edit review_map override</strong><span class="spacer"></span><a class="link" id="rm-close">close</a></div>
    <div class="muted">JSON object: <code>{ "family": ["convention-id", ...] }</code>. Each listed family REPLACES the profile's mapping for this project. Empty <code>{}</code> clears the override.</div>
    <textarea id="rm-body" style="min-height:180px;width:100%"></textarea>
    <div class="row" style="margin-top:6px"><span class="spacer"></span><button id="rm-save">Save</button></div></div>`);
  card.querySelector("#rm-body").value = JSON.stringify(current, null, 2);
  view.insertBefore(card, view.children[1] || null);
  card.querySelector("#rm-close").onclick = () => card.remove();
  card.querySelector("#rm-save").onclick = async () => {
    let review_map;
    try { review_map = JSON.parse(card.querySelector("#rm-body").value || "{}"); }
    catch (e) { toast("invalid JSON: " + e.message, true); return; }
    try {
      await api(`/api/project/${encodeURIComponent(name)}/review-map`, "POST", { review_map });
      toast("saved review_map override");
      renderProjectDetail(name);
    } catch (e) { toast(e.message, true); }
  };
}

function credentialsCard(name, cfg) {
  const card = h(`<div class="card"><h3>Credentials &amp; Integrations</h3>
    <label>auth profile</label><input id="cd-auth" value="${esc(cfg.auth_profile || "default")}">
    <div class="muted">Shared by all projects using this auth-profile name.</div></div>`);
  const CAPS = [
    ["issue_tracker", "Issue tracker", true],
    ["wiki", "Wiki", false],
    ["git_host", "Git host", true],
    ["design", "Design", false],
    ["ci", "CI", true],
    ["chat", "Chat", false],
  ];
  const intWrap = h(`<div></div>`);
  CAPS.forEach(([cap, label, hasRepo]) => {
    const cur = cfg.integrations[cap] || {};
    const opts = ["(none)", ...(cfg.providers[cap] || [])]
      .map(p => `<option${(cur.provider || "(none)") === p ? " selected" : ""}>${esc(p)}</option>`).join("");
    const row = h(`<div class="cd-int" data-cap="${cap}">
      <div class="row"><strong style="min-width:110px">${esc(label)}</strong>
        <select class="cd-prov">${opts}</select>
        <span class="spacer"></span><a class="link cd-verify">Verify</a> <span class="cd-vres"></span></div>
      <input class="cd-base" placeholder="base_url" value="${esc(cur.base_url || "")}">
      ${hasRepo ? `<input class="cd-repo" placeholder="repo (owner/name)" value="${esc(cur.repo || "")}">` : ""}
    </div>`);
    row.querySelector(".cd-verify").onclick = async () => {
      const res = row.querySelector(".cd-vres");
      res.textContent = "…"; res.className = "cd-vres muted";
      try {
        const r = await api(`/api/project/${encodeURIComponent(name)}/verify/${cap}`, "POST", {});
        res.textContent = r.ok ? ("✓ " + (r.message || "ok")) : ("✗ " + (r.error || "failed"));
        res.className = "cd-vres " + (r.ok ? "ok-pill" : "warn-pill");
      } catch (e) { res.textContent = "✗ " + e.message; res.className = "cd-vres warn-pill"; }
    };
    intWrap.appendChild(row);
  });
  card.appendChild(intWrap);

  const sec = cfg.secrets || {};
  const tok = (k, label) => `<label>${label}</label><input class="cd-sec" data-k="${k}" type="password" placeholder="${esc(sec[k] || "(unset)")} — blank = keep">`;
  const txt = (k, label) => `<label>${label}</label><input class="cd-txt" data-k="${k}" value="${esc(sec[k] || "")}">`;
  card.appendChild(h(`<div style="margin-top:10px;border-top:1px solid #2b313c;padding-top:8px"><strong>Tokens</strong>
    ${tok("jira_token", "jira_token")}${txt("jira_email", "jira_email")}
    ${tok("wiki_token", "wiki_token")}${txt("wiki_email", "wiki_email")}
    ${tok("figma_token", "figma_token")}
    ${txt("jenkins_user", "jenkins_user")}${tok("jenkins_token", "jenkins_token")}
    ${tok("git_token", "git_token")}${tok("chat_webhook", "chat_webhook")}
    <div class="muted">Blank token = unchanged. Stored in ~/.palugada/secrets.yaml (0600); never echoed back in full.</div></div>`));

  const save = h(`<div class="row" style="margin-top:10px"><span class="spacer"></span><button id="cd-save">Save credentials</button></div>`);
  card.appendChild(save);
  save.querySelector("#cd-save").onclick = async () => {
    const integrations = {};
    intWrap.querySelectorAll(".cd-int").forEach(row => {
      const repoEl = row.querySelector(".cd-repo");
      integrations[row.dataset.cap] = {
        provider: row.querySelector(".cd-prov").value,
        base_url: row.querySelector(".cd-base").value,
        repo: repoEl ? repoEl.value : "",
      };
    });
    const secrets = {};
    card.querySelectorAll(".cd-sec, .cd-txt").forEach(i => { secrets[i.dataset.k] = i.value; });
    try {
      await api(`/api/project/${encodeURIComponent(name)}/config`, "POST",
        { auth_profile: card.querySelector("#cd-auth").value, integrations, secrets });
      toast("saved credentials");
      renderProjectDetail(name);
    } catch (e) { toast(e.message, true); }
  };
  return card;
}

function skillCard(profile, s) {
  const card = h(`<div class="card"></div>`);
  const head = h(`<div class="row"><strong>${esc(s.name)}</strong> <span class="pill">${esc(s.kind)}</span><span class="spacer"></span></div>`);
  card.appendChild(head);
  if (s.command) card.appendChild(h(`<div class="muted"><code>${esc(s.command)}</code></div>`));
  if (s.kind === "flow") {
    const steps = h(`<div class="steps"></div>`);
    (s.steps || []).forEach(st => steps.appendChild(stepRow(profile, st)));
    if (!s.steps || !s.steps.length) steps.appendChild(h(`<div class="muted">no steps defined for this flow</div>`));
    card.appendChild(steps);
  } else if (s.kind === "tool") {
    head.appendChild(s.enabled
      ? h(`<span class="ok-pill">active</span>`)
      : h(`<span class="warn-pill">⚠ needs ${esc((s.needs || []).join(" or "))}</span>`));
  }
  return card;
}

function stepRow(profile, st) {
  if (st.kind === "engine")
    return h(`<div class="step"><span class="step-tag engine">engine</span> <span class="muted">${esc(st.token)} — ${esc(st.label)}</span></div>`);
  if (st.kind === "review_map") {
    const rows = (st.expand || []).map(e =>
      `<div class="muted" style="margin-left:18px">${esc(e.family)} → ${e.conventions.map(esc).join(", ")}</div>`).join("");
    return h(`<div class="step"><span class="step-tag review">review_map</span> <span class="muted">by changed file kind</span>${rows}</div>`);
  }
  const missing = st.exists === false;
  const row = h(`<div class="step"><span class="step-tag ${esc(st.kind)}">${esc(st.kind)}</span> <code>${esc(st.id)}</code>${
    missing ? ' <span class="warn-pill">⚠ missing</span>'
            : ' <a class="link doc-view">view</a> <a class="link doc-edit">edit</a>'
  }</div>`);
  if (!missing) {
    row.querySelector(".doc-view").onclick = () => viewDoc(profile, st.kind, st.id);
    row.querySelector(".doc-edit").onclick = () => editDoc(profile, st.kind, st.id);
  }
  return row;
}

async function viewDoc(profile, kind, id) {
  try {
    const b = await api(`/api/profile/${encodeURIComponent(profile)}/${kind}/${encodeURIComponent(id)}`);
    showBody(`${kind}: ${id}`, b.markdown);
  } catch (e) { toast(e.message, true); }
}

async function editDoc(profile, kind, id) {
  let b;
  try { b = await api(`/api/profile/${encodeURIComponent(profile)}/${kind}/${encodeURIComponent(id)}`); }
  catch (e) { toast(e.message, true); return; }
  let card = document.getElementById("bodyview");
  if (card) card.remove();
  card = h(`<div class="card" id="bodyview">
    <div class="row"><strong>edit ${esc(kind)}: ${esc(id)}</strong><span class="spacer"></span><a class="link" id="ed-close">close</a></div>
    <div class="muted">Edits the profile's knowledge in <code>~/.palugada</code> (shared by all projects on '${esc(profile)}'); your project repo is not touched.</div>
    <textarea id="ed-body" style="min-height:320px;width:100%"></textarea>
    <div class="row" style="margin-top:6px"><span class="spacer"></span><button id="ed-save">Save</button></div></div>`);
  card.querySelector("#ed-body").value = b.markdown;
  view.insertBefore(card, view.children[1] || null);
  card.querySelector("#ed-close").onclick = () => card.remove();
  card.querySelector("#ed-save").onclick = async () => {
    const markdown = card.querySelector("#ed-body").value;
    try {
      await api(`/api/profile/${encodeURIComponent(profile)}/${kind}/${encodeURIComponent(id)}/body`, "POST", { markdown });
      toast(`saved ${kind} ${id}`);
      card.remove();
    } catch (e) { toast(e.message, true); }
  };
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

  const cv = h(`<div class="card"><h3>Conventions</h3>
    <div class="muted">Standing standards for this stack — the "right way" to write code here (architecture,
    error handling, testing, style). Pulled automatically by <code>q</code> and <code>brief</code> while you code.</div></div>`);
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
  view.appendChild(importCard(id));

  const rc = h(`<div class="card"><h3>Recipes</h3>
    <div class="muted">Step-by-step guides for one task (scaffold a feature, add a subcommand, refactor).
    Pulled by <code>for &lt;task&gt;</code> and <code>brief feature/refactor</code>.</div></div>`);
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

  view.appendChild(h(`<div class="card"><h3>Fact families</h3>
    <div class="muted">Categories of symbols palugada extracts from YOUR code (e.g. viewmodel, repository, command).
    <code>palugada fact &lt;family&gt;</code> lists them with file:line. Defined in the profile's
    <code>extractors.yaml</code> (regex / tree-sitter) — not edited here.</div><div>${
    d.fact_families.map(f => `<span class="pill">${esc(f)}</span>`).join("") || '<span class="muted">none</span>'
  }</div><h3>Flows</h3>
    <div class="muted">Step sequences <code>palugada brief &lt;flow&gt;</code> assembles for each task type.</div><div>${
    Object.keys(d.flows).map(f => `<span class="pill">${esc(f)}</span>`).join("") || '<span class="muted">none</span>'
  }</div></div>`));

  view.appendChild(generateForm(id));
}

function importCard(id) {
  const box = h(`<div class="card"><h3>Import markdown file</h3>
    <div class="muted">Upload or paste a markdown doc; palugada splits it by <code># H1</code> into candidate
    conventions (sections come from <code>##</code>). Preview, edit, then import into this profile.</div>
    <input type="file" class="im-file" accept=".md,.markdown,text/markdown">
    <label>or paste markdown</label><textarea class="im-text" placeholder="# Firebase Integration&#10;## Setup&#10;..."></textarea>
    <div class="row" style="margin-top:6px"><button class="im-detect">Detect</button></div>
    <div class="im-preview"></div></div>`);
  const fileEl = box.querySelector(".im-file");
  const textEl = box.querySelector(".im-text");
  fileEl.onchange = () => {
    const f = fileEl.files[0];
    if (!f) return;
    const r = new FileReader();
    r.onload = () => { textEl.value = r.result; };
    r.readAsText(f);
  };
  box.querySelector(".im-detect").onclick = async () => {
    const markdown = textEl.value;
    if (!markdown.trim()) { toast("paste or choose a markdown file first", true); return; }
    try {
      const res = await api(`/api/profile/${encodeURIComponent(id)}/import/preview`, "POST", { markdown });
      renderImportPreview(box.querySelector(".im-preview"), id, res);
    } catch (e) { toast(e.message, true); }
  };
  return box;
}

function renderImportPreview(host, id, res) {
  host.innerHTML = "";
  if (res.warnings && res.warnings.length) {
    host.appendChild(h(`<div class="warn">${res.warnings.map(w => `⚠ ${esc(w)}`).join("<br>")}</div>`));
  }
  if (!res.candidates || !res.candidates.length) return;
  const rows = [];
  res.candidates.forEach(c => {
    const badge = c.exists ? `<span class="warn-pill">will update</span>` : `<span class="ok-pill">new</span>`;
    const row = h(`<div class="candidate">
      <label class="row"><input type="checkbox" class="im-inc" checked style="width:auto"> &nbsp;include</label> ${badge}
      <label>id</label><input class="im-id" value="${esc(c.id)}">
      <label>title</label><input class="im-title" value="${esc(c.title)}">
      <label>tags (comma-separated)</label><input class="im-tags" value="">
      <div class="muted">sections: ${c.sections.map(esc).join(", ") || "(none)"}</div>
    </div>`);
    row._body = c.body;
    rows.push(row);
    host.appendChild(row);
  });
  const go = h(`<div class="row" style="margin-top:6px"><span class="spacer"></span><button class="im-go">Import selected</button></div>`);
  host.appendChild(go);
  go.querySelector(".im-go").onclick = async () => {
    const pieces = rows.filter(r => r.querySelector(".im-inc").checked).map(r => ({
      id: r.querySelector(".im-id").value.trim(),
      title: r.querySelector(".im-title").value.trim(),
      description: "",
      tags: splitCsv(r.querySelector(".im-tags").value),
      body: r._body,
    }));
    if (!pieces.length) { toast("select at least one candidate", true); return; }
    if (pieces.some(p => !p.id || !p.title)) { toast("every selected piece needs id + title", true); return; }
    try {
      const r2 = await api(`/api/profile/${encodeURIComponent(id)}/import/commit`, "POST", { pieces });
      toast(`imported: ${r2.created} created, ${r2.updated} updated`);
      renderProfileDetail(id);
    } catch (e) { toast(e.message, true); }
  };
}

function addConventionForm(id) {
  const box = h(`<div style="margin-top:12px;border-top:1px solid #2b313c;padding-top:10px">
    <strong>+ Add convention</strong>
    <div class="muted" style="margin:2px 0 6px">A standing standard for this stack. palugada writes the front-matter,
    section ids, and index for you — you only fill the fields below. (CLI equivalent: <code>palugada convention add &lt;file.md&gt;</code>.)</div>
    <label>id</label><input class="ac-id" placeholder="errorhandling">
    <div class="muted">lowercase slug — used in <code>palugada q errorhandling</code> (a-z, 0-9, - _).</div>
    <label>title</label><input class="ac-title" placeholder="Error Handling">
    <label>description</label><input class="ac-desc" placeholder="one-line summary">
    <div class="muted">one line shown in <code>q --list</code> and search.</div>
    <label>tags (comma-separated)</label><input class="ac-tags" placeholder="error, coroutines">
    <label style="margin-top:8px;display:block"><strong>Sections</strong></label>
    <div class="muted">Split the rule into titled chunks — each is a token-cheap piece <code>q</code>/<code>brief</code> pull on demand.</div>
    <div class="ac-sections"></div>
    <div class="row" style="margin-top:6px"><button class="ghost ac-addsec">+ section</button><span class="spacer"></span><button class="ac-save">Save convention</button></div>
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
    <div class="muted" style="margin:2px 0 6px">A step-by-step guide for one task. (CLI equivalent: <code>palugada recipe add &lt;file.md&gt;</code>.)</div>
    <label>id</label><input class="ar-id" placeholder="pagination">
    <div class="muted">lowercase slug — used in <code>palugada for pagination</code>.</div>
    <label>title</label><input class="ar-title" placeholder="Add pagination">
    <label>description</label><input class="ar-desc" placeholder="one-line summary">
    <label>tags (comma-separated)</label><input class="ar-tags" placeholder="recipe, list">
    <label>body (markdown)</label><textarea class="ar-body" placeholder="## When to use this&#10;...&#10;## Steps&#10;1. ..."></textarea>
    <div class="muted">the full procedure — write it as plain markdown (use <code>## </code> for steps/sections).</div>
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
