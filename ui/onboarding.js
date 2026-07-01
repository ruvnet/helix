// ============================================================================
// onboarding.js — the 3-stage "Create your private twin" wizard for hybrid.html.
//
// Self-contained, additive layer over the wired dashboard (hybrid.js is left
// untouched). It talks to the local companion server (helix-ingest serve) over
// the fixed 127.0.0.1 loopback contract:
//
//   GET  /api/status      -> { vault_exists, unlocked, record_count, by_source,
//                              connectors:[{id,name,status,cadence,last_pull}], mode }
//   POST /api/vault/unlock -> { ok, first_time }               body {passphrase}
//   POST /api/import       -> { imported, by_source, sealed }  multipart file+kind
//   GET  /api/connectors   -> [ {id,name,status,cadence,last_pull} ]
//
// After a successful import the server writes ui/private/dossier.json; we simply
// reload — hybrid.js's existing pickDossier() then loads it and shows the PRIVATE
// banner. Where the backend isn't reachable we degrade to a clearly-marked
// preview so the flow is still demoable, and NOTHING is fabricated as real.
// No external calls (only the fixed loopback + same-origin assets).
// ============================================================================

const $ = (id) => document.getElementById(id);
const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

// Served over http(s) (i.e. BY the companion) -> same origin. Opened as a bare
// file:// with no companion -> talk to the fixed loopback per the contract.
const API = (location.protocol === "http:" || location.protocol === "https:") ? location.origin : "http://127.0.0.1:8799";

const STEPS = ["intro", "pass", "load", "fresh"];
const state = { status: null, offline: false, creating: false, step: "intro", imported: null, connectors: null, lastFocus: null };

// Contract-shaped connectors used ONLY when the companion is unreachable (preview).
const MOCK_CONNECTORS = [
  { id: "apple_health", name: "Apple Health", status: "live", cadence: "Auto-push · daily", last_pull: null },
  { id: "renpho", name: "RENPHO scale", status: "coming_soon", cadence: "Daily · cloud", last_pull: null },
  { id: "quest", name: "Quest Diagnostics", status: "coming_soon", cadence: "On new result", last_pull: null },
  { id: "walgreens", name: "Walgreens", status: "coming_soon", cadence: "Weekly", last_pull: null },
  { id: "lose_it", name: "Lose It", status: "coming_soon", cadence: "Daily · food log", last_pull: null },
];

// ---- inline icons ----------------------------------------------------------
const SVG = (p, sw = "1.8") => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="${sw}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${p}</svg>`;
const ICO = {
  labs: SVG('<path d="M9 3h6M10 3v5.5L4.8 17A2 2 0 0 0 6.5 20h11a2 2 0 0 0 1.7-3L14 8.5V3"/><path d="M7.5 14h9"/>'),
  apple: SVG('<path d="M3 12h4l2.5-7 4 15 2.5-8H21"/>'),
  pill: SVG('<path d="M10.5 20.5 3.5 13.5a5 5 0 0 1 7-7l7 7a5 5 0 0 1-7 7Z"/><path d="M7 10l7 7"/>'),
  food: SVG('<path d="M5 3v7a3 3 0 0 0 3 3v8M8 3v6M17 3c-1.6 0-3 2.2-3 6 0 2 1 3 2 3v9"/>'),
  lock: SVG('<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>'),
  shield: SVG('<path d="M12 3l7 4v5c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V7z"/><path d="M9 12l2 2 4-4"/>'),
  nocloud: SVG('<path d="M3 3l18 18M18 12a4 4 0 0 0-3.2-6.4A6 6 0 0 0 6 8"/>'),
  check: SVG('<path d="M20 6 9 17l-5-5"/>', "2.2"),
  alert: SVG('<circle cx="12" cy="12" r="9"/><path d="M12 8v5m0 3h.01"/>'),
  eye: SVG('<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/>'),
  eyeoff: SVG('<path d="M3 3l18 18M10.6 10.6a3 3 0 0 0 4.2 4.2M9.9 5.1A9.5 9.5 0 0 1 12 5c6.5 0 10 7 10 7a15 15 0 0 1-2.9 3.6M6.2 6.2A15 15 0 0 0 2 12s3.5 7 10 7a9.5 9.5 0 0 0 3-.5"/>'),
};

