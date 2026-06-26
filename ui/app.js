// Helix management console — runs the real Rust anti-hallucination pipeline
// (helix-core/score) compiled to WebAssembly. No business logic is duplicated
// in JS; this file is presentation + wiring only.
import init, {
  analyze_json,
  compose_score_json,
  redflag_registry_version,
  version,
  sensing_reading_json,
  genome_profile_json,
  bioage_json,
  focus_json,
  timeline_json,
  fhir_import_json,
  ocr_ingest_json,
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
  renderReport();
  wireNav();
  wireModals();
  wireImport();

  // Deep-link hooks (used for headless screenshot capture & shareable states).
  const h = location.hash;
  if (h === "#ask") {
    document.querySelector('.nav-item[data-view="ask"]').click();
    runAsk();
  } else if (h === "#guide") {
    openModal(guideModal());
  } else if (h === "#sensing") {
    document.querySelector('.nav-item[data-view="sources"]').click();
    runSensingDemo();
  } else if (h === "#genome") {
    document.querySelector('.nav-item[data-view="sources"]').click();
    runGenomeDemo();
  } else if (h === "#report") {
    document.querySelector('.nav-item[data-view="report"]').click();
  } else if (h === "#import") {
    document.querySelector('.nav-item[data-view="import"]').click();
  } else if (h === "#import-demo") {
    document.querySelector('.nav-item[data-view="import"]').click();
    importFhir(SAMPLE_FHIR);
    importCsv(SAMPLE_CSV);
  }
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

  renderTwin();
  renderBriefing();
}

// Status colour for a sub-score (schematic, from the user's own data — ADR-015).
function statusColor(v) {
  if (v >= 80) return "#34d399"; // good
  if (v >= 65) return "#f6a623"; // watch
  return "#fb7185"; // attention
}

// Anatomical digital twin — organ dots coloured by the real subsystem scores.
function renderTwin() {
  const byKey = Object.fromEntries(SUBSYSTEMS.map((s) => [s.subsystem, s]));
  // organ → subsystem mapping + position on the silhouette
  const organs = [
    { sys: "sleep", cx: 100, cy: 40, r: 11, label: "Brain · sleep" },
    { sys: "cardiometabolic", cx: 90, cy: 96, r: 12, label: "Heart · cardiometabolic" },
    { sys: "inflammation", cx: 112, cy: 120, r: 11, label: "Gut · inflammation" },
    { sys: "fitness", cx: 100, cy: 168, r: 11, label: "Legs · fitness" },
  ];
  const dots = organs
    .map((o) => {
      const s = byKey[o.sys];
      const c = statusColor(s.value);
      return `<g class="organ" data-sys="${o.sys}">
        <circle class="halo" cx="${o.cx}" cy="${o.cy}" r="${o.r + 8}" fill="${c}"></circle>
        <circle cx="${o.cx}" cy="${o.cy}" r="${o.r}" fill="${c}"></circle></g>`;
    })
    .join("");
  document.getElementById("twin").innerHTML = `
    <svg viewBox="0 0 200 230" role="img" aria-label="Body systems map">
      <circle cx="100" cy="28" r="16" class="body-stroke"/>
      <path d="M78 52 Q100 46 122 52 L130 138 Q100 150 70 138 Z" class="body-stroke"/>
      <g class="limb">
        <path d="M80 58 L54 120"/><path d="M120 58 L146 120"/>
        <path d="M86 146 L80 210"/><path d="M114 146 L120 210"/>
      </g>
      ${dots}
    </svg>
    <div class="twin-legend">
      <span><i style="background:#34d399"></i>good</span>
      <span><i style="background:#f6a623"></i>watch</span>
      <span><i style="background:#fb7185"></i>attention</span>
    </div>`;
  document.querySelectorAll("#twin .organ").forEach((g) => {
    g.onclick = () => {
      const s = SUBSYSTEMS.find((x) => x.subsystem === g.dataset.sys);
      openSystemModal(s);
    };
  });
}

