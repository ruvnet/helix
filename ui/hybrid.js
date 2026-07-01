// ============================================================================
// hybrid.js — wires ui/hybrid.html (the "Aurora Companion" hybrid mockup) to
// the REAL Helix anti-hallucination pipeline compiled to WebAssembly (ui/pkg).
//
// Nothing here re-implements analytic logic in JS. Every number, trend, tier,
// citation and abstention shown on the page is produced by the audited Rust
// core (helix-score / helix-bioage / helix-timeline / helix-focus /
// helix-pipeline / helix-refranges) over the imported dossier records. The
// mockup's aesthetic (aurora frame, warm proof-trail cards, three.js twin,
// score ring, event map, deep-dive instrument) is preserved verbatim; only the
// data feeding those components changed from hardcoded to live.
// ============================================================================

import init, {
  compose_score_json,
  bioage_json,
  timeline_json,
  focus_json,
  analyze_json,
  population_range_json,
  version,
  redflag_registry_version,
} from "./pkg/helix.js";

const DAY = 86_400_000;
const reduce = window.matchMedia && matchMedia("(prefers-reduced-motion: reduce)").matches;

// ---- dossier state ---------------------------------------------------------
let NOW = Date.UTC(2026, 5, 1);
let records = [];
let MEDS = [];
let NOTES = [];
let META = null;
let SUBSYSTEMS = [];
let TIMELINE = [];
let BIOAGE = null;
let isPrivate = false;
let byCode = new Map();

// Minimal synthetic fallback so the page still renders if the fetch fails.
const FALLBACK = {
  now: NOW,
  records: [
    { id: "f1", source: "Quest Diagnostics", measured_at: NOW - 85 * DAY, method: "lab_feed", code: "2276-4", concept: "Ferritin", value: 22, unit: "ng/mL", reference_range: { low: 30, high: 400 }, confidence: 1 },
    { id: "f2", source: "Quest Diagnostics", measured_at: NOW - 40 * DAY, method: "lab_feed", code: "2276-4", concept: "Ferritin", value: 26, unit: "ng/mL", reference_range: { low: 30, high: 400 }, confidence: 1 },
    { id: "f3", source: "Quest Diagnostics", measured_at: NOW - 8 * DAY, method: "lab_feed", code: "2276-4", concept: "Ferritin", value: 29, unit: "ng/mL", reference_range: { low: 30, high: 400 }, confidence: 1 },
    { id: "l1", source: "Quest Diagnostics", measured_at: NOW - 40 * DAY, method: "lab_feed", code: "13457-7", concept: "LDL cholesterol (calc)", value: 135, unit: "mg/dL", reference_range: { low: null, high: 100 }, confidence: 1 },
    { id: "l2", source: "Quest Diagnostics", measured_at: NOW - 8 * DAY, method: "lab_feed", code: "13457-7", concept: "LDL cholesterol (calc)", value: 128, unit: "mg/dL", reference_range: { low: null, high: 100 }, confidence: 1 },
  ],
  medications: [], notes: [], timeline: [{ days_before: 88, value: 68 }, { days_before: 0, value: 75 }],
  bioage_inputs: { albumin_g_l: 47, creatinine_umol_l: 82, glucose_mmol_l: 5, crp_mg_dl: 0.11, lymphocyte_pct: 33, mcv_fl: 86, rdw_pct: 13.8, alk_phosphatase_u_l: 70, wbc_1000_ul: 5.8, age_years: 45 },
  subsystems: [
    { subsystem: "cardiometabolic", value: 72, weight: 0.35, confidence: 0.85, trend: "improving", driver: "Lipids" },
    { subsystem: "sleep", value: 76, weight: 0.25, confidence: 0.8, trend: "improving", driver: "Deep sleep, HRV" },
    { subsystem: "inflammation", value: 80, weight: 0.2, confidence: 0.85, trend: "stable", driver: "hs-CRP, ferritin" },
    { subsystem: "fitness", value: 74, weight: 0.2, confidence: 0.7, trend: "improving", driver: "Resting HR, body fat" },
  ],
  meta: { persona: { name: "Alex Rivera" }, synthetic: true },
};

// ---- utils -----------------------------------------------------------------
const cap = (s) => String(s).charAt(0).toUpperCase() + String(s).slice(1);
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
function fmtNum(n) { if (n == null || !isFinite(n)) return "—"; const r = Math.round(n * 10) / 10; return Number.isInteger(r) ? String(r) : r.toFixed(1); }
function fmtDate(ms) { const d = new Date(ms); return d.toLocaleString("en", { month: "short" }) + " " + d.getDate(); }
function clk() { const d = new Date(), p = (n) => String(n).padStart(2, "0"); return p(d.getHours()) + ":" + p(d.getMinutes()) + ":" + p(d.getSeconds()); }

function applyDossier(d) {
  NOW = d.now ?? NOW;
  records = d.records || [];
  MEDS = d.medications || [];
  NOTES = d.notes || [];
  META = d.meta || null;
  SUBSYSTEMS = d.subsystems || [];
  TIMELINE = d.timeline || [];
  BIOAGE = d.bioage_inputs || null;
  byCode = new Map();
  for (const r of records) {
    if (!r.code) continue;
    let a = byCode.get(r.code);
    if (!a) { a = []; byCode.set(r.code, a); }
    a.push(r);
  }
  for (const [, a] of byCode) a.sort((x, y) => x.measured_at - y.measured_at);
}

// Resolve a reference range for a concept: the record's OWN range first
// (a lab's stated interval), else the NHANES population fallback, else none.
// Never invents a range — a null result drives an honest abstention.
function resolveRange(recs) {
  for (let i = recs.length - 1; i >= 0; i--) {
    const rr = recs[i].reference_range;
    if (rr && (rr.low != null || rr.high != null)) return { lo: rr.low ?? null, hi: rr.high ?? null, src: "own" };
  }
  const code = recs[0] && recs[0].code;
  if (code) {
    const pop = JSON.parse(population_range_json(code));
    if (pop) return { lo: pop.low ?? null, hi: pop.high ?? null, src: "population", name: pop.name };
  }
  return { lo: null, hi: null, src: null };
}

function analyzeConcept(code, recs, rng) {
  return JSON.parse(analyze_json(JSON.stringify({
    concept_code: code, records: recs, now: NOW,
    staleness_window_days: 365, confidence_floor: 0.5,
    reference_low: rng.lo, reference_high: rng.hi,
    flat_band_per_day: 0.02, flat_band_frac: 0,
  })));
}

const isLab = (recs) => recs[recs.length - 1] && recs[recs.length - 1].method === "lab_feed";
function tierFor(recs) {
  const m = recs[recs.length - 1] && recs[recs.length - 1].method;
  if (m === "lab_feed") return { letter: "A", word: "Strong — direct lab measurement", cls: "a" };
  if (m === "manual_entry") return { letter: "B", word: "Moderate — self-logged intake", cls: "b" };
  return { letter: "B", word: "Moderate — device / wearable-derived", cls: "b" };
}

// ---- dossier selection -----------------------------------------------------
// Order: explicit ?dossier= override, then a private drop-in, then the synthetic
// demo. Only same-origin relative paths are fetched (no external data).
async function pickDossier() {
  const q = new URLSearchParams(location.search).get("dossier");
  const tries = [];
  if (q) tries.push({ url: q, priv: true });
  tries.push({ url: "./private/dossier.json", priv: true });
  tries.push({ url: "./demo-dossier.json", priv: false });
  for (const t of tries) {
    try {
      const r = await fetch(t.url, { cache: "no-cache" });
      if (r.ok) return { data: await r.json(), priv: t.priv, url: t.url };
    } catch (_) { /* optional source absent — keep looking */ }
  }
  return { data: FALLBACK, priv: false, url: "(fallback)" };
}

function renderBanner() {
  const el = document.getElementById("banner");
  if (!el) return;
  const synthetic = !isPrivate && (META?.synthetic !== false);
  if (synthetic) {
    const name = META?.persona?.name || "Alex Rivera";
    const n = (records.length + MEDS.length).toLocaleString();
    el.className = "sample-banner";
    el.innerHTML =
      '<span class="dot" aria-hidden="true"></span>' +
      `<span><b>Sample · Synthetic demo data</b> — persona “${esc(name)}” is fictional (${n} on-device records); not real PHI</span>` +
      '<span class="dot" aria-hidden="true"></span>';
  } else {
    el.className = "sample-banner private";
    const lock = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/></svg>';
    el.innerHTML =
      '<span class="dot" aria-hidden="true"></span>' + lock +
      "<span><b>Private · your data</b> — decrypted locally, analysed on-device · 0 B egress</span>" +
      '<span class="dot" aria-hidden="true"></span>';
  }
}

