# ADR-015: Visual Health-Intelligence Layer (3D Anatomical Digital Twin)

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001, ADR-003, ADR-005, ADR-006, ADR-009, ADR-010, ADR-014, ADR-016

---

## Context

### The comprehension gap in health software

Approximately 99.9% of Helix users are not clinicians. The dominant interface paradigm for AI health products in 2026 is a text chat window: the user asks a question and receives a paragraph or a bulleted list. This is the entire surface of ChatGPT Health. Text is necessary; it is not sufficient.

Health comprehension research consistently shows that visual representations of the body dramatically improve lay understanding of health information. BioDigital Human — the most widely deployed 3D anatomy platform — reports a 43% increase in learning retention and a 16% improvement in health assessment accuracy among users who learn with interactive 3D anatomical models compared to traditional resources [B, biodigital.com]. A 2024 study on 3D muscle visualization in fitness apps found visual muscle highlighting improved exercise recall by 40% and reduced training-related injuries by 20% [C, wellally.tech]. The comprehension advantage is not minor; it is category-changing.

The insight Helix bets on: a person who can *see* their body — with organ systems color-coded by status, trend arrows on each region, and a tap-to-explore depth — understands their health state fundamentally differently from a person who reads a paragraph about it. "Your liver enzyme trend is concerning" lands differently when you can tap the liver on a rendered body and see: ALT 68 U/L (high; ref < 56), last measured Quest June 2026, trending up 18% over 3 draws.

### The existing landscape: schematic vs. clinical imagery

The visualization design space for health products separates sharply into two categories with very different risk profiles:

**Category 1 — Schematic/illustrative anatomical models:** Professionally authored 3D models that represent human anatomy at a generalized, illustrative level. These are used in medical education, patient education, and consumer health products. BioDigital Human has over 700 scientifically-validated 3D anatomy models, including complete male and female anatomical models and 600+ health condition visualizations, used by 3 million students and institutions including NYU Medical, Johnson & Johnson, Apple, and Google [A, biodigital.com]. Platforms like Visible Body, Complete Anatomy, and ZygoteBody represent the same category. These are **schematic** — they depict idealized human anatomy, not the user's specific body. They are the anatomical-illustration tradition digitized.

**Category 2 — Fabricated "personal scan" imagery:** AI-generated or algorithmically derived imagery presented as the user's own medical imaging — a "simulated MRI" or "your heart visualized." This category does not exist in a legitimate commercial product but represents the failure mode Helix must explicitly design against. The harm profile: a user who believes they are looking at a representation of their own specific anatomy may draw diagnostic conclusions from what is, in fact, a generic model colored by data. If the data colors the liver amber and the model shows a stylized liver in amber, the user may conclude "my liver looks bad" — conflating a data-driven color indicator with an actual imaging finding. This is the counterfeit-scan failure mode.

The visualization safety principles (established in the product spec, §12) exist precisely to maintain Category 1 and make Category 2 architecturally impossible.

### Render stack options: WebGL/Three.js ecosystem on mobile

The production render stack for mobile anatomical 3D must support: real-time 60 fps rendering of detailed meshes, touch interaction (tap, rotate, pinch-zoom), progressive detail loading for mobile bandwidth, and cross-platform deployment (iOS/Android and web). The mature options in 2026:

- **Three.js / React Three Fiber**: The dominant open-source WebGL library, with React Three Fiber achieving documented 60 fps for complex anatomy models in production [A, wellally.tech]. The Three.js ecosystem has native GLTF/GLB model loading, PBR materials, and built-in raycasting for interaction. Active community; anatomy visualization examples in production [A, three.js forum].
- **Babylon.js**: Microsoft-backed WebGL/WebGPU framework with explicit digital twin and IoT support [A, babylonjs.com]. Particularly strong for complex scene management and enterprise use cases. Heavier than Three.js for mobile-first targets.
- **Native (Metal/ARKit/SceneKit on iOS; Vulkan/OpenGL ES on Android)**: Maximum performance, but requires two codebases. Justified if the 3D twin is a core differentiating surface that warrants native investment.
- **React Native + Three.js (via react-three-fiber + expo-gl)**: Cross-platform without WebView. The path used by growing mobile 3D apps. Somewhat more complex build pipeline.
- **Unity/Unreal export**: Overkill for a UI component; introduces a large runtime dependency.

