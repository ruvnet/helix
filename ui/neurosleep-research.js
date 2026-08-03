import { NEUROSLEEP_RESEARCH_FLAGS } from "./neurosleep-research-config.js";

const MEASUREMENT = "NREM frontal-to-parietal theta coherence";
const CAVEAT = "Related findings come from a preclinical APP/PS1 mouse study and have not been validated as a human clinical marker.";
const METHOD = "rUv Neural NeuroSleep qEEG v1";
const SOURCE = "Verified signed derived nightly bundles";
const PROHIBITED_CLAIMS = [
  /alzheimer/i,
  /microgl/i,
  /amyloid burden/i,
  /diagnos/i,
  /risk score/i,
  /treatment/i,
  /medication/i,
  /therapy/i,
  /recommend/i,
];

export function createNeuroSleepResearchApp({
  root,
  wasm,
  flags = NEUROSLEEP_RESEARCH_FLAGS,
}) {
  requireRoot(root);
  const frozenFlags = validateFlags(flags);
  return Object.freeze({
    flags: frozenFlags,
    analyze(request) {
      if (!frozenFlags.import_v1 || !frozenFlags.shadow_v1 || !frozenFlags.research_ui_v1) {
        const outcome = { outcome: "disabled", code: "research_build_flags_off" };
        renderNeuroSleepPanel(root, outcome, frozenFlags);
        return outcome;
      }
      if (typeof wasm?.visualize_verified_nights_json !== "function") {
        throw new TypeError("dedicated NeuroSleep research WASM artifact is required");
      }
      const payload = { ...request, flags: frozenFlags };
      const outcome = JSON.parse(
        wasm.visualize_verified_nights_json(JSON.stringify(payload)),
      );
      validateOutcome(outcome);
      renderNeuroSleepPanel(root, outcome, frozenFlags);
      return outcome;
    },
  });
}

export function renderNeuroSleepPanel(root, outcome, flags = NEUROSLEEP_RESEARCH_FLAGS) {
  requireRoot(root);
  const doc = root.ownerDocument;
  root.replaceChildren();
  const panel = element(doc, "section", "neurosleep-panel");
  panel.setAttribute("aria-label", "NeuroSleep research observation");
  panel.append(element(doc, "p", "neurosleep-kicker", "Local research only"));

  if (!flags.research_ui_v1 || outcome.outcome === "disabled") {
    panel.append(element(doc, "h2", "neurosleep-title", "NeuroSleep research is disabled"));
    panel.append(
      element(
        doc,
        "p",
        "neurosleep-status",
        "This capability is available only in the separate offline research build.",
      ),
    );
    root.append(panel);
    return;
  }

  if (outcome.outcome === "rejected") {
    panel.append(element(doc, "h2", "neurosleep-title", "Session could not be verified"));
    panel.append(
      element(doc, "p", "neurosleep-status", "Reimport the signed derived bundle."),
    );
    root.append(panel);
    return;
  }

  if (outcome.outcome === "abstained") {
    panel.append(element(doc, "h2", "neurosleep-title", "Not enough compatible nights"));
    panel.append(
      element(
        doc,
        "p",
        "neurosleep-status",
        `${outcome.receipt?.verified_nights ?? 0} verified compatible night(s). Fourteen are required before showing a direction.`,
      ),
    );
    root.append(panel);
    return;
  }

  validateObserved(outcome);
  panel.append(element(doc, "h2", "neurosleep-title", outcome.panel.measurement));
  panel.append(element(doc, "p", "neurosleep-observation", statementFor(outcome)));
  panel.append(element(doc, "p", "neurosleep-caveat", outcome.panel.interpretation_caveat));
  panel.append(
    element(
      doc,
      "p",
      "neurosleep-quality",
      `${outcome.receipt.verified_nights} verified compatible nights · acquisition confidence ${formatConfidence(outcome.receipt.acquisition_confidence_min)}`,
    ),
  );
  panel.append(element(doc, "p", "neurosleep-method", `Method: ${outcome.panel.method}`));
  panel.append(element(doc, "p", "neurosleep-source", `Source: ${outcome.panel.source}`));
  root.append(panel);
}

function validateOutcome(outcome) {
  if (!outcome || !["disabled", "rejected", "abstained", "observed"].includes(outcome.outcome)) {
    throw new TypeError("invalid NeuroSleep research outcome");
  }
  const serialized = JSON.stringify(outcome).toLowerCase();
  for (const forbidden of [
    "study_id",
    "subject_pseudonym",
    "recording_id",
    "nonce",
    "raw_samples",
    "recommendation",
    "stimulation",
    "actuator",
  ]) {
    if (serialized.includes(forbidden)) {
      throw new TypeError("unsafe field in NeuroSleep research view model");
    }
  }
  if (outcome.outcome === "observed") validateObserved(outcome);
}

function validateObserved(outcome) {
  const panel = outcome?.panel;
  const receipt = outcome?.receipt;
  requireExactKeys(outcome, ["outcome", "panel", "receipt"]);
  requireExactKeys(receipt, [
    "acquisition_confidence_min",
    "interpretation_maturity",
    "verified_nights",
  ]);
  requireExactKeys(panel, [
    "compatible_baseline_change",
    "interpretation_caveat",
    "measurement",
    "method",
    "source",
    "unit",
  ]);
  if (
    panel?.measurement !== MEASUREMENT ||
    panel?.unit !== "ratio" ||
    !Number.isFinite(panel?.compatible_baseline_change) ||
    panel.interpretation_caveat !== CAVEAT ||
    panel.method !== METHOD ||
    panel.source !== SOURCE ||
    !Number.isInteger(receipt?.verified_nights) ||
    receipt.verified_nights < 14 ||
    !Number.isFinite(receipt.acquisition_confidence_min) ||
    receipt.acquisition_confidence_min < 0 ||
    receipt.acquisition_confidence_min > 1 ||
    receipt.interpretation_maturity !== "preclinical_mouse_model"
  ) {
    throw new TypeError("invalid NeuroSleep research observation");
  }
  const statement = statementFor(outcome);
  for (const pattern of PROHIBITED_CLAIMS) {
    if (pattern.test(`${panel.measurement} ${statement} ${panel.interpretation_caveat}`)) {
      throw new TypeError("prohibited NeuroSleep claim");
    }
  }
}

function requireExactKeys(value, expected) {
  if (!value || JSON.stringify(Object.keys(value).sort()) !== JSON.stringify([...expected].sort())) {
    throw new TypeError("unexpected NeuroSleep research view-model field");
  }
}

function statementFor(outcome) {
  const change = outcome.panel.compatible_baseline_change;
  const sign = change >= 0 ? "+" : "";
  return `Your ${MEASUREMENT} changed by ${sign}${change.toFixed(3)} relative to your compatible personal baseline across ${outcome.receipt.verified_nights} valid nights.`;
}

function validateFlags(flags) {
  const names = ["import_v1", "shadow_v1", "research_ui_v1", "rvf_v1"];
  const copy = {};
  for (const name of names) copy[name] = flags?.[name] === true;
  return Object.freeze(copy);
}

function formatConfidence(value) {
  return Number.isFinite(value) ? `${Math.round(value * 100)}%` : "unavailable";
}

function requireRoot(root) {
  if (!root?.ownerDocument || typeof root.replaceChildren !== "function") {
    throw new TypeError("a dedicated NeuroSleep panel root is required");
  }
}

function element(doc, tag, className, text) {
  const node = doc.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = String(text);
  return node;
}