// ---- composite score: ring + sub-rows (compose_score_json) -----------------
function computeScore() {
  const subs = SUBSYSTEMS.map((s) => ({
    subsystem: s.subsystem, value: s.value, weight: s.weight, confidence: s.confidence, trend: s.trend,
    drivers: [{ concept: s.driver || s.subsystem, points: s.value, trend: s.trend, source_record: "rec" }],
  }));
  return JSON.parse(compose_score_json(JSON.stringify(subs)));
}

function renderScore(score) {
  const prog = document.getElementById("ringProg");
  const numEl = document.getElementById("ringNum");
  const lblEl = document.getElementById("ringLbl");
  const C = 2 * Math.PI * 92;
  prog.style.strokeDasharray = C;
  prog.style.strokeDashoffset = C;
  let shown = 0;
  function animateNum(to) {
    if (reduce) { numEl.textContent = Math.round(to); shown = to; return; }
    const from = shown, start = performance.now(), dur = 950;
    (function step(t) {
      const k = Math.min(1, (t - start) / dur), e = 1 - Math.pow(1 - k, 3);
      numEl.textContent = Math.round(from + (to - from) * e);
      if (k < 1) requestAnimationFrame(step); else shown = to;
    })(performance.now());
  }
  function setRing(value, label) { prog.style.strokeDashoffset = C * (1 - value / 100); lblEl.textContent = label; animateNum(value); }

  const comp = { value: Math.round(score.value), label: "Composite" };
  const subList = document.getElementById("subList");
  subList.innerHTML = "";
  (score.subscores || []).forEach((s) => {
    const name = cap(s.subsystem), val = Math.round(s.value);
    const row = document.createElement("div");
    row.className = "subrow";
    row.setAttribute("role", "button"); row.setAttribute("tabindex", "0");
    row.setAttribute("aria-label", `${name} score ${val} of 100`);
    row.innerHTML = `<span class="name">${esc(name)}</span><span class="bar"><i data-w="${val}"></i></span><span class="val mono-num">${val}</span>`;
    const activate = () => {
      document.querySelectorAll(".subrow").forEach((r) => r.classList.remove("active"));
      row.classList.add("active"); setRing(val, name);
    };
    row.addEventListener("click", activate);
    row.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); activate(); } });
    subList.appendChild(row);
  });
  const holder = document.querySelector(".ring-holder");
  holder.addEventListener("click", () => {
    document.querySelectorAll(".subrow").forEach((r) => r.classList.remove("active"));
    setRing(comp.value, comp.label);
  });

  // kick-off animation (matches the mockup's timing/easing)
  document.querySelectorAll(".subs .bar>i").forEach((el, i) => {
    if (reduce) { el.style.transition = "none"; el.style.width = el.getAttribute("data-w") + "%"; }
    else setTimeout(() => { el.style.width = el.getAttribute("data-w") + "%"; }, 350 + i * 90);
  });
  if (reduce) { prog.style.transition = "none"; setRing(comp.value, comp.label); }
  else setTimeout(() => setRing(comp.value, comp.label), 500);
}

// ---- bio-age (bioage_json) -------------------------------------------------
function bioageInputs() {
  return BIOAGE || { albumin_g_l: 47, creatinine_umol_l: 82, glucose_mmol_l: 5, crp_mg_dl: 0.11, lymphocyte_pct: 33, mcv_fl: 86, rdw_pct: 13.8, alk_phosphatase_u_l: 70, wbc_1000_ul: 5.8, age_years: 45 };
}

// ---- hero stat pills -------------------------------------------------------
function renderHero(score, ba, delta, firstComposite) {
  const first = (META?.persona?.name || "").trim().split(/\s+/)[0];
  document.getElementById("heroName").textContent = (first || "there") + ".";

  const byr = Math.round(ba.phenoage_years);
  const chrono = Math.round(ba.chronological_years);
  const dy = ba.phenoage_years - ba.chronological_years;
  const younger = dy < 0;
  document.getElementById("heroBioage").innerHTML =
    `${byr} <small>· chrono ${chrono} · <span class="up">${Math.abs(dy).toFixed(1)} yrs ${younger ? "younger" : "older"}</span></small>`;

  const comp = Math.round(score.value);
  const arrow = delta > 0 ? `▲ ${delta}` : delta < 0 ? `▼ ${Math.abs(delta)}` : "→ 0";
  document.getElementById("heroComposite").innerHTML =
    `${comp} <small>· <span class="up">${arrow}</span> vs 90 days ago</small>`;

  const sources = new Set();
  records.forEach((r) => sources.add(r.source));
  MEDS.forEach((m) => sources.add(m.source));
  NOTES.forEach((n) => sources.add(n.author || n.source));
  document.getElementById("heroSources").innerHTML = `${sources.size} <small>· all on-device</small>`;

  document.getElementById("scoreHint").textContent =
    `${(score.subscores || []).length} domains · weighted blend · confidence ${Math.round(score.confidence * 100)}% · up from ${firstComposite}`;
  const rd = document.getElementById("ringDelta"); if (rd) rd.textContent = arrow;
  const holder = document.querySelector(".ring-holder");
  if (holder) holder.setAttribute("aria-label", `Composite health score ${comp} of 100, ${arrow} vs 90 days ago. Click to reset to composite.`);
  const nav = document.getElementById("navDate");
  if (nav) { const d = new Date(NOW); nav.textContent = d.toLocaleDateString("en", { weekday: "short", day: "2-digit", month: "short", year: "numeric" }).replace(",", " ·"); }
}

// ---- proof trail (shared by nudge + grounded cards) ------------------------
function seriesFlow(out, unit) {
  const ev = out?.claims?.[0]?.evidence || [];
  if (ev.length >= 2) return `<span class="flow">${ev.map((e, i) => (i ? '<span class="arw">→</span>' : "") + "<b>" + fmtNum(e.value) + "</b>").join("")} ${esc(unit)}</span>`;
  if (ev.length === 1) return `<b>${fmtNum(ev[0].value)}</b> ${esc(unit)}`;
  return "—";
}
function fmtRange(lo, hi, unit) {
  const u = unit ? " " + esc(unit) : "";
  if (lo != null && hi != null) return `${fmtNum(lo)}–${fmtNum(hi)}${u}`;
  if (hi != null) return `≤ ${fmtNum(hi)}${u}`;
  if (lo != null) return `≥ ${fmtNum(lo)}${u}`;
  return "—";
}
function buildProof(p) {
  const ref = fmtRange(p.lo, p.hi, p.unit);
  const refNote = p.rangeSrc === "population"
    ? '<span class="pt-note">Population fallback (NHANES) — not your lab\'s own range</span>'
    : (p.rangeSrc === "own" ? '<span class="pt-note">From your lab\'s stated reference interval</span>' : "");
  const cite = p.lastEv
    ? `<a class="cite" href="#" onclick="return false" aria-label="Cited source: ${esc(p.lastEv.source)}, ${esc(fmtDate(p.lastEv.measured_at))}"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1"/><path d="M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1"/></svg>${esc(p.lastEv.source)} · ${esc(fmtDate(p.lastEv.measured_at))}</a>`
    : "";
  return (
    '<details class="proof">' +
      "<summary>" +
        '<svg class="chev" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M9 6l6 6-6 6"/></svg>' +
        "Why? See the proof trail" +
        '<span class="lab">Anti-hallucination</span>' +
      "</summary>" +
      '<div class="proof-body">' +
        `<div class="pt"><span class="pt-k">Own data</span><span class="pt-v">${p.source}</span></div>` +
        `<div class="pt"><span class="pt-k">Observed</span><span class="pt-v">${p.obs}</span></div>` +
        `<div class="pt"><span class="pt-k">Evidence</span><span class="pt-v"><span class="pt-tier ${p.tier.cls}">Tier ${p.tier.letter}</span> — ${p.tier.word}</span></div>` +
        `<div class="pt"><span class="pt-k">Reference</span><span class="pt-v">${ref}${refNote}<br>${cite}</span></div>` +
        `<div class="pt"><span class="pt-k">Action</span><span class="pt-v">${p.action} <span class="nondiag">· non-diagnostic</span></span></div>` +
      "</div>" +
    "</details>"
  );
}

