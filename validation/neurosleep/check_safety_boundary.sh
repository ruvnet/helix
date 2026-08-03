#!/usr/bin/env bash
# ADR-051 static one-way boundary gate.
#
# Fails CLOSED: a missing search tool, an unreadable path, or a search error is
# a failure, never a pass. An earlier version shelled out to `rg` inside an
# `if` condition, so on a host without ripgrep every check evaluated false and
# the script printed "clean" while testing nothing.
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

# `grep -E` is POSIX and present on every developer and CI host.
if ! command -v grep >/dev/null 2>&1; then
  echo "FATAL: grep is unavailable; cannot evaluate the safety boundary" >&2
  exit 2
fi

# Search wrapper with three-state semantics:
#   0 = match found (a violation), 1 = no match (clean), >=2 = tool error.
# A tool error aborts the whole script rather than reading as "clean".
search() {
  local pattern=$1
  shift
  local status=0
  grep -rEn --include='*.rs' --include='*.js' --include='Cargo.toml' \
    -- "$pattern" "$@" || status=$?
  if ((status >= 2)); then
    echo "FATAL: grep failed (exit $status) while scanning: $*" >&2
    exit 2
  fi
  return $status
}

violations=0

# 1. NeuroSleep identifiers must not reach the generic health consumers.
prohibited_paths=()
for path in crates/helix-score crates/helix-focus crates/helix-escalation \
  crates/helix-llm crates/helix-retrieval ui/app.js mobile; do
  [[ -e $path ]] && prohibited_paths+=("$path")
done
if ((${#prohibited_paths[@]} == 0)); then
  echo "FATAL: none of the prohibited consumer paths exist; refusing to pass vacuously" >&2
  exit 2
fi
if search 'NeuroSleep|neurosleep|RUVN-QEEG' "${prohibited_paths[@]}"; then
  echo "VIOLATION: a NeuroSleep identifier reached a prohibited generic consumer" >&2
  violations=1
fi

# 2. The legacy loose neural adapter must not be silently upgraded.
legacy=crates/helix-neural/src/lib.rs
if [[ ! -f $legacy ]]; then
  echo "FATAL: expected legacy adapter at $legacy" >&2
  exit 2
fi
if search 'RUVN-QEEG|SignedNeuroSleepBundle|NeuroSleepPayload' "$legacy"; then
  echo "VIOLATION: the legacy loose neural adapter was silently upgraded" >&2
  violations=1
fi

# 3. Neither NeuroSleep crate may depend on a generic consumer.
for manifest in crates/helix-neurosleep/Cargo.toml crates/helix-neurosleep-wasm/Cargo.toml; do
  [[ -f $manifest ]] || continue
  if search 'helix-(pipeline|focus|score|escalation|llm|retrieval|neural)(\s|"|=)' "$manifest"; then
    echo "VIOLATION: $manifest depends on a prohibited generic consumer" >&2
    violations=1
  fi
done

# 4. Neither NeuroSleep crate may use a generic health type.
for src in crates/helix-neurosleep/src crates/helix-neurosleep-wasm/src; do
  [[ -d $src ]] || continue
  if search 'ProvRecord|session_to_records|compose_score|select_focus|LlmBackend' "$src"; then
    echo "VIOLATION: $src uses a prohibited generic health type" >&2
    violations=1
  fi
done

# 5. The research WASM artifact must link no verification or key-custody code.
if [[ -d crates/helix-neurosleep-wasm/src ]] &&
  search 'SignedNeuroSleepBundle|verify_bundle|enroll_signer|SealKey|helix_vault' \
    crates/helix-neurosleep-wasm/src; then
  echo "VIOLATION: the research WASM crate reached bundle verification or key custody" >&2
  violations=1
fi

if ((violations != 0)); then
  echo "downstream NeuroSleep static safety boundary: FAILED" >&2
  exit 1
fi

echo "downstream NeuroSleep static safety boundary: clean"
