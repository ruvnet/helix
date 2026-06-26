# ADR-012: Connector Abstraction with Graceful Degradation

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-001, ADR-002, ADR-003, ADR-004, ADR-007, ADR-009, ADR-010, ADR-013

---

## Context

### The integration matrix problem

Helix's value proposition requires ingesting data from 8–15 distinct source categories. The current state of these APIs — as of mid-2026 — is characterized by:

- **Uneven availability**: some sources have well-documented OAuth APIs; others require user-initiated PDF export; others have no programmatic access at all.
- **Gating and commercial requirements**: several wearable and lab APIs restrict access behind commercial agreements, developer approval processes, or ToS clauses that prohibit derived-metric use.
- **Frequent change**: APIs in this space are revised, deprecated, or rate-limited without notice. Google Fit APIs are being shut down (end of support 2026); Oura deprecated personal access tokens in December 2025; Dexcom API access has regulatory sensitivity around glucose data.
- **Platform asymmetry**: Apple HealthKit is iOS-only and on-device; Android Health Connect is Android 14+ natively (available via Play Store for Android 13); neither supports direct server-to-server access without user authorization.
- **PDF/OCR as the floor**: for labs, pharmacy history, and specialist records, PDF upload + OCR + LOINC mapping is not the edge case — it is the primary path for most users. Quest Diagnostics and Labcorp both have LOINC mapping tables but no direct consumer API for structured data export. **[A]**

A connector architecture that requires a clean API to function is a connector architecture that fails most of the time. Helix must be able to ship value to a user who has only a PDF lab report and an Apple Health export — before any wearable API is wired. This is the **graceful degradation** requirement.

### The fragility of point-to-point integration

Without a connector abstraction layer, each data source is wired directly to the ingestion pipeline. This creates:

- **Cascading failures**: one connector's API change breaks the ingestion pipeline for all sources sharing code with it.
- **No isolation**: a connector that hangs or returns malformed data can block or corrupt the normalization pipeline.
- **No versioning**: when an API changes (e.g., Oura v2 → v3), there is no mechanism to run the old and new versions concurrently while migrating stored data.
- **No graceful degradation**: if the Whoop API is unavailable, the user should still see their lab data, their sleep data from Oura, and their medication log. A monolithic ingestion pipeline cannot provide this.

### What each source category actually requires

The integration matrix below reflects verified-as-of-2026 access mechanisms. Treat the "Primary mechanism" column as the target; verify each at build time before a given connector ships.

