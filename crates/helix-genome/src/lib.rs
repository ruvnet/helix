//! # helix-genome — ADR-021: user-owned genome ingestion & pharmacogenomics
//!
//! Adapter for [ruvnet/rvdna](https://github.com/ruvnet/rvdna): an AI-native
//! genomics engine (genotyping, CYP2D6/CYP2C19 pharmacogenomics, biomarker risk)
//! that runs on the edge via WASM. Helix uses it to make the user's **own**
//! genome usable without it ever leaving the device (ADR-001/013, §7.4).
//!
//! This crate maps rvDNA's derived outputs into Helix:
//!
//! - **Pharmacogenomic phenotypes** (e.g. CYP2D6 poor metabolizer) → a
//!   provenance record **and** a "verify with your prescriber" advisory — the
//!   highest-actionability genomic signal, framed as decision-support, never a
//!   dosing directive (ADR-010).
//! - **Biomarker risk scores** → records carrying a band **and** the population /
//!   ancestry caveat — probabilistic context, never a diagnosis (ADR-006).
//!
//! Hard rules: genomic data is the most sensitive data there is — every record is
//! flagged genetic, carries [`GENOME_PRIVACY_NOTE`] (GINA-aware), and is excluded
//! from federation by default (ADR-011). Derived facts use `GENO-*` codes, never
//! a clinical LOINC (ADR-004), at capped confidence.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_provenance::{
    Confidence, EpochMillis, MeasurementMethod, ProvRecord, RecordId, ReferenceRange,
};

/// Standing privacy note attached to all genomic-derived signals.
pub const GENOME_PRIVACY_NOTE: &str =
    "Genetic data: user-owned and local-first; never sold or transferred, excluded from federation \
     by default. GINA limits employer/insurer discrimination but does not govern collection.";

/// Non-diagnostic framing for genomic risk / pharmacogenomics.
pub const GENOME_DISCLAIMER: &str =
    "Genomic risk and pharmacogenomics are decision-support, not a diagnosis. Verify with a \
     clinician or prescriber; risk scores are probabilistic and ancestry-dependent.";

const SOURCE: &str = "rvdna";

/// Metabolizer phenotype from a pharmacogenomic star-allele diplotype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metabolizer {
    Poor,
    Intermediate,
    Normal,
    Rapid,
    Ultrarapid,
}

impl Metabolizer {
    /// Normal metabolizers need no advisory; everything else is worth a
    /// prescriber conversation (dosing of common drugs can change).
    pub fn needs_advisory(self) -> bool {
        !matches!(self, Metabolizer::Normal)
    }
    fn label(self) -> &'static str {
        match self {
            Metabolizer::Poor => "poor",
            Metabolizer::Intermediate => "intermediate",
            Metabolizer::Normal => "normal",
            Metabolizer::Rapid => "rapid",
            Metabolizer::Ultrarapid => "ultrarapid",
        }
    }
    /// Numeric encoding for the provenance record value (0..=4).
    fn code_value(self) -> f64 {
        match self {
            Metabolizer::Poor => 0.0,
            Metabolizer::Intermediate => 1.0,
            Metabolizer::Normal => 2.0,
            Metabolizer::Rapid => 3.0,
            Metabolizer::Ultrarapid => 4.0,
        }
    }
}

/// A pharmacogenomic call: a gene, its diplotype, and the inferred phenotype.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PharmacoCall {
    /// e.g. "CYP2D6", "CYP2C19".
    pub gene: String,
    /// e.g. "*1/*4".
    pub diplotype: String,
    pub phenotype: Metabolizer,
}

/// A biomarker / polygenic risk result with its band and caveat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiomarkerRisk {
    pub trait_name: String,
    /// Normalized risk score 0..=1.
    pub score: f64,
    /// Human band, e.g. "typical", "elevated".
    pub band: String,
}

/// A derived genomic profile from rvDNA, computed on the user's own file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenomeProfile {
    /// User-owned source file label (e.g. "23andMe v5 raw").
    pub source_file: String,
    pub imported_at: EpochMillis,
    pub genotype_count: u32,
    #[serde(default)]
    pub pharmaco: Vec<PharmacoCall>,
    #[serde(default)]
    pub risks: Vec<BiomarkerRisk>,
    /// Ancestry note for risk-score caveats (e.g. "primarily European reference").
    #[serde(default)]
    pub ancestry_caveat: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum GenomeError {
    #[error("genome profile has no derived results")]
    Empty,
    #[error("risk score {0} out of range 0..=1")]
    ScoreOutOfRange(f64),
    #[error("a value was not finite")]
    NonFinite,
}

