# ADR-001: User-Owned, Local-First Encrypted Health Vault

**Status**: Proposed
**Date**: 2026-06-25
**Project**: Helix — Personal Health Intelligence (PHI)
**Prepared by**: ISO Vision LLC
**Substrate**: Ruflo + RuVector + Cognitum Seed + MetaHarness/Darwin
**Related**: ADR-003 (RuVector memory), ADR-011 (Federation), ADR-012 (Connector abstraction), ADR-013 (On-device inference), ADR-017 (MetaHarness minting)

---

## Context

### The problem: centralized health vaults are breach targets and bankruptcy assets

Health and genomic data is, by any reasonable measure, the most sensitive information
a person can generate. Unlike a leaked password, a leaked genome cannot be rotated. Unlike
a stolen credit card, a disclosed diagnosis cannot be retracted. Once exfiltrated or
transferred, the data is permanently accessible to whoever holds it.

The dominant model for personal health data platforms has been the centralized vault:
the company holds the data, the user holds a login. This architecture concentrates risk in
predictable ways:

1. **Security breach target.** A single server holding millions of health records is among the
   most attractive targets for criminal actors and state-sponsored attackers. A successful
   breach exposes every user simultaneously.

2. **Business failure.** A vendor that holds sensitive data as its primary asset can be acquired,
   merged, sold to a competitor, or — as the 23andMe case demonstrated at scale — forced into
   bankruptcy where that data becomes a saleable asset in a court-supervised auction.

3. **Policy reversal.** Privacy policies are contracts of adhesion that any company can revise
   unilaterally, subject only to providing notice. A company acquired by a data broker or an
   advertising firm has strong economic incentives to weaken protections on the corpus it
   acquired.

### The 23andMe event: a materialized risk at scale

**[A]** On March 23, 2025, 23andMe Inc. filed for Chapter 11 bankruptcy protection. The
company had accumulated a genetic database of approximately 15 million customers over a
decade of direct-to-consumer (DTC) DNA testing. The filing followed years of declining
revenue and the rejection by the company's Special Committee of CEO Anne Wojcicki's final
non-binding acquisition proposal in March 2025.

**[A]** The bankruptcy court approved a sale of substantially all of 23andMe's assets —
including its genetic database — to TTAM Research Institute, a nonprofit entity founded by
Wojcicki, the co-founder and former CEO of 23andMe. The sale price was $305 million; TTAM
prevailed in bidding over Regeneron Pharmaceuticals. A notice of closing was filed in
bankruptcy court on July 14, 2025.

**[A]** Attorneys general from more than two dozen states — including New York, California,
Arizona, Colorado, Connecticut, Florida, Illinois, Michigan, Minnesota, Oregon, Virginia,
Washington, and others — either warned users to delete their genetic data or joined a
multistate lawsuit seeking to block the transfer of that data without renewed, opt-in
consent. The New York AG's office explicitly filed suit against 23andMe to protect New
Yorkers' genetic data.

**[A]** A critical legal gap drove the AGs' alarm: Section 363(b)(1)(B) of the Bankruptcy
Code offers limited protections for "personally identifiable information" (PII) in
insolvency proceedings, but the statute does not expressly include genetic data. This means
the court's privacy trustee framework — designed for conventional PII — provided imperfect
protection for the qualitatively different exposure of an immutable biological identifier.

The outcome — data of 15 million people transferred to a new entity in a court-supervised
auction — is not a failure of 23andMe's security controls. It is the foreseeable consequence
of the *architectural* choice to hold user data as a company asset. Even if TTAM keeps the
data private as promised, the structural principle is unchanged: once your biology lives in
someone else's vault, it is their asset to protect or dispose of, not yours.

### HIPAA does not protect direct-to-consumer health data

**[A]** The Health Insurance Portability and Accountability Act of 1996 (HIPAA) and its
implementing regulations at 45 CFR Parts 160 and 164 apply only to "covered entities" — a
defined class comprising health plans, health care clearinghouses, and health care providers
who transmit health information electronically in connection with standard transactions
(45 CFR §160.103). A company's obligations as a "business associate" arise only when it
receives protected health information from a covered entity.