const ICONS = {
  sleep: '<path d="M20 14.5A8 8 0 1 1 9.5 4 6.5 6.5 0 0 0 20 14.5z"/>',
  iron: '<path d="M12 3v18M6 8l6-5 6 5M7 14c0 2.8 2.2 5 5 5s5-2.2 5-5"/>',
  heart: '<path d="M20 12s-4-7-8-7-8 7-8 7 4 7 8 7 8-7 8-7Z"/><circle cx="12" cy="12" r="2.4"/>',
  body: '<path d="M4 20c0-4 3.6-7 8-7s8 3 8 7M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z"/>',
};
function iconFor(concept) {
  const c = concept.toLowerCase();
  if (/ferritin|iron|transferrin|hemoglob|hematocrit|rbc|red cell|mcv|mch/.test(c)) return "iron";
  if (/ldl|hdl|cholesterol|apolipo|triglyc|lipid|crp|heart|cardio/.test(c)) return "heart";
  if (/sleep|hrv|rem/.test(c)) return "sleep";
  if (/body|fat|weight|muscle|bmi|visceral/.test(c)) return "body";
  return "heart";
}

// ---- nudge cards (focus_json, enriched with analyze_json) ------------------
function renderNudges() {
  const grid = document.getElementById("nudgeGrid");
  grid.innerHTML = "";
  const focus = JSON.parse(focus_json(JSON.stringify({ records, now: NOW })));
  if (!focus.length) { grid.innerHTML = '<p class="report-empty">Nothing needs your attention right now — everything on file is inside its reference range.</p>'; return; }

  focus.forEach((f, i) => {
    const cited = f.cites.map((id) => records.find((r) => r.id === id)).filter(Boolean);
    const code = cited.length ? cited[cited.length - 1].code : null;
    const recs = (code && byCode.get(code)) || cited;
    const rng = resolveRange(recs.length ? recs : cited);
    let out = null;
    try { if (code) out = analyzeConcept(code, recs, rng); } catch (e) { console.warn("analyze failed for", code, e); }

    const unit = recs[recs.length - 1]?.unit || "";
    const latest = out?.trend?.latest_value ?? (cited.length ? cited[cited.length - 1].value : null);
    const below = rng.lo != null && latest != null && latest < rng.lo;
    const above = rng.hi != null && latest != null && latest > rng.hi;
    const dir = out?.trend?.direction;
    const improving = (below && dir === "rising") || (above && dir === "falling");
    const escalate = out?.escalation?.level === "critical";
    const tone = escalate ? "warm" : improving ? "cool" : "warm";
    const tier = tierFor(recs);

    const statusWord = below ? "below its reference range" : above ? "above its reference range" : "within range";
    const trendWord = dir === "rising" ? "trending up" : dir === "falling" ? "trending down" : "holding steady";
    const title = escalate
      ? `${esc(f.concept)} needs a clinician’s eyes.`
      : `Your ${esc(f.concept)} is ${statusWord}${improving ? " — and " + trendWord : ""}.`;
    const body = escalate ? esc(out.escalation.message) : esc(out?.claims?.[0]?.text || f.message);
    const action = escalate
      ? "Bring this to a clinician now — optimisation tips are suppressed on a red-flag value."
      : esc(out?.recommendation?.text || "Worth a mention at your next visit.");
    const ownSrc = cited[0]?.source || recs[0]?.source || "your records";

    const proof = buildProof({
      source: `${esc(ownSrc)} — ${esc(f.concept)} ×${recs.length} (own records)`,
      obs: `${seriesFlow(out, unit)} <span style="color:var(--text-faint)">(${statusWord}, ${trendWord})</span>`,
      tier, lo: rng.lo, hi: rng.hi, unit, rangeSrc: rng.src,
      lastEv: out?.claims?.[0]?.evidence?.slice(-1)[0] || cited[cited.length - 1],
      action,
    });

    const card = document.createElement("article");
    card.className = `nudge reveal ${tone === "cool" ? "cool" : ""}${escalate ? " escalate" : ""}`;
    card.style.setProperty("--i", i + 1);
    const stroke = tone === "cool" ? "#83e6a6" : "#f6bd7c";
    card.innerHTML =
      '<div class="nudge-top">' +
        `<span class="n-ico ${tone === "cool" ? "cool" : ""}"><svg viewBox="0 0 24 24" fill="none" stroke="${stroke}" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[iconFor(f.concept)]}</svg></span>` +
        `<span class="tier ${tier.cls}">Tier ${tier.letter}</span>` +
      "</div>" +
      `<h3>${title}</h3>` +
      `<p class="n-body">${body}</p>` +
      `<span class="own-chip"><span class="d" aria-hidden="true"></span>Own · ${esc(ownSrc)}</span>` +
      `<p class="n-action"><b>${escalate ? "Escalated" : "Next"} ·</b> ${action}</p>` +
      proof;
    grid.appendChild(card);
  });
}

// ---- per-concept grounded answers — grouped, filterable, flagged-first ------
// Enumerates every DISTINCT concept code, resolves a reference range
// (own → population fallback → none) and runs analyze_json ONCE, caching an
// item model. Results are grouped into clinical panels (Lipids, Metabolic,
// Iron, CBC, …) derived from LOINC/concept. Out-of-range markers render as rich
// proof-trail cards; the in-range bulk collapses into a dense per-panel table;
// unresolved concepts fall to the quiet "held for insufficient reference data"
// strip. A search box + status chips filter the whole set live. Nothing guessed.

const PANELS = [
  ["lipids", "Lipids"], ["metabolic", "Metabolic / Glucose"], ["iron", "Iron studies"],
  ["cbc", "CBC / Hematology"], ["kidney", "Kidney"], ["liver", "Liver"],
  ["electrolytes", "Electrolytes"], ["thyroid", "Thyroid"], ["inflammation", "Inflammation"],
  ["vitamins", "Vitamins"], ["body", "Body composition"], ["vitals", "Vitals & activity"],
  ["other", "Other"],
];
const PANEL_ORDER = PANELS.map((p) => p[0]);
const PANEL_TITLE = Object.fromEntries(PANELS);
// Exact LOINC → panel map for the known lab set (authoritative); regex on the
// concept name is the fallback so private dossiers with other codes still group.
const CODE_PANEL = {
  "1884-6": "lipids", "2085-9": "lipids", "13457-7": "lipids", "2093-3": "lipids", "2571-8": "lipids",
  "2345-7": "metabolic", "4548-4": "metabolic",
  "2276-4": "iron", "2498-4": "iron", "2500-7": "iron", "2502-3": "iron",
  "4544-3": "cbc", "718-7": "cbc", "736-9": "cbc", "786-4": "cbc", "785-6": "cbc", "787-2": "cbc",
  "770-8": "cbc", "777-3": "cbc", "789-8": "cbc", "788-0": "cbc", "6690-2": "cbc",
  "2160-0": "kidney", "62238-1": "kidney", "3094-0": "kidney",
  "6768-6": "liver", "1742-6": "liver", "1920-8": "liver", "1975-2": "liver", "1751-7": "liver", "2885-2": "liver",
  "2951-2": "electrolytes", "2075-0": "electrolytes", "2823-3": "electrolytes", "2028-9": "electrolytes", "17861-6": "electrolytes",
  "3016-3": "thyroid", "30522-7": "inflammation", "1989-3": "vitamins",
};
function panelFor(code, concept) {
  if (code && CODE_PANEL[code]) return CODE_PANEL[code];
  if (code && /^RENPHO/i.test(code)) return "body";
  if (code && /^HK-/i.test(code)) return "vitals";
  const c = (concept || "").toLowerCase();
  if (/ldl|hdl|cholesterol|apolipo|triglyc|lipid/.test(c)) return "lipids";
  if (/glucose|hba1c|a1c|insulin/.test(c)) return "metabolic";
  if (/ferritin|transferrin|\biron\b|tibc|iron binding|iron sat/.test(c)) return "iron";
  if (/hemoglob|hematocrit|corpuscular|\bmcv\b|\bmch\b|\bmchc\b|\brdw\b|platelet|leukocyt|lymphocyt|neutrophil|red (blood )?cell|white (blood )?cell|\bwbc\b|\brbc\b/.test(c)) return "cbc";
  if (/creatinin|egfr|urea|\bbun\b|cystatin/.test(c)) return "kidney";
  if (/albumin|bilirubin|\balt\b|\bast\b|alkaline phosph|\bggt\b|total protein|hepatic/.test(c)) return "liver";
  if (/sodium|potassium|chloride|calcium|magnesium|phosphate|bicarb|carbon dioxide|electrolyte|\bco2\b/.test(c)) return "electrolytes";
  if (/tsh|thyro|\bt3\b|\bt4\b/.test(c)) return "thyroid";
  if (/crp|c-reactive|\besr\b|sed rate|inflamm/.test(c)) return "inflammation";
  if (/vitamin|25-oh|folate|\bb12\b|cobalamin/.test(c)) return "vitamins";
  if (/body fat|body mass|\bbmi\b|muscle|lean|visceral|weight|body water|bone mass|fat mass|waist/.test(c)) return "body";
  if (/sleep|heart rate|\bhrv\b|spo|steps|workout|oxygen satur/.test(c)) return "vitals";
  return "other";
}

