# Vendored upstream NeuroSleep v1 fixtures

Copied verbatim from `ruv-neural-core` at the exact version this crate pins,
`=0.2.0-alpha.1`:

| File | Upstream path | SHA-256 |
|---|---|---|
| `valid_bundle.json` | `ruv-neural-core/tests/fixtures/neurosleep-v1/valid_bundle.json` | `dc25a31e53f156141d156cd6e7b58869e9fab3b05740739dc8e075df50d4fcfd` |
| `trust_profile.json` | `ruv-neural-core/tests/fixtures/neurosleep-v1/trust_profile.json` | `0d148365ad2664c6ada0f6dfd340c73420059cac64328dfe321758777bd96cb4` |

`trust_profile.json` holds an Ed25519 **verifying** (public) key only. No
private key material is stored in this repository.

## Why these are vendored rather than referenced

These were previously read straight out of a sibling `../../../../ruv-neural`
checkout via `include_bytes!`. That only ever worked on a machine that happened
to have the upstream repository checked out next to this one — CI has no such
checkout, so the crate did not compile there at all. `include_bytes!` also
cannot reach into a registry dependency's sources, so pinning the published
crate does not by itself make the upstream fixtures reachable.

## Why the copies cannot silently drift

A vendored copy normally risks going stale against its source. It does not here,
because these fixtures are not merely parsed — they are **cryptographically
verified** by the same `ruv-neural-core` contract code this crate pins. The
bundle's signature covers every analytic and method field under RFC 8785
canonicalization, and the tests check it against the enrolled key in
`trust_profile.json`.

So if the upstream contract changes in any way that matters — a renamed field, a
changed unit, a different canonicalization, a new required key, a recomputed
compatibility fingerprint — this fixture stops verifying and the test suite
fails. Refreshing the copy is then part of the same change that bumps the pinned
upstream version, which is exactly when a human should be looking at it.

The failure mode this protects against is the dangerous one: an upstream schema
or extractor change that would otherwise let already-stored records be
reinterpreted under different rules without a new fingerprint.