// Dynamic "Today" briefing — the real grounded insight from the pipeline.
function renderBriefing() {
  const q = QUESTIONS[0]; // ferritin
  const out = JSON.parse(
    analyze_json(
      JSON.stringify({
        concept_code: q.code,
        records: records.filter((r) => r.code === q.code),
        now: NOW, staleness_window_days: 365, confidence_floor: 0.5,
        reference_low: q.lo, reference_high: q.hi, flat_band_per_day: 0.01,
      })
    )
  );
  const el = document.getElementById("briefing");
  if (out.outcome === "answered" && out.claims?.length) {
    const tr = out.trend;
    const e = out.claims[0].evidence[out.claims[0].evidence.length - 1];
    el.innerHTML = `Low ferritin may be behind your afternoon energy dips — it's ${e.value} ${e.unit}
      and ${tr.direction === "falling" ? "trending down" : tr.direction}.
      <span class="b-cite">${e.concept} ${e.value} ${e.unit} · ${e.source}</span>
      <span class="b-act" id="brief-ask">See the full answer →</span>`;
    document.getElementById("brief-ask").onclick = () => {
      document.querySelector('.nav-item[data-view="ask"]').click();
      runAsk();
    };
  } else {
    el.textContent = "Everything looks steady today.";
  }
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
    { n: "RuView (WiFi-CSI)", s: "live", d: "Contactless breathing/HR + fall/apnea screening — tap to run", demo: "sensing" },
    { n: "ruv-neural", s: "available", d: "40 Hz gamma-entrainment / EEG research signal (screening, not diagnosis)" },
    { n: "rvDNA genome", s: "live", d: "User-owned genome + pharmacogenomics — tap to run", demo: "genome" },
  ];
  const g = document.getElementById("sources-grid");
  g.innerHTML = sources
    .map((s, i) => `<div class="source" ${s.demo ? `data-demo="${s.demo}" style="cursor:pointer"` : ""} data-i="${i}"><h3>${s.n}</h3>
      <span class="st ${s.s === "live" ? "connected" : s.s}">${s.s}</span>
      <p class="muted" style="margin:10px 0 0;font-size:13px">${s.d}</p></div>`)
    .join("");
  g.querySelectorAll("[data-demo]").forEach((el) => {
    el.onclick = () => (el.dataset.demo === "sensing" ? runSensingDemo() : runGenomeDemo());
  });
}