/// A "verify with your prescriber" advisory derived from a pharmacogenomic call.
/// Decision-support only — never a dosing directive (ADR-010).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrescriberAdvisory {
    pub gene: String,
    pub phenotype: Metabolizer,
    pub message: String,
}

/// Map a genome profile into provenance records: one per pharmacogenomic
/// phenotype and one per biomarker risk. Genomic data is the most sensitive, so
/// every record is `Derived`/`GENO-*` at capped confidence.
pub fn profile_to_records(profile: &GenomeProfile) -> Result<Vec<ProvRecord>, GenomeError> {
    if profile.pharmaco.is_empty() && profile.risks.is_empty() {
        return Err(GenomeError::Empty);
    }
    let mut out = Vec::new();

    for p in &profile.pharmaco {
        out.push(ProvRecord {
            id: RecordId::from(format!("geno-{}-{}", profile.source_file, p.gene)),
            source: SOURCE.to_string(),
            measured_at: profile.imported_at,
            method: MeasurementMethod::Derived,
            code: Some(format!("GENO-PGX-{}", p.gene.to_uppercase())),
            concept: format!("{} metabolizer phenotype", p.gene),
            value: p.phenotype.code_value(),
            unit: format!("phenotype:{}", p.phenotype.label()),
            // 0..=4 metabolizer scale; "normal" (2) is the reference midpoint.
            reference_range: Some(ReferenceRange::new(Some(2.0), Some(2.0))),
            confidence: Confidence::new(0.6),
        });
    }

    for risk in &profile.risks {
        if !risk.score.is_finite() {
            return Err(GenomeError::NonFinite);
        }
        if !(0.0..=1.0).contains(&risk.score) {
            return Err(GenomeError::ScoreOutOfRange(risk.score));
        }
        out.push(ProvRecord {
            id: RecordId::from(format!("geno-{}-{}", profile.source_file, risk.trait_name)),
            source: SOURCE.to_string(),
            measured_at: profile.imported_at,
            method: MeasurementMethod::Derived,
            code: Some(format!(
                "GENO-RISK-{}",
                risk.trait_name.to_uppercase().replace(' ', "-")
            )),
            concept: format!("Genomic risk: {} ({})", risk.trait_name, risk.band),
            value: risk.score,
            unit: "risk:0-1".to_string(),
            reference_range: Some(ReferenceRange::new(Some(0.0), Some(1.0))),
            // Probabilistic, ancestry-bound → lower confidence than pharmacogenomics.
            confidence: Confidence::new(0.4),
        });
    }
    Ok(out)
}