WebGL-based approaches (Three.js / R3F or Babylon.js) with GLTF model assets are the pragmatic choice for Phase 1–2. They share models across web and mobile, reduce iteration time, and benefit from a large tooling ecosystem. Native rendering becomes the right call in Phase 3+ when the 3D surface is proven central to retention.

### Accessibility requirements

WCAG 2.1 AA is the minimum standard for the Helix visual layer. Health data visualization has specific accessibility concerns that go beyond typical app accessibility:

- **Color blindness**: Approximately 8% of males and 0.5% of females have some form of color vision deficiency. A health score visualization that relies on red/green for good/bad is inaccessible to a large fraction of male users. The standard mitigation is a color-blind-safe palette (e.g., IBM Colorblind-safe, Okabe-Ito, or WCAG-compliant hue choices) paired with icons/labels that do not rely on color alone.
- **Motion sensitivity**: Animated transitions between body states should respect `prefers-reduced-motion`.
- **Text contrast**: All text overlaid on the 3D model or dark background must meet WCAG AA contrast ratio (4.5:1 for normal text, 3:1 for large text).
- **Plain language**: Medical terminology in tap-to-reveal panels must be accompanied by plain-language explanation by default. Depth is available on demand; jargon is never the first presentation.
- **Screen reader compatibility**: The 3D model is not screen-reader-traversable in a meaningful way. Helix must provide an accessible text/list equivalent of all information shown on the twin.

### The ethical risk of fabricated medical imagery

This risk deserves its own named sub-section because the failure mode is subtle and the stakes are high.

In 2026, generative AI can produce photorealistic MRI, CT, and ultrasound images. A product that receives a user's lab data (elevated ALT, mildly enlarged liver on an old ultrasound report) and generates a "visualization" of their liver using a generative model would be producing a fabricated medical image. The user may believe this represents their actual organ. Clinical decisions made based on fabricated imagery — "the image shows X, so I don't need to see a doctor about it" — create direct patient harm.

The visualization safety principles exist to make this failure mode impossible by architecture, not just policy:

1. **Grounded-only**: The model renders only user data that exists in the RuVector vault with a confirmed source (ADR-005). Color-coding, overlays, and numerical annotations derive from actual measurements. Nothing is invented.
2. **Schematic, not counterfeit**: The anatomical geometry is a professional schematic illustration — not a scan, not a generated image, not a photorealistic representation. It is the equivalent of a textbook anatomy diagram, not a medical image.
3. **Non-alarming**: The visual design aims to motivate and inform, not to produce fear responses. Red is not used as the primary indicator color; warm amber for "attention warranted" and cool teal for "good" align with accessible, non-alarming palettes.
4. **Accessible**: Color-blind-safe palette, redundant encoding (icon + color + label), WCAG AA contrast, plain language, text-equivalent mode for screen readers.

These four constraints are encoded as hard requirements — not guidelines — in the Visualization agent (ADR-002) and in the design system. Any feature that would produce imagery beyond the schematic model boundary is out of scope.

---

## Decision

### Decision 1: Render stack — WebGL/React Three Fiber for Phase 1–2, native render option in Phase 3+

**Phase 1–2:** Implement the 3D twin using React Three Fiber (R3F) with Expo GL for cross-platform mobile, sharing GLTF/GLB model assets across iOS, Android, and web. The twin renders at target 60 fps on mid-range hardware (the performance profile documented by R3F anatomy implementations).