// ---- API (all guarded; never throw to the caller for status/connectors) ----
async function apiStatus() {
  try { const r = await fetch(`${API}/api/status`, { cache: "no-cache" }); return r.ok ? await r.json() : null; }
  catch (_) { return null; }
}
async function apiUnlock(passphrase) {
  const r = await fetch(`${API}/api/vault/unlock`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ passphrase }) });
  if (!r.ok) return { ok: false };
  return await r.json();
}
async function apiImport(file, kind) {
  const fd = new FormData();
  fd.append("kind", kind);
  fd.append("file", file, file.name);
  const r = await fetch(`${API}/api/import`, { method: "POST", body: fd });
  if (!r.ok) throw new Error(`import failed (${r.status})`);
  return await r.json();
}
async function apiConnectors() {
  try { const r = await fetch(`${API}/api/connectors`, { cache: "no-cache" }); if (r.ok) return await r.json(); } catch (_) {}
  return null;
}

function ingestHost() {
  const h = location.hostname;
  if (!h || /^(127\.|localhost$|::1$|\[)/.test(h)) return "<this-mac>.local";
  return h;
}
function offlineRibbon() {
  return state.offline
    ? '<div class="wiz-offline"><span class="d" aria-hidden="true"></span>Companion not detected — this is a live preview. Nothing is imported until you launch <b>helix-ingest serve</b>.</div>'
    : "";
}

// ---- wizard shell ----------------------------------------------------------
function setHead(eyebrow, warm, title, lede) {
  const eb = $("wizEyebrow");
  eb.textContent = eyebrow; eb.className = "eyebrow" + (warm ? " warm" : "");
  $("wizTitle").textContent = title;
  $("wizLede").innerHTML = lede;
}
function updateRail() {
  const idx = STEPS.indexOf(state.step);
  document.querySelectorAll(".wiz-step").forEach((li, i) => {
    li.classList.toggle("active", i === idx);
    li.classList.toggle("done", i < idx);
    if (i === idx) li.setAttribute("aria-current", "step"); else li.removeAttribute("aria-current");
  });
}
function focusFirst() {
  const body = $("wizBody"), foot = $("wizFoot");
  const el = body.querySelector("input, button, [tabindex]") || foot.querySelector(".hx-btn:not(.ghost)") || foot.querySelector("button");
  if (el) el.focus();
}
function go(step) { state.step = step; renderStep(); }

function renderStep() {
  updateRail();
  const body = $("wizBody"), foot = $("wizFoot");
  ({ intro: renderIntro, pass: renderPass, load: renderLoad, fresh: renderFresh }[state.step])(body, foot);
  focusFirst();
}

// ---- Stage 1: intro --------------------------------------------------------
function renderIntro(body, foot) {
  setHead("Private by design", false, "Create your private twin",
    "Import your own records and Helix builds a living, on-device twin. Nothing is uploaded — your data is decrypted only on this machine and <b>sealed in a vault only you can open</b>.");
  body.innerHTML = offlineRibbon() +
    '<div class="wiz-trust">' +
      `<div class="row"><span class="ti">${ICO.lock}</span><span><b>On-device</b> — parsing and analysis run locally in Rust/WASM. No account, no cloud.</span></div>` +
      `<div class="row"><span class="ti">${ICO.shield}</span><span><b>Sealed vault</b> — every record is encrypted at rest (Argon2id → XChaCha20-Poly1305).</span></div>` +
      `<div class="row"><span class="ti">${ICO.nocloud}</span><span><b>0 B egress</b> — your health data never leaves this device unless you choose.</span></div>` +
    "</div>";
  foot.innerHTML =
    '<button class="hx-btn ghost" data-act="demo" type="button">Explore the Alex demo</button>' +
    '<span class="spacer"></span>' +
    '<button class="hx-btn" data-act="start" type="button">Get started ›</button>';
  foot.querySelector('[data-act="demo"]').onclick = closeWizard;
  foot.querySelector('[data-act="start"]').onclick = () => go("pass");
}

// ---- Stage 2: passphrase ---------------------------------------------------
function passField(id, label, ph, autocomplete) {
  return `<div class="wiz-field"><label for="${id}">${label}</label>` +
    `<div class="wiz-input"><input id="${id}" type="password" placeholder="${ph}" autocomplete="${autocomplete}" spellcheck="false" />` +
    `<button class="peek" type="button" aria-label="Show passphrase" data-peek="${id}">${ICO.eye}</button></div></div>`;
}
function renderPass(body, foot) {
  const creating = state.creating;
  setHead("Step 2 · Your key", false, creating ? "Set your vault passphrase" : "Unlock your vault",
    creating
      ? "Choose a strong passphrase. It never leaves this device and <b>cannot be recovered</b> — Helix can't read it either."
      : "Enter your passphrase to decrypt your vault on this device.");
  body.innerHTML = offlineRibbon() +
    passField("wizPass", creating ? "Create a passphrase" : "Passphrase", "••••••••••••", creating ? "new-password" : "current-password") +
    (creating ? passField("wizPass2", "Confirm passphrase", "••••••••••••", "new-password") : "") +
    `<p class="wiz-hint">${creating ? "Use something long you'll remember — a short sentence works well." : "Forgot it? Your data stays sealed; there is no back door."}</p>` +
    '<div class="wiz-err" id="wizPassErr" role="alert" hidden></div>';
  body.querySelectorAll("[data-peek]").forEach((b) => b.onclick = () => {
    const inp = $(b.dataset.peek), showing = inp.type === "text";
    inp.type = showing ? "password" : "text";
    b.innerHTML = showing ? ICO.eye : ICO.eyeoff;
    b.setAttribute("aria-label", showing ? "Show passphrase" : "Hide passphrase");
  });
  body.querySelectorAll("input").forEach((inp) => inp.addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); submitPass(); } }));

  foot.innerHTML =
    '<button class="hx-btn ghost" data-act="back" type="button">Back</button>' +
    '<span class="spacer"></span>' +
    `<button class="hx-btn" data-act="go" type="button">${state.offline ? "Continue in preview" : creating ? "Create & continue" : "Unlock & continue"}</button>`;
  foot.querySelector('[data-act="back"]').onclick = () => go("intro");
  foot.querySelector('[data-act="go"]').onclick = submitPass;
}
async function submitPass() {
  const err = $("wizPassErr");
  const showErr = (m) => { err.hidden = false; err.innerHTML = `${ICO.alert}<span>${esc(m)}</span>`; };
  const pass = ($("wizPass") || {}).value || "";
  if (!state.offline && !pass) return showErr("Enter your passphrase to continue.");
  if (state.creating) {
    const c = ($("wizPass2") || {}).value || "";
    if (!state.offline && pass.length < 8) return showErr("Use at least 8 characters.");
    if (!state.offline && pass !== c) return showErr("The two passphrases don't match.");
  }
  if (state.offline) { go("load"); return; } // preview: skip the (unreachable) unlock call

  const btn = document.querySelector('#wizFoot [data-act="go"]');
  if (btn) { btn.disabled = true; btn.textContent = "Unlocking…"; }
  try {
    const res = await apiUnlock(pass);
    if (res && res.ok) { go("load"); return; }
    showErr(state.creating ? "Couldn't create the vault. Please try again." : "That passphrase didn't unlock your vault.");
  } catch (_) {
    showErr("Couldn't reach the vault. Is the companion still running?");
  } finally {
    if (btn) { btn.disabled = false; btn.textContent = state.creating ? "Create & continue" : "Unlock & continue"; btn.focus(); }
  }
}