const GROUNDED = { items: [], abstains: [] };
let gStatus = "all";

function buildGroundedModel() {
  const items = [], abstains = [];
  for (const [code, recs] of byCode) {
    const concept = recs[0].concept;
    const panel = panelFor(code, concept);
    const rng = resolveRange(recs);
    if (rng.lo == null && rng.hi == null) { abstains.push({ concept, panel, code }); continue; }
    let out;
    try { out = analyzeConcept(code, recs, rng); } catch (e) { abstains.push({ concept, panel, code }); continue; }
    if (out.outcome !== "answered") { abstains.push({ concept, panel, code }); continue; }
    const lv = out.trend?.latest_value ?? recs[recs.length - 1].value;
    const outOfRange = (rng.lo != null && lv < rng.lo) || (rng.hi != null && lv > rng.hi);
    items.push({ code, concept, panel, status: outOfRange ? "out" : "in", c: { code, recs, out, rng, lv, outOfRange, lab: isLab(recs), concept } });
  }
  // out-of-range first, then direct-lab, then alphabetical (per-panel order preserved)
  items.sort((a, b) => (b.c.outOfRange - a.c.outOfRange) || (b.c.lab - a.c.lab) || a.concept.localeCompare(b.concept));
  GROUNDED.items = items; GROUNDED.abstains = abstains;
}

function renderGrounded() {
  buildGroundedModel();
  const total = GROUNDED.items.length + GROUNDED.abstains.length;
  document.getElementById("groundedIntro").innerHTML =
    `<b>${GROUNDED.items.length}</b> of your ${total} tracked concepts returned a grounded answer, grouped into clinical panels below; ` +
    `<b>${GROUNDED.abstains.length}</b> are held for insufficient reference data — Helix declines rather than guesses. Search or filter to narrow the set.`;

  const search = document.getElementById("gSearch");
  if (search && !search.dataset.wired) {
    search.dataset.wired = "1";
    search.addEventListener("input", applyGroundedFilter);
    document.querySelectorAll(".gf-chip").forEach((btn) => btn.addEventListener("click", () => {
      gStatus = btn.dataset.status;
      document.querySelectorAll(".gf-chip").forEach((b) => { const on = b === btn; b.classList.toggle("active", on); b.setAttribute("aria-pressed", on ? "true" : "false"); });
      applyGroundedFilter();
    }));
  }
  applyGroundedFilter();
}

function applyGroundedFilter() {
  const q = ((document.getElementById("gSearch") || {}).value || "").trim().toLowerCase();
  const items = GROUNDED.items.filter((it) => {
    if (q && !it.concept.toLowerCase().includes(q)) return false;
    if (gStatus === "out") return it.status === "out";
    if (gStatus === "in") return it.status === "in";
    if (gStatus === "abstain") return false;
    return true;
  });
  const abstains = GROUNDED.abstains.filter((a) => {
    if (q && !a.concept.toLowerCase().includes(q)) return false;
    return gStatus === "all" || gStatus === "abstain";
  });
  renderPanels(items, q);
  renderAbstainList(abstains);
  const outN = items.filter((i) => i.status === "out").length;
  const inN = items.length - outN;
  const cEl = document.getElementById("gCount");
  if (cEl) cEl.textContent = `${outN} out of range · ${inN} in range · ${abstains.length} held`;
}

function renderPanels(items, q) {
  const wrap = document.getElementById("panelsWrap");
  wrap.hidden = gStatus === "abstain";
  wrap.innerHTML = "";
  if (gStatus === "abstain") return;
  const groups = new Map();
  for (const it of items) { let g = groups.get(it.panel); if (!g) { g = { out: [], in: [] }; groups.set(it.panel, g); } g[it.status === "out" ? "out" : "in"].push(it.c); }
  let any = false, gi = 0;
  for (const key of PANEL_ORDER) {
    const g = groups.get(key); if (!g) continue;
    const n = g.out.length + g.in.length; if (!n) continue;
    any = true;
    const sec = document.createElement("section");
    sec.className = "panel reveal"; sec.style.setProperty("--i", (gi % 6) + 1); gi++;
    const flag = g.out.length ? `<span class="p-flag">${g.out.length} flagged</span>`
      : (gStatus === "all" ? `<span class="p-ok">all in range</span>` : "");
    let html = `<div class="panel-head"><span class="p-name">${esc(PANEL_TITLE[key])}</span><span class="p-meta">${n} marker${n === 1 ? "" : "s"}</span>${flag}</div>`;
    if (g.out.length) html += `<div class="nudge-grid panel-cards">${g.out.map((c, i) => groundedCardHTML(c, i)).join("")}</div>`;
    if (g.in.length && gStatus !== "out") {
      const open = gStatus === "in" || !!q;
      html += `<details class="inrange"${open ? " open" : ""}><summary>` +
        `<svg class="chev" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M9 6l6 6-6 6"/></svg>` +
        `<span class="ir-sum">${g.in.length} in-range marker${g.in.length === 1 ? "" : "s"}</span></summary>` +
        `<div class="inrange-table">${g.in.map(inRangeRow).join("")}</div></details>`;
    }
    sec.innerHTML = html;
    wrap.appendChild(sec);
  }
  if (!any) wrap.innerHTML = '<p class="gf-empty">No markers match — clear the search or switch filters.</p>';
}

function inRangeRow(c) {
  const unit = c.recs[c.recs.length - 1].unit || "";
  const dir = c.out.trend?.direction;
  const arrow = dir === "rising" ? "▲" : dir === "falling" ? "▼" : "→";
  const tcls = dir === "rising" ? "up" : dir === "falling" ? "down" : "flat";
  const tword = dir === "rising" ? "up" : dir === "falling" ? "down" : "steady";
  return `<div class="ir-row"><span class="ir-name">${esc(c.concept)}</span>` +
    `<span class="ir-val mono-num">${fmtNum(c.lv)} <u>${esc(unit)}</u></span>` +
    `<span class="ir-range">${fmtRange(c.rng.lo, c.rng.hi, unit)}</span>` +
    `<span class="ir-trend ${tcls}"><i>${arrow}</i> ${tword}</span>` +
    `<span class="ir-tag">in range</span></div>`;
}

function renderAbstainList(abstains) {
  const wrap = document.getElementById("abstainWrap");
  if (!wrap) return;
  if (!abstains.length) { wrap.hidden = true; return; }
  wrap.hidden = false;
  wrap.open = gStatus === "abstain";
  document.getElementById("abstainSummary").textContent = `Held for insufficient reference data · ${abstains.length}`;
  document.getElementById("abstainStrip").innerHTML = abstains
    .slice().sort((a, b) => a.concept.localeCompare(b.concept))
    .map((a) => `<span class="abstain-chip"><span class="d" aria-hidden="true"></span>${esc(a.concept)}</span>`).join("");
}

function groundedCardHTML(c, i) {
  const { out, recs, rng, lv, outOfRange } = c;
  const concept = c.concept;
  const unit = recs[recs.length - 1].unit || "";
  const dir = out.trend?.direction;
  const tier = tierFor(recs);
  const tone = outOfRange ? "warm" : "cool";
  const below = rng.lo != null && lv < rng.lo;
  const rangeLbl = outOfRange ? (below ? "below range" : "above range") : "in range";
  const arrow = dir === "rising" ? "▲" : dir === "falling" ? "▼" : "→";
  const trendCls = dir === "rising" ? "up" : dir === "falling" ? "down" : "";
  const trendWord = dir === "rising" ? "trending up" : dir === "falling" ? "trending down" : "steady";
  const statusWord = below ? "below its reference range" : outOfRange ? "above its reference range" : "within range";
  const proof = buildProof({
    source: `${esc(recs[0].source)} — ${esc(concept)} ×${recs.length} (own records)`,
    obs: `${seriesFlow(out, unit)} <span style="color:var(--text-faint)">(${statusWord}, ${trendWord})</span>`,
    tier, lo: rng.lo, hi: rng.hi, unit, rangeSrc: rng.src,
    lastEv: out.claims?.[0]?.evidence?.slice(-1)[0],
    action: esc(out.recommendation?.text || "Track on your next panel to confirm the trend."),
  });
  const stroke = tone === "cool" ? "#83e6a6" : "#f6bd7c";
  return `<article class="nudge compact reveal ${tone === "cool" ? "cool" : ""}" style="--i:${(i % 8) + 1}">` +
    '<div class="nudge-top">' +
      `<span class="n-ico ${tone === "cool" ? "cool" : ""}"><svg viewBox="0 0 24 24" fill="none" stroke="${stroke}" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[iconFor(concept)]}</svg></span>` +
      `<span class="tier ${tier.cls}">Tier ${tier.letter}</span>` +
    "</div>" +
    `<h3>${esc(concept)}</h3>` +
    `<div class="gv"><span class="num">${fmtNum(lv)}</span><span class="unit">${esc(unit)}</span>` +
      `<span class="range-chip ${outOfRange ? "out" : "inr"}">${rangeLbl}</span>` +
      `<span class="trend-tag ${trendCls}"><i>${arrow}</i> ${trendWord}</span></div>` +
    proof +
  "</article>";
}