**[A]** A direct-to-consumer health or genomic data application — including 23andMe, Apple
Health (for non-clinical data flows), and a product like Helix — is not a covered entity
under HIPAA unless it acts on behalf of one. The data it holds is therefore not "protected
health information" (PHI) within HIPAA's meaning, regardless of how sensitive it is. When
23andMe filed for bankruptcy, HIPAA offered no protection to its users' genetic data,
because 23andMe was never subject to HIPAA in the first place.

**[C]** "Voluntarily HIPAA-grade" is a common positioning for DTC health products. It
means applying HIPAA's technical safeguard standards (§164.312) and administrative
safeguard requirements (§164.308) by design choice, not legal obligation. This is
meaningfully better than nothing — it sets a baseline of encryption-at-rest,
encryption-in-transit, audit logging, and access controls — but it does not create HIPAA
liability if controls fail, and it cannot override bankruptcy law.

### GINA does not govern how consumer apps handle genetic data

**[A]** The Genetic Information Nondiscrimination Act of 2008 (GINA) prohibits health
insurers from requesting or using genetic information in coverage and rate decisions, and
prohibits employers from using it in hiring, firing, and promotion decisions. These are
important anti-discrimination protections.

**[A]** GINA does not, however, govern how a DTC company collects, stores, or shares
genetic data. It does not apply to life insurance, long-term care insurance, or disability
insurance. It does not prevent a genomic data company from selling data to pharmaceutical
companies or other third parties. It offers no protection in bankruptcy.

**[B]** Several legal analyses have described GINA and HIPAA together as creating a "gap"
that leaves DTC genetic data substantially unprotected at the federal level. The gap is
widely documented; see the 23andMe event for the real-world consequence.

### State-level genetic privacy: a patchwork, accelerating

**[B]** In the absence of comprehensive federal genetic privacy law, states have enacted an
accelerating patchwork of laws with uneven coverage:

- **Illinois**: The Genetic Information Privacy Act (GIPA), one of the earliest and most
  robust state genetic privacy laws, covering collection, storage, use, and disclosure of
  genetic information. Amendments pending (SB 2886) to extend biomarker testing protections.
- **Montana**: SB 163 (signed May 2025, effective October 1, 2025) amended Montana's GIPA
  with deidentification exemptions and research carve-outs, and added neural-data protection.
- **Texas**: HB 130 (Texas Genomic Act of 2025, signed May 2025) specifically targets
  foreign-adversary access to genomic sequencing data, with statutory damages up to $5,000
  per violation.
- **Utah / South Dakota**: Enacted genetic privacy laws in 2026 as new entrants.
- **California, Colorado, Virginia, Connecticut, Washington, and others**: Comprehensive
  consumer privacy laws (CCPA/CPRA, CPA, VCDPA, etc.) include sensitive data categories
  covering health and genetic data, with heightened consent requirements.

**[B]** This patchwork creates a compliance burden for any product operating nationally —
and an incentive to design to the strictest reasonable standard rather than track each
jurisdiction individually. The correct engineering response is not a compliance-by-geography
policy layer; it is an architecture that makes the compliance question close to moot because
the data is never held by the company in the first place.

### GDPR and international context

**[A]** Under GDPR Article 9, health data and genetic data are "special categories" of
personal data, the processing of which is generally prohibited absent explicit consent or
other specified lawful basis (Art. 9(2)). Member States retain the power to impose
additional conditions or limitations on genetic data processing (Art. 9(4)), and several
have done so.

**[C]** If Helix operates in EEA, UK, or jurisdictions with analogous frameworks, the
combination of a local-first vault with user-controlled data flow is strongly aligned with
GDPR's data-minimization and storage-limitation principles (Arts. 5(1)(c) and (e)) — the
company genuinely cannot process data it does not hold.

> **Note**: This ADR provides architectural guidance, not legal advice. Engage privacy
> counsel before launch and before expanding to non-US jurisdictions.

---

## Decision

Helix stores the canonical copy of every user's health and genomic data in an **encrypted,
user-owned vault**, resident on the user's device(s), with optional user-controlled encrypted
backup/sync. Helix the company is architecturally structured to never hold the raw corpus in
plaintext and is never positioned to monetize or transfer that corpus.

### Encryption scheme

**At-rest encryption** (vault files and RuVector graph shards):

- Primary cipher: **XChaCha20-Poly1305** (256-bit key, 192-bit nonce — misuse-resistant
  against nonce reuse on mobile, no FIPS requirement for DTC consumer context).
