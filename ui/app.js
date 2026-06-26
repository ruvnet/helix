// Helix management console — runs the real Rust anti-hallucination pipeline
// (helix-core/score) compiled to WebAssembly. No business logic is duplicated
// in JS; this file is presentation + wiring only.
import init, {
  analyze_json,
  compose_score_json,
  redflag_registry_version,
  version,
} from "./pkg/helix.js";

const DAY = 86_400_000;
const NOW = Date.UTC(2026, 5, 25); // fixed "now" for reproducible demos

// ---- Sample dossier (stands in for the user's decrypted vault records) ----
const records = [
  rec("f1", "2276-4", "Ferritin", "Quest", 45, 45, "ng/mL", 30, 400),
  rec("f2", "2276-4", "Ferritin", "Quest", 30, 33, "ng/mL", 30, 400),
  rec("f3", "2276-4", "Ferritin", "Quest", 5, 28, "ng/mL", 30, 400),
  rec("k1", "2823-3", "Serum potassium", "Labcorp", 6, 4.3, "mmol/L", 3.5, 5.1),
  rec("k2", "2823-3", "Serum potassium", "Labcorp", 2, 4.4, "mmol/L", 3.5, 5.1),
  rec("d1", "1989-3", "Vitamin D", "Quest", 40, 22, "ng/mL", 30, 100),
];

function rec(id, code, concept, source, daysAgo, value, unit, lo, hi) {
  return {
    id, source, measured_at: NOW - daysAgo * DAY, method: "lab_feed",
    code, concept, value, unit,
    reference_range: { low: lo, high: hi }, confidence: 1.0,
  };
}

const QUESTIONS = [
  { label: "Why am I tired in the afternoons?", code: "2276-4", lo: 30, hi: 400 },
  { label: "How is my potassium?", code: "2823-3", lo: 3.5, hi: 5.1 },
  { label: "What about my testosterone?", code: "2986-8", lo: 300, hi: 1000 }, // no records → abstain
];

const SUBSYSTEMS = [
  { subsystem: "cardiometabolic", value: 78, weight: 0.35, confidence: 0.9, trend: "stable", driver: "ApoB, lipids", color: "#34e0c4" },
  { subsystem: "sleep", value: 84, weight: 0.25, confidence: 0.8, trend: "improving", driver: "Deep sleep, HRV", color: "#38bdf8" },
  { subsystem: "inflammation", value: 71, weight: 0.20, confidence: 0.85, trend: "stable", driver: "hs-CRP, ferritin", color: "#9b8cff" },
  { subsystem: "fitness", value: 66, weight: 0.20, confidence: 0.6, trend: "worsening", driver: "VO2max, RHR", color: "#f6a623" },
];

let score = null;

async function boot() {
  const pill = document.getElementById("engine-pill");
  try {
    await init();
    pill.classList.add("ready");
    document.getElementById("engine-text").textContent =
      `engine v${version()} · ${redflag_registry_version()}`;
  } catch (e) {
    pill.classList.add("err");
    document.getElementById("engine-text").textContent = "engine failed";
    console.error(e);
    return;
  }
  computeScore();
  renderDashboard();
  renderAskChips();
  renderRecords();
  renderSources();
  wireNav();
  wireModals();
}

// ---- Score (real compose() in wasm) ----
function computeScore() {
  const subs = SUBSYSTEMS.map((s) => ({
    subsystem: s.subsystem, value: s.value, weight: s.weight, confidence: s.confidence,
    trend: s.trend,
    drivers: [{ concept: s.driver, points: s.value, trend: s.trend, source_record: "rec" }],
  }));
  score = JSON.parse(compose_score_json(JSON.stringify(subs)));
}

function renderDashboard() {
  document.getElementById("score-value").textContent = Math.round(score.value);
  const off = 327 - (327 * score.value) / 100;
  document.getElementById("ring-fg").style.strokeDashoffset = off;
  const t = { improving: "▲ improving", stable: "→ holding steady", worsening: "▼ slipping" };
  document.getElementById("score-trend").textContent =
    `${t[overallTrend()] || ""} · confidence ${Math.round(score.confidence * 100)}%`;

  const sys = document.getElementById("systems");
  sys.innerHTML = "";
  SUBSYSTEMS.forEach((s) => {
    const el = document.createElement("div");
    el.className = "system";
    el.innerHTML = `<div class="s-top"><span class="s-name">${cap(s.subsystem)}</span>
      <span class="s-val">${s.value}</span></div>
      <div class="s-bar"><i style="width:${s.value}%;background:${s.color}"></i></div>`;
    el.onclick = () => openSystemModal(s);
    sys.appendChild(el);
  });
}