// ---- score over time (trajectory) — real chart from timeline_json ----------
// The composite 0–100 series the sparkline already uses, drawn as a readable
// line/area chart with a date axis and a plain-language read. Per-subsystem
// HISTORY is NOT in the exports (only current value + trend), so we do NOT
// fabricate sub-lines — we show the composite trajectory prominently and a
// current per-domain trend strip beneath it, honestly labelled.
function renderTrajectory(tl) {
  const host = document.getElementById("trajChart");
  const readEl = document.getElementById("trajRead");
  const badgeEl = document.getElementById("trajBadge");
  if (!host) return;
  const pts = (tl && tl.points) || [];

  const first = pts.length ? Math.round(pts[0].value) : null;
  const last = pts.length ? Math.round(pts[pts.length - 1].value) : null;
  const delta = (first != null && last != null) ? last - first : 0;
  const days = pts.length >= 2 ? Math.max(1, Math.round((pts[pts.length - 1].at - pts[0].at) / DAY)) : 0;
  const dir = tl && tl.direction;
  const word = dir === "rising" ? "Improving" : dir === "falling" ? "Declining" : "Holding steady";
  const cls = dir === "rising" ? "up" : dir === "falling" ? "down" : "flat";

  if (readEl) {
    if (pts.length < 2) {
      readEl.innerHTML = `<span class="tr-word flat">Not enough history yet.</span> Your trajectory appears once a second dated reading lands.`;
    } else {
      const move = delta > 0 ? `up <b>${delta}</b> point${delta === 1 ? "" : "s"}`
        : delta < 0 ? `down <b>${Math.abs(delta)}</b> point${Math.abs(delta) === 1 ? "" : "s"}` : "unchanged";
      readEl.innerHTML = `<span class="tr-word ${cls}">${word}.</span> Your composite is ${move} over the last <b>${days}</b> days (${first} → ${last})${tl.change_point_at ? ' · <span class="tr-cp">change-point detected</span>' : ""}.`;
    }
  }
  if (badgeEl) {
    if (pts.length >= 2) { badgeEl.hidden = false; badgeEl.className = `traj-badge ${cls}`; badgeEl.textContent = (delta > 0 ? "▲ +" : delta < 0 ? "▼ −" : "→ ") + (delta === 0 ? "0" : Math.abs(delta)); }
    else badgeEl.hidden = true;
  }

  if (pts.length < 2) { host.innerHTML = ""; renderTrajSubs(); return; }

  // fractional plot box (0..100 in both axes; HTML overlays keep text/dots crisp)
  const GX0 = 7, GX1 = 98, GY0 = 7, GY1 = 92;
  const xs = pts.map((p) => p.at), ys = pts.map((p) => p.value);
  const x0 = Math.min(...xs), x1 = Math.max(...xs);
  const vmin = Math.min(...ys), vmax = Math.max(...ys);
  let lo = Math.max(0, Math.floor((vmin - 8) / 10) * 10);
  let hi = Math.min(100, Math.ceil((vmax + 8) / 10) * 10);
  if (hi - lo < 20) { hi = Math.min(100, lo + 20); if (hi - lo < 20) lo = Math.max(0, hi - 20); }
  const px = (x) => GX0 + ((x - x0) / ((x1 - x0) || 1)) * (GX1 - GX0);
  const py = (v) => GY0 + (1 - (v - lo) / ((hi - lo) || 1)) * (GY1 - GY0);

  let gridSvg = "", yLabels = "";
  for (let v = lo; v <= hi + 0.001; v += 10) {
    const yy = py(v).toFixed(2);
    gridSvg += `<line class="tj-grid" x1="${GX0}" y1="${yy}" x2="${GX1}" y2="${yy}" vector-effect="non-scaling-stroke"/>`;
    yLabels += `<span class="tj-yl" style="top:${yy}%">${v}</span>`;
  }
  const line = pts.map((p, i) => `${i ? "L" : "M"}${px(p.at).toFixed(2)} ${py(p.value).toFixed(2)}`).join(" ");
  const area = `${line} L${px(x1).toFixed(2)} ${GY1} L${px(x0).toFixed(2)} ${GY1} Z`;
  const cp = tl.change_point_at
    ? `<line class="tj-cp" x1="${px(tl.change_point_at).toFixed(2)}" y1="${GY0}" x2="${px(tl.change_point_at).toFixed(2)}" y2="${GY1}" vector-effect="non-scaling-stroke"/>` : "";

  const mfmt = (t) => new Date(t).toLocaleString("en", { month: "short", day: "numeric" });
  const nodes = pts.map((p, i) => {
    const isLast = i === pts.length - 1;
    return `<span class="tj-node${isLast ? " last" : ""}" style="left:${px(p.at).toFixed(2)}%;top:${py(p.value).toFixed(2)}%" tabindex="0" aria-label="${fmtDate(p.at)}: ${Math.round(p.value)} of 100"${isLast ? ` data-v="${Math.round(p.value)}"` : ""}></span>`;
  }).join("");
  const idxs = [...new Set([0, Math.round((pts.length - 1) / 3), Math.round((pts.length - 1) * 2 / 3), pts.length - 1])];
  const xLabels = idxs.map((i) => {
    const p = pts[i];
    const tx = i === 0 ? "0" : i === pts.length - 1 ? "-100%" : "-50%";
    return `<span class="tj-xl" style="left:${px(p.at).toFixed(2)}%;transform:translateX(${tx})">${mfmt(p.at)}</span>`;
  }).join("");

  host.innerHTML =
    `<div class="tj-plot">` +
      `<svg class="tj-svg" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">` +
        `<defs><linearGradient id="tjArea" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#37e6d0" stop-opacity="0.30"/><stop offset="1" stop-color="#37e6d0" stop-opacity="0"/></linearGradient>` +
        `<linearGradient id="tjLine" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#17b7ab"/><stop offset="0.6" stop-color="#37e6d0"/><stop offset="1" stop-color="#8bf3a4"/></linearGradient></defs>` +
        gridSvg +
        `<path class="tj-area" d="${area}" fill="url(#tjArea)"/>` +
        `<path class="tj-line" d="${line}" fill="none" stroke="url(#tjLine)" vector-effect="non-scaling-stroke"/>` +
        cp +
      `</svg>` +
      yLabels + nodes +
      `<div class="tj-xaxis">${xLabels}</div>` +
    `</div>`;

  renderTrajSubs();
}

function renderTrajSubs() {
  const list = document.getElementById("trajSubList");
  const note = document.getElementById("trajNote");
  if (!list) return;
  const subs = (SUBSYSTEMS && SUBSYSTEMS.length) ? SUBSYSTEMS : [];
  if (!subs.length) { list.innerHTML = ""; if (note) note.textContent = ""; return; }
  list.innerHTML = subs.map((s) => {
    const name = cap(s.subsystem), v = Math.round(s.value), t = s.trend;
    const up = t === "improving", down = t === "declining" || t === "slipping";
    const arrow = up ? "▲" : down ? "▼" : "→";
    const tcls = up ? "up" : down ? "down" : "flat";
    const tword = up ? "improving" : down ? "declining" : "holding";
    return `<div class="tsub"><span class="tsub-name">${esc(name)}</span>` +
      `<span class="tsub-bar"><i style="width:${v}%"></i></span>` +
      `<span class="tsub-val mono-num">${v}</span>` +
      `<span class="tsub-trend ${tcls}"><i>${arrow}</i> ${tword}</span></div>`;
  }).join("");
  if (note) note.innerHTML = "Today’s level &amp; direction per domain — per-domain history isn’t in this export yet, so only the composite is charted above (no fabricated sub-lines).";
}