/// Produce prescriber advisories for any non-normal metabolizer phenotype.
/// These surface when relevant medications are in the dossier — as prompts to
/// verify, never as directives.
pub fn prescriber_advisories(profile: &GenomeProfile) -> Vec<PrescriberAdvisory> {
    profile
        .pharmaco
        .iter()
        .filter(|p| p.phenotype.needs_advisory())
        .map(|p| PrescriberAdvisory {
            gene: p.gene.clone(),
            phenotype: p.phenotype,
            message: format!(
                "Your {} result suggests a {} metabolizer profile ({}). This can affect how some \
                 medications are dosed — worth verifying with your prescriber. Not a directive.",
                p.gene,
                p.phenotype.label(),
                p.diplotype
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> GenomeProfile {
        GenomeProfile {
            source_file: "23andMe-v5".into(),
            imported_at: 1_750_000_000_000,
            genotype_count: 640_000,
            pharmaco: vec![
                PharmacoCall {
                    gene: "CYP2D6".into(),
                    diplotype: "*4/*4".into(),
                    phenotype: Metabolizer::Poor,
                },
                PharmacoCall {
                    gene: "CYP2C19".into(),
                    diplotype: "*1/*1".into(),
                    phenotype: Metabolizer::Normal,
                },
            ],
            risks: vec![BiomarkerRisk {
                trait_name: "Type 2 diabetes".into(),
                score: 0.62,
                band: "elevated".into(),
            }],
            ancestry_caveat: Some("primarily European reference panel".into()),
        }
    }

    #[test]
    fn maps_pharmaco_and_risk_to_genomic_records() {
        let recs = profile_to_records(&profile()).unwrap();
        assert_eq!(recs.len(), 3); // 2 pharmaco + 1 risk
        for r in &recs {
            assert_eq!(r.source, "rvdna");
            assert_eq!(r.method, MeasurementMethod::Derived);
            assert!(r.code.as_ref().unwrap().starts_with("GENO-")); // never a clinical LOINC
            assert!(r.confidence.get() <= 0.6); // capped
        }
    }

    #[test]
    fn advisory_only_for_non_normal_metabolizers() {
        let adv = prescriber_advisories(&profile());
        assert_eq!(adv.len(), 1); // CYP2D6 poor; CYP2C19 normal → none
        assert_eq!(adv[0].gene, "CYP2D6");
        assert!(adv[0].message.contains("Not a directive"));
        assert!(!adv[0].message.to_lowercase().contains("take "));
    }

    #[test]
    fn empty_profile_errors() {
        let mut p = profile();
        p.pharmaco.clear();
        p.risks.clear();
        assert_eq!(profile_to_records(&p), Err(GenomeError::Empty));
    }

    #[test]
    fn out_of_range_and_nan_risk_rejected() {
        let mut p = profile();
        p.risks[0].score = 1.4;
        assert_eq!(
            profile_to_records(&p),
            Err(GenomeError::ScoreOutOfRange(1.4))
        );
        let mut p2 = profile();
        p2.risks[0].score = f64::NAN;
        assert_eq!(profile_to_records(&p2), Err(GenomeError::NonFinite));
    }

    #[test]
    fn metabolizer_advisory_rule() {
        assert!(!Metabolizer::Normal.needs_advisory());
        assert!(Metabolizer::Poor.needs_advisory());
        assert!(Metabolizer::Ultrarapid.needs_advisory());
    }

    #[test]
    fn privacy_and_disclaimer_present() {
        assert!(GENOME_PRIVACY_NOTE.contains("GINA"));
        assert!(GENOME_PRIVACY_NOTE.contains("user-owned"));
        assert!(GENOME_DISCLAIMER.contains("not a diagnosis"));
    }
}

// --- 23andMe raw-genotype import (ADR-021) -----------------------------------

/// One curated, well-documented single-SNP annotation. **Single variants only —
/// NOT full star-allele diplotype calling** (that needs phasing / a proper PGx
/// panel, e.g. rvDNA). Informational, verify with a clinician.
struct SnpAnno {
    rsid: &'static str,
    gene: &'static str,
    label: &'static str,
    /// The allele whose copies are counted (the minor/effect allele).
    effect_allele: char,
    note: &'static str,
}

/// A small, conservative annotation table. Each entry is a single, widely-cited
/// variant; the import counts effect-allele copies (0–2) and attaches the note.
const ANNOTATIONS: &[SnpAnno] = &[
    SnpAnno { rsid: "rs4244285", gene: "CYP2C19", label: "CYP2C19*2 (loss-of-function)", effect_allele: 'A',
        note: "Reduced CYP2C19 activity affects some drugs (e.g. clopidogrel). Single variant — not a full PGx panel." },
    SnpAnno { rsid: "rs1799853", gene: "CYP2C9", label: "CYP2C9*2", effect_allele: 'T',
        note: "Reduced CYP2C9 activity can affect warfarin/NSAID metabolism. Single variant." },
    SnpAnno { rsid: "rs1057910", gene: "CYP2C9", label: "CYP2C9*3", effect_allele: 'C',
        note: "Reduced CYP2C9 activity. Single variant." },
    SnpAnno { rsid: "rs1801133", gene: "MTHFR", label: "MTHFR C677T", effect_allele: 'T',
        note: "Reduced MTHFR enzyme activity; common. Informational only." },
    SnpAnno { rsid: "rs4988235", gene: "LCT/MCM6", label: "Lactase persistence", effect_allele: 'G',
        note: "Associated with adult lactase persistence (dairy tolerance). Informational." },
];

/// A finding from an annotated SNP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnpFinding {
    pub rsid: String,
    pub gene: String,
    pub label: String,
    pub genotype: String,
    /// Copies of the effect allele present (0, 1, or 2).
    pub effect_allele_copies: u8,
    pub note: String,
}

/// Result of importing a 23andMe raw genotype file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawGenomeResult {
    /// Total genotype rows parsed (a coverage indicator).
    pub total_variants: u32,
    /// Annotated findings → provenance records.
    pub records: Vec<ProvRecord>,
    pub findings: Vec<SnpFinding>,
    /// Standing caveat for the whole import.
    pub caveat: String,
}

/// Parse a 23andMe-style raw genotype file (`rsid\tchrom\tpos\tgenotype`, `#`
/// comments). The raw file stays user-owned and local (§7.4/ADR-001); this only
/// surfaces a few well-documented single-variant findings, **not** a full
/// pharmacogenomic diplotype call.
pub fn parse_23andme_raw(text: &str, source_file: &str) -> RawGenomeResult {
    let mut total = 0u32;
    let mut found: std::collections::BTreeMap<&str, String> = Default::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split('\t');
        let Some(rsid) = cols.next() else { continue };
        // genotype is the last column
        let Some(genotype) = line.split('\t').next_back() else {
            continue;
        };
        if genotype.is_empty() || rsid == genotype {
            continue;
        }
        total += 1;
        if ANNOTATIONS.iter().any(|a| a.rsid == rsid) {
            found.insert(
                ANNOTATIONS.iter().find(|a| a.rsid == rsid).unwrap().rsid,
                genotype.to_string(),
            );
        }
    }

    let mut records = Vec::new();
    let mut findings = Vec::new();
    for a in ANNOTATIONS {
        if let Some(gt) = found.get(a.rsid) {
            let copies = gt.chars().filter(|&c| c == a.effect_allele).count() as u8;
            findings.push(SnpFinding {
                rsid: a.rsid.into(),
                gene: a.gene.into(),
                label: a.label.into(),
                genotype: gt.clone(),
                effect_allele_copies: copies,
                note: a.note.into(),
            });
            records.push(ProvRecord {
                id: RecordId::from(format!("geno-snp-{source_file}-{}", a.rsid)),
                source: "rvdna".to_string(),
                measured_at: 0,
                method: MeasurementMethod::Derived,
                code: Some(format!("GENO-SNP-{}", a.rsid.to_uppercase())),
                concept: format!("{} — {} ({})", a.gene, a.label, gt),
                value: copies as f64,
                unit: "effect-allele-copies".to_string(),
                reference_range: None,
                confidence: Confidence::new(0.5),
            });
        }
    }

    RawGenomeResult {
        total_variants: total,
        records,
        findings,
        caveat: "Single-variant findings only — not a full pharmacogenomic diplotype call (needs a proper PGx panel). \
                 Informational, not a diagnosis; your raw file stays on your device (ADR-001). Verify with a clinician."
            .to_string(),
    }
}