Asset pipeline:
- Anatomical mesh library: commission or license a professionally authored, medically reviewed GLTF body model set. Candidate sources: BioDigital Human Studio (commercial API/export license), Visible Body GLTF assets (commercial license), or a bespoke commission from a medical illustrator. **Do not use free models without medical review — anatomical inaccuracy in a health product erodes trust.**
- Organ/system granularity: the base model exposes 12–18 named anatomical systems: cardiovascular, respiratory, hepatic, renal, endocrine, gastrointestinal, musculoskeletal, neurological, reproductive (optional/opt-in), integumentary, immune/lymphatic, metabolic.
- Each system is a named mesh group with a defined data binding contract (which LOINC codes or data categories drive its color state).
- LOD (level of detail) variants for mobile bandwidth: full (web), medium (flagship mobile), low (mid-range mobile).

**Phase 3+:** Evaluate native iOS (SceneKit/RealityKit) and Android (Vulkan/OpenGL ES) rendering if performance benchmarks or feature requirements (e.g., AR overlay) justify the investment.

### Decision 2: Organ/system color-coding driven strictly by user data (ADR-005 binding)

The color state of every organ/system mesh is computed deterministically from the user's own data by the Trend/Numeric agent (ADR-007). The computation is:

```
SystemColorState = f(most_recent_values, reference_ranges, trend_direction, data_recency)
```