// ---- Health report (full medical dashboard: bio-age, timeline, vitals, focus, recs) ----
function renderReport() {
  // Bio-age (ADR-034) — real PhenoAge from a sample routine panel.
  const pheno = {
    albumin_g_l: 47, creatinine_umol_l: 80, glucose_mmol_l: 5.0, crp_mg_dl: 0.5,
    lymphocyte_pct: 30, mcv_fl: 90, rdw_pct: 13, alk_phosphatase_u_l: 70,
    wbc_1000_ul: 5.5, age_years: 50,
  };
  const ba = JSON.parse(bioage_json(JSON.stringify(pheno))).bioage;
  const delta = ba.delta_years;
  const younger = delta < 0;
  document.getElementById("bioage").innerHTML = `
    <div class="ba-num">${ba.phenoage_years.toFixed(1)}<small style="font-size:16px;color:var(--muted2)"> yrs</small></div>
    <div class="ba-delta ${younger ? "younger" : "older"}">${younger ? "▼" : "▲"} ${Math.abs(delta).toFixed(1)} yrs ${younger ? "younger" : "older"} than your age (${ba.chronological_years})</div>
    <div class="ba-sub">Estimate from routine labs (PhenoAge) — not a diagnosis</div>`;

  // Timeline (ADR-031) — score over time from dated snapshots.
  const snap = (d, v) => ({
    at: NOW - d * DAY,
    subscores: [{ subsystem: "sleep", value: v, weight: 1, confidence: 0.9,
      drivers: [{ concept: "composite", points: v, trend: "stable", source_record: "r" }], trend: "stable" }],
  });
  const tl = JSON.parse(timeline_json(JSON.stringify({
    snapshots: [snap(180, 71), snap(140, 69), snap(100, 73), snap(60, 78), snap(20, 76), snap(0, 82)],
    flat_band: 0.001,
  })));
  document.getElementById("timeline").innerHTML = sparkline(tl);

  // Vitals & key markers — latest value per concept from the dossier.
  const byCode = {};
  records.forEach((r) => { if (!byCode[r.code] || r.measured_at > byCode[r.code].measured_at) byCode[r.code] = r; });
  document.getElementById("vitals-body").innerHTML = Object.values(byCode).map((r) => {
    const out = r.value < r.reference_range.low ? "low" : r.value > r.reference_range.high ? "high" : "ok";
    const col = out === "ok" ? "var(--ok)" : out === "low" ? "var(--vital2)" : "var(--warm)";
    return `<tr><td>${r.concept}</td><td><b>${r.value}</b> ${r.unit}</td>
      <td class="muted">${r.reference_range.low}–${r.reference_range.high}</td>
      <td style="color:${col};font-weight:700">${out.toUpperCase()}</td></tr>`;
  }).join("");

  // Focus areas (ADR-032) — real rules over the records.
  const focus = JSON.parse(focus_json(JSON.stringify({ records, now: NOW })));
  const fl = document.getElementById("focus-list");
  fl.innerHTML = focus.length ? focus.map((f) =>
    `<div class="focus-item"><span class="focus-dot fd-${f.severity}"></span>
      <div><div class="fi-msg">${f.message}</div>
      <div class="fi-cite">${f.reason.replace("_", " ")} · ${f.cites.length} reading(s)</div></div></div>`).join("")
    : `<div class="report-empty">Nothing needs attention right now.</div>`;

  // Updates & recommendations (ADR-033) — grounded, evidence-tiered.
  const ans = JSON.parse(analyze_json(JSON.stringify({
    concept_code: "2276-4", records: records.filter((r) => r.code === "2276-4"),
    now: NOW, staleness_window_days: 365, confidence_floor: 0.5,
    reference_low: 30, reference_high: 400, flat_band_per_day: 0.01,
  })));
  const recs = [];
  if (ans.outcome === "answered" && ans.recommendation)
    recs.push({ tier: "TIER 1 · YOUR DATA", text: ans.recommendation.text });
  focus.forEach((f) => recs.push({ tier: "TIER 1 · YOUR DATA", text: f.message }));
  document.getElementById("recs-list").innerHTML = recs.length ? recs.map((r) =>
    `<div class="rec-item"><span class="rec-tier">${r.tier}</span>${r.text}</div>`).join("")
    : `<div class="report-empty">No new recommendations — everything's steady.</div>`;
}

// Render a small SVG sparkline for the score timeline.
function sparkline(tl) {
  const pts = tl.points;
  const w = 520, h = 90, pad = 6;
  const xs = pts.map((p) => p.at), ys = pts.map((p) => p.value);
  const x0 = Math.min(...xs), x1 = Math.max(...xs);
  const sx = (x) => pad + ((x - x0) / (x1 - x0 || 1)) * (w - 2 * pad);
  const sy = (y) => h - pad - ((y - 40) / 60) * (h - 2 * pad); // 40..100 range
  const line = pts.map((p, i) => `${i ? "L" : "M"}${sx(p.at).toFixed(1)},${sy(p.value).toFixed(1)}`).join(" ");
  const area = `${line} L${sx(x1).toFixed(1)},${h - pad} L${sx(x0).toFixed(1)},${h - pad} Z`;
  const dots = pts.map((p) => `<circle class="tl-dot" cx="${sx(p.at).toFixed(1)}" cy="${sy(p.value).toFixed(1)}" r="3"/>`).join("");
  const cp = tl.change_point_at ? `<line class="tl-cp" x1="${sx(tl.change_point_at).toFixed(1)}" y1="${pad}" x2="${sx(tl.change_point_at).toFixed(1)}" y2="${h - pad}"/>` : "";
  const dir = { rising: "▲ improving", falling: "▼ slipping", flat: "→ steady" }[tl.direction] || "";
  return `<svg viewBox="0 0 ${w} ${h}" preserveAspectRatio="none">
    <defs><linearGradient id="tlg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#34e0c4"/><stop offset="1" stop-color="#34e0c4" stop-opacity="0"/></linearGradient></defs>
    <path class="tl-area" d="${area}"/><path class="tl-line" d="${line}"/>${cp}${dots}
  </svg><div class="tl-cap">${dir} · ${pts.length} points${tl.change_point_at ? " · change-point detected" : ""}</div>`;
}