| Source | Primary mechanism | Current-state caveats | [Evidence] |
|---|---|---|---|
| **EMR / clinical records** | FHIR R4 via SMART on FHIR (PKCE); Apple Health Records (HealthKit FHIR API on iOS); SMART Health Links import | Per-provider onboarding; patient-mediated access via MyChart and similar; scope of available FHIR resource types varies significantly across providers | **[A]** |
| **Apple Health / HealthKit** | On-device HealthKit API (iOS only); per-type consent granted by user; background delivery available | iOS-only; no server-to-server; background delivery frequency limits per data type; requires explicit HKHealthStore authorization per type | **[A]** |
| **Android Health Connect** | Health Connect API (built into Android 14; Play Store install for Android 13); per-permission consent; local-to-device storage | OEM Health Connect adoption varies; Google Fit APIs deprecated May 2024, end of support end-2026; no server-to-server without explicit data export | **[A]** |
| **Whoop** | WHOOP Developer Platform OAuth 2.0 API; two-tier rate limiting | Rate limits enforced at client and aggregate level; developer registration required; derived metrics ToS — verify commercial use terms before shipping | **[A — developer.whoop.com]** |
| **Oura** | Oura Cloud API v2 (OAuth 2.0); app-level tokens; personal tokens deprecated Dec 2025 | Personal access tokens no longer available; app-level OAuth required for new integrations; Oura requires developer account approval | **[A]** |
| **Garmin** | Garmin Health API (OAuth 1.0a) | Commercial partner agreement required for production access; developer sandbox available | **[B — verify commercial agreement requirement at build time]** |
| **Dexcom (CGM)** | Dexcom API (OAuth 2.0) | Regulatory sensitivity around glucose data; Dexcom has historically required commercial agreements for derived-data access; CGM data still in development on several aggregator platforms | **[B — verify at build time]** |
| **Apple Watch** | HealthKit (same as Apple Health above; workouts, HR, HRV, SpO₂ via HealthKit) | Same iOS-only, on-device constraints | **[A]** |
| **Eight Sleep** | Eight Sleep Partner API (OAuth) | Requires partner agreement; rate limits apply | **[B — verify at build time]** |
| **Quest Diagnostics** | No direct consumer API; user-authorized FHIR via participating patient portals; primary path: PDF export + OCR | Quest is a LOINC adopter (loinc.org/adopters) but does not offer a public consumer data API; PDF fallback is essential | **[A]** |
| **Labcorp** | No direct consumer API; PDF export + OCR is primary; FHIR via some portal integrations | Labcorp publishes LOINC mapping table; no public consumer data API as of 2026 | **[A]** |
| **Function Health** | PDF export; uses Quest for draws; no direct API | PDF OCR is the only current ingestion path; high LOINC mapping quality due to standardized report format | **[A]** |
| **Pharmacy / meds** | Limited direct consumer APIs; SureScripts network (requires partner agreement); fallback: barcode scan + manual entry + OTC/supplement logging | Design for manual entry as primary; API as enhancement | **[B]** |
| **Genome (VCF/raw)** | User-owned file import (23andMe raw data, VCF, FASTQ) | Never depend on a third-party DTC vault as system of record; 23andMe bankruptcy (2025) is the canonical cautionary tale (ADR-001) | **[A]** |
| **Cognitum Seed** | Local WASM runtime on the Seed device; MCP-over-local-network; on-device extraction only | Raw mmWave data never leaves device; only derived vitals (respiration rate, estimated HR, restlessness index) are transmitted to vault (ADR-014) | **[A]** |

---

## Decision

### Implement a uniform connector abstraction with per-connector versioning, fault isolation, and a four-tier graceful degradation ladder

#### Connector interface (Rust trait)

Every data source connector implements the same `HelixConnector` trait, regardless of whether it uses a live API, a file import, or OCR:

```rust
#[async_trait]
pub trait HelixConnector: Send + Sync {
    /// Unique stable identifier for this connector
    fn connector_id(&self) -> &str;

    /// Semantic version of the connector implementation
    fn connector_version(&self) -> SemVer;

    /// Capabilities this connector can provide
    fn capabilities(&self) -> ConnectorCapabilities;

    /// OAuth or credential setup; returns None if no auth needed (file import)
    async fn authorize(&self, context: &AuthContext) -> Result<AuthToken, ConnectorError>;

    /// Fetch a batch of health facts since the last sync watermark
    async fn fetch_since(
        &self,
        auth: &AuthToken,
        since: Option<DateTime>,
        limit: usize,
    ) -> Result<Vec<RawHealthFact>, ConnectorError>;

    /// Graceful degradation tier for this connector
    fn degradation_tier(&self) -> DegradationTier;

    /// Whether this connector can run on-device (WASM-compatible)
    fn supports_on_device(&self) -> bool;

    /// Health check — is the upstream API currently reachable?
    async fn health_check(&self) -> ConnectorHealth;
}

pub struct ConnectorCapabilities {
    pub data_domains:      Vec<HealthDomain>,  // Labs | Medications | Vitals | ...
    pub fhir_resources:    Vec<FhirResourceType>,
    pub supports_webhook:  bool,
    pub supports_backfill: bool,
    pub min_api_version:   Option<String>,
    pub max_lookback_days: Option<u32>,
}

pub enum DegradationTier {
    Tier1LiveApi,       // Primary: live OAuth API with near-real-time data
    Tier2UserExport,    // Secondary: user-initiated export (CSV, JSON, ZIP)
    Tier3PdfOcr,        // Tertiary: PDF upload + OCR + ontology mapping
    Tier4ManualEntry,   // Floor: user types values; barcode scan for medications
}

pub enum ConnectorError {
    AuthExpired(String),
    RateLimited { retry_after_secs: u32 },
    ApiUnavailable { upstream_status: u16 },
    ParseError(String),
    PermissionDenied(String),
    CommercialAgreementRequired,  // Garmin, Dexcom-commercial path
    Degraded { reason: String, suggested_tier: DegradationTier },
}
```