- FIPS-required fallback (e.g., federal healthcare partner context): **AES-256-GCM** with
  deterministic nonce construction from a counter + file-ID to prevent reuse.
- All vault writes are authenticated encryption (AEAD); there is no separate MAC step.

**Key derivation hierarchy** (KEK/DEK pattern):

```
Passphrase + device-specific entropy
         │
         ▼
    Argon2id (m=65536 KiB, t=3, p=4)
         │
         ▼
    User Master Key (256-bit, per-device)
         │
    HKDF-SHA-256 (info="helix-kek-v1")
         │
         ▼
    Key Encryption Key (KEK, 256-bit, lives in device Secure Enclave / Android Keystore)
         │
    wraps per-vault Data Encryption Key (DEK, 256-bit, random on vault creation)
```

- The DEK encrypts vault contents; the KEK wraps the DEK.
- The KEK never leaves the Secure Enclave (iOS) or Android StrongBox Keystore.
- Helix servers never see the passphrase, the Master Key, the KEK, or the DEK.

**In-transit encryption**:
- All backup / sync blobs are encrypted client-side before transmission; servers see only
  ciphertext. Transport layer: TLS 1.3 minimum.
- Helix backend (if any exists) is a relay/CDN for opaque encrypted blobs, not a data
  processor in the GDPR sense.

### Key custody model

1. **Primary key**: device Secure Enclave / Android StrongBox-backed, biometric / PIN
   unlocked by the user. Not exportable; not visible to the app process.
2. **Passphrase-derived recovery key**: Argon2id-derived from a user-chosen passphrase;
   can unlock the vault independently of the device. Required for cross-device access and
   device-loss recovery.
3. **Recovery shares** (optional, recommended): Shamir Secret Sharing (e.g., 3-of-5 shares)
   lets the user distribute recovery capability across: a printed paper backup, a trusted
   person / guardian, a hardware key, and a second device. No single share is sufficient to
   recover the vault.

Helix does NOT implement server-side key escrow. There is no "forgot my password" flow that
recovers keys from Helix servers. This is a deliberate, non-negotiable design constraint,
with the consequence that key-loss equals data-loss. The UX must communicate this clearly
at setup.

### Sync and backup model

- **Local-first**: the vault is always available offline on the user's primary device.
- **Encrypted sync**: optional cross-device sync via user-controlled storage backends:
  - iCloud Keychain-adjacent (iOS device-to-device, encrypted before leaving device)
  - Android Keystore-adjacent (encrypted blob to Google Drive or user-specified storage)
  - User-provided S3-compatible storage (object key = hash(vault_id + DEK), value = ciphertext)
  - IPFS/local NAS with user-provided credentials
- **No Helix-owned cloud storage by default.** If Helix provides a relay service, it is
  zero-knowledge: the relay stores and forwards encrypted blobs but holds no keys.

### Structural constraint on the company

Helix's entity structure, terms of service, and technical architecture jointly implement a
"can't, not just won't" model:

- No decryption keys held server-side means no ability to read or sell the corpus even if
  legally compelled.
- The user is the sole data controller for the vault under GDPR terminology. Helix acts as
  a data processor only to the extent of transmitting encrypted blobs.
- Genomic data (raw VCF / genotype files) follows the same vault architecture. It is never
  uploaded to third-party DTC genomics platforms from within Helix.

### Connection to related ADRs

- ADR-003 (RuVector): RuVector's graph and vector indices are shards within the encrypted
  vault, encrypted at the same layer.
- ADR-011 (Federation): cohort signals leave the vault only as differentially-private,
  PII-stripped aggregates with explicit opt-in consent per session.
- ADR-013 (On-device inference): inference happens on-device against the decrypted vault
  loaded into RAM; no health data leaves the device for model inference unless the user
  explicitly consents to cloud escalation (see ADR-013).
- ADR-017 (MetaHarness): the MetaHarness minting uses Ed25519 witness-signed releases to
  ensure software integrity; the vault's trust model depends on verifiable app integrity.

---

## Alternatives Considered

### Alternative A: Cloud-first encrypted vault (server-side keys, server-side search)

User data is stored encrypted on Helix servers with keys managed by Helix, allowing
server-side full-text search, ML, and cross-device access without user friction.