// ---- Import (FHIR / OCR image / CSV → real records into the live dossier) ----
let importedCount = 0;

function addImportedRecords(recs, badge = "imported") {
  recs.forEach((r) => records.push(r));
  importedCount += recs.length;
  // re-render everything that consumes records
  renderRecords();
  renderReport();
  const log = document.getElementById("import-log");
  if (importedCount === recs.length) log.innerHTML = "";
  recs.forEach((r) => {
    const div = document.createElement("div");
    div.className = "imp-row";
    div.innerHTML = `<span>${r.concept} <b>${r.value}</b> ${r.unit}
      <span class="imp-src">· ${r.source}${r.code ? " · " + r.code : ""}</span></span>
      <span class="imp-badge imp-ok">${badge}</span>`;
    log.appendChild(div);
  });
  document.getElementById("import-count").textContent = `· ${importedCount} value(s)`;
}
function logQueued(label, reason) {
  const log = document.getElementById("import-log");
  const div = document.createElement("div");
  div.className = "imp-row";
  div.innerHTML = `<span>${label} <span class="imp-src">· held: ${reason.replace(/_/g, " ")}</span></span>
    <span class="imp-badge imp-queue">review</span>`;
  log.appendChild(div);
}

const SAMPLE_FHIR = JSON.stringify({
  resourceType: "Bundle",
  entry: [
    { resource: { resourceType: "Observation", id: "hdl1",
      code: { coding: [{ system: "http://loinc.org", code: "2085-9", display: "HDL Cholesterol" }] },
      valueQuantity: { value: 58, unit: "mg/dL" }, effectiveDateTime: "2026-06-10",
      referenceRange: [{ low: { value: 40 }, high: { value: 100 } }] } },
    { resource: { resourceType: "Observation", id: "tsh1",
      code: { coding: [{ system: "http://loinc.org", code: "3016-3", display: "TSH" }] },
      valueQuantity: { value: 2.1, unit: "mIU/L" }, effectiveDateTime: "2026-06-10",
      referenceRange: [{ low: { value: 0.4 }, high: { value: 4.0 } }] } },
  ],
}, null, 2);
const SAMPLE_CSV = "Vitamin B12, 2132-9, 540, pg/mL, 200, 900\nHbA1c, 4548-4, 5.4, %, 4.0, 5.6";

function wireImport() {
  document.getElementById("fhir-in").value = SAMPLE_FHIR;
  document.getElementById("csv-in").value = SAMPLE_CSV;

  // ① FHIR
  document.getElementById("fhir-import").onclick = () => importFhir(document.getElementById("fhir-in").value);
  document.getElementById("fhir-file").onchange = (e) => {
    const f = e.target.files[0]; if (!f) return;
    const r = new FileReader();
    r.onload = () => { document.getElementById("fhir-in").value = r.result; importFhir(r.result); };
    r.readAsText(f);
  };

  // ② OCR image
  const drop = document.getElementById("ocr-drop");
  const fileIn = document.getElementById("ocr-file");
  drop.ondragover = (e) => { e.preventDefault(); drop.classList.add("drag"); };
  drop.ondragleave = () => drop.classList.remove("drag");
  drop.ondrop = (e) => { e.preventDefault(); drop.classList.remove("drag"); if (e.dataTransfer.files[0]) ocrImage(e.dataTransfer.files[0]); };
  fileIn.onchange = (e) => { if (e.target.files[0]) ocrImage(e.target.files[0]); };

  // ③ CSV
  document.getElementById("csv-import").onclick = () => importCsv(document.getElementById("csv-in").value);
}