// ---- Stage 3A: load my data now --------------------------------------------
const CATS = [
  { kind: "fhir", name: "Labs", sub: "FHIR export (.json)", accept: ".json,application/json", icon: ICO.labs, active: true },
  { kind: "apple", name: "Apple Health", sub: "export.xml or export.zip", accept: ".xml,.zip,application/zip,text/xml", icon: ICO.apple, active: true },
  { kind: null, name: "Pharmacy", sub: "Prescription PDF", icon: ICO.pill, active: false },
  { kind: null, name: "Food log", sub: "Lose It export", icon: ICO.food, active: false },
];
function catCard(c) {
  if (!c.active) {
    return `<div class="wiz-cat soon"><span class="soon-tag">Coming soon</span>` +
      `<span class="c-ico">${c.icon}</span><h4>${esc(c.name)}</h4><p class="c-sub">${esc(c.sub)}</p>` +
      `<span class="c-accept">On the roadmap</span></div>`;
  }
  return `<div class="wiz-cat" role="button" tabindex="0" data-kind="${c.kind}" data-accept="${esc(c.accept)}" aria-label="Import ${esc(c.name)} — ${esc(c.sub)}">` +
    `<span class="c-ico">${c.icon}</span><h4>${esc(c.name)}</h4><p class="c-sub">${esc(c.sub)}</p>` +
    `<span class="c-drop">Drop a file, or click to browse</span>` +
    `<span class="c-accept">Accepts ${esc(c.accept.split(",")[0])}</span></div>`;
}
function renderLoad(body, foot) {
  setHead("Step 3 · Load my data now", true, "Bring in your records",
    "Drop a file onto a card, or click to browse. Each import is parsed, normalised and <b>sealed into your vault</b> on this device.");
  body.innerHTML = offlineRibbon() +
    `<div class="wiz-cats">${CATS.map(catCard).join("")}</div>` +
    '<div id="wizResult" aria-live="polite"></div>' +
    '<input id="wizFile" type="file" hidden />';

  const input = $("wizFile");
  let activeKind = null;
  input.onchange = () => { if (input.files && input.files[0] && activeKind) doImport(input.files[0], activeKind); input.value = ""; };

  body.querySelectorAll(".wiz-cat[data-kind]").forEach((card) => {
    const kind = card.dataset.kind;
    const pick = () => { activeKind = kind; input.accept = card.dataset.accept || ""; input.click(); };
    card.addEventListener("click", pick);
    card.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pick(); } });
    card.addEventListener("dragover", (e) => { e.preventDefault(); card.classList.add("drag"); });
    card.addEventListener("dragleave", () => card.classList.remove("drag"));
    card.addEventListener("drop", (e) => {
      e.preventDefault(); card.classList.remove("drag");
      const f = e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files[0];
      if (f) doImport(f, kind);
    });
  });

  foot.innerHTML =
    '<button class="hx-btn ghost" data-act="back" type="button">Back</button>' +
    '<span class="spacer"></span>' +
    '<button class="hx-btn" data-act="fresh" type="button">Keep it fresh ›</button>';
  foot.querySelector('[data-act="back"]').onclick = () => go("pass");
  foot.querySelector('[data-act="fresh"]').onclick = () => go("fresh");
}
async function doImport(file, kind) {
  const out = $("wizResult");
  out.innerHTML = `<div class="wiz-prog"><p class="plabel">Sealing <b>${esc(file.name)}</b> into your vault…</p><div class="pbar"><i></i></div></div>`;

  const renderSuccess = (imported, bySource, mock) => {
    state.imported = { imported, by_source: bySource, mock: !!mock };
    const chips = Object.entries(bySource || {}).map(([s, n]) => `<span class="sc"><span class="d"></span>${esc(s)} · ${Number(n).toLocaleString()}</span>`).join("");
    out.innerHTML = `<div class="wiz-success"><div class="seal">${ICO.check}</div>` +
      `<h4>Sealed ${Number(imported).toLocaleString()} record${imported === 1 ? "" : "s"} into your vault${mock ? ' <span class="mock-tag">preview</span>' : ""}</h4>` +
      `<p>${mock ? "Companion offline — this is a mocked result; nothing was actually imported." : "Encrypted at rest · 0 B egress."}</p>` +
      (chips ? `<div class="wiz-chips">${chips}</div>` : "") + "</div>";
    const panel = out.querySelector(".wiz-success");
    if (panel) panel.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "center" });
  };

  if (state.offline) { // preview only — never claim a real import happened
    setTimeout(() => renderSuccess(kind === "apple" ? 682 : 78, kind === "apple" ? { "Apple Health": 682 } : { "Quest labs": 78 }, true), 650);
    return;
  }
  try {
    const res = await apiImport(file, kind);
    renderSuccess(res.imported ?? 0, res.by_source || {}, false);
  } catch (e) {
    out.innerHTML = `<div class="wiz-err" role="alert" style="position:static">${ICO.alert}<span>Couldn't import ${esc(file.name)} — ${esc(e.message || "the companion rejected it")}. Check the file kind and try again.</span></div>`;
  }
}

