import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import { createOpenMedAdapter } from "./openmed-adapter.js";

const artifact = new TextEncoder().encode("pinned-model");
const sha256 = createHash("sha256").update(artifact).digest("hex");
const lock = {
  model_id: "openmed/privacy-filter",
  revision: "4ee5b28d0f118a2a521cb781551c1dcd343f8db2",
  files: [{ path: "model_int8.onnx", sha256 }],
};

test("runs every window through OpenMed then delegates release to WASM", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async () => new Response(artifact);
  const calls = [];
  let gatedRequest;
  const runtime = {
    async loadOnnxModel(model, options) {
      assert.equal(model, "/models/openmed/");
      assert.equal(options.localFilesOnly, true);
      assert.equal(options.allowRemoteModels, false);
      return "pipeline";
    },
    async extractPii(text, options) {
      calls.push(text);
      assert.equal(options.pipeline, "pipeline");
      return [
        {
          start: 0,
          end: 1,
          entity_type: "PERSON",
          policy_label: "DIRECT_IDENTIFIER",
          canonical_label: "PERSON",
          score: 0.99,
        },
      ];
    },
  };
  const wasm = {
    openmed_plan_windows_json(text) {
      return JSON.stringify({
        text_utf8_len: new TextEncoder().encode(text).byteLength,
        windows: [
          { start_byte: 0, end_byte: 3, ordinal: 0 },
          { start_byte: 3, end_byte: 6, ordinal: 1 },
        ],
      });
    },
    openmed_gate_json(payload, key) {
      gatedRequest = JSON.parse(payload);
      assert.equal(key.byteLength, 32);
      return JSON.stringify({ outcome: "approved", redacted_text: "safe" });
    },
  };

  try {
    const adapter = createOpenMedAdapter({
      runtime,
      wasm,
      artifactLock: lock,
      modelUrl: "/models/openmed/",
    });
    const result = await adapter.deidentify("abcdef", new Uint8Array(32).fill(7));
    assert.equal(result.outcome, "approved");
    assert.deepEqual(calls, ["abc", "def"]);
    assert.deepEqual(
      gatedRequest.spans.map((span) => [span.start, span.end, span.offset_unit]),
      [
        [0, 1, "utf8_bytes"],
        [3, 4, "utf8_bytes"],
      ],
    );
    assert.equal(gatedRequest.loaded_artifacts[0].sha256, sha256);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("rejects remote model origins before loading", () => {
  assert.throws(
    () =>
      createOpenMedAdapter({
        runtime: {},
        wasm: { openmed_plan_windows_json() {}, openmed_gate_json() {} },
        artifactLock: lock,
        modelUrl: "https://models.example.com/openmed/",
      }),
    /same-origin or loopback/,
  );
});

test("rejects model digest drift before inference", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async () => new Response("different-model");
  let loaded = false;
  const adapter = createOpenMedAdapter({
    runtime: {
      async loadOnnxModel() {
        loaded = true;
      },
      async extractPii() {
        return [];
      },
    },
    wasm: {
      openmed_plan_windows_json() {
        return JSON.stringify({
          text_utf8_len: 4,
          windows: [{ start_byte: 0, end_byte: 4, ordinal: 0 }],
        });
      },
      openmed_gate_json() {
        throw new Error("must not reach gate");
      },
    },
    artifactLock: lock,
    modelUrl: "/models/openmed/",
  });
  try {
    await assert.rejects(
      adapter.deidentify("note", new Uint8Array(32).fill(3)),
      /digest mismatch/,
    );
    assert.equal(loaded, false);
  } finally {
    globalThis.fetch = originalFetch;
  }
});
