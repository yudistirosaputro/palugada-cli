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
function slugify(s) {
  return String(s == null ? "" : s).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}
// Strip a leading `---\n...\n---` YAML front-matter block.
function stripFrontMatter(md) {
  const m = String(md || "");
  if (m.startsWith("---")) {
    const end = m.indexOf("\n---", 3);
    if (end !== -1) {
      const nl = m.indexOf("\n", end + 1);
      return (nl === -1 ? "" : m.slice(nl + 1)).replace(/^\s+/, "");
    }
  }
  return m;
}
// Brief view: drop fenced code blocks (keep a marker). FULL view keeps everything.
function briefMarkdown(md) {
  return String(md || "").replace(/```[\s\S]*?```/g, "\n_(code — switch to FULL)_\n");
}
// Inline spans on already-escaped text: `code`, **bold**.
function mdInline(s) {
  let t = esc(s);
  t = t.replace(/`([^`]+)`/g, (_, c) => "<code>" + c + "</code>");
  t = t.replace(/\*\*([^*]+)\*\*/g, (_, b) => "<strong>" + b + "</strong>");
  return t;
}
// Minimal, safe markdown → HTML for the Doc Reader. Escapes all text; supports
// #/##/### headings (slug ids for scroll targets), fenced code, inline code,
// bold, unordered/ordered lists, GFM tables, blockquotes, and paragraphs.
// Not full CommonMark — covers palugada convention/recipe bodies.
function mdToHtml(md) {
  const lines = String(md || "").replace(/\r\n/g, "\n").split("\n");
  let html = "";
  let para = [];
  let i = 0;
  const flush = () => { if (para.length) { html += "<p>" + mdInline(para.join(" ")) + "</p>"; para = []; } };
  while (i < lines.length) {
    const line = lines[i];
    const fence = line.match(/^```/);
    if (fence) {
      flush();
      const code = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) { code.push(lines[i]); i++; }
      i++; // closing fence
      html += '<pre class="code"><code>' + esc(code.join("\n")) + "</code></pre>";
      continue;
    }
    const head = line.match(/^(#{1,4})\s+(.*?)(?:\s*\{#([a-z0-9-]+)\})?\s*$/);
    if (head) {
      flush();
      const level = head[1].length;
      const text = head[2].trim();
      const id = head[3] || slugify(text);
      const tag = level <= 2 ? "h2" : level === 3 ? "h3" : "h4";
      html += "<" + tag + ' id="sec-' + id + '">' + mdInline(text) + "</" + tag + ">";
      i++;
      continue;
    }
    const isTableHead = /^\s*\|.*\|\s*$/.test(line)
      && i + 1 < lines.length
      && /^\s*\|?[\s:\-|]+\|?\s*$/.test(lines[i + 1])
      && lines[i + 1].includes("-");
    if (isTableHead) {
      flush();
      const cells = (r) => r.trim().replace(/^\||\|$/g, "").split("|").map(c => c.trim());
      const ths = cells(line).map(c => "<th>" + mdInline(c) + "</th>").join("");
      i += 2;
      let rows = "";
      while (i < lines.length && /^\s*\|.*\|\s*$/.test(lines[i])) {
        rows += "<tr>" + cells(lines[i]).map(c => "<td>" + mdInline(c) + "</td>").join("") + "</tr>";
        i++;
      }
      html += '<div class="table-scroll"><table class="rules"><thead><tr>' + ths + "</tr></thead><tbody>" + rows + "</tbody></table></div>";
      continue;
    }
    if (/^\s*[-*]\s+/.test(line)) {
      flush();
      let items = "";
      while (i < lines.length && /^\s*[-*]\s+/.test(lines[i])) {
        items += "<li>" + mdInline(lines[i].replace(/^\s*[-*]\s+/, "")) + "</li>";
        i++;
      }
      html += "<ul>" + items + "</ul>";
      continue;
    }
    if (/^\s*\d+\.\s+/.test(line)) {
      flush();
      let items = "";
      while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
        items += "<li>" + mdInline(lines[i].replace(/^\s*\d+\.\s+/, "")) + "</li>";
        i++;
      }
      html += "<ol>" + items + "</ol>";
      continue;
    }
    if (/^\s*>\s?/.test(line)) {
      flush();
      const quote = [];
      while (i < lines.length && /^\s*>\s?/.test(lines[i])) { quote.push(lines[i].replace(/^\s*>\s?/, "")); i++; }
      html += "<blockquote>" + mdInline(quote.join(" ")) + "</blockquote>";
      continue;
    }
    if (/^\s*$/.test(line)) { flush(); i++; continue; }
    para.push(line.trim());
    i++;
  }
  flush();
  return html;
}
function splitCsv(s) {
  return (s || "").split(",").map(x => x.trim()).filter(Boolean);
}
// Standard view header: comic eyebrow + display title + optional subtitle.
// `subtitle` may contain trusted markup (e.g. <code>); callers escape user data.
function viewHead(eyebrow, title, subtitle) {
  return `<div class="view-head"><div class="eyebrow">${esc(eyebrow)}</div>` +
    `<h1>${esc(title)}</h1>` + (subtitle ? `<p class="subtitle">${subtitle}</p>` : "") + `</div>`;
}
// A back link styled as a comic chip with a chevron.
function backLink(label) {
  return `<a class="link back-link" id="back"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M15 18l-6-6 6-6"/></svg>${esc(label)}</a>`;
}
// Place a view/edit panel right below the row/section the user clicked (its
// nearest .row/.step/.candidate, or the enclosing table for a <tr>); fall back
// to the top of the view when there's no anchor.
function placePanel(card, anchor) {
  if (anchor && anchor.closest) {
    const tr = anchor.closest("tr");
    const host = tr ? tr.closest("table") : anchor.closest(".row, .step, .candidate");
    if (host && host.parentNode) { host.insertAdjacentElement("afterend", card); return; }
  }
  view.insertBefore(card, view.children[1] || null);
}

