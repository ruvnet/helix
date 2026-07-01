# ADR-050: Design System & Visual Language

**Status**: Proposed
**Date**: 2026-07-01
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-015 (digital twin palette/accessibility origin), ADR-016 (health score), ADR-031 (timeline), ADR-032 (focus areas), ADR-033 (dashboard recommendations), ADR-010 (SaMD copy constraints), ADR-051 (adaptive shell), ADR-052 (proof/reasoning-trace UX), ADR-054 (digital twin)

---

## Context

Five separate ADRs (015, 016, 031, 032, 033) each specify visual rules for their own
surface — ADR-015 defines a teal/amber/warm-orange palette, dark base (`#0B1322`),
color-blind-safety, and WCAG AA for the 3D twin only; ADR-016's score, ADR-031's
timeline, and ADR-032/033's dashboard cards each inherit that intent informally but have
no single source of truth to inherit *from*. Left alone, this drifts: each feature
reinvents hex codes, type scales, and motion rules, and the "world-class UI" push (a
distinct-feeling, professional, dark aesthetic — the "two-strand"/double-helix motif) has
nowhere to live. This ADR promotes ADR-015's palette/accessibility rules to an
app-wide design system and adds the tokens ADR-015 never needed to specify (typography,
spacing, motion, a brand motif) so every other UI-facing ADR consumes one contract.

---

## Decision

**One versioned design-token registry is the single source of visual truth; every
surface (twin, score, timeline, dashboard, proof panel, shell) consumes it — none
define their own colors, type, or motion.**

1. **Color tokens.** Inherit ADR-015's triad unchanged: dark base `#0B1322`, teal
   (good/in-range), amber (attention/approaching boundary), warm orange-red
   (out-of-range) — pure red reserved exclusively for the Escalation Guardian
   (ADR-009), never for routine status, app-wide. Neutral gray is the only valid
   "no data" state anywhere in the app, not just the twin.
2. **Typography scale.** A fixed scale (display / heading / body / caption) at a
   10th-grade plain-language reading level for body text (extending ADR-015 Decision 3
   app-wide), with line-height and size tuned for WCAG AA contrast at every step.
3. **Two-strand (double-helix) brand motif.** A recurring paired-intertwined-strand
   graphic used for structural/navigational chrome (loading states, section dividers,
   the app's visual "spine") — brand grammar only. It never substitutes for or
   overlaps a grounded data visualization (ADR-015's V-1/V-2 constraints apply: the
   motif is decorative chrome, not a data-bearing or anatomical element).
4. **Motion language.** Purposeful, restrained transitions; `prefers-reduced-motion`
   honored everywhere (ADR-015 V-4 generalized app-wide, not twin-only). Motion is
   never used to imply urgency outside Escalation Guardian output — keeps ADR-015's
   Constraint V-3 (non-alarming) intact as a whole-app rule.
5. **Accessibility as a release gate, app-wide.** WCAG 2.1 AA, color-blind-safe
   palette (Okabe-Ito derived, per ADR-015 reference 8), redundant icon+color+label
   encoding, and reduced-motion support are release-blocking for every screen, not
   just the twin.
6. **One shared component library.** Buttons, cards, the evidence-tier chip (ADR-006),
   the citation chip (ADR-005), and gap-notice styling (ADR-006) are implemented once
   and consumed by both the ADR-051 shell (desktop + mobile) and any future surface —
   no per-platform restyle.
7. **Token governance.** Tokens live in one versioned file (design-tokens
   JSON/CSS custom properties). Any new status color or type-scale step requires a
   registry change reviewed against this ADR, not an ad hoc hex code in feature code.

---

## Alternatives Considered

- **Leave each ADR (015/016/031/032/033) to its own styling (status quo).** Rejected:
  already producing drift (ADR-015's palette is not consistently referenced elsewhere);
  undermines the "one trust gesture" goal ADR-052 depends on.
- **Adopt an off-the-shelf design system unmodified (e.g., Material Design).**
  Rejected as the *visual language*: generic systems have no concept of an
  evidence-tier chip or a non-alarming health palette. Acceptable only as an
  engineering primitive layer underneath Helix's own tokens, not as the source of
  Helix's look.
- **Design-only (Figma), no enforced token contract in code.** Rejected: unenforceable;
  design and implementation drift within a release cycle without a code-level registry.

---

## Consequences

### Positive
- Consistent, accessible, trustworthy visual language across every ADR-owned surface.
- Single accessibility audit surface instead of five per-feature audits.
- Less engineering rework — one component library, many callers.

### Negative
- Upfront design-system investment before feature work can consume it.
- Retrofitting ADR-015/016/031/032/033 surfaces that already have informal styling.
- Ongoing token-registry governance overhead as new statuses/components are proposed.

### Mitigations
| Risk | Mitigation |
|---|---|
| Feature drift back to ad hoc styling | Token registry change required for any new color/type value; reviewed against this ADR |
| Retrofit cost | Migrate call sites incrementally; new surfaces (ADR-051/052/054) consume tokens from day one |
| Motif overreach into data territory | V-1/V-2 constraints (ADR-015) explicitly bound the two-strand motif to chrome, not data |

---

## Open Questions

1. Token tooling: raw CSS custom properties vs. a build-time token pipeline (e.g.
   Style Dictionary) for cross-platform (web/Tauri/PWA) consistency.
2. Does the two-strand motif need medical-illustration review like ADR-015's
   explanatory library? Current view: no — it is abstract/brand, not anatomical.
3. Dark-mode-only for MVP, or does a light-mode variant need its own token pass?

---

## References

- ADR-015 §Decision 2, 5 — palette, accessibility rules generalized here **[A]**
- W3C, "Web Content Accessibility Guidelines 2.1" **[A]**
- Okabe & Ito, "Color Universal Design" (2002) — color-blind-safe palette basis **[A]**

---

> Architectural/product guidance, not legal or medical advice. Visual design choices
> do not alter the clinical-safety or evidence-tiering rules of ADR-006/009/010.