#### Per-connector versioning and fault isolation

Each connector is:
- **Independently versioned**: a semver bump to the Oura connector does not require a release of the Labcorp connector. The connector version is stamped into every provenance record it produces (ADR-003/004).
- **Independently fault-isolated**: each connector runs as a separate Ruflo Ingestion agent (ADR-002). A connector that throws, hangs, or rate-limits does not block other connectors or the normalization pipeline.
- **Independently retryable**: `ConnectorError::RateLimited` triggers exponential backoff within the agent; `ApiUnavailable` triggers a circuit breaker that prevents repeated hammering of an unavailable API; `AuthExpired` triggers a re-authorization flow.
- **Feature-flagged**: connectors can be enabled/disabled per user account without a code release (e.g., a user who does not have a Whoop device simply has the Whoop connector disabled; its Ingestion agent is never spawned).

```rust
pub struct ConnectorRegistry {
    connectors: HashMap<String, Box<dyn HelixConnector>>,
    feature_flags: HashMap<String, bool>,  // user-specific enables
    circuit_breakers: HashMap<String, CircuitBreakerState>,
}
```

#### The four-tier graceful degradation ladder

For each data domain, Helix attempts tiers in order, stopping at the first tier that succeeds:

```
Tier 1: Live API (OAuth)
    │
    │ If API unavailable / not authorized / commercial-gated:
    ▼
Tier 2: User-initiated export
    (structured file: FHIR export bundle, Apple Health export ZIP,
     Oura CSV export, Google Health export, Garmin FIT files)
    │
    │ If no export available:
    ▼
Tier 3: PDF / image upload + OCR + LOINC mapping
    (lab results PDFs, medication bottles, prescription sheets,
     specialist reports, discharge summaries)
    │
    │ If no digital artifact available:
    ▼
Tier 4: Manual entry
    (barcode scan for OTC medications, manual lab value entry,
     symptom / mood / supplement log)
```

The degradation tier is transparent to the FM Analyst and Verifier — the provenance record carries the tier used (`source_connector_id` encodes both the source and tier, e.g., `labcorp-pdf-ocr-v1` vs `labcorp-fhir-api-v2`). The UI surfaces the tier in citations so the user understands the reliability of each data point.

The degradation ladder means Helix ships value to a user who has *only* a PDF lab report and a HealthKit export on day one — before any live API is connected. As the user connects additional sources over time, the live API tier automatically supersedes the PDF/OCR tier for subsequent data (while the historical PDF data remains in the vault with its lower-tier provenance marker).

#### Per-connector specifications

**EMR / SMART on FHIR connector**
- Primary: FHIR R4 `Patient/$everything` or scoped resource requests via SMART on FHIR OAuth 2.0 with PKCE.
- Supported FHIR resources: `Observation`, `MedicationRequest`, `MedicationStatement`, `Condition`, `AllergyIntolerance`, `Immunization`, `Procedure`, `DiagnosticReport`, `DocumentReference`.
- Apple Health Records: iOS HealthKit FHIR API (`HKClinicalRecord`, supported since iOS 11.3) — on-device, no server call required.
- SMART Health Links: user can paste an SHL URL or scan a QR code to import a signed FHIR bundle; connector validates the SHL signature before ingesting.
- Degradation: Tier 2 → CCDA XML or CCD export (downloadable from most patient portals) → converted to FHIR R4 via open-source CCDA-to-FHIR transformer.
- Note: per-provider onboarding required; scope of available resources varies. Some providers may not expose `Observation` resources for lab results. Validate coverage at build time for target providers. **[B]**

