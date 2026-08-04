#!/usr/bin/env bash
# ADR-051 AT-10 prohibited-claim gate.
#
# Fails CLOSED: a missing search tool or a search error is a failure, never a
# pass. An earlier version shelled out to `rg` inside an `if` condition, so on a
# host without ripgrep the scan silently matched nothing and printed "clean".
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

if ! command -v grep >/dev/null 2>&1; then
  echo "FATAL: grep is unavailable; cannot evaluate prohibited claims" >&2
  exit 2
fi

targets=()
[[ -d crates/helix-neurosleep ]] && targets+=(crates/helix-neurosleep)
# The research WASM crate holds the user-facing measurement label and caveat
# copy, so it must be scanned alongside the native crate.
[[ -d crates/helix-neurosleep-wasm ]] && targets+=(crates/helix-neurosleep-wasm)
# The README is the most public surface this project has. It now describes the
# NeuroSleep rail, so an overclaim there would reach more readers than one
# buried in a crate.
[[ -f README.md ]] && targets+=(README.md)
while IFS= read -r file; do
  targets+=("$file")
done < <(find ui -maxdepth 2 -type f -iname '*neurosleep*' -print 2>/dev/null | sort)

if ((${#targets[@]} == 0)); then
  echo "forbidden-claim surface absent: H1/H3 not implemented (this is not AT-10 completion)"
  exit 0
fi

pattern='microglia (are|is|look)|microglial activation detected|Alzheimer.{0,24}(risk|probability|likely)|amyloid burden (is|looks|detected)|recommend.{0,40}(pexidartinib|metformin|stiripentol|gamma entrainment)'

status=0
grep -rEni -- "$pattern" "${targets[@]}" || status=$?
if ((status >= 2)); then
  echo "FATAL: grep failed (exit $status) while scanning prohibited claims" >&2
  exit 2
fi
if ((status == 0)); then
  echo "VIOLATION: a forbidden diagnostic or therapeutic NeuroSleep claim was found" >&2
  exit 1
fi

echo "NeuroSleep forbidden-claim static scan: clean (${#targets[@]} targets)"