Rules:
- A system with no data in the vault renders in a **neutral gray with a "no data" indicator** — never in a color that implies a health status.
- A system with data in range and stable trend renders in **teal** (#34E0C4 / #38BDF8 spectrum).
- A system with data approaching range boundary (within 10%) or a worsening trend renders in **amber** (#F6A623).
- A system with data outside reference range or a sharply worsening trend renders in **warm orange-red** (#E2703A) — never pure red, to avoid alarm-first UX.
- A system with data that meets Escalation Guardian thresholds (ADR-009) overrides the visual layer to route to the guardian rather than displaying a color state.
- Trend direction is indicated by a directional indicator (arrow/chevron) overlaid on the system glyph, computed by the Trend/Numeric agent from the time-series index.

The data-binding contract is explicit and version-controlled. A change to which data drives a system's color requires a data-binding schema revision, not an ad-hoc code change.

Color-blind-safety: the teal/amber/warm-orange palette is tested against deuteranopia, protanopia, and tritanopia simulation. Shape indicators (circle for good, triangle for attention, exclamation for alert) provide redundant encoding that does not depend on hue discrimination. All palette choices target WCAG AA contrast ratios against the dark (#0B1322) background.

### Decision 3: Tap-to-reveal information architecture

Tapping a body region reveals a panel containing:

1. **System label and one-line plain-language status** (e.g., "Your liver: attention warranted — one value slightly above range").
2. **Driving data points** — the 2–5 specific measurements that determined the system's color state, each showing: value, unit, reference range, source, date. Formatted as the product's standard citation card (ADR-005).
3. **Trend sparkline** — 90-day trend for each driving metric, from the time-series index (ADR-007).
4. **Plain-language explanation** — one paragraph explaining what this system does and why the flagged values matter, written at a 10th-grade reading level. If no values are flagged, a brief health-literacy blurb about what healthy looks like for this system.
5. **Evidence tier label** (ADR-006) — for any interpretive statement ("this combination may relate to…"), the evidence tier is shown.
6. **Citation to source** — the "View source" link opens the original record in the user's vault.
7. **Schematic disclaimer** — persistent, small-text note: "This illustration is schematic — it shows your data, not a scan of your body."

This panel is the same anti-hallucination architecture as the conversational interface — every claim grounded, every source cited, abstention where data is absent.

### Decision 4: Schematic explanatory imagery library

Beyond the interactive body model, Helix commissions and maintains a library of schematic explanatory visuals for complex health topics. These are static or lightly animated diagrams — not generative, not photorealistic — authored by medical illustrators and reviewed by the clinical governance board. Examples:

- "What is ApoB and why does it differ from LDL-C?" — a labeled diagram of lipoprotein particle types.
- "What does chronically elevated cortisol do?" — a labeled pathway diagram: HPA axis → adrenal cortex → cortisol → downstream effects (sleep, immune, metabolic, bone).
- "How does metformin work?" — mechanism schematic for a common medication.
- "What is HRV and what drives it?" — nervous system diagram linking autonomic balance to heart rate variability.

These visuals are tagged with the topics/LOINC codes they explain and are surfaced by the Functional-Medicine Analyst and Visualization agent when relevant to the user's data. They are **explanatory** (about how something works) not **diagnostic** (not about the user's specific anatomy). This distinction is maintained by design: explanatory visuals show generic biological processes; the interactive body model shows user data.

Content standards for the library:
- Medical illustrator authorship, not AI-generated imagery.
- Clinical governance review before publication.
- WCAG AA accessible.
- Plain language captions and alt text.
- No fabricated scan-like imagery under any circumstances.

### Decision 5: The four hard visualization-safety constraints (non-negotiable)

These four constraints are encoded as architectural requirements, not design guidelines. Any feature that would require violating one is rejected regardless of user demand or competitive pressure:

**Constraint V-1 — Grounded-only.** No visual element may imply a health status that is not directly derivable from data in the user's RuVector vault with a confirmed source and provenance record (ADR-005). Gray/neutral is the correct render state for any system without data. The Visualization agent must query the data-binding contract before rendering any non-neutral color.

**Constraint V-2 — Schematic, not counterfeit.** The anatomical geometry is a professional schematic illustration. No generative image model is used to produce, augment, or enhance the anatomical visualization. No output from an image-generating model may be shown in the twin surface or the explanatory library. The persistent disclaimer ("This illustration is schematic — it shows your data, not a scan of your body") is displayed in every session and cannot be disabled.

**Constraint V-3 — Non-alarming.** The visual design uses a palette and iconography that informs and motivates without producing fear responses. Pure red is never used as a status color (the Escalation Guardian, not the twin, handles true emergency states). Uncertainty is shown visually (confidence intervals, "low data" indicators) rather than hidden behind a confident-looking status. When data is absent, the body renders in neutral — not in a color suggesting unknown danger.

**Constraint V-4 — Accessible.** The visual layer meets WCAG 2.1 AA for all users. Color is never the sole encoding channel for health status. A text-equivalent mode is available that presents all twin information as a structured list. Font sizes, contrast ratios, and interactive target sizes conform to accessibility standards. `prefers-reduced-motion` is respected. The explanatory library provides alt text for all visuals.

### Decision 6: Daily glanceable briefing

One screen, seen on app open:
- The 0–100 health score (ADR-016) with a 7-day trend indicator.
- A miniaturized body silhouette showing the top 2–3 systems with status changes since the last session.
- One insight from the Functional-Medicine Analyst: the single most actionable finding from the current data state, with a "view source" link. This is pulled from the full analysis pipeline with Verifier/Critic gate (ADR-008) — not from a daily template.
- Ambient sensing summary if the Seed is connected (ADR-014): last night's respiration rate vs. baseline, motion level.

The briefing does not push alarms or red-flag states — those are handled by the Escalation Guardian (ADR-009) via push notification, not embedded in the daily briefing. The briefing is motivational context.

---

## Alternatives Considered

### Alternative A: Text-only interface (no 3D visualization)

The simplest, lowest-engineering path is a chat-and-list interface, which is the ChatGPT Health approach. It is also the Apple Health approach, the most wearable apps' approach, and the norm across health software.

**Why rejected:** The comprehension gap is real and documented. A product that tells non-clinicians their "hepatic biomarker panel shows elevated ALT trending upward" delivers less actionable understanding than one that shows them a body, taps the liver, and says "your liver shows one value slightly above range — your ALT was 68, the upper limit is 56, and it's gone up three times in a row." The 3D twin plus grounded-answer interface is the differentiator the product spec designates as "hardest for a chat-first competitor to replicate." Rejecting it is rejecting the product thesis.

### Alternative B: 2D chart-based dashboard (no 3D anatomy)

A rich 2D dashboard of sparklines, gauge charts, and trend cards — the approach taken by most health analytics products (Levels, Heads Up Health, InsideTracker) — is technically simpler than 3D and still highly informative.

**Why rejected:** 2D chart dashboards require the user to bring their own mental model of which biomarkers connect to which systems, what "elevated ALT" means anatomically, and why the cardiovascular row and the liver row both turned amber this month. The body as navigation metaphor is the key innovation: the body tells the user where to look; the charts tell them the specifics. They are complementary, not alternatives. Helix includes rich trend charts within the tap-to-reveal panel; the 3D model is the entry point, not a replacement for charts.

### Alternative C: AI-generated "personalized" body visualization (photorealistic scan-like imagery)

Generative image models in 2026 can produce plausible-looking MRI, CT, and ultrasound imagery. A feature that generates a "here's what your body might look like based on your data" image could be superficially compelling.

**Why rejected with prejudice:** This is the counterfeit-scan failure mode described in the context section. Fabricated medical imagery shown to lay users risks: (a) false reassurance ("the visualization of my liver looks fine"), (b) false alarm ("the generated image looks scary"), (c) substitution for actual clinical imaging ("I can see from Helix that my liver is fine, I don't need that ultrasound"). None of these outcomes are acceptable in a product positioned as health intelligence. Constraint V-2 exists precisely to make this option permanently off the table. The professional schematic illustration approach — which is what BioDigital Human, Visible Body, and every legitimate medical education platform uses — is the correct model.

---

## Consequences

### Positive

- **Comprehension lift**: Users understand their health status meaningfully better than with text alone. This is the product's core value proposition made tangible — documented comprehension advantage of 43% in analogous platforms.
- **Navigation metaphor**: The body as a table of contents for one's own health makes the product usable for people who have no idea where to start. Tap what is bothering you; Helix shows you what data is available about it.
- **Differentiator durability**: A well-executed, data-grounded 3D body twin is the hardest visual feature for a chat-first product to replicate. It requires both the data graph (RuVector) and the visualization layer working in concert. Neither alone produces it.
- **Engagement without alarming**: The visual framing (non-red palette, motivational trend direction, neutral gray for missing data) encourages engagement without producing anxiety responses from users confronting their health data.
- **Accessible to non-biohackers**: The core design target — a person who wants to dump everything in one place and get a clear picture — gets exactly that: a body they can see, values they can read, plain language they can understand.

### Negative

- **3D engineering investment**: Commissioning medically-reviewed anatomy models, implementing the data-binding contract, achieving 60 fps on mid-range mobile, and maintaining the library through platform OS updates is significant engineering investment.
- **Asset licensing cost**: Licensed anatomical model libraries (BioDigital Studio, Visible Body) carry commercial licensing costs that must be factored into unit economics. Bespoke medical illustration is expensive to commission and maintain.
- **Clinical governance for explanatory library**: Every schematic in the explanatory library requires medical review before publication. This is an ongoing operational cost, not a one-time expense. Incorrect explanatory content (wrong biochemical pathway, wrong anatomical label) in a health product is a reputational and liability risk.
- **Screen reader limitation**: A 3D model is inherently inaccessible to screen reader users. The text-equivalent mode must be kept current with every data-binding and content change — an easily neglected maintenance task.
- **Constraint V-2 enforcement discipline**: As the product grows, feature proposals will arrive that want to "enhance" the body visualization with generated imagery, AR overlays, or "personalized" renderings. Constraint V-2 must be actively enforced by the clinical governance board and engineering leads; it will require saying no to plausible-seeming feature requests.

### Mitigations

- Use a licensed, professionally-maintained anatomical model library for Phase 1–2 to reduce internal medical illustration burden.
- Define the data-binding contract as a versioned schema in the first sprint; this prevents ad-hoc color state decisions from creeping in.
- Budget the clinical governance review cycle for explanatory library additions as a standing quarterly process.
- Implement the text-equivalent mode as a first-class feature, not an afterthought, in the initial release.
- Document Constraint V-2 explicitly in the product contribution guidelines and the design system. It should be a named rule, not just a principle.

---

## Open Questions

1. **Anatomy model licensing vs. bespoke commission**: Compare BioDigital Studio API/export license terms, Visible Body commercial license, and bespoke medical illustration commission cost and maintenance implications. Phase 1 should use a licensed library; Phase 3+ may warrant a custom model library that owns the asset permanently.

2. **System granularity calibration**: The 12–18 named systems listed above — is this the right granularity? Systems with very little commonly collected consumer health data (immune/lymphatic, neurological) may render gray for most users and feel like dead zones. Consider whether low-data systems should be visually de-emphasized on the model or surfaced progressively as data is added.

3. **AR overlay potential**: A future feature could overlay the 3D twin onto the user's camera view (AR). This is architecturally possible with ARKit/ARCore but raises Constraint V-2 questions — does an AR overlay of a schematic body in the real world feel more "scan-like" to users? Requires user research before any AR feature enters the roadmap.

4. **Pediatric visualization**: If Helix supports family health management (a plausible future feature), the anatomical model must have pediatric variants. Gender-variant and intersex representation should also be addressed. These are both clinical governance and engineering questions.

5. **Explanatory library update cadence**: Medical understanding evolves. Some content in the explanatory library will require updates as clinical guidance changes (e.g., LDL vs. ApoB framing). Define a review cadence and a mechanism for clinical advisors to flag outdated content.

---

## References

1. BioDigital, "The BioDigital Human Platform — Interactive 3D Anatomy, Disease and Health Conditions." https://www.biodigital.com/ and https://human.biodigital.com/ [A — platform in production use; statistics from commercial product page]

2. BioDigital / Wolters Kluwer, "BioDigital Digital Human Anatomy." https://www.wolterskluwer.com/en/solutions/ovid/platforms-products/bio-digital [A — confirms 3M+ users, 5,000 institutions, 43% retention lift claim]

3. Wellally.tech, "3D Anatomy Models: React Three Fiber for Muscle Visualization." https://www.wellally.tech/blog/react-three-fiber-3d-anatomy-model-fitness-app [B — R3F performance and comprehension data for anatomy; secondary technical source]

4. Three.js Forum, "A 3D Interactive System for Exploring Human Anatomy by Anatomical Layers." https://discourse.threejs.org/t/a-3d-interactive-system-for-exploring-human-anatomy-by-anatomical-layers/88813 [B — community reference for Three.js anatomy use cases]

5. Babylon.js, "Digital Twins and IoT." https://www.babylonjs.com/digitalTwinIot/ [A — confirms Babylon.js as a production digital twin stack]

6. ResearchGate, "WebGL-based interactive rendering of whole body anatomy for web-oriented visualisation of avatar-centered digital health data." https://www.researchgate.net/publication/261090827 [B — academic precedent for WebGL whole-body health visualization]

7. WCAG 2.1 Working Group, "Web Content Accessibility Guidelines 2.1," W3C. https://www.w3.org/TR/WCAG21/ [A — normative reference for accessibility requirements]

8. Okabe & Ito, "Color Universal Design," 2002 — foundational reference for the color-blind-safe palette approach. Published in JMSA; widely cited in visualization design literature. [A — establishes the 8-color palette standard used throughout]

9. Medevel, "15 WebGL Medical Visualization Projects." https://medevel.com/15-webgl-medical-visualization-projects/ [C — landscape survey; individual projects vary in rigor]

10. SaiBasati / GitHub, "Body-Browser: A 3D model of Human Body." https://github.com/SaiBasati/Body-Browser--A-3D-model-of-Human-Body [C — open-source reference implementation; not production-ready]

11. Helix PHI ADR Product Specification, §12, "The visual health-intelligence layer (3D digital twin)," ISO Vision LLC, 2026. [A — primary product requirement source for this ADR]