function overallTrend() {
  const w = { improving: 0, stable: 0, worsening: 0 };
  SUBSYSTEMS.forEach((s) => (w[s.trend] += s.weight));
  return Object.entries(w).sort((a, b) => b[1] - a[1])[0][0];
}

// ---- Ask (real analyze() in wasm) ----
function renderAskChips() {
  const box = document.getElementById("ask-chips");
  box.innerHTML = "";
  QUESTIONS.forEach((q, i) => {
    const b = document.createElement("button");
    b.className = "ask-chip" + (i === 0 ? " sel" : "");
    b.textContent = q.label;
    b.onclick = () => {
      [...box.children].forEach((c) => c.classList.remove("sel"));
      b.classList.add("sel");
      box.dataset.sel = i;
    };
    box.appendChild(b);
  });
  box.dataset.sel = 0;
  document.getElementById("run-ask").onclick = runAsk;
}

function runAsk() {
  const q = QUESTIONS[+document.getElementById("ask-chips").dataset.sel];
  const payload = {
    concept_code: q.code,
    records: records.filter((r) => r.code === q.code),
    now: NOW,
    staleness_window_days: 365,
    confidence_floor: 0.5,
    reference_low: q.lo, reference_high: q.hi,
    flat_band_per_day: 0.01,
  };
  const out = JSON.parse(analyze_json(JSON.stringify(payload)));
  document.getElementById("answer").innerHTML = renderAnswer(out, q);
}

function renderAnswer(out, q) {
  if (out.outcome === "abstained") {
    return `<div class="ans-card ans-abstain">
      <div class="ans-h">✗ I don't have that yet</div>
      <p>${out.message}</p>
      <p class="cite">→ ${out.suggested_action}</p>
      <div class="disclaimer">Helix abstains instead of guessing — this is a feature, not a failure.</div>
    </div>`;
  }
  const a = out; // answered, fields flattened
  if (a.escalation && a.escalation.level === "critical") {
    return `<div class="ans-card ans-escalate">
      <div class="ans-h">⚠ This needs a clinician now</div>
      <p>${a.escalation.message}</p>
      <div class="disclaimer">Optimization tips are suppressed when a red-flag value fires (ADR-009). Augments, never replaces, your clinician.</div>
    </div>`;
  }
  const tr = a.trend;
  const dir = { rising: "trending up", falling: "trending down", flat: "stable" }[tr.direction];
  const claim = a.claims[0];
  const cites = claim.evidence
    .map((e) => `<div class="cite">• ${e.concept} ${e.value} ${e.unit} — ${e.source}, ${fmtDate(e.measured_at)}</div>`)
    .join("");
  const pct = tr.percent_change != null ? ` (${(tr.percent_change * 100).toFixed(0)}% vs first reading)` : "";
  return `<div class="ans-card ans-grounded">
    <div class="ans-h">✓ Grounded answer</div>
    <p>${claim.text}</p>
    ${cites}
    <p class="cite">trend: ${dir}${pct} · ${tr.sample_size} readings${tr.crossings.length ? " · crossed reference range" : ""}</p>
    <span class="tier-chip">TIER 1 · YOUR DATA</span>
    ${a.recommendation ? `<p style="margin-top:10px">${a.recommendation.text}</p>` : ""}
    <div class="disclaimer">Not a diagnosis · every claim traces to a dated source you can open.</div>
  </div>`;
}

// ---- Records ----
function renderRecords() {
  const body = document.getElementById("records-body");
  body.innerHTML = "";
  records.forEach((r) => {
    const tr = document.createElement("tr");
    tr.innerHTML = `<td>${r.concept}</td><td>${r.value} ${r.unit}</td>
      <td>${fmtDate(r.measured_at)}</td><td>${r.source}</td>
      <td>${r.reference_range.low}–${r.reference_range.high}</td>`;
    body.appendChild(tr);
  });
  document.getElementById("add-record").onclick = () => openModal(addRecordModal());
}

// ---- Sources ----
function renderSources() {
  const sources = [
    { n: "Apple Health / Connect", s: "connected", d: "Steps, HR, HRV, sleep" },
    { n: "Quest · Labcorp", s: "connected", d: "Blood panels, hormones, micronutrients" },
    { n: "Oura · Whoop", s: "available", d: "Recovery, strain, sleep stages" },
    { n: "Cognitum Seed", s: "available", d: "Contactless mmWave vitals (screening)" },
    { n: "ruv-neural", s: "available", d: "40 Hz gamma-entrainment / EEG research signal (screening, not diagnosis)" },
    { n: "Genome (VCF)", s: "available", d: "User-owned import — never a third-party vault" },
  ];
  const g = document.getElementById("sources-grid");
  g.innerHTML = sources
    .map((s) => `<div class="source"><h3>${s.n}</h3>
      <span class="st ${s.s}">${s.s}</span>
      <p class="muted" style="margin:10px 0 0;font-size:13px">${s.d}</p></div>`)
    .join("");
}

