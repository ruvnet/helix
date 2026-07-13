// OpenMed browser adapter. Model execution stays local; the Rust/WASM gate is
// the only component allowed to produce releasable text.

const DEFAULT_WINDOW_SCALARS = 384;
const DEFAULT_OVERLAP_SCALARS = 64;

/**
 * Create a pinned local OpenMed runner.
 *
 * `runtime` is the API introduced by openmed#1550 and must provide
 * `loadOnnxModel` and `extractPii`. The model URL is restricted to same-origin
 * or loopback resources to prevent accidental PHI egress and mutable remote
 * model loading.
 */
export function createOpenMedAdapter({
  runtime,
  wasm,
  artifactLock,
  modelUrl,
  variant = "int8",
  maxScalars = DEFAULT_WINDOW_SCALARS,
  overlapScalars = DEFAULT_OVERLAP_SCALARS,
}) {
  if (!runtime || !wasm?.openmed_plan_windows_json || !wasm?.openmed_gate_json) {
    throw new TypeError("OpenMed runtime and Helix WASM gate are required");
  }
  assertLocalUrl(modelUrl);
  validateLockShape(artifactLock, variant);

  let modelPromise;
  async function load() {
    if (!modelPromise) {
      modelPromise = (async () => {
        const loadedArtifacts = await verifyArtifacts(artifactLock, modelUrl);
        if (!runtime.loadOnnxModel || !runtime.extractPii) {
          throw new TypeError("OpenMed 1.9 loadOnnxModel and extractPii are required");
        }
        const pipeline = await runtime.loadOnnxModel(modelUrl, {
          variant,
          localFilesOnly: true,
          allowRemoteModels: false,
          revision: artifactLock.revision,
        });
        return { pipeline, loadedArtifacts };
      })();
    }
    return modelPromise;
  }

  return Object.freeze({
    async deidentify(text, hmacKey, policy = {}) {
      if (typeof text !== "string") throw new TypeError("text must be a string");
      if (!(hmacKey instanceof Uint8Array) || hmacKey.byteLength < 32) {
        throw new TypeError("hmacKey must be a Uint8Array of at least 32 bytes");
      }
      const coverage = JSON.parse(
        wasm.openmed_plan_windows_json(text, maxScalars, overlapScalars),
      );
      const loaded = await load();
      const spans = [];
      for (const window of coverage.windows) {
        const slice = sliceUtf8Bytes(text, window.start_byte, window.end_byte);
        const raw = await runtime.extractPii(slice, {
          pipeline: loaded.pipeline,
          threshold: policy.minimum_score ?? 0.5,
          hashSecret: hmacKey,
          detector: "openmed-1.9.0",
        });
        spans.push(...normalizeResult(raw, slice, window.start_byte));
      }
      const request = {
        text,
        spans,
        coverage,
        artifact_lock: artifactLock,
        loaded_artifacts: loaded.loadedArtifacts,
        policy,
      };
      return JSON.parse(wasm.openmed_gate_json(JSON.stringify(request), hmacKey));
    },
  });
}

function normalizeResult(result, windowText, windowStartByte) {
  const candidates = Array.isArray(result) ? result : result?.spans ?? result?.entities ?? [];
  return candidates.map((span) => {
    // OpenMed 1.9 uses JavaScript string offsets, which are UTF-16 code units.
    const unit = span.offset_unit ?? "utf16_code_units";
    let start = Number(span.start);
    let end = Number(span.end);
    let offsetUnit = unit;
    if (unit === "utf8_bytes") {
      start += windowStartByte;
      end += windowStartByte;
    } else {
      // Convert window-local units to global UTF-8 bytes here. This avoids
      // ambiguity when a window begins after non-ASCII text.
      start = windowStartByte + localOffsetToUtf8(windowText, start, unit);
      end = windowStartByte + localOffsetToUtf8(windowText, end, unit);
      offsetUnit = "utf8_bytes";
    }
    return {
      start,
      end,
      offset_unit: offsetUnit,
      entity_type: String(span.entity_type ?? span.entity ?? span.label ?? ""),
      canonical_label: span.canonical_label ?? null,
      policy_label: span.policy_label ?? null,
      score: span.score == null ? null : Number(span.score),
      detector: String(span.detector ?? "openmed"),
    };
  });
}

function localOffsetToUtf8(text, offset, unit) {
  if (!Number.isSafeInteger(offset) || offset < 0) throw new TypeError("invalid span offset");
  let prefix;
  if (unit === "utf16_code_units") {
    prefix = text.slice(0, offset);
    if (prefix.length !== offset) throw new RangeError("UTF-16 offset exceeds window");
  } else if (unit === "unicode_scalars") {
    prefix = Array.from(text).slice(0, offset).join("");
    if (Array.from(prefix).length !== offset) throw new RangeError("scalar offset exceeds window");
  } else {
    throw new TypeError(`unsupported OpenMed offset unit: ${unit}`);
  }
  return new TextEncoder().encode(prefix).byteLength;
}

function sliceUtf8Bytes(text, start, end) {
  const bytes = new TextEncoder().encode(text).slice(start, end);
  return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

function assertLocalUrl(value) {
  const url = new URL(value, globalThis.location?.href ?? "http://localhost/");
  const loopback = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  const sameOrigin = globalThis.location && url.origin === globalThis.location.origin;
  if (!loopback && !sameOrigin) {
    throw new TypeError("OpenMed model artifacts must be same-origin or loopback");
  }
}

function validateLockShape(lock, variant) {
  if (!lock?.model_id || !/^[a-f0-9]{12,}$/i.test(lock.revision)) {
    throw new TypeError("OpenMed artifact lock requires an immutable revision");
  }
  const graph = { int8: "model_int8.onnx", fp32: "model.onnx", fp16: "model_fp16.onnx" }[
    variant
  ];
  if (!graph) throw new TypeError("unsupported OpenMed ONNX variant");
  if (!Array.isArray(lock.files) || !lock.files.some((file) => file.path.endsWith(graph))) {
    throw new TypeError(`OpenMed artifact lock must include ${graph}`);
  }
}

async function verifyArtifacts(lock, modelUrl) {
  const base = new URL(modelUrl, globalThis.location?.href ?? "http://localhost/");
  const observed = [];
  for (const file of lock.files) {
    if (
      file.path.startsWith("/") ||
      file.path.split("/").includes("..") ||
      /[\\:?#]/.test(file.path)
    ) {
      throw new TypeError("unsafe OpenMed artifact path");
    }
    const artifactUrl = new URL(file.path, base);
    if (artifactUrl.origin !== base.origin) {
      throw new TypeError("OpenMed artifact escaped the local model origin");
    }
    const response = await fetch(artifactUrl, { cache: "no-store" });
    if (!response.ok) throw new Error(`OpenMed artifact unavailable: ${file.path}`);
    const digest = await crypto.subtle.digest("SHA-256", await response.arrayBuffer());
    const sha256 = Array.from(new Uint8Array(digest), (byte) =>
      byte.toString(16).padStart(2, "0"),
    ).join("");
    if (sha256 !== file.sha256.toLowerCase()) {
      throw new Error(`OpenMed artifact digest mismatch: ${file.path}`);
    }
    observed.push({ path: file.path, sha256 });
  }
  return observed;
}