function importFhir(text) {
  try {
    const out = JSON.parse(fhir_import_json(text, "Imported FHIR"));
    if (out.records.length) addImportedRecords(out.records, "FHIR");
    if (out.queued) logQueued(`${out.queued} FHIR resource(s)`, "unparseable");
    if (!out.records.length && !out.queued) flashImport("No Observations found in that bundle.");
  } catch (e) { flashImport("Couldn't parse that JSON: " + e.message); }
}

function importCsv(text) {
  const recs = [];
  text.split(/\n+/).forEach((line) => {
    const p = line.split(",").map((s) => s.trim());
    if (p.length < 4 || !p[0]) return;
    const [concept, code, value, unit, lo, hi] = p;
    const v = parseFloat(value);
    if (!isFinite(v)) return;
    recs.push({
      id: `csv-${concept}-${Date.now()}-${recs.length}`, source: "Manual / CSV",
      measured_at: NOW, method: "manual_entry", code: code || null, concept, value: v, unit: unit || "",
      reference_range: { low: lo ? parseFloat(lo) : null, high: hi ? parseFloat(hi) : null },
      confidence: 0.8,
    });
  });
  if (recs.length) addImportedRecords(recs, "CSV"); else flashImport("No valid rows parsed.");
}

// In-browser OCR of a lab image (tesseract.js from CDN), then the helix-ocr safety gate.
async function ocrImage(file) {
  const status = document.getElementById("ocr-status");
  status.innerHTML = "Loading on-device OCR…";
  try {
    const { default: Tesseract } = await import("https://esm.sh/tesseract.js@5");
    status.innerHTML = "Reading the image…";
    const url = URL.createObjectURL(file);
    const { data } = await Tesseract.recognize(url, "eng");
    URL.revokeObjectURL(url);
    const candidates = extractAnalytes(data.text);
    if (!candidates.length) { status.innerHTML = `<span class="warn">No values found in that image.</span>`; return; }
    const doc = { doc_label: file.name || "lab-image", imported_at: NOW, candidates };
    const gated = JSON.parse(ocr_ingest_json(JSON.stringify(doc), 0.5));
    const accepted = gated.filter((g) => g.status === "accepted").map((g) => g.record);
    let queued = 0;
    gated.forEach((g) => { if (g.status === "queued") { queued++; logQueued(g.candidate.label, g.reason); } });
    if (accepted.length) addImportedRecords(accepted, "OCR");
    status.innerHTML = `<span class="ok">✓ ${accepted.length} value(s) imported</span>${queued ? ` · <span class="warn">${queued} held for review</span>` : ""}`;
  } catch (e) {
    status.innerHTML = `<span class="warn">OCR unavailable here (${e.message}). Try the CSV paste instead.</span>`;
  }
}

// Naive analyte extraction: lines like "Ferritin 28 ng/mL  (30-400)".
function extractAnalytes(text) {
  const out = [];
  text.split(/\n+/).forEach((line) => {
    const m = line.match(/^([A-Za-z][A-Za-z0-9 \-]{1,30}?)\s+(-?\d+(?:\.\d+)?)\s*([A-Za-z%\/µ]+(?:\/[A-Za-z]+)?)?/);
    if (!m) return;
    const rr = line.match(/(\d+(?:\.\d+)?)\s*[-–]\s*(\d+(?:\.\d+)?)/);
    out.push({
      label: m[1].trim(), value: parseFloat(m[2]), unit: (m[3] || "").trim(),
      reference_low: rr ? parseFloat(rr[1]) : null, reference_high: rr ? parseFloat(rr[2]) : null,
      ocr_confidence: 0.9,
    });
  });
  return out;
}

function flashImport(msg) {
  const log = document.getElementById("import-log");
  log.innerHTML = `<div class="report-empty">${msg}</div>`;
}