// ---- Stage 3B: keep it fresh -----------------------------------------------
async function renderFresh(body, foot) {
  setHead("Step 4 · Keep it fresh", false, "Stay up to date, automatically",
    "Connect a source once and Helix keeps your twin current — on your schedule, on your device.");
  body.innerHTML = offlineRibbon() + '<div class="wiz-conns" id="wizConns"><p class="wiz-hint">Loading connectors…</p></div>';

  foot.innerHTML =
    '<button class="hx-btn ghost" data-act="back" type="button">Back</button>' +
    '<span class="spacer"></span>' +
    '<button class="hx-btn" data-act="done" type="button">Done — view my dashboard</button>';
  foot.querySelector('[data-act="back"]').onclick = () => go("load");
  foot.querySelector('[data-act="done"]').onclick = finish;

  if (!state.connectors) {
    if (state.status && Array.isArray(state.status.connectors) && state.status.connectors.length) state.connectors = state.status.connectors;
    else state.connectors = state.offline ? MOCK_CONNECTORS : (await apiConnectors()) || MOCK_CONNECTORS;
  }
  const host = ingestHost();
  const wrap = $("wizConns");
  if (!wrap) return;
  wrap.innerHTML = state.connectors.map((c) => {
    const initials = esc((c.name || "?").slice(0, 2).toUpperCase());
    if (c.status === "live") {
      const url = `http://${host}:8799/health/ingest`;
      return `<div class="wiz-conn live"><span class="k-ico">${initials}</span><div class="k-main">` +
        `<div class="k-name">${esc(c.name)}${state.offline ? ' <span class="mock-tag">preview</span>' : ""}</div>` +
        `<div class="k-meta">${esc(c.cadence || "Ready")} · push-based, no password stored</div>` +
        `<div class="wiz-hintbox"><p>Point your <b>Health Auto Export</b> app at this on-device endpoint:</p>` +
        `<div class="url"><code>${esc(url)}</code><button class="wiz-copy" type="button" data-copy="${esc(url)}">Copy</button></div></div>` +
        `</div><span class="k-pill"><span class="p" aria-hidden="true"></span>Live</span></div>`;
    }
    return `<div class="wiz-conn soon"><span class="k-ico">${initials}</span><div class="k-main">` +
      `<div class="k-name">${esc(c.name)}</div><div class="k-meta">${esc(c.cadence || "")}</div></div>` +
      `<span class="k-pill">Connect — coming soon</span></div>`;
  }).join("");
  wrap.querySelectorAll(".wiz-copy").forEach((b) => b.onclick = async () => {
    try { await navigator.clipboard.writeText(b.dataset.copy); b.textContent = "Copied ✓"; b.classList.add("ok"); setTimeout(() => { b.textContent = "Copy"; b.classList.remove("ok"); }, 1600); }
    catch (_) { b.textContent = "Copy failed"; }
  });
}
function finish() {
  if (state.imported && !state.imported.mock && !state.offline) { location.reload(); return; } // hand off to hybrid.js -> private dossier + private banner
  closeWizard();
}

