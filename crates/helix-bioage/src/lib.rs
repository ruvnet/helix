//! # helix-bioage — ADR-034: biological-age estimate (Levine PhenoAge)
//!
//! A deterministic (ADR-007) estimate of biological age from **routine labs** —
//! the Levine *PhenoAge* algorithm (Levine et al., *Aging* 2018), computed from 9
//! standard blood markers + chronological age. No special assay, no LLM doing
//! math.
//!
//! It is an **estimate, never a verdict or diagnosis** (ADR-010): the headline is
//! the *delta* (PhenoAge − chronological age), with a clear "this is an estimate of
//! how your labs compare to typical aging" framing and capped confidence. Any
//! missing marker → **abstain** (ADR-006), never silent imputation.
//!
//! ✅ **Coefficient gate cleared (ADR-034):** validated against NHANES 2021–2023
//! (3,134 adults) — PhenoAge vs. chronological age `r = 0.922` (Levine 2018 reported
//! ~0.94), mean delta −0.64 yrs. The coefficients/unit conversions reproduce PhenoAge
//! on its source population (`cargo run -p helix-bioage --example nhanes_validate`).
//! Clinical-governance sign-off for production use remains separate.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_provenance::{Confidence, EpochMillis, MeasurementMethod, ProvRecord, RecordId};

/// The 9 routine biomarkers + chronological age PhenoAge requires, **in the units
/// the algorithm expects** (documented per field — unit mistakes are the main
/// failure mode).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhenoInputs {
    /// Albumin, **g/L** (e.g. 4.7 g/dL → 47).
    pub albumin_g_l: f64,
    /// Creatinine, **µmol/L** (e.g. 0.9 mg/dL → ~80).
    pub creatinine_umol_l: f64,
    /// Fasting glucose, **mmol/L** (e.g. 90 mg/dL → ~5.0).
    pub glucose_mmol_l: f64,
    /// C-reactive protein, **mg/dL** (e.g. 5 mg/L → 0.5).
    pub crp_mg_dl: f64,
    /// Lymphocytes, **percent**.
    pub lymphocyte_pct: f64,
    /// Mean corpuscular volume, **fL**.
    pub mcv_fl: f64,
    /// Red cell distribution width, **percent**.
    pub rdw_pct: f64,
    /// Alkaline phosphatase, **U/L**.
    pub alk_phosphatase_u_l: f64,
    /// White blood cell count, **1000 cells/µL** (10³/µL).
    pub wbc_1000_ul: f64,
    /// Chronological age, **years**.
    pub age_years: f64,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum BioAgeError {
    #[error("input '{0}' is missing, non-finite, or non-physiological")]
    BadInput(&'static str),
}

/// A biological-age estimate. The `delta_years` (PhenoAge − chronological) is the
/// honest headline; `phenoage_years` is the absolute estimate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BioAge {
    pub phenoage_years: f64,
    pub chronological_years: f64,
    /// PhenoAge − chronological. Negative = labs "younger than" calendar age.
    pub delta_years: f64,
}

// --- Published PhenoAge coefficients (VERIFY against Levine 2018, ADR-034 §5). ---
const INTERCEPT: f64 = -19.9067;
const C_ALBUMIN: f64 = -0.0336;
const C_CREATININE: f64 = 0.0095;
const C_GLUCOSE: f64 = 0.1953;
const C_LN_CRP: f64 = 0.0954;
const C_LYMPH: f64 = -0.0120;
const C_MCV: f64 = 0.0268;
const C_RDW: f64 = 0.3306;
const C_ALP: f64 = 0.00188;
const C_WBC: f64 = 0.0554;
const C_AGE: f64 = 0.0804;
const GAMMA: f64 = 0.0076927;

fn require(name: &'static str, v: f64, lo: f64, hi: f64) -> Result<f64, BioAgeError> {
    if !v.is_finite() || v < lo || v > hi {
        return Err(BioAgeError::BadInput(name));
    }
    Ok(v)
}

/// Compute the PhenoAge biological-age estimate. Validates each input is finite
/// and within a generous physiological range (abstain otherwise, ADR-006).
pub fn phenoage(i: &PhenoInputs) -> Result<BioAge, BioAgeError> {
    let albumin = require("albumin_g_l", i.albumin_g_l, 10.0, 70.0)?;
    let creat = require("creatinine_umol_l", i.creatinine_umol_l, 10.0, 2000.0)?;
    let glucose = require("glucose_mmol_l", i.glucose_mmol_l, 1.0, 50.0)?;
    let crp = require("crp_mg_dl", i.crp_mg_dl, 0.001, 100.0)?;
    let lymph = require("lymphocyte_pct", i.lymphocyte_pct, 1.0, 100.0)?;
    let mcv = require("mcv_fl", i.mcv_fl, 50.0, 150.0)?;
    let rdw = require("rdw_pct", i.rdw_pct, 5.0, 40.0)?;
    let alp = require("alk_phosphatase_u_l", i.alk_phosphatase_u_l, 5.0, 1000.0)?;
    let wbc = require("wbc_1000_ul", i.wbc_1000_ul, 0.5, 100.0)?;
    let age = require("age_years", i.age_years, 1.0, 120.0)?;

    // Linear combination (mortality hazard score).
    let xb = INTERCEPT
        + C_ALBUMIN * albumin
        + C_CREATININE * creat
        + C_GLUCOSE * glucose
        + C_LN_CRP * crp.ln()
        + C_LYMPH * lymph
        + C_MCV * mcv
        + C_RDW * rdw
        + C_ALP * alp
        + C_WBC * wbc
        + C_AGE * age;

    // 10-year mortality score (Gompertz), then invert to phenotypic age.
    let mort = 1.0 - (-xb.exp() * ((GAMMA * 120.0).exp() - 1.0) / GAMMA).exp();
    // Guard the logs (mort in (0,1)).
    let mort = mort.clamp(1e-12, 1.0 - 1e-12);
    let phenoage = 141.50225 + (-0.00553 * (1.0 - mort).ln()).ln() / 0.090165;

    if !phenoage.is_finite() {
        return Err(BioAgeError::BadInput("phenoage_nonfinite"));
    }
    Ok(BioAge {
        phenoage_years: phenoage,
        chronological_years: age,
        delta_years: phenoage - age,
    })
}