// Run a sample RuView WiFi-CSI reading through the real wasm adapter (ADR-020).
function runSensingDemo() {
  const reading = {
    node_id: "esp-bedroom", room: "bedroom", recorded_at: NOW,
    vitals: { breathing_bpm: 14.2, heart_rate_bpm: 61 },
    states: ["someone-sleeping", "apnea-screening", "possible-distress"],
    witness_signature: "ed25519:demo",
  };
  const out = JSON.parse(sensing_reading_json(JSON.stringify(reading)));
  const recs = out.records
    .map((r) => `<div class="cite">• ${r.concept} ${r.value} ${r.unit} — ${r.source} · conf ${r.confidence}</div>`)
    .join("");
  const flags = out.flags
    .map((f) => `<div class="bd-row"><span>${f.state} <span class="muted">(${f.room})</span></span><b style="color:${f.level === "critical" ? "#fb7185" : "#f6c177"}">${f.level}</b></div>`)
    .join("");
  openModal(`<h2>RuView WiFi-CSI · live</h2>
    <p class="muted">Contactless vitals extracted on-device, mapped through the real Rust adapter (ADR-020).</p>
    <div style="margin:12px 0">${recs}</div>
    <div class="muted" style="font-size:11.5px;letter-spacing:1px;font-weight:700">ESCALATION SCREENING FLAGS</div>
    ${flags}
    <p class="disclaimer" style="margin-top:12px">Screening only, not a diagnosis. Safety flags route to the Escalation Guardian (ADR-009).</p>`);
}

// Run a sample genome profile through the real wasm adapter (ADR-021).
function runGenomeDemo() {
  const profile = {
    source_file: "23andMe-v5", imported_at: NOW, genotype_count: 640000,
    pharmaco: [
      { gene: "CYP2D6", diplotype: "*4/*4", phenotype: "poor" },
      { gene: "CYP2C19", diplotype: "*1/*1", phenotype: "normal" },
    ],
    risks: [{ trait_name: "Type 2 diabetes", score: 0.62, band: "elevated" }],
    ancestry_caveat: "primarily European reference panel",
  };
  const out = JSON.parse(genome_profile_json(JSON.stringify(profile)));
  const recs = out.records
    .map((r) => `<div class="cite">• ${r.concept} — ${r.unit} · conf ${r.confidence}</div>`)
    .join("");
  const adv = out.advisories
    .map((a) => `<p style="margin:6px 0;font-size:13px">⚕ ${a.message}</p>`)
    .join("");
  openModal(`<h2>rvDNA genome · live</h2>
    <p class="muted">User-owned genome analyzed on-device, mapped through the real Rust adapter (ADR-021).</p>
    <div style="margin:12px 0">${recs}</div>
    ${adv}
    <p class="disclaimer" style="margin-top:8px">${out.privacy_note}</p>
    <p class="disclaimer">Decision-support, not a diagnosis — verify with your prescriber.</p>`);
}

// ---- Nav ----
function wireNav() {
  const titles = {
    dashboard: ["Dashboard", "Your body, assembled into one living dossier."],
    report: ["Health report", "Score over time, vitals, focus areas, and your biological age."],
    ask: ["Ask Helix", "Answered only from your data — with citations."],
    records: ["Records", "Every value, sourced and dated."],
    import: ["Import", "Bring in records, lab images, and values — parsed on-device."],
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
  document.querySelectorAll("[data-modal='bioage-info']").forEach((el) => (el.onclick = () => openModal(bioageInfoModal())));
}

function bioageInfoModal() {
  return `<h2>About biological age</h2>
    <p>Computed with the peer-reviewed <b>PhenoAge</b> algorithm (Levine et al., 2018) from <b>9 routine
    blood markers</b> + your age — no special test required.</p>
    <p>It's an <b>estimate of how your labs compare to typical aging — not a measurement and not a
    diagnosis</b>. The headline is the <i>difference</i> from your calendar age; it's population-derived
    (ancestry-dependent). Discuss with a clinician.</p>
    <p class="muted" style="font-size:12.5px;margin-top:10px">Computed deterministically in Rust/WASM (helix-bioage, ADR-034).</p>`;
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