// ---- timeline: composite trajectory + sparklines + 90-day event map --------
function buildTimeline(score) {
  const src = (Array.isArray(TIMELINE) && TIMELINE.length) ? TIMELINE : [{ days_before: 0, value: Math.round(score.value) }];
  // NOTE: Snapshot.subscores[].subsystem is a fixed Rust enum (cardiometabolic |
  // sleep | inflammation | fitness). We only need a single carrier for the
  // already-composited daily value, so we use "sleep" with weight 1 — the
  // timeline value equals the composite we pass in (same approach as app.js).
  const snaps = src.map((p) => ({
    at: NOW - p.days_before * DAY,
    subscores: [{ subsystem: "sleep", value: p.value, weight: 1, confidence: 0.9, drivers: [{ concept: "composite", points: p.value, trend: "stable", source_record: "r" }], trend: "stable" }],
  }));
  return JSON.parse(timeline_json(JSON.stringify({ snapshots: snaps, flat_band: 0.001 })));
}

function sparkSVG(data, color, id) {
  const w = 100, h = 34, pad = 4, min = Math.min(...data), max = Math.max(...data), rng = (max - min) || 1;
  const pts = data.map((v, i) => [(i / (data.length - 1)) * (w - pad * 2) + pad, h - pad - ((v - min) / rng) * (h - pad * 2)]);
  const d = pts.map((p, i) => (i ? "L" : "M") + p[0].toFixed(1) + " " + p[1].toFixed(1)).join(" ");
  const area = d + " L " + pts[pts.length - 1][0].toFixed(1) + " " + h + " L " + pts[0][0].toFixed(1) + " " + h + " Z";
  const last = pts[pts.length - 1];
  return `<svg viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" aria-hidden="true"><defs><linearGradient id="${id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${color}" stop-opacity="0.28"/><stop offset="1" stop-color="${color}" stop-opacity="0"/></linearGradient></defs><path d="${area}" fill="url(#${id})"/><path d="${d}" fill="none" stroke="${color}" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="${last[0].toFixed(1)}" cy="${last[1].toFixed(1)}" r="2.4" fill="${color}"/></svg>`;
}

function renderTimeline(score, tl) {
  const sparkRow = document.getElementById("sparkRow");
  sparkRow.innerHTML = "";
  const sparks = [];
  // #1 — composite trajectory straight from timeline_json
  sparks.push({
    name: "Composite", data: tl.points.map((p) => p.value), val: Math.round(tl.points[tl.points.length - 1].value), unit: "/100",
    dir: tl.direction, color: "#8bf3a4", tag: (tl.direction === "rising" ? "improving" : tl.direction === "falling" ? "slipping" : "steady") + (tl.change_point_at ? " · change-point" : ""),
    cls: tl.direction === "falling" ? "watch" : "good",
  });
  // #2..N — a few real concept series that exist in this dossier
  const wanted = [["2276-4", "#8bf3a4"], ["13457-7", "#37e6d0"], ["RENPHO-BFP", "#9fe9df"], ["HK-SLEEP-DEEP", "#f2c778"]];
  for (const [code, color] of wanted) {
    const recs = byCode.get(code);
    if (!recs || recs.length < 2) continue;
    const data = recs.map((r) => r.value);
    const d = data[data.length - 1] - data[0];
    const dir = Math.abs(d) < 1e-9 ? "flat" : d > 0 ? "rising" : "falling";
    const rng = resolveRange(recs);
    const lv = data[data.length - 1];
    const out = (rng.lo != null && lv < rng.lo) || (rng.hi != null && lv > rng.hi);
    sparks.push({ name: recs[0].concept, data, val: fmtNum(lv), unit: recs[0].unit, dir, color, tag: out ? "outside range" : "in range", cls: out ? "watch" : "good" });
    if (sparks.length >= 4) break;
  }
  sparks.forEach((s, i) => {
    const arrow = s.dir === "rising" ? "▲" : s.dir === "falling" ? "▼" : "→";
    const el = document.createElement("div");
    el.className = "spark";
    el.innerHTML =
      `<div class="s-top"><span class="s-name">${esc(s.name)}</span><span class="s-val mono-num">${s.val} <u>${esc(s.unit)}</u></span></div>` +
      sparkSVG(s.data, s.color, "sp" + i) +
      `<span class="s-tag ${s.cls}"><i>${arrow}</i> ${esc(s.tag)}</span>`;
    sparkRow.appendChild(el);
  });

  buildEventMap();
}

function buildEventMap() {
  const laneWrap = document.getElementById("laneWrap");
  laneWrap.innerHTML = "";
  const start = NOW - 90 * DAY;
  const pct = (t) => Math.max(0, Math.min(100, ((t - start) / (90 * DAY)) * 100));
  const inWin = (t) => t >= start - DAY && t <= NOW + DAY;

  // month tick labels + range header driven by the REAL 90-day window
  const mon = (t) => new Date(t).toLocaleString("en", { month: "short" });
  const ticksEl = document.getElementById("mapTicks");
  if (ticksEl) ticksEl.innerHTML = [0, 1 / 3, 2 / 3, 1].map((f) => `<span>${mon(start + f * 90 * DAY)}</span>`).join("");
  const rangeEl = document.getElementById("mapRange");
  if (rangeEl) rangeEl.textContent = `${mon(start)} → ${mon(NOW)} ${new Date(NOW).getFullYear()}`;

  const lane = (name, sw, inner) => {
    const el = document.createElement("div");
    el.className = "lane-a";
    el.innerHTML = `<span class="l-name"><span class="sw" style="background:${sw}"></span>${esc(name)}</span><div class="track"><div class="base"></div>${inner}</div>`;
    laneWrap.appendChild(el);
  };
  const dot = (p, color, label) => `<span class="mk dot" style="left:${p.toFixed(1)}%;background:${color}" data-l="${esc(label)}" tabindex="0" aria-label="${esc(label)}"></span>`;

  // Labs — one dot per distinct draw date (lab_feed records)
  const labDays = [...new Set(records.filter((r) => r.method === "lab_feed" && inWin(r.measured_at)).map((r) => new Date(r.measured_at).toISOString().slice(0, 10)))].sort();
  lane("Labs", "#37e6d0", labDays.map((d) => { const t = Date.parse(d + "T00:00:00Z"); return dot(pct(t), "#37e6d0", "Lab panel · " + fmtDate(t)); }).join("") || "");

  // Supplements — medication start → running bar to today
  if (MEDS.length) {
    const m = MEDS.slice().sort((a, b) => a.measured_at - b.measured_at)[0];
    const p = pct(m.measured_at);
    const shortName = String(m.concept).split(" ")[0];
    lane("Supplements", "#3fd489",
      `<span class="mk medbar" style="left:${p.toFixed(1)}%;right:0"></span>` +
      `<span class="mk medstart" style="left:${p.toFixed(1)}%"></span>` +
      dot(p, "#8bf3a4", `${shortName} started · ${fmtDate(m.measured_at)}`));
  }

  // Sleep — nightly deep-sleep bars, height scaled to value
  const deep = (byCode.get("HK-SLEEP-DEEP") || []).filter((r) => inWin(r.measured_at));
  if (deep.length) {
    const vals = deep.map((r) => r.value), mn = Math.min(...vals), mx = Math.max(...vals), rng = (mx - mn) || 1;
    const bars = deep.map((r) => {
      const h = 6 + ((r.value - mn) / rng) * 16;
      const p = pct(r.measured_at);
      const low = r.value < mn + rng * 0.35;
      return `<span class="mk sbar" style="left:${p.toFixed(1)}%;height:${h.toFixed(0)}px;width:2px;background:${low ? "#f2c778" : "rgba(242,199,120,.45)"}"></span>`;
    }).join("");
    lane("Sleep", "#f2c778", bars);
  }

  // Body — body-fat trend line + latest dot
  const bfp = (byCode.get("RENPHO-BFP") || []).filter((r) => inWin(r.measured_at));
  if (bfp.length >= 2) {
    const vals = bfp.map((r) => r.value), mn = Math.min(...vals), mx = Math.max(...vals), rng = (mx - mn) || 1;
    const pts = bfp.map((r) => [pct(r.measured_at), 22 - ((r.value - mn) / rng) * 14]);
    const d = pts.map((p, i) => (i ? "L" : "M") + p[0].toFixed(1) + " " + p[1].toFixed(1)).join(" ");
    const svg = `<svg style="position:absolute;inset:0;width:100%;height:100%" viewBox="0 0 100 26" preserveAspectRatio="none" aria-hidden="true"><path d="${d}" fill="none" stroke="#9fe9df" stroke-width="1.6" stroke-linecap="round"/></svg>`;
    const lastV = bfp[bfp.length - 1].value;
    lane("Body", "#9fe9df", svg + dot(pct(bfp[bfp.length - 1].measured_at), "#9fe9df", `${fmtNum(lastV)}% body fat · today`));
  }

  // Workouts — a tick per session
  const wk = (byCode.get("HK-WORKOUT") || []).filter((r) => inWin(r.measured_at));
  if (wk.length) {
    const now14 = NOW - 14 * DAY;
    const recent = wk.filter((r) => r.measured_at >= now14).length;
    const ticks = wk.map((r) => {
      const p = pct(r.measured_at), hot = r.measured_at >= now14;
      return `<span class="mk tick" style="left:${p.toFixed(1)}%;background:${hot ? "#17b7ab" : "rgba(23,183,171,.5)"}${hot ? ";box-shadow:0 0 6px rgba(23,183,171,.7)" : ""}"></span>`;
    }).join("");
    lane("Workouts", "#17b7ab", ticks + `<span style="position:absolute;right:0;top:-2px;font-size:.6rem;color:var(--green)">${recent} in last 14d</span>`);
  }
}