**Rejected** for three reasons: (1) Helix would hold decryption keys, making it a
custodian of the raw corpus and subject to the same bankruptcy risk that exposed 23andMe
users; (2) a key compromise at the server layer is a compromise of all users simultaneously;
(3) the privacy proposition — "your data is yours" — is weakened to "trust us," which is
exactly what 23andMe customers were told.

### Alternative B: Hybrid — plaintext cloud, encrypted local cache

User data is stored in plaintext in the cloud for full functionality, with an encrypted
local cache for offline use.

**Rejected** because it recreates the 23andMe failure mode. The cloud copy is a
breach/bankruptcy target. The "local cache" framing implies the cloud is canonical, which
means the user's data is the company's data.

### Alternative C: Federated identity with user-held cloud keys (zero-knowledge cloud)

User data stored encrypted in the cloud; user holds keys server-side in a hardware security
module (HSM) under their own identity (e.g., passkey-based). Helix's cloud infrastructure
can run certain computations under homomorphic encryption or secure multi-party computation.

**Not adopted for v1** because homomorphic encryption over health knowledge graphs at
interactive latency is not yet computationally practical. Preserved as a possible long-term
evolution path. Does not eliminate the bankruptcy-as-saleable-asset risk unless the vault
ownership is legally decoupled from Helix's insolvency estate.

### Alternative D: Blockchain / NFT-based data sovereignty

User health data notarized to a public chain, access controlled by smart contracts.

**Rejected** as performatively decentralized but practically ineffective for the actual use
case: (a) raw health data cannot be published to a public chain; (b) smart-contract access
control does not prevent a party that already holds a copy from retaining it; (c) adds
latency and complexity without solving the custodial risk.

---

## Consequences

### Positive

- **Maximum privacy posture.** The company genuinely cannot read, sell, or transfer user
  data. This is architecturally verifiable, not a promise.
- **Bankruptcy-resilient.** The vault is not an asset of the company because the company
  holds no plaintext and no keys. A court-supervised data sale cannot include data the
  company cannot read.
- **Regulatory simplification.** A company that cannot access user health data faces a
  much simpler regulatory profile (HIPAA data controller obligations, GDPR processor
  obligations) than one that holds and processes the corpus.
- **Differentiation.** In a landscape where ChatGPT Health explicitly is not HIPAA-covered
  and stores data cloud-resident, the "user holds keys, company cannot sell" property is
  a genuine competitive differentiator for the privacy-conscious segment.
- **23andMe lesson applied.** Helix's architecture is specifically designed to make the
  failure mode that harmed 23andMe's 15 million users architecturally impossible.

### Negative

- **No "forgot my password" recovery.** Key loss = data loss. This creates a UX and
  support burden: clear, repeated communication at setup; recovery-share prompts; no
  customer support path to data restoration if all keys are lost.
- **Cross-device sync complexity.** Syncing encrypted vault shards without server-side
  decryption requires a robust, user-friendly key-transfer mechanism. Poorly implemented,
  this is the #1 user-facing friction point.
- **Server-side ML and federation are constrained.** Certain product features (cross-user
  population baselines, model fine-tuning on health data) require creative architectural
  solutions (on-device inference, differentially-private federation) rather than direct
  cloud ML. This is the correct constraint; it is also a real engineering cost.
- **Lost-key onboarding failure recovery.** A user who loses all keys and all recovery
  shares has no path to vault recovery. Acceptable for the privacy model; requires
  deliberate UX to minimize incidence.

### Mitigations

- Mandatory setup flow: primary device key + passphrase recovery key + at least one
  additional recovery share before the vault is considered "safe."
- In-app reminders at 30/90/180 days to verify recovery share integrity.
- Export: users can export their vault in a portable format (e.g., FHIR-formatted JSON,
  encrypted) at any time.
- Sync UX: guided cross-device pairing that validates the DEK transfer before any data
  moves.

---

## Open Questions

1. **Legal entity structure**: Does the "can't access the data" property need to be reflected
   in Helix's corporate structure (e.g., explicit charter clause, trust structure) to be
   credible to sophisticated users, privacy advocates, and regulators? Engage corporate
   counsel.

2. **Bankruptcy code gap**: The 23andMe case revealed that generic PII protections in
   §363(b)(1)(B) of the Bankruptcy Code do not expressly cover genetic data. Should Helix
   advocate for legislative fix, and does the technical architecture need to be supplemented
   with a legal instrument (e.g., a data-not-transferred-in-bankruptcy clause in terms)?

