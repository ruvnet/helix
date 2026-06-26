// Helix mobile PWA — same Rust pipeline (helix-wasm) as the desktop console,
// reused via the shared ui/pkg. Mobile-first layout, bottom sheet, install-able.
import init, {
  analyze_json, compose_score_json, version, redflag_registry_version,
} from "../ui/pkg/helix.js";

const DAY = 86_400_000;
const NOW = Date.UTC(2026, 5, 25);

const records = [
  r("f1", "2276-4", "Ferritin", "Quest", 45, 45, "ng/mL", 30, 400),
  r("f2", "2276-4", "Ferritin", "Quest", 30, 33, "ng/mL", 30, 400),
  r("f3", "2276-4", "Ferritin", "Quest", 5, 28, "ng/mL", 30, 400),
];
function r(id, code, concept, source, daysAgo, value, unit, lo, hi) {
  return { id, source, measured_at: NOW - daysAgo * DAY, method: "lab_feed", code,
    concept, value, unit, reference_range: { low: lo, high: hi }, confidence: 1.0 };
}
const SYS = [
  { n: "Cardio", v: 78, c: "#34e0c4" }, { n: "Sleep", v: 84, c: "#38bdf8" },
  { n: "Inflam.", v: 71, c: "#9b8cff" }, { n: "Fitness", v: 66, c: "#f6a623" },
];

async function boot() {
  const badge = document.getElementById("m-engine");
  try { await init(); badge.textContent = `wasm v${version()}`; }
  catch (e) { badge.textContent = "engine failed"; console.error(e); return; }

  const subs = SYS.map((s, i) => ({
    subsystem: ["cardiometabolic", "sleep", "inflammation", "fitness"][i],
    value: s.v, weight: [0.35, 0.25, 0.2, 0.2][i], confidence: 0.85, trend: "stable",
    drivers: [{ concept: s.n, points: s.v, trend: "stable", source_record: "r" }],
  }));
  const score = JSON.parse(compose_score_json(JSON.stringify(subs)));
  document.getElementById("m-score").textContent = Math.round(score.value);
  document.getElementById("m-ring").style.strokeDashoffset = 377 - (377 * score.value) / 100;
  document.getElementById("m-trend").textContent = `health score · ${Math.round(score.confidence * 100)}% conf`;

  document.getElementById("m-systems").innerHTML = SYS.map((s) =>
    `<div class="m-sys"><div class="n">${s.n}</div><div class="v">${s.v}</div>
      <div class="bar"><i style="width:${s.v}%;background:${s.c}"></i></div></div>`).join("");

  document.getElementById("m-brief").onclick = openAnswer;
  document.getElementById("ask-tab").onclick = openAnswer;
  document.getElementById("guide-tab").onclick = openGuide;
  document.querySelectorAll("[data-to]").forEach((b) => (b.onclick = () => show(b.dataset.to)));
  document.getElementById("sheet-host").onclick = (e) => { if (e.target.id === "sheet-host") closeSheet(); };

  // run the real pipeline for the answer screen
  const out = JSON.parse(analyze_json(JSON.stringify({
    concept_code: "2276-4", records, now: NOW, staleness_window_days: 365,
    confidence_floor: 0.5, reference_low: 30, reference_high: 400, flat_band_per_day: 0.01,
  })));
  document.getElementById("m-answer").innerHTML = renderAnswer(out);
}

function renderAnswer(out) {
  if (out.outcome === "abstained")
    return `<div class="m-acard m-abstain"><div class="m-ah">✗ I don't have that yet</div>
      <p style="color:#ebd9bd">${out.message}</p><p class="m-cite">→ ${out.suggested_action}</p></div>`;
  const tr = out.trend, claim = out.claims[0];
  const dir = { rising: "trending up", falling: "trending down", flat: "stable" }[tr.direction];
  const cites = claim.evidence.map((e) =>
    `<div class="m-cite">• ${e.concept} ${e.value} ${e.unit} — ${e.source}</div>`).join("");
  const pct = tr.percent_change != null ? ` (${(tr.percent_change * 100).toFixed(0)}%)` : "";
  return `<div class="m-acard m-grounded"><div class="m-ah">✓ Grounded answer</div>
    <p style="color:#d7eee3">${claim.text}</p>${cites}
    <div class="m-cite">trend: ${dir}${pct} · ${tr.sample_size} readings</div>
    <span class="m-tier">TIER 1 · YOUR DATA</span>
    ${out.recommendation ? `<p style="margin-top:8px;color:#d7e3f2;font-size:13px">${out.recommendation.text}</p>` : ""}
    <div class="m-disc">Not a diagnosis · every claim traces to a dated source.</div></div>`;
}

function show(id) {
  document.querySelectorAll(".screen").forEach((s) => s.classList.toggle("active", s.id === `screen-${id}`));
  document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t.dataset.to === id));
}
function openAnswer() { show("answer"); }
function openGuide() {
  document.getElementById("sheet").innerHTML = `<div class="grab"></div>
    <h2>Get started in 4 steps</h2>
    <ol class="steps">
      <li><h4>Connect your sources</h4><p>Labs, wearables, genome, the Cognitum Seed, and ruv-neural flow into one encrypted on-device vault.</p></li>
      <li><h4>Helix normalizes everything</h4><p>Each value gets a code, units, a reference range, and provenance.</p></li>
      <li><h4>Ask anything</h4><p>Answered only from your data — or an honest "I don't have that."</p></li>
      <li><h4>See and act</h4><p>A score you can open and a body you can tap, with a clear next step.</p></li>
    </ol>`;
  document.getElementById("sheet-host").classList.add("open");
}
function closeSheet() { document.getElementById("sheet-host").classList.remove("open"); }

if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => navigator.serviceWorker.register("./sw.js").catch(() => {}));
}
boot();