// ---- deep-dive instrument (opt-in) -----------------------------------------
function wireDeepDive(score, ba) {
  const btn = document.getElementById("instToggle");
  const panel = document.getElementById("instPanel");
  let ready = false;
  btn.addEventListener("click", () => {
    const open = panel.hasAttribute("hidden");
    const label = btn.querySelector(".inst-label");
    if (open) {
      panel.removeAttribute("hidden"); panel.classList.add("reveal-in");
      btn.setAttribute("aria-expanded", "true"); label.textContent = "Close instrument view";
      if (!ready) { ready = true; requestAnimationFrame(() => initInstrument(score, ba)); }
    } else {
      panel.setAttribute("hidden", ""); panel.classList.remove("reveal-in");
      btn.setAttribute("aria-expanded", "false"); label.textContent = "Open instrument view";
    }
  });
}

function setReadout(id, code) {
  const el = document.getElementById(id);
  const recs = byCode.get(code);
  if (!el || !recs || !recs.length) return;
  const v = fmtNum(recs[recs.length - 1].value);
  if (el.firstChild && el.firstChild.nodeType === 3) el.firstChild.nodeValue = v; else el.textContent = v;
}

function initInstrument(score, ba) {
  // channel readouts ← the real weighted sub-scores
  const chWrap = document.getElementById("channels");
  chWrap.innerHTML = (score.subscores || []).map((s) => {
    const v = Math.round(s.value), cls = v >= 80 ? "g-good" : v >= 65 ? "g-mid" : "g-low";
    return `<div class="ch ${cls}"><span class="cl"><span class="d"></span>${esc(cap(s.subsystem))}</span><span class="meter"><i data-w="${v}"></i></span><span class="cv">${v}</span></div>`;
  }).join("");
  const meters = chWrap.querySelectorAll(".meter i");
  if (reduce) meters.forEach((el) => { el.style.transition = "none"; el.style.width = el.getAttribute("data-w") + "%"; });
  else meters.forEach((el, i) => setTimeout(() => { el.style.width = el.getAttribute("data-w") + "%"; }, 120 * i + 120));
  const chSub = document.getElementById("chSub");
  if (chSub) chSub.textContent = `Channel readouts · ${(score.subscores || []).length} weighted sub-scores`;

  // bio-age readout ← bioage_json
  const dy = ba.phenoage_years - ba.chronological_years;
  document.getElementById("conBioAge").textContent = ba.phenoage_years.toFixed(1);
  document.getElementById("conChrono").textContent = Math.round(ba.chronological_years);
  document.getElementById("conBioGap").innerHTML =
    `${dy < 0 ? "◀ younger" : dy > 0 ? "older ▶" : "on par"}<br><span style="font-size:.5rem;color:var(--text-faint)">gap ${dy >= 0 ? "+" : ""}${dy.toFixed(1)} yrs</span>`;

  // oscilloscope numeric readouts ← real latest device/lab values (no jitter, no fabrication)
  setReadout("v-hr", "HK-RHR");
  setReadout("v-hrv", "HK-HRV");
  setReadout("v-spo", "59408-5");
  setReadout("v-glu", "2345-7");

  buildStream(score);
  initScopes();
}

function buildStream(score) {
  const log = document.getElementById("streamLog");
  if (!log) return;
  log.innerHTML = "";
  const lines = [];
  const ferr = byCode.get("2276-4");
  if (ferr?.length) lines.push(["quest", `ferritin <span class="g">${ferr.map((r) => fmtNum(r.value)).join("→")} ng/mL</span> · own records`]);
  const ldl = byCode.get("13457-7");
  if (ldl?.length) lines.push(["quest", `LDL <span class="a">${ldl.map((r) => fmtNum(r.value)).join("→")} mg/dL</span>`]);
  const bfp = byCode.get("RENPHO-BFP");
  if (bfp?.length) lines.push(["renpho", `body-fat <span class="g">${fmtNum(bfp[0].value)}→${fmtNum(bfp[bfp.length - 1].value)}%</span>`]);
  const deep = byCode.get("HK-SLEEP-DEEP");
  if (deep?.length) { const last7 = deep.slice(-7); const avg = Math.round(last7.reduce((s, r) => s + r.value, 0) / last7.length); lines.push(["apple-watch", `deep-sleep 7-night avg <span class="a">${avg} min</span>`]); }
  lines.push(["helix-local", `composite index <span class="g">${Math.round(score.value)}/100</span> · ${esc(score.methodology_version)}`]);
  lines.push(["vault", `${records.length.toLocaleString()} own-data points indexed · sealed`]);
  lines.push(["proof-engine", `red-flag registry ${esc(redflag_registry_version())} · engine v${esc(version())}`]);
  lines.push(["net", 'cloud uplink <span class="a">severed</span> · 0 B egress']);

  lines.forEach((l, i) => {
    const add = () => {
      const d = document.createElement("div");
      d.className = "ln";
      d.innerHTML = `<span class="ts">${clk()}</span><span class="src">${l[0]}</span><span class="msg">${l[1]}</span>`;
      log.appendChild(d); log.scrollTop = log.scrollHeight;
    };
    if (reduce) add(); else setTimeout(add, i * 110);
  });
}

// Decorative oscilloscope traces (abstract waveforms — no fabricated values are
// labelled). Ported verbatim from the mockup to keep the instrument's look.
function initScopes() {
  const scopes = [];
  document.querySelectorAll("#instPanel .scope").forEach((cv) => {
    const ctx = cv.getContext("2d");
    const color = cv.dataset.color, kind = cv.dataset.kind, buf = [];
    function size() {
      const rct = cv.parentElement.getBoundingClientRect(), dpr = Math.min(window.devicePixelRatio || 1, 2);
      cv.width = Math.max(1, rct.width * dpr); cv.height = Math.max(1, rct.height * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0); cv._w = rct.width; cv._h = rct.height;
    }
    size(); window.addEventListener("resize", size);
    scopes.push({ cv, ctx, color, kind, buf, get w() { return cv._w; }, get h() { return cv._h; } });
  });
  function sample(kind, t) {
    switch (kind) {
      case "ecg": { const p = t % 2.2; let v = 0; v += 0.08 * Math.sin(p * 3.14 / 0.2) * (p < 0.2 ? 1 : 0);
        if (p > 0.5 && p < 0.62) { const q = (p - 0.5) / 0.12; v = q < 0.35 ? -0.18 * (q / 0.35) : q < 0.55 ? -0.18 + (q - 0.35) / 0.2 * 1.15 : 1.0 - (q - 0.55) / 0.45 * 1.18; }
        if (p > 0.7 && p < 0.95) v += 0.22 * Math.sin((p - 0.7) / 0.25 * 3.14); return v * 0.7; }
      case "hrv": return (Math.sin(t * 1.1) + Math.sin(t * 2.7 + 1) * 0.4 + Math.sin(t * 0.6) * 0.3) * 0.28;
      case "resp": return Math.sin(t * 0.9) * 0.5 + Math.sin(t * 1.8) * 0.04;
      case "glu": return Math.sin(t * 0.35) * 0.4 + Math.sin(t * 0.11) * 0.2 + (Math.random() - 0.5) * 0.03;
      default: return 0;
    }
  }
  function renderScope(s) {
    const ctx = s.ctx, w = s.w, h = s.h, color = s.color;
    ctx.clearRect(0, 0, w, h);
    ctx.strokeStyle = "rgba(255,255,255,0.03)"; ctx.lineWidth = 1;
    for (let x = 0; x < w; x += 16) { ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, h); ctx.stroke(); }
    ctx.beginPath();
    const n = s.buf.length;
    for (let i = 0; i < n; i++) { const xx = i / (n - 1) * w, yy = h / 2 - s.buf[i] * h * 0.42; i ? ctx.lineTo(xx, yy) : ctx.moveTo(xx, yy); }
    ctx.strokeStyle = color; ctx.lineWidth = 1.6; ctx.shadowColor = color; ctx.shadowBlur = 6; ctx.stroke(); ctx.shadowBlur = 0;
    if (n) { const lx = w, ly = h / 2 - s.buf[n - 1] * h * 0.42; ctx.fillStyle = color; ctx.beginPath(); ctx.arc(lx - 1, ly, 2, 0, 7); ctx.fill(); }
  }
  if (reduce) {
    scopes.forEach((s) => { const cap = Math.max(60, Math.floor(s.w)); for (let x = 0; x < cap; x++) s.buf.push(sample(s.kind, x * 0.08)); renderScope(s); });
  } else {
    let t = 0;
    (function draw() {
      t += 0.055;
      scopes.forEach((s) => { const cap = Math.max(60, Math.floor(s.w)); s.buf.push(sample(s.kind, t)); while (s.buf.length > cap) s.buf.shift(); renderScope(s); });
      requestAnimationFrame(draw);
    })();
  }
}