**Apple HealthKit connector**
- Primary: on-device HealthKit API (iOS). Helix requests `HKHealthStore` authorization for each quantity type and category type it needs. Reads are on-device; no network call.
- Background delivery: `HKObserverQuery` + `enableBackgroundDelivery(for:frequency:)` enables Helix to receive updates when new data is available, up to once per hour for most types (per Apple's frequency limits).
- Key types: `HKQuantityTypeIdentifier.heartRate`, `.heartRateVariabilitySDNN`, `.oxygenSaturation`, `.stepCount`, `.bodyMass`, `.bodyFatPercentage`, `.restingHeartRate`, `.activeEnergyBurned`, `.sleepAnalysis` (category), `.mindfulSession` (category).
- Privacy: all data stays on device. Helix reads it locally and writes it to the on-device RuVector vault. No data transmitted unless the user explicitly enables cloud sync.
- Degradation: Tier 2 → Apple Health export (`.zip` containing `export.xml` in CDA/HealthKit export format); Helix parses the XML.

**Android Health Connect connector**
- Primary: Health Connect API (Android 14+ native; Play Store install for Android 13).
- Permission model: `HealthPermission.READ_HEART_RATE`, `READ_STEPS`, `READ_SLEEP`, etc. — per-record-type consent, granted by user in Android Settings.
- Key record types: `HeartRateRecord`, `StepsRecord`, `SleepSessionRecord`, `WeightRecord`, `OxygenSaturationRecord`, `RestingHeartRateRecord`, `HeartRateVariabilityRmssdRecord`.
- Storage: Health Connect stores data on-device, encrypted per-user. No server-to-server access without explicit export.
- OEM note: Health Connect adoption varies across Android OEMs. Helix must handle `HealthConnectException.ERROR_NOT_SUPPORTED` gracefully on devices where Health Connect is not available — fall through to Tier 4 (manual entry for those data types).
- Google Fit: support for reading legacy Google Fit data via Google Fit migration guide is available until end of 2026. After that, Tier 4 is the fallback for historical Fit data not already migrated.

**Whoop connector**
- Primary: WHOOP Developer Platform API v1 (OAuth 2.0, Authorization Code with PKCE).
- Data: `Recovery`, `Cycle`, `Sleep`, `Workout`, `Body Measurements` endpoints.
- Rate limits: two tiers — per-client rate limit and per-user rate limit. Specific limits not published; implement exponential backoff on HTTP 429.
- ToS: verify commercial use terms at build time. Developer terms may restrict use of WHOOP-derived metrics in competing products. **[B — verify]**
- Degradation: Tier 2 → WHOOP data export (CSV, downloadable from WHOOP member portal).

**Oura connector**
- Primary: Oura Cloud API v2 (OAuth 2.0 app tokens — personal tokens deprecated December 2025).
- App-level OAuth requires Oura developer account approval; new integrations must use the OAuth flow.
- Data: `Daily Readiness`, `Daily Sleep`, `Daily Activity`, `Daily Spo2`, `Heart Rate` (time series), `HRV`.
- Rate limits: per-access-token and per-application limits; implement HTTP 429 backoff.
- Degradation: Tier 2 → Oura data export (CSV from oura.com account settings).

**Garmin connector**
- Primary: Garmin Health API (OAuth 1.0a).
- Commercial partner agreement appears required for production access to Garmin Health API. Verify at build time — this may be a significant partnership dependency for Phase 3 roadmap. **[B]**
- Developer sandbox available for non-commercial testing.
- Data: Daily Summary, Activities, Sleep, Heart Rate, Body Composition, Stress.
- Degradation: Tier 2 → Garmin Connect data export (FIT files, GPX, CSV); FIT file parser provides structured activity, sleep, and biometric data.

**Dexcom / CGM connector**
- Primary: Dexcom API (OAuth 2.0). Access to real-time glucose data for third-party apps; historically required commercial agreement for derived-data use. Regulatory sensitivity: glucose data has FDA oversight context.
- Note: CGM connector is in "development" status on several third-party aggregators as of 2026. Verify current Dexcom partner program requirements. **[B]**
- Degradation: Tier 2 → Dexcom Clarity export (CSV); other CGM devices (Libre) may have different export formats. Parse glucose time-series from export.

**Lab connectors (Quest, Labcorp, Function Health)**
- Primary: no direct consumer API for structured lab data as of 2026. **[A]**
  - Quest and Labcorp both publish LOINC mapping tables at loinc.org/adopters, but these are reference tables for their local test codes — not APIs.
  - Quest has B2B ordering APIs used by clinical systems, not accessible to consumer apps.
  - Function Health uses Quest for draws; no separate API.
- Tier 2: some patient portal integrations expose labs via FHIR (e.g., via SMART on FHIR at Quest MyQuest portal). Validate availability.
- **Tier 3 (primary path)**: PDF export + OCR pipeline:
  1. User uploads PDF (or email-forwarded lab report).
  2. OCR engine (on-device WASM or local) extracts text.
  3. Table parser identifies test name, value, unit, reference range, date.
  4. Normalization agent maps test name to LOINC via Quest/Labcorp LOINC mapping tables first, then fuzzy NLP fallback.
  5. Confidence gating routes low-confidence extractions to the review queue (ADR-004).
  6. Validated results enter the health graph as Tier-1 evidence (user's own data) with `source_connector_id = "labcorp-pdf-ocr-v1"`.

**Pharmacy / medication connector**
- Primary: limited direct consumer APIs. SureScripts (e-prescribing network) requires partner agreement. CVS, Walgreens, and other pharmacy chains have medication history APIs for their own apps but not for third parties.
- Design for manual entry as primary path: medication name (barcode scan or text) → RxNorm lookup → dose/frequency entry.
- OTC/supplement entry: barcode scan against UPC database → product lookup → ingredient/dose extraction → map to RxNorm where possible (supplement gaps handled per ADR-004).
- Degradation: Tier 4 is the floor; Tier 2 available if pharmacy export feature exists on the user's pharmacy app.

**Genome import connector**
- Primary: user-owned file import (VCF, 23andMe raw `.txt`, AncestryDNA `.txt`, FASTQ).
- Never depend on a third-party DTC vault as the system of record. The 23andMe Chapter 11 (March 2025) and subsequent sale of a 15M-person genetic database for $305M is the canonical illustration of why user-owned, local-first storage is non-negotiable for genomic data (ADR-001). **[A]**
- Import pipeline: parse VCF → extract rsID + allele + chromosome + position → map to ClinVar for clinical significance → store as `GenomicVariant` nodes in the health graph.
- On-device only: raw genomic data is among the most sensitive health data a person holds. Genomic processing runs locally; no raw variant data is transmitted.

**Cognitum Seed connector**
- Primary: local WASM runtime on the Seed device; communication via MCP-over-local-network (same local subnet or Tailscale VPN for remote access).
- On-device extraction: mmWave radar signal → first-pass vital extraction runs on the Seed (Rust/WASM). Only derived signals (respiration rate, estimated HR, restlessness index) are transmitted to the vault.
- Raw sensor data never leaves the Seed device (ADR-014).
- The Ambient Sensing agent (ADR-002) subscribes to the Seed's MCP stream and ingests normalized vitals into the health vault with appropriate provenance (`source_connector_id = "cognitum-seed-mmwave-v1"`, `evidence_tier = 1`).
- Screening framing: Seed-derived signals are stored with `measurement_method = "contactless_mmwave_radar"` and `confidence_score` computed from signal quality. The Escalation Guardian (ADR-009) interprets these as screening signals, not diagnostic values.

#### Connector registry and lifecycle

```rust
// Session start: spawn only the connectors the user has authorized
let active_connectors = registry
    .enabled_for_user(&user_id)
    .filter(|c| c.health_check().await == ConnectorHealth::Available);

// Background sync loop (Ruflo hooks_worker-dispatch "preload")
for connector in active_connectors {
    let agent_id = swarm.spawn_ingestion_agent(connector, &namespace).await;
    // Each agent is independently fault-isolated
}
```

The sync loop runs on a configurable cadence per connector:
- HealthKit / Health Connect: near-real-time via background delivery callbacks.
- Live wearable APIs: 15–30 minute polling interval (respecting rate limits).
- Lab / manual connectors: user-triggered or daily sweep.
- Cognitum Seed: streaming (continuous low-latency vital ingestion).

A Ruflo `post-edit` hook fires on every batch of normalized facts written to the vault, updating the HNSW index and triggering downstream trend computation for changed metrics.

---

## Alternatives Considered

### Alternative 1: Point-to-point integrations without an abstraction layer

Wire each data source directly to the normalization pipeline — a separate code path per source, with no uniform interface.

**Rejected because:**
- Any API change in one source affects only that source's code path in isolation — this sounds like an advantage, but it means there is no shared error-handling, retry logic, rate-limit management, or circuit-breaker pattern. Each connector must re-implement these from scratch.
- No fault isolation between connectors: a blocking connector stalls the ingestion pipeline for all sources.
- Versioning is implicit and untracked. When Oura deprecated personal tokens in December 2025, a point-to-point integration would have silently broken all Oura data ingestion with no structured migration path.
- Testing is impossible to systematize: each source requires its own mock, test fixture, and integration test suite with no shared framework.
- The graceful degradation ladder cannot be expressed without an abstraction — the concept of "fall back to PDF if the API is unavailable" requires the system to know that the API and the PDF connector are alternatives for the same domain.

### Alternative 2: Third-party health data aggregator (Terra API, Vital, Validic)

Use a commercial health data aggregation service (Terra API, Vital Health, Validic) to abstract the connector layer entirely.

**Rejected because:**
- Sends user health data to a third-party cloud service — directly contradicts ADR-001's user-owned, local-first vault guarantee. PHI on a third-party aggregator is subject to that company's security, privacy policy, and business model.
- The 23andMe cautionary tale (ADR-001) applies: a commercial aggregator's business model depends on the data it holds, and bankruptcy or acquisition can expose it.
- Commercial aggregators charge per-user-per-month pricing that becomes significant at scale and creates a fixed-cost floor that does not align with the Helix local-first model.
- Aggregators may not support all Helix target sources (genomics, Cognitum Seed, custom lab PDF pipelines).
- Loss of provenance control: the aggregator becomes an additional layer between the raw source and the health graph, complicating the per-fact provenance model required by the Verifier agent.

### Alternative 3: Require clean API access — no PDF/OCR fallback

Only ingest data via structured APIs; exclude sources that do not have accessible APIs (labs, pharmacy).

**Rejected because:**
- Labs are the single most valuable data domain for health intelligence — ferritin, HbA1c, lipid panels, thyroid function are not available from wearables or phone sensors. Excluding labs because there is no API excludes the backbone of evidence-based health monitoring.
- The practical reality is that Quest and Labcorp do not have consumer data APIs as of 2026. Waiting for them to build one is not a product strategy.
- PDF/OCR is the primary ingestion path for labs in every existing health intelligence product (including ChatGPT Health's lab import). Helix that cannot ingest labs from a PDF is categorically inferior to the status quo.
- Pharmacy medication history, specialist reports, and discharge summaries are frequently PDF-only. Excluding these eliminates a significant fraction of clinically relevant historical data.

---

## Consequences

### Positive

- **Ship value before every API is wired**: a user with only a PDF lab report and an Apple Health export gets a working health graph on day one. Phase 0 roadmap is achievable without any wearable API partnership.
- **Fault isolation**: any connector failure is contained to that connector's Ingestion agent. Other data domains continue syncing normally.
- **Provenance clarity**: the `source_connector_id` and `connector_version` in every provenance record make it clear to the user (and the Verifier agent) exactly what tier of data quality each fact carries.
- **Partnership-agnostic**: commercial API gating (Garmin partner agreement, Dexcom commercial terms) does not block the product — those sources degrade to Tier 2/3/4 while the rest of the vault functions normally.
- **Versioned migration**: when a connector's upstream API changes, the old and new connector versions can coexist; historical data remains tagged with the version that ingested it.

### Negative

- **Many connectors to maintain**: the connector registry is a long-term maintenance burden. Every API change, deprecated endpoint, or new wearable device requires connector updates.
- **PDF/OCR is messy**: OCR accuracy is sensitive to PDF quality (scanned vs. digitally generated), page layout, font size, and table structure. OCR errors in lab values carry patient-safety risk. The review queue (ADR-004) is an essential safety net.
- **Commercial agreement dependencies**: Garmin, Dexcom, and potentially Whoop may require partnership agreements before live API access is available in production. These are non-technical dependencies that can block Phase 3 wearable integrations.
- **Platform asymmetry**: HealthKit (iOS-only) and Health Connect (Android 14+) mean Helix's mobile data ingestion path differs significantly across platforms. The vault must be platform-agnostic; the connector layer is not.

### Mitigations

| Risk | Mitigation |
|---|---|
| OCR misread of critical lab value | Confidence gating routes all OCR-extracted values through the normalization review queue (ADR-004); user confirms before any P1 value enters the analytic graph |
| Wearable API ToS violation | Legal review of each wearable ToS before shipping that connector; commercial use restrictions (Garmin, Dexcom) handled as partnership tracks in the Phase 3 roadmap |
| ConnectorHealth false positive (API claims up, actually broken) | Health check validated by a synthetic request to a known-good endpoint, not just TCP ping |
| Rate limit exhaustion blocking data freshness | Per-connector adaptive polling: reduce polling frequency when rate limits are detected; prioritize user-query-driven syncs over background polling |
| 23andMe-style DTC vault risk for genomic data | Genomic import is always user-owned file import; never authenticated read from a DTC service vault; no DTC service is a Tier-1 connector for genomics |

---

## Open Questions

1. **Garmin commercial partnership**: does the Phase 3 wearable connector roadmap depend on a Garmin Health API partner agreement? If so, what is the timeline and commercial structure? If a partnership is not achievable, is the Tier 2 FIT file import path sufficient for the user experience?
2. **Dexcom regulatory path**: does ingesting Dexcom glucose data and surfacing it in Helix's trend engine (via the Escalation Guardian) constitute use of a medical device's output in a way that requires SaMD classification under FDA guidance? Get regulatory counsel before shipping the CGM connector (ADR-010).
3. **SMART on FHIR coverage per EHR vendor**: which specific EMR vendors (Epic/MyChart, Cerner, Allscripts, athenahealth) have patient-facing SMART on FHIR scopes enabled, and what resources are available? Build a connector compatibility matrix for each.
4. **Apple Health Records FHIR scope**: the HealthKit FHIR API returns clinical records in FHIR DSTU2 or R4 depending on the provider. Does the EMR FHIR connector and the HealthKit connector share normalization logic, or are they separate connectors for the same domain?
5. **PDF OCR model selection**: should Helix use a local WASM OCR model (preserving ADR-001 privacy) or allow optional cloud OCR for difficult PDFs (with explicit user consent and PII-gating)? Local WASM OCR accuracy on scanned PDFs may be insufficient without a higher-quality model.

---

## References

- Android Health Connect API: https://developer.android.com/health-and-fitness/health-connect — Android 14+ native, Play Store for Android 13 **[A]**
- Google Fit Migration Guide: https://developer.android.com/health-and-fitness/health-connect/migration/fit — Fit APIs deprecated May 2024, end of support end-2026 **[A]**
- WHOOP Developer Platform (OAuth, rate limiting): https://developer.whoop.com/ and https://developer.whoop.com/docs/developing/rate-limiting/ **[A]**
- Oura Cloud API v2 (personal tokens deprecated Dec 2025): https://cloud.ouraring.com/v2/docs **[A]**
- SMART App Launch v2.2.0 (PKCE, patient-mediated access): https://build.fhir.org/ig/HL7/smart-app-launch/app-launch.html **[A]**
- SMART Health Cards and Links IG v1.0.0: https://build.fhir.org/ig/HL7/smart-health-cards-and-links/ **[A]**
- Quest Diagnostics LOINC Adopter: https://loinc.org/adopters/quest-diagnostics/ — LOINC mapping tables available, no consumer API **[A]**
- Labcorp LOINC Adopter: https://loinc.org/adopters/labcorp/ — LOINC mapping tables available, no consumer API **[A]**
- Function Health PDF + OCR pattern: Health3.app analysis of Function Health PDF imports, confirming OCR as primary path **[B]**
- 23andMe Chapter 11 (March 2025) and data sale — canonical cautionary tale for ADR-001 genomic vault rationale **[A — publicly documented]**
- HL7 FHIR R4 Specification, Patient.$everything operation: https://hl7.org/fhir/R4/patient-operations.html **[A]**
- Open Wearables integrations matrix (Garmin, Whoop, Oura, Dexcom status): https://openwearables.io/integrations **[B]**
- Ruflo AIDefence (`aidefence_scan`, `transfer_detect-pii`): inbound connector payload scanning **[A — ruvnet substrate]**