// The comic Doc Reader: rendered markdown for one convention/recipe. Opens FULL
// (entire doc incl. code) by default; a pill toggles a brief (code-stripped) view.
// `meta` carries id/title/description and (for the structured panels in Task 5)
// sections + cross-refs; the markdown body is fetched here.
async function renderDoc(meta, kind, profileId, anchor) {
  let body;
  try {
    const b = await api(`/api/profile/${encodeURIComponent(profileId)}/${kind}/${encodeURIComponent(meta.id)}`);
    body = b.markdown;
  } catch (e) { toast(e.message, true); return null; }

  let card = document.getElementById("bodyview");
  if (card) card.remove();
  card = h(`<div class="card" id="bodyview"></div>`);
  const head = h(`<div class="card-head">
    <span class="id-chip">${esc(meta.id)}</span>
    <h3 style="margin:0">${esc(meta.title || meta.id)}</h3>
    <span class="spacer"></span>
    <span class="pill doc-mode" style="cursor:pointer" title="toggle full / brief">FULL</span>
    <a class="link" id="bodyclose">close</a>
  </div>`);
  card.appendChild(head);
  if (meta.description) card.appendChild(h(`<div class="card-note">${esc(meta.description)}</div>`));

  const prose = h(`<div class="prev"></div>`);
  const stripped = stripFrontMatter(body);
  let full = true; // FULL by default (user decision)
  const renderBody = () => { prose.innerHTML = mdToHtml(full ? stripped : briefMarkdown(stripped)); };
  card.appendChild(prose);
  renderBody();

  // Conventions: a numbered Sections panel (reuses the flow .step language).
  if (kind === "convention" && Array.isArray(meta.sections) && meta.sections.length) {
    const panel = h(`<div class="doc-sections"><h4 style="margin:0 0 6px">Sections</h4><div class="steps"></div></div>`);
    const steps = panel.querySelector(".steps");
    meta.sections.forEach((s, i) => {
      const sid = s.id || slugify(s.title);
      const tok = s.tokens ? ` <span class="hint">~${s.tokens} tok</span>` : "";
      const sectionBadge = s.origin === "inherited"
        ? ` <span class="sticker">${"inherited · " + esc(s.from || "")}</span>`
        : s.origin === "overridden"
        ? ` <span class="sticker">overridden</span>`
        : "";
      const stepRowEl = h(`<div class="step" style="cursor:pointer"><span class="num">${i + 1}</span><span class="id-chip">#${esc(sid)}</span> <span class="arg">${esc(s.title)}</span>${tok}${sectionBadge}</div>`);
      stepRowEl.onclick = () => {
        const el = prose.querySelector("#sec-" + (window.CSS && CSS.escape ? CSS.escape(sid) : sid));
        if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
      };
      steps.appendChild(stepRowEl);
    });
    card.insertBefore(panel, prose);
  }

  // Recipes: clickable convention cross-refs + related-recipe chips.
  if (kind === "recipe") {
    const chips = [];
    (meta.convention_refs || []).forEach(cr => {
      const label = cr.section ? `${cr.topic}#${cr.section}` : cr.topic;
      const chip = h(`<span class="step-tag convention" style="cursor:pointer;min-width:0">${esc(label)}</span>`);
      chip.onclick = () => openConventionAt(profileId, cr.topic, cr.section, card);
      chips.push(chip);
    });
    (meta.related_recipes || []).forEach(rid => {
      const chip = h(`<span class="step-tag recipe" style="cursor:pointer;min-width:0">${esc(rid)}</span>`);
      chip.onclick = () => renderDoc({ id: rid, title: rid }, "recipe", profileId, card);
      chips.push(chip);
    });
    if (chips.length) {
      const refRow = h(`<div class="row" style="margin-top:6px"><span class="muted" style="font-weight:700;margin-right:4px">refs:</span></div>`);
      chips.forEach(c => refRow.appendChild(c));
      card.insertBefore(refRow, prose);
    }
  }

  head.querySelector(".doc-mode").onclick = (e) => {
    full = !full;
    e.target.textContent = full ? "FULL" : "BRIEF";
    renderBody();
  };
  head.querySelector("#bodyclose").onclick = () => card.remove();
  placePanel(card, anchor);
  return card;
}