// ---- open / close + focus trap --------------------------------------------
function focusables() {
  return [...$("wizOverlay").querySelectorAll('button, input, [tabindex]:not([tabindex="-1"])')].filter((el) => !el.disabled && el.offsetParent !== null);
}
// Trap is bound at DOCUMENT level so it still fires if focus ever slips to
// <body> (e.g. a submit button disabled mid-request blurs itself).
function onKeydown(e) {
  const ov = $("wizOverlay");
  if (ov.hidden) return;
  if (e.key === "Escape") { e.preventDefault(); closeWizard(); return; }
  if (e.key !== "Tab") return;
  const f = focusables(); if (!f.length) return;
  const first = f[0], last = f[f.length - 1], active = document.activeElement;
  if (!ov.contains(active)) { e.preventDefault(); first.focus(); return; } // pull escaped focus back in
  if (e.shiftKey && active === first) { e.preventDefault(); last.focus(); }
  else if (!e.shiftKey && active === last) { e.preventDefault(); first.focus(); }
}
function openWizard() {
  // (Re)derive whether this is a first-time (create) or returning (unlock) vault.
  state.creating = !!state.status && state.status.vault_exists === false;
  state.imported = null;
  state.lastFocus = document.activeElement;
  $("wizOverlay").hidden = false;
  document.body.style.overflow = "hidden";
  document.addEventListener("keydown", onKeydown);
  state.step = "intro";
  renderStep();
}
function closeWizard() {
  $("wizOverlay").hidden = true;
  document.removeEventListener("keydown", onKeydown);
  document.body.style.overflow = "";
  if (state.lastFocus && state.lastFocus.focus) state.lastFocus.focus();
}