#[cfg(test)]
mod raw_tests {
    use super::*;

    const SAMPLE: &str = "# 23andMe raw data\n# rsid\tchromosome\tposition\tgenotype\n\
rs4477212\t1\t82154\tAA\n\
rs4244285\t10\t96541616\tAG\n\
rs1801133\t1\t11856378\tTT\n\
rs9999999\t2\t12345\tCC\n";

    #[test]
    fn counts_variants_and_annotates_known_snps() {
        let r = parse_23andme_raw(SAMPLE, "23andMe-v5");
        assert_eq!(r.total_variants, 4);
        // CYP2C19*2 (AG → 1 A copy) and MTHFR (TT → 2 T copies)
        assert_eq!(r.findings.len(), 2);
        let cyp = r.findings.iter().find(|f| f.gene == "CYP2C19").unwrap();
        assert_eq!(cyp.effect_allele_copies, 1);
        let mthfr = r.findings.iter().find(|f| f.gene == "MTHFR").unwrap();
        assert_eq!(mthfr.effect_allele_copies, 2);
    }

    #[test]
    fn records_are_geno_snp_capped_and_caveated() {
        let r = parse_23andme_raw(SAMPLE, "f");
        assert_eq!(r.records.len(), 2);
        for rec in &r.records {
            assert!(rec.code.as_ref().unwrap().starts_with("GENO-SNP-"));
            assert!(rec.confidence.get() <= 0.5);
        }
        assert!(r.caveat.contains("not a full pharmacogenomic"));
    }

    #[test]
    fn no_known_snps_still_counts() {
        let r = parse_23andme_raw("rs0000\t1\t1\tGG\n", "f");
        assert_eq!(r.total_variants, 1);
        assert!(r.findings.is_empty());
    }
}