// Open the convention reader (with its Sections panel) and scroll to a section.
async function openConventionAt(profileId, topic, section, anchor) {
  let meta = { id: topic, title: topic };
  try {
    const pd = await api("/api/profile/" + encodeURIComponent(profileId));
    const found = (pd.conventions || []).find(c => c.id === topic);
    if (found) meta = found;
  } catch (e) { /* fall back to the minimal meta */ }
  const card = await renderDoc(meta, "convention", profileId, anchor);
  if (card && section) {
    setTimeout(() => {
      const el = card.querySelector("#sec-" + (window.CSS && CSS.escape ? CSS.escape(section) : section));
      if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 60);
  }
}

// ── nav ─────────────────────────────────────────────────────────────────--
const VIEWS = { overview: renderOverview, projects: renderProjects, profiles: renderProfiles, knowledge: renderKnowledge, connectors: renderConnectors };
document.querySelectorAll(".nav-item").forEach(a => {
  a.onclick = () => setView(a.dataset.view);
});
function setView(name) {
  document.querySelectorAll(".nav-item").forEach(n => n.classList.toggle("active", n.dataset.view === name));
  const sidebar = document.getElementById("sidebar");
  if (sidebar) sidebar.classList.remove("open");
  (VIEWS[name] || renderOverview)();
}

// ── views ───────────────────────────────────────────────────────────────--
async function renderOverview() {
  view.innerHTML = viewHead("Workspace", "Overview",
    "Your engineering know-how, indexed once and served token-cheap to every agent and CLI call.");
  try {
    const o = await api("/api/overview");
    document.getElementById("kn-dir").textContent = o.knowledge_dir;
    view.appendChild(h(`<div class="stat-grid" style="grid-template-columns:repeat(2,1fr);max-width:540px">
      <div class="stat"><div class="k">Profiles</div><div class="v">${o.profile_count}</div></div>
      <div class="stat"><div class="k">Projects</div><div class="v">${o.project_count}</div></div>
    </div>`));
    view.appendChild(h(`<div class="card"><div class="card-head"><h3>Workspace</h3></div>
      <div class="kv-row"><span class="kk">Knowledge dir</span><span class="vv mono">${esc(o.knowledge_dir)}</span></div>
      <div class="kv-row"><span class="kk">Active project</span><span class="vv">${
        o.active_project ? esc(o.active_project) + ' <span class="sticker active dot">active</span>' : '<span class="muted">(none)</span>'
      }</span></div>
      <div class="kv-row"><span class="kk">Default profile</span><span class="vv">${
        o.default_profile ? '<span class="id-chip">' + esc(o.default_profile) + '</span>' : '<span class="muted">(none)</span>'
      }</span></div>
    </div>`));
  } catch (e) { toast(e.message, true); }
}

async function renderProjects() {
  view.innerHTML = viewHead("Repos", "Projects",
    "Repositories bound to a profile. Each can override its profile's rules in a committed <code>.palugada/</code> overlay.");
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
    const card = h(`<div class="card">
      <div class="card-head"><h3 class="projlink" style="margin:0;cursor:pointer">${esc(p.name)}</h3>${p.active ? ' <span class="sticker active dot">active</span>' : ""}</div>
      <div class="kv-row"><span class="kk">Repo path</span><span class="vv mono">${esc(p.repo_path)}</span></div>
      <div class="kv-row"><span class="kk">Profile</span><span class="vv"><select class="proj-profile" style="max-width:260px">${opts}</select></span></div></div>`);
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
  view.innerHTML = backLink("Projects") + viewHead("Project", name);
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
  const card = h(`<div class="card"><div class="card-head"><h3>Effective Rules</h3></div>
    <div class="card-note">Edits here touch THIS project's overlay in its repo (<code>.palugada/</code>),
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
    if (editable) row.querySelector(".r-edit").onclick = () => editOverlayConvention(name, c.id, row);
    else row.querySelector(".r-view").onclick = () => viewDoc(data.profile, "convention", c.id, row);
    ctbody.appendChild(row);
  });
  const cwrap = h(`<div class="table-scroll"></div>`);
  cwrap.appendChild(ctab);
  card.appendChild(cwrap);

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
  const rwrap = h(`<div class="table-scroll"></div>`);
  rwrap.appendChild(rtab);
  card.appendChild(rwrap);
  const override = {};
  data.review_map.filter(e => e.origin === "project").forEach(e => { override[e.family] = e.conventions; });
  const rmBtn = h(`<div class="row" style="margin-top:6px"><button class="ghost r-rm">Edit review_map override</button></div>`);
  card.appendChild(rmBtn);
  rmBtn.querySelector(".r-rm").onclick = () => editReviewMap(name, override, rmBtn);
  return card;
}

async function editOverlayConvention(name, id, anchor) {
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
  placePanel(card, anchor);
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
  const box = h(`<div class="overlay-add" style="margin-top:10px;border-top:1px solid var(--ink);padding-top:10px">
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

function editReviewMap(name, current, anchor) {
  let card = document.getElementById("bodyview");
  if (card) card.remove();
  card = h(`<div class="card" id="bodyview">
    <div class="row"><strong>edit review_map override</strong><span class="spacer"></span><a class="link" id="rm-close">close</a></div>
    <div class="muted">JSON object: <code>{ "family": ["convention-id", ...] }</code>. Each listed family REPLACES the profile's mapping for this project. Empty <code>{}</code> clears the override.</div>
    <textarea id="rm-body" style="min-height:180px;width:100%"></textarea>
    <div class="row" style="margin-top:6px"><span class="spacer"></span><button id="rm-save">Save</button></div></div>`);
  card.querySelector("#rm-body").value = JSON.stringify(current, null, 2);
  placePanel(card, anchor);
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
    <label>auth profile</label><input id="cd-auth" list="cd-auth-list" value="${esc(cfg.auth_profile || "default")}"><datalist id="cd-auth-list"></datalist>
    <div class="muted">Shared by all projects using this auth-profile name. Pick an existing one or type a new name.</div></div>`);
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
  card.appendChild(h(`<div style="margin-top:10px;border-top:1px solid var(--ink);padding-top:8px"><strong>Tokens</strong>
    ${tok("jira_token", "jira_token")}${txt("jira_email", "jira_email")}
    ${tok("wiki_token", "wiki_token")}${txt("wiki_email", "wiki_email")}
    ${tok("figma_token", "figma_token")}
    ${txt("jenkins_user", "jenkins_user")}${tok("jenkins_token", "jenkins_token")}
    ${tok("git_token", "git_token")}${tok("chat_webhook", "chat_webhook")}
    <div class="muted">Blank token = unchanged. Stored in ~/.palugada/secrets.yaml (0600); never echoed back in full.</div></div>`));

  // Switching the auth profile must show THAT profile's tokens, not the bound
  // profile's stale values (otherwise a Save copies the old tokens across).
  // Integrations are per-project, so only the secret fields reload.
  card.querySelector("#cd-auth").onchange = async (e) => {
    const prof = e.target.value.trim() || "default";
    let s;
    try { s = await api(`/api/auth-profile/${encodeURIComponent(prof)}/secrets`); }
    catch (err) { toast(err.message, true); return; }
    card.querySelectorAll(".cd-sec").forEach(i => {
      i.value = ""; i.placeholder = (s[i.dataset.k] || "(unset)") + " — blank = keep";
    });
    card.querySelectorAll(".cd-txt").forEach(i => { i.value = s[i.dataset.k] || ""; });
  };

  // Populate the auth-profile combobox with the existing profile names.
  api("/api/auth-profiles").then(d => {
    const dl = card.querySelector("#cd-auth-list");
    if (dl) dl.innerHTML = (d.profiles || []).map(p => `<option value="${esc(p.name)}"></option>`).join("");
  }).catch(() => {});

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

// ── connectors (global setup) ──────────────────────────────────────────────
const CX = [
  { cap: "git_host", title: "Git Host", powers: ["pr", "git"],
    icon: '<path d="M9 7H6a3 3 0 0 0 0 6h3M15 7h3a3 3 0 0 1 0 6h-3M8 10h8"/>' },
  { cap: "issue_tracker", title: "Issue Tracker", powers: ["issue"],
    icon: '<path d="M9 11l3 3 7-7"/><path d="M21 12a9 9 0 1 1-6.2-8.5"/>' },
  { cap: "wiki", title: "Docs & Wiki", powers: ["wiki", "prd"],
    icon: '<path d="M4 5a2 2 0 0 1 2-2h12v18H6a2 2 0 0 1-2-2z"/><path d="M8 7h7M8 11h7"/>' },
  { cap: "ci", title: "CI / Pipelines", powers: ["ci"],
    icon: '<path d="M5 12h4l2 5 3-10 2 5h3"/>' },
  { cap: "design", title: "Design", powers: ["design"],
    icon: '<circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3"/>' },
  { cap: "chat", title: "Chat & Notify", powers: ["notify"],
    icon: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>' },
];

// Mirrors credentials::verify_kind on the backend (kept here so provider changes
// re-classify live before save).
function verifyKind(cap, provider) {
  if (cap === "issue_tracker" && provider === "github_issues") return "repo";
  if (cap === "ci" && (provider === "github_actions" || provider === "gitlab_ci")) return "repo";
  return "now";
}

// Providers that store a repo (owner/name) — mirrors the clients that read p.repo.
// git_host (github/gitlab) needs it for `pr`/`git` even though its verify is repo-free.
function usesRepo(cap, provider) {
  return (cap === "git_host" && (provider === "github" || provider === "gitlab"))
    || (cap === "issue_tracker" && provider === "github_issues")
    || (cap === "ci" && (provider === "github_actions" || provider === "gitlab_ci"));
}

// Which base_url / identifier / token fields a (cap, provider) shows.
function cxFields(cap, provider) {
  const F = {
    "git_host": { base: { hint: "blank = api.github.com (GitHub) / gitlab.com (GitLab)" }, token: { k: "git_token", l: "API token (git_token)" } },
    "issue_tracker:jira": { base: { hint: "e.g. https://you.atlassian.net", req: true }, email: { k: "jira_email", l: "Account email" }, token: { k: "jira_token", l: "API token (jira_token)" } },
    "issue_tracker:github_issues": { inherits: "git_token" },
    "wiki:confluence": { base: { hint: "e.g. https://you.atlassian.net/wiki", req: true }, email: { k: "wiki_email", l: "Account email" }, token: { k: "wiki_token", l: "API token (wiki_token)" } },
    "wiki:notion": { token: { k: "wiki_token", l: "API token (wiki_token)" } },
    "design": { token: { k: "figma_token", l: "API token (figma_token)" } },
    "ci:jenkins": { base: { hint: "e.g. https://ci.you.com", req: true }, email: { k: "jenkins_user", l: "Username (jenkins_user)" }, token: { k: "jenkins_token", l: "API token (jenkins_token)" } },
    "ci:github_actions": { inherits: "git_token" },
    "ci:gitlab_ci": { inherits: "git_token" },
    "chat": { token: { k: "chat_webhook", l: "Webhook URL (chat_webhook)" } },
  };
  return F[cap + ":" + provider] || F[cap] || {};
}

function cxStatus(cap, provider, f, sec) {
  if (!provider) return ["off", "Not set"];
  if (f.inherits) return ["info", "Uses Git Host"];
  if (verifyKind(cap, provider) === "repo") return ["info", "Verify in project"];
  const k = f.token && f.token.k;
  const has = k && sec[k] && sec[k] !== "(unset)";
  return has ? ["ok", "Configured"] : ["off", "Key needed"];
}

// Connectors target: "" = global defaults, else a project name. Set on first
// render to the active project (what the CLI uses by default).
let connTarget;
let connProfile;

async function renderConnectors() {
  view.innerHTML = viewHead("Setup", "Connectors & API Keys",
    "Wire each connector and set its API key, then Verify (works before you Save). Tokens live in <code>~/.palugada/secrets.yaml</code> (chmod 600, never shown in full).");

  // Target selector: a project (writes its config.yaml) or the global default.
  let pj;
  try { pj = await api("/api/projects"); }
  catch (e) { toast(e.message, true); return; }
  const projects = pj.projects || [];
  if (connTarget === undefined) connTarget = projects.length ? (pj.active || projects[0].name) : "";
  // guard a stale selection (e.g. project removed mid-session)
  if (connTarget !== "" && !projects.some(p => p.name === connTarget)) connTarget = pj.active && projects.some(p => p.name === pj.active) ? pj.active : "";

  const isProject = connTarget !== "";
  // The global-default page scopes its token edits to a named auth profile.
  let authProfiles = [];
  if (!isProject) {
    try { const ap = await api("/api/auth-profiles"); authProfiles = ap.profiles || []; }
    catch (e) { toast(e.message, true); return; }
    if (connProfile === undefined) connProfile = "default";
    if (!authProfiles.some(p => p.name === connProfile)) connProfile = "default";
  }
  const prefix = isProject
    ? `/api/connectors/project/${encodeURIComponent(connTarget)}`
    : `/api/auth-profiles/${encodeURIComponent(connProfile)}/connectors`;

  const opts = [`<option value=""${!isProject ? " selected" : ""}>Global default (~/.palugada.yaml)</option>`]
    .concat(projects.map(p => `<option value="${esc(p.name)}"${p.name === connTarget ? " selected" : ""}>${esc(p.name)}${p.active ? " — active" : ""}</option>`))
    .join("");
  const sel = h(`<div class="profile-chip"><span class="lbl">Editing</span>
    <select class="conn-target" style="max-width:320px">${opts}</select>
    <span class="soon">${isProject ? "writes this project's config.yaml" : "global defaults — projects inherit per-field"}</span></div>`);
  sel.querySelector(".conn-target").onchange = e => { connTarget = e.target.value; connProfile = "default"; renderConnectors(); };
  view.appendChild(sel);

  let d;
  try { d = await api(prefix); }
  catch (e) { toast(e.message, true); return; }
  if (isProject) {
    view.appendChild(h(`<div class="profile-chip"><span class="lbl">Auth profile</span> <span class="id-chip">${esc(d.auth_profile || "default")}</span> <span class="soon">from this project's config.yaml</span></div>`));
  } else {
    const inUse = (authProfiles.find(p => p.name === connProfile) || {}).in_use_by || [];
    const apOpts = authProfiles.map(p => `<option value="${esc(p.name)}"${p.name === connProfile ? " selected" : ""}>${esc(p.name)}${(p.in_use_by || []).length ? ` — used by ${(p.in_use_by || []).length}` : ""}</option>`).join("");
    const chip = h(`<div class="profile-chip"><span class="lbl">Auth profile</span>
      <select class="ap-switch" style="max-width:240px">${apOpts}</select>
      <button class="ghost ap-new" type="button">+ New</button>
      <button class="ghost ap-del" type="button"${connProfile === "default" || inUse.length ? " disabled" : ""}>Delete</button>
      <span class="soon">tokens are per-profile; the wiring below is the shared global default</span></div>`);
    chip.querySelector(".ap-switch").onchange = e => { connProfile = e.target.value; renderConnectors(); };
    chip.querySelector(".ap-new").onclick = async () => {
      const name = (prompt("New auth profile name (e.g. client-acme):") || "").trim();
      if (!name) return;
      try { await api("/api/auth-profiles", "POST", { name }); connProfile = name; toast(`created ${name}`); renderConnectors(); }
      catch (e) { toast(e.message, true); }
    };
    chip.querySelector(".ap-del").onclick = async () => {
      if (!confirm(`Delete auth profile "${connProfile}"? Its stored tokens are removed.`)) return;
      try { await api(`/api/auth-profiles/${encodeURIComponent(connProfile)}`, "DELETE"); toast(`deleted ${connProfile}`); connProfile = "default"; renderConnectors(); }
      catch (e) { toast(e.message, true); }
    };
    view.appendChild(chip);
  }
  let configured = 0, repoReady = 0, notset = 0;
  CX.forEach(c => {
    const w = (d.wiring && d.wiring[c.cap]) || { provider: "" };
    if (!w.provider) { notset++; return; }
    if (verifyKind(c.cap, w.provider) === "repo") repoReady++;
    configured++;
  });
  view.appendChild(h(`<div class="stat-grid">
    <div class="stat"><div class="k">Connectors</div><div class="v">${CX.length}</div></div>
    <div class="stat"><div class="k">Configured</div><div class="v">${configured}</div></div>
    <div class="stat"><div class="k">${isProject ? "Repo-bound" : "Verify in project"}</div><div class="v">${repoReady}</div></div>
    <div class="stat"><div class="k">Not set</div><div class="v">${notset}</div></div>
  </div>`));
  CX.forEach(c => view.appendChild(connectorCard(c, d, prefix, isProject)));
  view.appendChild(h(`<div class="card" style="box-shadow:var(--shadow-sm)">
    <div class="card-head"><h3>How this saves</h3></div>
    <p class="card-note">${isProject
      ? `Provider &amp; base URL are written to <code>${esc(connTarget)}</code>'s <code>.palugada/config.yaml</code> — exactly what <code>palugada</code> uses for this project. `
      : `Provider &amp; base URL save as <strong>global defaults</strong>; a project only uses them where its own <code>config.yaml</code> leaves that connector blank. `}Tokens go to <code>~/.palugada/secrets.yaml</code> (<code>0600</code>) and never come back to the browser — you only see <code>•••• (N chars)</code>. Blank token = keep the saved one.</p></div>`));
}

function connectorCard(c, d, prefix, isProject) {
  const w = (d.wiring && d.wiring[c.cap]) || { provider: "", base_url: "", repo: "" };
  const sec = d.secrets || {};
  const provList = ["(none)", ...(((d.providers && d.providers[c.cap]) || []))];
  let provider = w.provider || "(none)";
  const card = h(`<div class="card cx collapsed" data-cap="${esc(c.cap)}">
    <div class="cx-head">
      <span class="cx-ic"><svg viewBox="0 0 24 24" fill="none" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">${c.icon}</svg></span>
      <div><div class="cx-title">${esc(c.title)}</div><div class="cx-cap"></div></div>
      <span class="cx-spacer"></span>
      <span class="status"></span>
      <button class="cx-expand" type="button" aria-label="Expand"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9l6 6 6-6"/></svg></button>
    </div>
    <p class="cx-powers">Powers ${c.powers.map(p => `<code>palugada ${esc(p)}</code>`).join(" ")}</p>
    <div class="cx-body"></div></div>`);
  const capEl = card.querySelector(".cx-cap");
  const statusEl = card.querySelector(".status");
  const body = card.querySelector(".cx-body");

  function paintHead() {
    const prov = provider === "(none)" ? "" : provider;
    capEl.textContent = c.cap + (prov ? " · " + prov : "");
    const [cls, text] = cxStatus(c.cap, prov, cxFields(c.cap, prov), sec);
    statusEl.className = "status " + cls;
    statusEl.innerHTML = `<span class="dot"></span>${esc(text)}`;
  }
  function paintBody() {
    const prov = provider === "(none)" ? "" : provider;
    const f = cxFields(c.cap, prov);
    const provOpts = provList.map(p => `<option${provider === p ? " selected" : ""}>${esc(p)}</option>`).join("");
    let html = `<div class="cx-fields"><div class="field"><label>Provider</label><select class="cx-prov">${provOpts}</select></div>`;
    if (f.base) html += `<div class="field"><label>Base URL${f.base.req ? " (required)" : ""}</label><input class="cx-base" value="${esc(w.base_url || "")}" placeholder="${esc(f.base.hint || "")}"></div>`;
    if (f.email) html += `<div class="field"><label>${esc(f.email.l)}</label><input class="cx-email" data-k="${esc(f.email.k)}" value="${esc(sec[f.email.k] || "")}"></div>`;
    if (f.inherits) html += `<div class="field full"><label>API key</label><div class="locked-note">↳ Inherited from <strong>Git Host</strong> (<code>${esc(f.inherits)}</code>). No separate key needed.</div></div>`;
    else if (f.token) {
      const masked = sec[f.token.k] || "(unset)"; const has = masked !== "(unset)";
      html += `<div class="field full"><label>${esc(f.token.l)}</label>
        <div class="key-wrap"><input class="cx-token" data-k="${esc(f.token.k)}" type="password" placeholder="${esc(has ? masked + " · blank = keep" : "Paste token…")}"><button class="reveal" type="button">Show</button></div></div>`;
    }
    const repoBound = verifyKind(c.cap, prov) === "repo";
    // repo lives on the project, not the global default — only editable per-project.
    // Render it for ANY repo-using provider (incl. git_host) so a Save doesn't blank it.
    if (isProject && prov && usesRepo(c.cap, prov)) html += `<div class="field"><label>Repo (owner/name${repoBound ? ", required" : ""})</label><input class="cx-repo" value="${esc(w.repo || "")}" placeholder="owner/name"></div>`;
    html += `</div><div class="cx-actions"><button class="btn cx-save" type="button">Save ${esc(c.title)}</button>`;
    if (repoBound && !isProject) html += `<span class="vres info">↳ needs a repo — pick a project above to verify</span>`;
    else if (prov) html += `<button class="btn secondary cx-verify" type="button">Verify</button> <span class="vres"></span>`;
    html += `</div>`;
    body.innerHTML = html;
    body.querySelector(".cx-prov").onchange = e => { provider = e.target.value; paintHead(); paintBody(); };
    const rv = body.querySelector(".reveal");
    if (rv) rv.onclick = () => {
      const inp = body.querySelector(".cx-token");
      const show = inp.type === "password"; inp.type = show ? "text" : "password"; rv.textContent = show ? "Hide" : "Show";
    };
    body.querySelector(".cx-save").onclick = saveConnector;
    const vb = body.querySelector(".cx-verify");
    if (vb) vb.onclick = verifyConnector;
  }
  // Snapshot the CURRENT form fields — used by both Save and Verify, so Verify
  // checks exactly what you typed (no Save required first).
  function formPayload() {
    const payload = { provider: provider, base_url: "", repo: "", secrets: {} };
    const be = body.querySelector(".cx-base"); if (be) payload.base_url = be.value;
    const re = body.querySelector(".cx-repo"); if (re) payload.repo = re.value;
    const ee = body.querySelector(".cx-email"); if (ee) payload.secrets[ee.dataset.k] = ee.value;
    const te = body.querySelector(".cx-token"); if (te) payload.secrets[te.dataset.k] = te.value;
    return payload;
  }
  async function saveConnector() {
    try { await api(`${prefix}/${c.cap}`, "POST", formPayload()); toast("Saved " + c.title); renderConnectors(); }
    catch (e) { toast(e.message, true); }
  }
  async function verifyConnector() {
    const res = body.querySelector(".vres");
    res.textContent = "…"; res.className = "vres muted";
    try {
      const r = await api(`${prefix}/${c.cap}/verify`, "POST", formPayload());
      if (r.needs_repo) { res.textContent = "↳ " + (r.message || "verify from a project"); res.className = "vres info"; }
      else { res.textContent = (r.ok ? "✓ " : "✗ ") + (r.message || r.error || ""); res.className = "vres " + (r.ok ? "ok" : "err"); }
    } catch (e) { res.textContent = "✗ " + e.message; res.className = "vres err"; }
  }
  card.querySelector(".cx-head").onclick = e => { if (!e.target.closest(".cx-prov")) card.classList.toggle("collapsed"); };
  paintHead(); paintBody();
  return card;
}

function skillCard(profile, s) {
  const card = h(`<div class="card"></div>`);
  const head = h(`<div class="card-head"><h3 style="margin:0">${esc(s.name)}</h3> <span class="pill">${esc(s.kind)}</span><span class="spacer"></span></div>`);
  card.appendChild(head);
  if (s.command) card.appendChild(h(`<div class="card-note"><code>${esc(s.command)}</code></div>`));
  if (s.kind === "flow") {
    const steps = h(`<div class="steps"></div>`);
    (s.steps || []).forEach((st, i) => steps.appendChild(stepRow(profile, st, i + 1)));
    if (!s.steps || !s.steps.length) steps.appendChild(h(`<div class="muted">no steps defined for this flow</div>`));
    card.appendChild(steps);
  } else if (s.kind === "tool") {
    head.appendChild(s.enabled
      ? h(`<span class="ok-pill">active</span>`)
      : h(`<span class="warn-pill">⚠ needs ${esc((s.needs || []).join(" or "))}</span>`));
  }
  return card;
}

function stepRow(profile, st, n) {
  const num = `<span class="num">${n}</span>`;
  if (st.kind === "engine")
    return h(`<div class="step">${num}<span class="step-tag engine">engine</span> <span class="arg">${esc(st.token)}</span> <span class="hint">— ${esc(st.label)}</span></div>`);
  if (st.kind === "review_map") {
    const rows = (st.expand || []).map(e =>
      `<div class="hint" style="margin-left:40px">${esc(e.family)} → ${e.conventions.map(esc).join(", ")}</div>`).join("");
    return h(`<div class="step">${num}<span class="step-tag review_map">review_map</span> <span class="hint">by changed file kind</span>${rows}</div>`);
  }
  const missing = st.exists === false;
  const row = h(`<div class="step">${num}<span class="step-tag ${esc(st.kind)}">${esc(st.kind)}</span> <span class="arg">${esc(st.id)}</span>${
    missing ? ' <span class="warn-pill">⚠ missing</span>'
            : ' <a class="link doc-view">view</a> <a class="link doc-edit">edit</a>'
  }</div>`);
  if (!missing) {
    row.querySelector(".doc-view").onclick = () => viewDoc(profile, st.kind, st.id, row);
    row.querySelector(".doc-edit").onclick = () => editDoc(profile, st.kind, st.id, row);
  }
  return row;
}

async function viewDoc(profile, kind, id, anchor) {
  await renderDoc({ id, title: id }, kind, profile, anchor);
}

async function editDoc(profile, kind, id, anchor) {
  let b;
  // Prefill from the LOCAL (un-merged) body via /raw — editing the merged view
  // would let Save overwrite an overridden child's file with the whole merged
  // body, silently freezing its per-section inheritance. viewDoc keeps merged.
  try { b = await api(`/api/profile/${encodeURIComponent(profile)}/${kind}/${encodeURIComponent(id)}/raw`); }
  catch (e) { toast(e.message, true); return; }
  let card = document.getElementById("bodyview");
  if (card) card.remove();
  card = h(`<div class="card" id="bodyview">
    <div class="row"><strong>edit ${esc(kind)}: ${esc(id)}</strong><span class="spacer"></span><a class="link" id="ed-close">close</a></div>
    <div class="muted">Edits the profile's knowledge in <code>~/.palugada</code> (shared by all projects on '${esc(profile)}'); your project repo is not touched.</div>
    <textarea id="ed-body" style="min-height:320px;width:100%"></textarea>
    <div class="row" style="margin-top:6px"><span class="spacer"></span><button id="ed-save">Save</button></div></div>`);
  card.querySelector("#ed-body").value = b.markdown;
  placePanel(card, anchor);
  card.querySelector("#ed-close").onclick = () => card.remove();
  card.querySelector("#ed-save").onclick = async () => {
    const markdown = card.querySelector("#ed-body").value;
    card.querySelectorAll(".ed-result").forEach(el => el.remove());
    try {
      await api(`/api/profile/${encodeURIComponent(profile)}/${kind}/${encodeURIComponent(id)}/body`, "POST", { markdown });
      toast(`saved ${kind} ${id}`);
      card.querySelector("#ed-save").insertAdjacentElement("beforebegin", h(`<span class="ok-pill ed-result">saved ✓</span>`));
    } catch (e) {
      toast(e.message, true);
      card.querySelector("#ed-save").insertAdjacentElement("beforebegin", h(`<span class="warn-pill ed-result">✗ ${esc(e.message)}</span>`));
    }
  };
}

async function renderProfiles() {
  view.innerHTML = viewHead("Knowledge", "Profiles",
    "A profile is one stack's playbook — its conventions, recipes, and the flows your agents assemble. Pick one to edit, or start a new stack.");
  const form = h(`<div class="card"><div class="card-head"><h3>New profile</h3></div>
    <label>id (a-z0-9-_)</label><input id="np-id" placeholder="web-react">
    <label>title</label><input id="np-title" placeholder="Web · React">
    <label>Extends (optional base profile)<select id="np-extends"><option value="">(none — standalone)</option></select></label>
    <label>languages (comma-separated)</label><input id="np-langs" placeholder="typescript, tsx">
    <div class="row" style="margin-top:8px"><button id="np-create">Create profile</button></div></div>`);
  view.appendChild(form);
  form.querySelector("#np-create").onclick = async () => {
    const id = form.querySelector("#np-id").value.trim();
    if (!id) { toast("id required", true); return; }
    const extendsVal = form.querySelector("#np-extends").value;
    try {
      await api("/api/profile", "POST", {
        id,
        title: form.querySelector("#np-title").value.trim(),
        languages: splitCsv(form.querySelector("#np-langs").value),
        extends: extendsVal || undefined,
      });
      toast("created profile: " + id);
      renderProfiles();
    } catch (e) { toast(e.message, true); }
  };
  const listCard = h(`<div class="card"><div class="card-head"><h3>Your profiles</h3></div><div class="list" id="prof-list"></div></div>`);
  view.appendChild(listCard);
  const list = listCard.querySelector("#prof-list");
  try {
    const d = await api("/api/profiles");
    const extendsSelect = form.querySelector("#np-extends");
    d.profiles.forEach(p => {
      const opt = document.createElement("option");
      opt.value = p.id;
      opt.textContent = `${esc(p.id)} — ${esc(p.title)}`;
      extendsSelect.appendChild(opt);
    });
    d.profiles.forEach(p => {
      const item = h(`<div class="lrow" style="cursor:pointer"><span class="id-chip">${esc(p.id)}</span> <span class="ttl">${esc(p.title)}</span><span class="actions"><a class="link">Open</a></span></div>`);
      item.onclick = () => renderProfileDetail(p.id);
      list.appendChild(item);
    });
  } catch (e) { toast(e.message, true); }
}

// One convention/recipe list row, shared by Profile detail and Knowledge so both
// read identically. Convention rows show section count; recipe rows show ref count.
function docRow(meta, kind, profileId) {
  const metaBits = kind === "convention"
    ? `${(meta.sections || []).length} sections`
    : `${(meta.convention_refs || []).length} refs`;
  const originBadge = meta.origin === "inherited"
    ? ` <span class="sticker">${"inherited · " + esc(meta.from || "")}</span>`
    : meta.origin === "overridden"
    ? ` <span class="sticker">overridden</span>`
    : "";
  const isInherited = meta.origin === "inherited";
  const secondAction = isInherited
    ? ` <a class="link doc-override">Override</a>`
    : ` <a class="link doc-edit">Edit</a>`;
  const row = h(`<div class="lrow"><span class="id-chip">${esc(meta.id)}</span> <span class="ttl">${esc(meta.title || meta.id)}</span>${originBadge} <span class="meta">· ${metaBits}</span><span class="actions"><a class="link doc-view">View</a>${secondAction}</span></div>`);
  row.querySelector(".doc-view").onclick = () => renderDoc(meta, kind, profileId, row);
  if (isInherited) {
    row.querySelector(".doc-override").onclick = () => {
      // Find the nearest add-form host or create one after the row's parent list
      const list = row.parentElement;
      const existingHost = list.querySelector(".doc-override-host-" + CSS.escape(meta.id));
      if (existingHost) { existingHost.remove(); return; }
      const host = h(`<div class="doc-override-host-${esc(meta.id)}"></div>`);
      const form = kind === "convention" ? addConventionForm(profileId, meta.id) : addRecipeForm(profileId, meta.id);
      host.appendChild(form);
      row.insertAdjacentElement("afterend", host);
    };
  } else {
    row.querySelector(".doc-edit").onclick = () => editDoc(profileId, kind, meta.id, row);
  }
  return row;
}

async function renderProfileDetail(id) {
  view.innerHTML = backLink("Profiles") + viewHead("Profile", id);
  document.getElementById("back").onclick = renderProfiles;
  let d;
  try { d = await api("/api/profile/" + encodeURIComponent(id)); }
  catch (e) { toast(e.message, true); return; }

  if (d.extends) {
    view.appendChild(h(`<div class="muted" style="margin:-8px 0 12px">extends <span class="id-chip">${esc(d.extends)}</span></div>`));
  }

  // ── Browse: conventions ──
  const cv = h(`<div class="card"><div class="card-head"><h3>Conventions</h3><span class="count">${d.conventions.length}</span></div>
    <div class="card-note">Standing standards for this stack — the "right way" to write code here. Agents pull these automatically (CLI: <code>q</code>, <code>brief</code>).</div>
    <div class="list" id="cv-list"></div>
    <div class="row" id="cv-addrow" style="margin-top:6px"><button class="ghost cv-add">+ Add convention</button></div></div>`);
  const cvList = cv.querySelector("#cv-list");
  if (!d.conventions.length) cvList.appendChild(h(`<div class="muted">No conventions yet — add one to start this profile's playbook.</div>`));
  d.conventions.forEach(c => cvList.appendChild(docRow(c, "convention", id)));
  cv.querySelector(".cv-add").onclick = () => {
    if (cv.querySelector(".ac-host")) return;
    const host = h(`<div class="ac-host"></div>`);
    host.appendChild(addConventionForm(id));
    cv.querySelector("#cv-addrow").insertAdjacentElement("beforebegin", host);
  };
  view.appendChild(cv);

  // ── Browse: recipes ──
  const rc = h(`<div class="card"><div class="card-head"><h3>Recipes</h3><span class="count">${d.recipes.length}</span></div>
    <div class="card-note">Step-by-step guides for one task. Agents pull these by name (CLI: <code>for &lt;task&gt;</code>, <code>brief feature/refactor</code>).</div>
    <div class="list" id="rc-list"></div>
    <div class="row" id="rc-addrow" style="margin-top:6px"><button class="ghost rc-add">+ Add recipe</button></div></div>`);
  const rcList = rc.querySelector("#rc-list");
  if (!d.recipes.length) rcList.appendChild(h(`<div class="muted">No recipes yet.</div>`));
  d.recipes.forEach(r => rcList.appendChild(docRow(r, "recipe", id)));
  rc.querySelector(".rc-add").onclick = () => {
    if (rc.querySelector(".ar-host")) return;
    const host = h(`<div class="ar-host"></div>`);
    host.appendChild(addRecipeForm(id));
    rc.querySelector("#rc-addrow").insertAdjacentElement("beforebegin", host);
  };
  view.appendChild(rc);

  // ── Author & configure (separated lower region) ──
  view.appendChild(h(`<div class="view-head" style="margin-top:var(--s7)"><div class="eyebrow">Author &amp; configure</div><h2 class="head" style="font-size:30px">Build this profile</h2></div>`));
  view.appendChild(importCard(id));
  view.appendChild(h(`<div class="card"><h3>Fact families</h3>
    <div class="muted">Categories of symbols palugada extracts from YOUR code (e.g. viewmodel, repository, command).
    <code>palugada fact &lt;family&gt;</code> lists them with file:line. Defined in the profile's
    <code>extractors.yaml</code> (regex / tree-sitter) — not edited here.</div><div>${
    d.fact_families.map(f => `<span class="pill">${esc(f)}</span>`).join("") || '<span class="muted">none</span>'
  }</div></div>`));
  view.appendChild(flowsCard(id, d));
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

function flowsCard(id, d) {
  const flows = JSON.parse(JSON.stringify(d.flows || {}));
  const convIds = (d.conventions || []).map(c => c.id);
  const recipeIds = (d.recipes || []).map(r => r.id);
  const ENGINE = ["code.recent", "symbol.find", "module.info", "diff.scan", "prd.context"];
  const card = h(`<div class="card"><h3>Flows</h3>
    <div class="muted">Steps <code>palugada brief &lt;flow&gt;</code> assembles. Saved to the profile's <code>profile.yaml</code>; <code>brief</code> uses it live — no skill regeneration needed.</div>
    <div class="flows-body"></div>
    <div class="row" style="margin-top:8px"><input class="fl-new" placeholder="new flow name (e.g. optimize)">
      <button class="ghost fl-add-flow">+ add flow</button><span class="spacer"></span><button class="fl-save">Save flows</button></div></div>`);
  const bodyEl = card.querySelector(".flows-body");

  function rerender() {
    bodyEl.innerHTML = "";
    Object.keys(flows).forEach(fname => {
      const fl = h(`<div class="flow"><div class="row"><strong>${esc(fname)}</strong> <a class="link fl-del-flow">× flow</a></div><div class="fl-steps"></div></div>`);
      const stepsEl = fl.querySelector(".fl-steps");
      flows[fname].forEach((step, i) => {
        const chip = h(`<span class="step-chip"><code>${esc(step)}</code> <a class="link fl-up">↑</a> <a class="link fl-down">↓</a> <a class="link fl-rm">×</a></span>`);
        chip.querySelector(".fl-rm").onclick = () => { flows[fname].splice(i, 1); rerender(); };
        chip.querySelector(".fl-up").onclick = () => { if (i > 0) { const a = flows[fname]; [a[i - 1], a[i]] = [a[i], a[i - 1]]; rerender(); } };
        chip.querySelector(".fl-down").onclick = () => { const a = flows[fname]; if (i < a.length - 1) { [a[i], a[i + 1]] = [a[i + 1], a[i]]; rerender(); } };
        stepsEl.appendChild(chip);
      });
      const add = h(`<div class="row" style="margin-top:4px">
        <select class="fl-kind"><option value="engine">engine</option><option value="convention">convention(…)</option><option value="recipe">recipe(…)</option><option value="bfk">convention(by-file-kind)</option></select>
        <select class="fl-val"></select><button class="ghost fl-add-step">add step</button></div>`);
      const kindEl = add.querySelector(".fl-kind");
      const valEl = add.querySelector(".fl-val");
      function fillVal() {
        const k = kindEl.value;
        const opts = k === "engine" ? ENGINE : k === "convention" ? convIds : k === "recipe" ? recipeIds : [];
        valEl.style.display = opts.length ? "" : "none";
        valEl.innerHTML = opts.map(o => `<option>${esc(o)}</option>`).join("");
      }
      kindEl.onchange = fillVal;
      fillVal();
      add.querySelector(".fl-add-step").onclick = () => {
        const k = kindEl.value;
        let step;
        if (k === "engine") step = valEl.value;
        else if (k === "convention") step = valEl.value ? `convention(${valEl.value})` : "";
        else if (k === "recipe") step = valEl.value ? `recipe(${valEl.value})` : "";
        else step = "convention(by-file-kind)";
        if (!step) { toast("nothing to add (no conventions/recipes?)", true); return; }
        flows[fname].push(step);
        rerender();
      };
      fl.appendChild(add);
      fl.querySelector(".fl-del-flow").onclick = () => {
        if (confirm(`delete flow '${fname}'?`)) { delete flows[fname]; rerender(); }
      };
      bodyEl.appendChild(fl);
    });
  }
  rerender();

  card.querySelector(".fl-add-flow").onclick = () => {
    const name = card.querySelector(".fl-new").value.trim();
    if (!/^[a-z0-9_-]+$/.test(name)) { toast("flow name: a-z 0-9 - _ only", true); return; }
    if (flows[name]) { toast("flow already exists", true); return; }
    flows[name] = [];
    card.querySelector(".fl-new").value = "";
    rerender();
  };
  card.querySelector(".fl-save").onclick = async () => {
    try {
      await api(`/api/profile/${encodeURIComponent(id)}/flows`, "POST", { flows });
      toast("saved flows");
      renderProfileDetail(id);
    } catch (e) { toast(e.message, true); }
  };
  return card;
}

function addConventionForm(id, presetId) {
  const box = h(`<div style="margin-top:12px;border-top:1px solid var(--ink);padding-top:10px">
    <strong>${presetId ? "Override convention: " + esc(presetId) : "+ Add convention"}</strong>
    <div class="muted" style="margin:2px 0 6px">A standing standard for this stack. palugada writes the front-matter,
    section ids, and index for you — you only fill the fields below. (CLI equivalent: <code>palugada convention add &lt;file.md&gt;</code>.)</div>
    <label>id</label><input class="ac-id" placeholder="errorhandling"${presetId ? ' value="' + esc(presetId) + '"' : ""}>
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
  if (presetId) box.querySelector(".ac-id").setAttribute("readonly", "readonly");
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

function addRecipeForm(id, presetId) {
  const box = h(`<div style="margin-top:12px;border-top:1px solid var(--ink);padding-top:10px">
    <strong>${presetId ? "Override recipe: " + esc(presetId) : "+ Add recipe"}</strong>
    <div class="muted" style="margin:2px 0 6px">A step-by-step guide for one task. (CLI equivalent: <code>palugada recipe add &lt;file.md&gt;</code>.)</div>
    <label>id</label><input class="ar-id" placeholder="pagination"${presetId ? ' value="' + esc(presetId) + '"' : ""}>
    <div class="muted">lowercase slug — used in <code>palugada for pagination</code>.</div>
    <label>title</label><input class="ar-title" placeholder="Add pagination">
    <label>description</label><input class="ar-desc" placeholder="one-line summary">
    <label>tags (comma-separated)</label><input class="ar-tags" placeholder="recipe, list">
    <label>body (markdown)</label><textarea class="ar-body" placeholder="## When to use this&#10;...&#10;## Steps&#10;1. ..."></textarea>
    <div class="muted">the full procedure — write it as plain markdown (use <code>## </code> for steps/sections).</div>
    <div class="row" style="margin-top:6px"><span class="spacer"></span><button class="ar-save">Save recipe</button></div>
  </div>`);
  if (presetId) box.querySelector(".ar-id").setAttribute("readonly", "readonly");
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
  view.innerHTML = viewHead("Browse", "Knowledge", "Browse and edit any profile's conventions and recipes.");
  let d;
  try { d = await api("/api/profiles"); } catch (e) { toast(e.message, true); return; }
  if (!d.profiles.length) {
    view.appendChild(h(`<div class="empty"><div class="t">No profiles yet</div><div>Create one under Profiles to start browsing.</div></div>`));
    return;
  }
  const sel = h(`<div class="card"><label>Profile</label><select id="kn-prof">${
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
    const cl = h(`<div class="card"><div class="card-head"><h3>Conventions</h3><span class="count">${pd.conventions.length}</span></div><div class="list"></div></div>`);
    const clList = cl.querySelector(".list");
    pd.conventions.forEach(c => clList.appendChild(docRow(c, "convention", id)));
    out.appendChild(cl);
    const rl = h(`<div class="card"><div class="card-head"><h3>Recipes</h3><span class="count">${pd.recipes.length}</span></div><div class="list"></div></div>`);
    const rlList = rl.querySelector(".list");
    pd.recipes.forEach(c => rlList.appendChild(docRow(c, "recipe", id)));
    out.appendChild(rl);
  };
  document.getElementById("kn-prof").onchange = load;
  load();
}

// ── chrome: theme toggle (persisted) + mobile drawer ───────────────────────
(function () {
  const btn = document.getElementById("themeBtn");
  const label = document.getElementById("themeLabel");
  const sync = () => {
    const dark = document.documentElement.getAttribute("data-theme") === "dark";
    if (label) label.textContent = dark ? "Light" : "Dark";
  };
  if (btn) btn.onclick = () => {
    const dark = document.documentElement.getAttribute("data-theme") === "dark";
    const next = dark ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", next);
    try { localStorage.setItem("palugada-theme", next); } catch (e) {}
    sync();
  };
  sync();
  const sidebar = document.getElementById("sidebar");
  const menu = document.getElementById("menuBtn");
  if (menu && sidebar) menu.onclick = () => sidebar.classList.toggle("open");
})();

// ── boot ──────────────────────────────────────────────────────────────────
setView("overview");