// ---- "Make it mine" CTA (demo / offline) -----------------------------------
function mountCTA(mode) { // mode: "demo" | "offline" | none
  const main = document.querySelector("main");
  const old = $("makeMineWrap"); if (old) old.remove();
  if (!mode) return;
  const wrap = document.createElement("div");
  wrap.className = "wrap"; wrap.id = "makeMineWrap";
  wrap.innerHTML = mode === "offline"
    ? '<div class="make-mine offline" role="note"><div class="mm-spark" aria-hidden="true">⌘</div>' +
      '<div class="mm-copy"><h3>You\'re viewing the Alex demo</h3><p>Launch with <code>helix-ingest serve</code> to import your own data and build a private twin. You can still <b>preview the flow</b>.</p></div>' +
      '<div class="mm-go"><button class="hx-btn ghost" id="ctaStart" type="button">Preview the wizard</button></div></div>'
    : '<div class="make-mine" role="region" aria-label="Make Helix yours"><div class="mm-spark" aria-hidden="true">✨</div>' +
      '<div class="mm-copy"><h3>Make it mine — complete &amp; private</h3><p>You\'re viewing <b>Alex</b>, a synthetic demo. Import your own labs &amp; Apple Health — parsed and <b>sealed on-device</b>, 0&nbsp;B egress.</p></div>' +
      '<div class="mm-go"><button class="hx-btn" id="ctaStart" type="button">Create my private twin ›</button></div></div>';
  main.insertBefore(wrap, main.firstElementChild);
  $("ctaStart").onclick = openWizard;
}

// ---- boot ------------------------------------------------------------------
(async function boot() {
  const openBtn = $("wizardOpen");
  if (openBtn) openBtn.onclick = openWizard;
  const ov = $("wizOverlay");
  if (ov) ov.addEventListener("click", (e) => { if (e.target === ov) closeWizard(); });
  const x = $("wizClose"); if (x) x.onclick = closeWizard;

  const status = await apiStatus();
  state.status = status;
  state.offline = !status;

  if (status && status.vault_exists && status.record_count > 0) {
    mountCTA(null); // real data present — hybrid.js shows the PRIVATE banner; no CTA
  } else if (status) {
    mountCTA("demo"); // companion up, no data yet
  } else {
    mountCTA("offline"); // opened statically, no companion
  }
})();
