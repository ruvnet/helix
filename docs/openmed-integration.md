# OpenMed integration

Helix consumes the browser API introduced by OpenMed PR #1550 without giving
the JavaScript model runtime authority over disclosure. The integration has
three parts:

- `helix-openmed` is the Rust policy, artifact, coverage, offset, deterministic
  detector, redaction, and audit-receipt implementation.
- `helix-wasm` exports Unicode-safe window planning and the final policy gate.
- `ui/openmed-adapter.js` runs the pinned OpenMed 1.9 ONNX pipeline locally and
  sends only candidate spans and full coverage evidence into Rust.

## Model layout

Bundle a reviewed OpenMed model under the same origin as the application. Do
not use a Hugging Face branch name or a live remote model reference.

```text
ui/models/openmed/privacy-filter/
├── config.json
├── model_int8.onnx
├── tokenizer.json
└── tokenizer_config.json
```

Create an artifact lock containing the upstream commit or immutable content ID
and the SHA-256 of every file the runtime can load:

```js
const artifactLock = {
  model_id: "OpenMed/privacy-filter-transformersjs",
  revision: "<immutable hexadecimal revision>",
  files: [
    { path: "model_int8.onnx", sha256: "<64 lowercase hex characters>" },
    { path: "tokenizer.json", sha256: "<64 lowercase hex characters>" },
    { path: "config.json", sha256: "<64 lowercase hex characters>" },
    { path: "tokenizer_config.json", sha256: "<64 lowercase hex characters>" },
  ],
};
```

## Browser use

OpenMed 1.9 is currently represented by the linked upstream pull request. Build
its `js/openmedkit-web` package until that version is published, then pass the
module into the adapter. The Helix UI intentionally does not add an unresolved
npm dependency while the upstream release is pending.

```js
import * as openmed from "openmed";
import * as wasm from "./pkg/helix.js";
import { createOpenMedAdapter } from "./openmed-adapter.js";

const adapter = createOpenMedAdapter({
  runtime: openmed,
  wasm,
  artifactLock,
  modelUrl: "/helix/ui/models/openmed/privacy-filter/",
  variant: "int8",
});

// Derive or unwrap this key from the local vault. Never persist it in source,
// localStorage, telemetry, or a serialized gate request.
const hmacKey = crypto.getRandomValues(new Uint8Array(32));
const decision = await adapter.deidentify(localClinicalText, hmacKey);

if (decision.outcome !== "approved") {
  throw new Error(`Clinical-text release blocked: ${decision.code}`);
}
sendOnlyAfterApproval(decision.redacted_text, decision.receipt);
```

`approved` means that this configured software gate completed. It is not a
HIPAA Safe Harbor or Expert Determination opinion. Threshold selection, model
evaluation, residual-risk assessment, and production disclosure policy remain
clinical, privacy, and legal governance responsibilities.