// ---- three.js digital twin (kept as-is from the mockup) --------------------
function initTwin() {
  const canvas = document.getElementById("twin");
  const stage = canvas.parentElement;
  if (!window.THREE) { stage.classList.add("twin-fallback"); return; }
  let renderer;
  try { renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true }); }
  catch (e) { stage.classList.add("twin-fallback"); return; }

  const dims = () => ({ w: stage.clientWidth || 420, h: stage.clientHeight || 420 });
  const d0 = dims();
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setSize(d0.w, d0.h, false);
  renderer.setClearColor(0x000000, 0);

  const scene = new THREE.Scene();
  scene.fog = new THREE.FogExp2(0x04060b, 0.045);
  const camera = new THREE.PerspectiveCamera(40, d0.w / d0.h, 0.1, 100);
  camera.position.set(0, 0, 15.5);
  const group = new THREE.Group();
  scene.add(group);
  const TEAL = 0x37e6d0, GREEN = 0x8bf3a4;

  function makeGlow() {
    const c = document.createElement("canvas"); c.width = c.height = 128;
    const g = c.getContext("2d");
    const grd = g.createRadialGradient(64, 64, 0, 64, 64, 64);
    grd.addColorStop(0, "rgba(255,255,255,1)"); grd.addColorStop(0.22, "rgba(190,255,240,0.75)"); grd.addColorStop(1, "rgba(255,255,255,0)");
    g.fillStyle = grd; g.fillRect(0, 0, 128, 128);
    return new THREE.CanvasTexture(c);
  }
  const glowTex = makeGlow();

  const N = 200, turns = 3, R = 2.15, H = 9.4, p1 = [], p2 = [];
  for (let i = 0; i <= N; i++) {
    const f = i / N, t = f * turns * Math.PI * 2, y = (f - 0.5) * H;
    p1.push(new THREE.Vector3(Math.cos(t) * R, y, Math.sin(t) * R));
    p2.push(new THREE.Vector3(Math.cos(t + Math.PI) * R, y, Math.sin(t + Math.PI) * R));
  }
  const tube1 = new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(p1), 380, 0.072, 10, false), new THREE.MeshBasicMaterial({ color: TEAL, transparent: true, opacity: 0.95 }));
  const tube2 = new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3(p2), 380, 0.072, 10, false), new THREE.MeshBasicMaterial({ color: GREEN, transparent: true, opacity: 0.92 }));
  group.add(tube1, tube2);

  const rungMat = new THREE.MeshBasicMaterial({ color: 0xbfeee6, transparent: true, opacity: 0.32 });
  for (let r = 6; r < N; r += 9) {
    const a = p1[r], b = p2[r], mid = a.clone().add(b).multiplyScalar(0.5), dir = b.clone().sub(a), len = dir.length();
    const cyl = new THREE.Mesh(new THREE.CylinderGeometry(0.018, 0.018, len, 6), rungMat);
    cyl.position.copy(mid); cyl.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir.clone().normalize());
    group.add(cyl);
  }
  function addNodes(pts, coreHex, glowHex) {
    const nodeMat = new THREE.MeshBasicMaterial({ color: coreHex });
    for (let i = 0; i <= N; i += 10) {
      const s = new THREE.Mesh(new THREE.SphereGeometry(0.11, 12, 12), nodeMat);
      s.position.copy(pts[i]); group.add(s);
      const sp = new THREE.Sprite(new THREE.SpriteMaterial({ map: glowTex, color: glowHex, transparent: true, opacity: 0.85, blending: THREE.AdditiveBlending, depthWrite: false }));
      sp.scale.set(1.5, 1.5, 1); sp.position.copy(pts[i]); group.add(sp);
    }
  }
  addNodes(p1, 0xdffff8, TEAL); addNodes(p2, 0xeafff0, GREEN);

  const bloom = new THREE.Sprite(new THREE.SpriteMaterial({ map: glowTex, color: 0x1fc9bb, transparent: true, opacity: 0.42, blending: THREE.AdditiveBlending, depthWrite: false }));
  bloom.scale.set(12, 14, 1); scene.add(bloom);

  const dustGeo = new THREE.BufferGeometry(), cnt = 150, pos = new Float32Array(cnt * 3);
  for (let k = 0; k < cnt; k++) { pos[k * 3] = (Math.random() - 0.5) * 16; pos[k * 3 + 1] = (Math.random() - 0.5) * 14; pos[k * 3 + 2] = (Math.random() - 0.5) * 8; }
  dustGeo.setAttribute("position", new THREE.BufferAttribute(pos, 3));
  const dust = new THREE.Points(dustGeo, new THREE.PointsMaterial({ color: 0x8fe9df, size: 0.05, transparent: true, opacity: 0.5, blending: THREE.AdditiveBlending, depthWrite: false }));
  scene.add(dust);

  group.rotation.x = 0.16;
  let mx = 0, my = 0, tx = 0, ty = 0;
  window.addEventListener("pointermove", (e) => { tx = (e.clientX / window.innerWidth - 0.5); ty = (e.clientY / window.innerHeight - 0.5); });

  let t0 = performance.now();
  (function loop(now) {
    const dt = (now - t0) / 1000; t0 = now;
    if (!reduce) group.rotation.y += dt * 0.26;
    mx += (tx - mx) * 0.045; my += (ty - my) * 0.045;
    group.rotation.x = 0.16 + my * 0.45; group.position.x = mx * 0.9;
    dust.rotation.y -= dt * 0.04;
    renderer.render(scene, camera);
    requestAnimationFrame(loop);
  })(performance.now());

  function onResize() { const s = dims(); camera.aspect = s.w / s.h; camera.updateProjectionMatrix(); renderer.setSize(s.w, s.h, false); }
  window.addEventListener("resize", onResize); setTimeout(onResize, 60);
}

// ---- boot ------------------------------------------------------------------
(async function boot() {
  try { await init(); }
  catch (e) { console.error("Helix WASM failed to initialise:", e); const gi = document.getElementById("groundedIntro"); if (gi) gi.textContent = "On-device engine failed to load."; return; }

  const picked = await pickDossier();
  applyDossier(picked.data);
  isPrivate = picked.priv || (META?.synthetic === false);
  renderBanner();

  let score;
  try { score = computeScore(); }
  catch (e) { console.error("compose_score failed:", e); score = { value: 0, confidence: 0, subscores: [], methodology_version: "n/a" }; }
  const ba = JSON.parse(bioage_json(JSON.stringify(bioageInputs()))).bioage;
  const tl = buildTimeline(score);
  const firstComposite = Math.round(tl.points[0].value);
  const lastComposite = Math.round(tl.points[tl.points.length - 1].value);
  const delta = lastComposite - firstComposite;

  renderScore(score);
  renderHero(score, ba, delta, firstComposite);
  renderTrajectory(tl);
  renderNudges();
  renderGrounded();
  renderTimeline(score, tl);
  wireDeepDive(score, ba);
  initTwin();
})();
