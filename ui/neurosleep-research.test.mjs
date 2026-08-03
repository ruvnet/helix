import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { NEUROSLEEP_RESEARCH_FLAGS } from "./neurosleep-research-config.js";
import {
  createNeuroSleepResearchApp,
  renderNeuroSleepPanel,
} from "./neurosleep-research.js";

class FakeElement {
  constructor(ownerDocument, tagName) {
    this.ownerDocument = ownerDocument;
    this.tagName = tagName;
    this.children = [];
    this.attributes = {};
    this.className = "";
    this.textContent = "";
  }
  append(...children) {
    this.children.push(...children);
  }
  replaceChildren(...children) {
    this.children = [...children];
  }
  setAttribute(name, value) {
    this.attributes[name] = String(value);
  }
}

class FakeDocument {
  createElement(tagName) {
    return new FakeElement(this, tagName);
  }
}

function root() {
  const doc = new FakeDocument();
  return new FakeElement(doc, "root");
}

function textTree(node) {
  return [node.textContent, ...node.children.map(textTree)].join(" ");
}

function observed() {
  return {
    outcome: "observed",
    receipt: {
      verified_nights: 14,
      acquisition_confidence_min: 0.91,
      interpretation_maturity: "preclinical_mouse_model",
    },
    panel: {
      measurement: "NREM frontal-to-parietal theta coherence",
      compatible_baseline_change: 0.02,
      unit: "ratio",
      interpretation_caveat:
        "Related findings come from a preclinical APP/PS1 mouse study and have not been validated as a human clinical marker.",
      method: "rUv Neural NeuroSleep qEEG v1",
      source: "Verified signed derived nightly bundles",
    },
  };
}

test("all dedicated research flags default off and prevent WASM reachability", () => {
  assert.deepEqual(NEUROSLEEP_RESEARCH_FLAGS, {
    import_v1: false,
    shadow_v1: false,
    research_ui_v1: false,
    rvf_v1: false,
  });
  let called = false;
  const panelRoot = root();
  const app = createNeuroSleepResearchApp({
    root: panelRoot,
    wasm: {
      visualize_verified_nights_json() {
        called = true;
      },
    },
  });
  const result = app.analyze({ nights: [] });
  assert.equal(result.outcome, "disabled");
  assert.equal(called, false);
  assert.match(textTree(panelRoot), /separate offline research build/i);
});

test("bundle-derived injection copy is rejected before rendering", () => {
  const marker = '<img src=x onerror="globalThis.pwned=true">';
  const panelRoot = root();
  const outcome = observed();
  outcome.panel.statement = marker;
  assert.throws(
    () => renderNeuroSleepPanel(
      panelRoot,
      outcome,
      { import_v1: true, shadow_v1: true, research_ui_v1: true, rvf_v1: false },
    ),
    /unexpected NeuroSleep research view-model field/,
  );
  assert.equal(globalThis.pwned, undefined);
  assert.equal(panelRoot.children.length, 0);
});

test("observed panel uses only approved baseline and preclinical copy", () => {
  const panelRoot = root();
  const outcome = observed();
  outcome.panel.compatible_baseline_change = -0.015;
  renderNeuroSleepPanel(
    panelRoot,
    outcome,
    { import_v1: true, shadow_v1: true, research_ui_v1: true, rvf_v1: false },
  );
  const text = textTree(panelRoot);
  assert.match(text, /compatible personal baseline/);
  assert.match(text, /preclinical APP\/PS1 mouse study/);
  assert.match(text, /Method: rUv Neural NeuroSleep qEEG v1/);
  assert.match(text, /Source: Verified signed derived nightly bundles/);
  for (const forbidden of [
    /alzheimer/i,
    /microgl/i,
    /amyloid burden/i,
    /treatment/i,
    /medication/i,
    /recommend/i,
  ]) {
    assert.doesNotMatch(text, forbidden);
  }
});

test("research module is statically isolated from unsafe DOM and generic rails", async () => {
  const source = await readFile(new URL("./neurosleep-research.js", import.meta.url), "utf8");
  for (const forbidden of [
    "innerHTML",
    "outerHTML",
    "insertAdjacentHTML",
    "addImportedRecords",
    "openModal",
    "app.js",
    "score_json",
    "focus_json",
    "neural_session_to_records_json",
  ]) {
    assert.equal(source.includes(forbidden), false, `research module reached ${forbidden}`);
  }
  assert.match(source, /createElement/);
  assert.match(source, /textContent/);
});

test("adapter rejects identity-bearing WASM output before rendering", () => {
  const app = createNeuroSleepResearchApp({
    root: root(),
    flags: { import_v1: true, shadow_v1: true, research_ui_v1: true, rvf_v1: false },
    wasm: {
      visualize_verified_nights_json() {
        return JSON.stringify({ outcome: "rejected", subject_pseudonym: "leak" });
      },
    },
  });
  assert.throws(() => app.analyze({ nights: [] }), /unsafe field/);
});