// ---- Nav ----
function wireNav() {
  const titles = {
    dashboard: ["Dashboard", "Your body, assembled into one living dossier."],
    ask: ["Ask Helix", "Answered only from your data — with citations."],
    records: ["Records", "Every value, sourced and dated."],
    sources: ["Data sources", "Connect everything; it just works."],
  };
  document.querySelectorAll(".nav-item").forEach((btn) => {
    btn.onclick = () => {
      document.querySelectorAll(".nav-item").forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      const v = btn.dataset.view;
      document.querySelectorAll(".view").forEach((s) => s.classList.toggle("hidden", s.dataset.view !== v));
      document.getElementById("view-title").textContent = titles[v][0];
      document.getElementById("view-sub").textContent = titles[v][1];
    };
  });
}

// ---- Modals ----
function wireModals() {
  document.getElementById("modal-close").onclick = closeModal;
  document.getElementById("modal-host").onclick = (e) => { if (e.target.id === "modal-host") closeModal(); };
  document.getElementById("open-guide").onclick = () => openModal(guideModal());
  document.getElementById("open-score-breakdown").onclick = () => openModal(breakdownModal());
  document.querySelectorAll("[data-modal='score-info']").forEach((el) => (el.onclick = () => openModal(scoreInfoModal())));
}
function openModal(html) {
  document.getElementById("modal-content").innerHTML = html;
  document.getElementById("modal-host").classList.remove("hidden");
}
function closeModal() { document.getElementById("modal-host").classList.add("hidden"); }

function guideModal() {
  return `<h2>Get started in 4 steps</h2>
    <p>Helix turns scattered records into one interrogable dossier — grounded, private, and visual.</p>
    <ol class="steps">
      <li><h4>Connect your sources</h4><p>EMR, labs, wearables, genome, and the Cognitum Seed flow into one encrypted, on-device vault.</p></li>
      <li><h4>Helix normalizes everything</h4><p>Every value is mapped to LOINC/RxNorm/SNOMED with provenance, units, and a reference range attached.</p></li>
      <li><h4>Ask anything</h4><p>Answers are built only from your data, computed deterministically, and verified before you see them. No data → an honest "I don't have that."</p></li>
      <li><h4>See and act</h4><p>A decomposable 0–100 score and body-system view show where you stand and which way you're heading — with a clear next step.</p></li>
    </ol>`;
}
function scoreInfoModal() {
  return `<h2>About the health score</h2>
    <p>A single 0–100 number that is <b>fully decomposable</b> — never a black box. It is a weighted
    roll-up of subsystem sub-scores, each tracing to the exact measurements that drove it. It is a
    <b>wellness orientation aid, not a medical risk diagnosis</b>.</p>
    <p class="muted" style="margin-top:10px;font-size:12.5px">Methodology: ${score.methodology_version} · computed in Rust/WASM (helix-score).</p>`;
}
function breakdownModal() {
  const rows = score.subscores
    .map((s) => `<div class="bd-row"><span>${cap(s.subsystem)} <span class="muted">(${(s.weight * 100).toFixed(0)}% weight, ${(s.confidence * 100).toFixed(0)}% conf · ${s.trend})</span></span><b>${s.value}</b></div>`)
    .join("");
  return `<h2>Score breakdown</h2><p class="muted">${score.disclaimer}</p>
    <div style="margin-top:14px">${rows}
    <div class="bd-row" style="border-top:2px solid var(--vital2)"><b>Composite</b><b>${score.value} / 100</b></div></div>`;
}
function openSystemModal(s) {
  openModal(`<h2>${cap(s.subsystem)}</h2>
    <p>Sub-score <b>${s.value}/100</b> · ${(s.weight * 100).toFixed(0)}% of composite · trend ${s.trend}.</p>
    <p class="muted">Driven by: ${s.driver}. Each driver links to a dated source record (ADR-005).</p>`);
}
function addRecordModal() {
  return `<h2>Add a record</h2>
    <p class="muted">In the full app this imports from a connector or a lab PDF (OCR). For this demo, records
    are seeded so you can see the pipeline run end to end.</p>
    <p class="muted" style="font-size:12.5px;margin-top:10px">Every added value gets provenance, units, and a
    reference range before it can be used in an answer.</p>`;
}

// ---- utils ----
function cap(s) { return s.charAt(0).toUpperCase() + s.slice(1); }
function fmtDate(ms) { const d = new Date(ms); return `${d.toLocaleString("en", { month: "short" })} ${d.getDate()} ${d.getFullYear()}`; }

boot();