3. **Secure Enclave availability**: On Android devices below certain API levels (< API 28
   for StrongBox), the cryptographic guarantees are weaker. Define minimum Android API
   floor for full vault security, with graceful degradation policy for older devices.

4. **Forensic law-enforcement access**: If Helix has no server-side keys, it cannot comply
   with a server-side court order for user data. What is the legal advice on how to handle
   law-enforcement compelled-access requests? Engage counsel before launch.

5. **EU data residency**: Even with encrypted blobs, does the relay infrastructure need EU
   data residency for GDPR purposes? Architectural answer is probably yes for relay servers
   if EU users are targeted.

---

## References

| # | Source | Evidence | URL |
|---|--------|----------|-----|
| 1 | 23andMe Chapter 11 press release, March 2025 | [A] | https://mediacenter.23andme.com/press-releases/23andme-initiates-voluntary-chapter-11-process-maximize/ |
| 2 | HIPAA Journal: Bankruptcy Court approves 23andMe sale, TTAM/Wojcicki $305M | [A] | https://www.hipaajournal.com/genetic-testing-company-23andme-files-for-bankruptcy/ |
| 3 | Fortune: TTAM Research Institute wins bidding, $305M, Jul 2025 | [A] | https://fortune.com/2025/07/01/23andme-sale-ceo-anne-wojcicki-dna-test-health/ |
| 4 | Stateline: State AGs warn users to delete genetic data, May 2025 | [A] | https://stateline.org/2025/05/02/23andme-users-genetic-data-is-at-risk-state-ags-warn/ |
| 5 | NY AG James: Lawsuit to protect NY genetic data | [A] | https://ag.ny.gov/press-release/2025/attorney-general-james-sues-23andme-protect-new-yorkers-genetic-data |
| 6 | Public Citizen: Bankruptcy Code gap for genetic data | [B] | https://www.citizen.org/article/house-must-update-bankruptcy-code-in-wake-of-23andme-dna-data-sale/ |
| 7 | HHS.gov: HIPAA Covered Entities and Business Associates | [A] | https://www.hhs.gov/hipaa/for-professionals/covered-entities/index.html |
| 8 | eCFR 45 CFR §160.103: Definitions (Covered Entity) | [A] | https://www.ecfr.gov/current/title-45/subtitle-A/subchapter-C/part-160/subpart-A/section-160.103 |
| 9 | AccountableHQ: HIPAA covered entity definition plain-English guide | [B] | https://www.accountablehq.com/post/hipaa-covered-entity-definition-45-cfr-160-103-plain-english-guide-with-exclusions-and-edge-cases |
| 10 | NIH/Genome.gov: GINA overview and scope | [A] | https://www.genome.gov/genetics-glossary/Genetic-Information-Nondiscrimination-Act-GINA |
| 11 | CDC Genomics: GINA limitations and discrimination gaps, 2022 | [B] | https://blogs.cdc.gov/genomics/2022/10/03/genetic-discrimination/ |
| 12 | Orrick: Navigating privacy gaps for genetic data companies, 2025 | [B] | https://www.orrick.com/en/Insights/2025/08/Navigating-Privacy-Gaps-and-New-Legal-Requirements-for-Companies-Processing-Genetic-Data |
| 13 | Inside Privacy: Multiple states enact genetic privacy legislation 2025 | [B] | https://www.insideprivacy.com/health-privacy/multiple-states-enact-genetic-privacy-legislation-in-a-busy-start-to-2025/ |
| 14 | FPF: Montana, Tennessee, Texas, Virginia enter 2024 with new genetic privacy laws | [B] | https://fpf.org/blog/the-dna-of-genetic-privacy-legislation-montana-tennessee-texas-and-virginia-enter-2024-with-new-genetic-privacy-laws-incorporating-fpfs-best-practices/ |
| 15 | GDPR Article 9 text: Processing of special categories | [A] | https://gdpr-info.eu/art-9-gdpr/ |
| 16 | Foley Hoag: 23andMe bankruptcy highlights best practices for genetic data transfer | [B] | https://foleyhoag.com/news-and-insights/publications/alerts-and-updates/2025/july/23andme-bankruptcy-update-how-the-proceedings-highlight-best-practices-for-handling-and-transferring/ |
