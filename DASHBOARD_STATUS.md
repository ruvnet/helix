# Helix Medical Dashboard — Loop Ledger

**Loop:** `/loop 5m finish the medical dashboard (ADR-031/032/033/034) until implemented, tested, secure, deployed`
**Repo:** https://github.com/ruvnet/helix · cron 17ec9b8d

## Plan (crate/feature → ADR)
| # | Piece | ADR | Status |
|---|-------|-----|--------|
| 1 | `helix-bioage` — PhenoAge biological age | 034 | ✅ done (prev turn) |
| 2 | `helix-focus` — focus-area selection | 032 | ✅ done |
| 3 | `helix-timeline` — score-over-time | 031 | ✅ done |
| 4 | `helix-wasm` exposure (bioage/focus/timeline) | — | ⬜ next |
| 5 | Dashboard UI view (score-over-time chart, vitals, focus cards, bio-age delta) | 031-034 | ⬜ |
| 6 | Recommendations panel (grounded diff, tiered) | 033 | ⬜ |
| 7 | Rebuild wasm, browser-validate, push, redeploy Pages | — | ⬜ |

## Exit criteria
All dashboard crates implemented + tested; exposed via wasm; a new Dashboard view in the UI shows the
score timeline, vitals, focus areas, bio-age delta, and recommendations — all from the real pipeline;
clippy/fmt/audit clean; pushed to ruvnet/helix main; live UI updated (Pages rebuild).

## Ledger
- **Iter 1 (2026-06-26):** `helix-focus` (ADR-032: deterministic focus-area rules — out-of-range /
  worsening-trend [severity-upgraded] / stale-critical; cites records; non-diagnostic) + `helix-timeline`
  (ADR-031: composes the ADR-016 score per dated snapshot → versioned ScorePoints, deterministic trend +
  CUSUM change-point). 10 tests; clippy/fmt clean. Reuses helix-numeric/score/provenance.

## Next picks
1. Expose bioage/focus/timeline via helix-wasm (JSON in/out).
2. Add a "Dashboard" view to ui/ + mobile: score-over-time SVG sparkline, vitals table, focus cards,
   bio-age delta card, recommendations feed. Wire to the new wasm fns.
3. Rebuild wasm pkg, headless-screenshot validate, push, let Pages redeploy.

- **Iter 2 (2026-06-26):** Wired the dashboard end to end. helix-wasm now exposes bioage_json/focus_json/
  timeline_json. New **"Health report" view** in the console (+ deep-link #report): biological-age card
  (PhenoAge delta, e.g. 42.5 yrs / 7.5 younger, + non-diagnostic modal), health-score-over-time SVG sparkline
  with trend + change-point marker, vitals table (LOW/OK/HIGH vs range), focus-area cards (real ADR-032
  rules), and Tier-1 grounded recommendations. wasm rebuilt; validated in headless Chrome (all panels render
  real pipeline data); screenshot committed + embedded in docs/ui. 168 tests; clippy/fmt clean; audit clean
  (200 deps). Pushed to main → Pages redeploying.

## Status: dashboard COMPLETE — deployment propagating
Implemented ✅ · tested ✅ (168) · secure ✅ (audit clean) · pushed to ruvnet/helix main ✅ · UI updated ✅.
Live Pages CDN was still building at end of iter 2 (external lag); next fire verifies the live #report URL,
then closes the loop.
