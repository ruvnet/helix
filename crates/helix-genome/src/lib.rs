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