/// Render the estimate as a derived, provenance-tagged record (ADR-005). Code is
/// `BIOAGE-PHENOAGE` (never a clinical LOINC, ADR-004); confidence is capped — this
/// is a population-derived estimate, not a measurement.
pub fn to_record(b: &BioAge, measured_at: EpochMillis, source_file: &str) -> ProvRecord {
    ProvRecord {
        id: RecordId::from(format!("bioage-{source_file}-{measured_at}")),
        source: "helix-bioage".to_string(),
        measured_at,
        method: MeasurementMethod::Derived,
        code: Some("BIOAGE-PHENOAGE".to_string()),
        concept: "Biological age (PhenoAge, estimate)".to_string(),
        value: b.phenoage_years,
        unit: "years".to_string(),
        reference_range: None,
        confidence: Confidence::new(0.5), // estimate, not a measurement
    }
}

/// The framing that must accompany every biological-age estimate (ADR-034/010).
pub const DISCLAIMER: &str =
    "An estimate of how your routine labs compare to typical aging — not a measurement, \
     not a diagnosis. Population-derived (ancestry-dependent); discuss with a clinician.";

#[cfg(test)]
mod tests {
    use super::*;

    /// A healthy ~50-year-old profile.
    fn healthy_50() -> PhenoInputs {
        PhenoInputs {
            albumin_g_l: 47.0,
            creatinine_umol_l: 80.0,
            glucose_mmol_l: 5.0,
            crp_mg_dl: 0.5,
            lymphocyte_pct: 30.0,
            mcv_fl: 90.0,
            rdw_pct: 13.0,
            alk_phosphatase_u_l: 70.0,
            wbc_1000_ul: 5.5,
            age_years: 50.0,
        }
    }

    #[test]
    fn healthy_profile_is_biologically_younger_and_sane() {
        let b = phenoage(&healthy_50()).unwrap();
        // hand-computed ≈ 42.6 for this profile
        assert!(
            b.phenoage_years > 35.0 && b.phenoage_years < 50.0,
            "phenoage {}",
            b.phenoage_years
        );
        assert!(
            b.delta_years < 0.0,
            "healthy → younger, delta {}",
            b.delta_years
        );
    }

    #[test]
    fn worse_markers_raise_biological_age() {
        let healthy = phenoage(&healthy_50()).unwrap();
        let mut bad = healthy_50();
        // inflammation + dysglycemia + anisocytosis
        bad.crp_mg_dl = 8.0;
        bad.glucose_mmol_l = 9.5;
        bad.rdw_pct = 16.0;
        bad.wbc_1000_ul = 9.0;
        let worse = phenoage(&bad).unwrap();
        assert!(
            worse.phenoage_years > healthy.phenoage_years,
            "worse {} should exceed healthy {}",
            worse.phenoage_years,
            healthy.phenoage_years
        );
    }

    #[test]
    fn monotonic_in_age() {
        let mut older = healthy_50();
        older.age_years = 70.0;
        assert!(
            phenoage(&older).unwrap().phenoage_years
                > phenoage(&healthy_50()).unwrap().phenoage_years
        );
    }

    #[test]
    fn missing_or_nonphysiological_input_abstains() {
        let mut i = healthy_50();
        i.crp_mg_dl = f64::NAN;
        assert_eq!(phenoage(&i), Err(BioAgeError::BadInput("crp_mg_dl")));
        let mut j = healthy_50();
        j.albumin_g_l = 4.7; // looks like g/dL — out of g/L physiological range → caught
        assert_eq!(phenoage(&j), Err(BioAgeError::BadInput("albumin_g_l")));
    }

    #[test]
    fn record_is_derived_capped_and_non_loinc() {
        let b = phenoage(&healthy_50()).unwrap();
        let r = to_record(&b, 1000, "panel");
        assert_eq!(r.method, MeasurementMethod::Derived);
        assert_eq!(r.code.as_deref(), Some("BIOAGE-PHENOAGE"));
        assert!(r.confidence.get() <= 0.5);
        assert!(DISCLAIMER.contains("not a diagnosis"));
    }
}
