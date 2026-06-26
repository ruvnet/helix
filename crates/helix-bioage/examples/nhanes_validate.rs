//! Validate the PhenoAge biological-age implementation against **NHANES**
//! (CDC, public domain) — the population PhenoAge was derived from.
//! Run: `cargo run -p helix-bioage --example nhanes_validate`
//! Expects NHANES 2021–2023 .XPT files in $HELIX_NHANES_DIR (default /tmp/healthdata/nhanes):
//! DEMO_L, BIOPRO_L, GLU_L, HSCRP_L, CBC_L.
//!
//! This clears the ADR-034 coefficient-verification gate empirically: if our
//! deterministic PhenoAge reproduces the expected population behaviour (strong
//! correlation with chronological age, near-zero mean delta), the coefficients and
//! unit handling are right.

use std::collections::HashMap;
use std::fs;

use helix_bioage::{phenoage, PhenoInputs};

// --- minimal SAS XPT v5 reader (no deps) -------------------------------------

/// IBM hexadecimal floating point (as stored in XPT) → f64. SAS missing → NaN.
fn ibm_to_f64(raw: &[u8]) -> f64 {
    let mut b = [0u8; 8];
    b[..raw.len().min(8)].copy_from_slice(&raw[..raw.len().min(8)]);
    if b.iter().all(|&x| x == 0) {
        return 0.0;
    }
    // SAS missing values: first byte '.', '_', or 'A'..'Z' with the rest zero.
    if b[1..].iter().all(|&x| x == 0) && (b[0] == b'.' || b[0] == b'_' || b[0].is_ascii_uppercase())
    {
        return f64::NAN;
    }
    let sign = if b[0] & 0x80 != 0 { -1.0 } else { 1.0 };
    let exp = (b[0] & 0x7f) as i32 - 64;
    let mut mant: u64 = 0;
    for &x in &b[1..8] {
        mant = (mant << 8) | x as u64;
    }
    sign * (mant as f64 / 2f64.powi(56)) * 16f64.powi(exp)
}

struct Var {
    name: String,
    ntype: i16, // 1 numeric, 2 char
    len: usize,
    pos: usize,
}

fn be_i16(b: &[u8]) -> i16 {
    i16::from_be_bytes([b[0], b[1]])
}
fn be_i32(b: &[u8]) -> i32 {
    i32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// Read an XPT file; return one map {varname → f64} per observation, only for
/// `wanted` numeric variables.
fn read_xpt(path: &str, wanted: &[&str]) -> Vec<HashMap<String, f64>> {
    let data = fs::read(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    // Locate the NAMESTR header record.
    let find = |needle: &str, from: usize| -> Option<usize> {
        data.windows(needle.len())
            .skip(from)
            .position(|w| w == needle.as_bytes())
            .map(|p| p + from)
    };
    let ns_hdr = find("HEADER RECORD*******NAMESTR HEADER RECORD", 0).expect("no NAMESTR header");
    // number of variables is in the header record (cols 55-58, "0000nnnn0000...").
    let hdr = &data[ns_hdr..ns_hdr + 80];
    let nvars: usize = std::str::from_utf8(&hdr[54..58])
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let ns_start = ns_hdr + 80;
    let mut vars = Vec::new();
    for i in 0..nvars {
        let r = &data[ns_start + i * 140..ns_start + i * 140 + 140];
        let name = String::from_utf8_lossy(&r[8..16]).trim().to_string();
        vars.push(Var {
            ntype: be_i16(&r[0..2]),
            len: be_i16(&r[4..6]) as usize,
            pos: be_i32(&r[84..88]) as usize,
            name,
        });
    }
    let obs_len: usize = vars.iter().map(|v| v.len).sum();
    // NAMESTR block is padded to a multiple of 80, then the OBS header (80 bytes).
    let ns_bytes = nvars * 140;
    let ns_pad = ns_bytes.div_ceil(80) * 80;
    let obs_hdr = ns_start + ns_pad;
    let data_start = obs_hdr + 80; // skip "HEADER RECORD...OBS..."

    let want: Vec<&Var> = vars
        .iter()
        .filter(|v| wanted.contains(&v.name.as_str()))
        .collect();
    let mut out = Vec::new();
    let mut off = data_start;
    while off + obs_len <= data.len() {
        // stop at trailing padding (all spaces/zeros)
        let row = &data[off..off + obs_len];
        if row.iter().all(|&x| x == b' ' || x == 0) {
            break;
        }
        let mut m = HashMap::new();
        for v in &want {
            if v.ntype == 1 {
                let val = ibm_to_f64(&row[v.pos..v.pos + v.len]);
                m.insert(v.name.clone(), val);
            }
        }
        out.push(m);
        off += obs_len;
    }
    out
}

/// Index a file's observations by SEQN.
fn by_seqn(path: &str, wanted: &[&str]) -> HashMap<i64, HashMap<String, f64>> {
    let mut all = wanted.to_vec();
    if !all.contains(&"SEQN") {
        all.push("SEQN");
    }
    let mut m = HashMap::new();
    for row in read_xpt(path, &all) {
        if let Some(&s) = row.get("SEQN") {
            if s.is_finite() {
                m.insert(s as i64, row);
            }
        }
    }
    m
}

fn main() {
    let dir = std::env::var("HELIX_NHANES_DIR").unwrap_or_else(|_| "/tmp/healthdata/nhanes".into());
    let f = |n: &str| format!("{dir}/{n}.XPT");

    let demo = by_seqn(&f("DEMO_L"), &["RIDAGEYR"]);
    let bio = by_seqn(&f("BIOPRO_L"), &["LBXSAL", "LBXSCR", "LBXSAPSI"]);
    let glu = by_seqn(&f("GLU_L"), &["LBXGLU"]);
    let crp = by_seqn(&f("HSCRP_L"), &["LBXHSCRP"]);
    let cbc = by_seqn(&f("CBC_L"), &["LBXLYPCT", "LBXMCVSI", "LBXRDW", "LBXWBCSI"]);
    println!(
        "NHANES 2021–2023 loaded: demo={} biopro={} glu={} crp={} cbc={}",
        demo.len(),
        bio.len(),
        glu.len(),
        crp.len(),
        cbc.len()
    );

    let g = |m: &HashMap<String, f64>, k: &str| m.get(k).copied().filter(|v| v.is_finite());

    let mut pairs: Vec<(f64, f64)> = Vec::new(); // (phenoage, chrono age)
    let mut deltas = Vec::new();
    for (&seqn, d) in &demo {
        let (Some(age), Some(b), Some(gl), Some(cr), Some(cb)) = (
            g(d, "RIDAGEYR"),
            bio.get(&seqn),
            glu.get(&seqn),
            crp.get(&seqn),
            cbc.get(&seqn),
        ) else {
            continue;
        };
        let (Some(alb), Some(creat), Some(alp)) =
            (g(b, "LBXSAL"), g(b, "LBXSCR"), g(b, "LBXSAPSI"))
        else {
            continue;
        };
        let (Some(glucose), Some(crp_v)) = (g(gl, "LBXGLU"), g(cr, "LBXHSCRP")) else {
            continue;
        };
        let (Some(lymph), Some(mcv), Some(rdw), Some(wbc)) = (
            g(cb, "LBXLYPCT"),
            g(cb, "LBXMCVSI"),
            g(cb, "LBXRDW"),
            g(cb, "LBXWBCSI"),
        ) else {
            continue;
        };
        if age < 18.0 {
            continue; // PhenoAge is for adults
        }
        // Unit conversions into PhenoAge units (ADR-034 field docs).
        let inputs = PhenoInputs {
            albumin_g_l: alb * 10.0,          // g/dL → g/L
            creatinine_umol_l: creat * 88.42, // mg/dL → µmol/L
            glucose_mmol_l: glucose * 0.0555, // mg/dL → mmol/L
            crp_mg_dl: crp_v * 0.1,           // mg/L → mg/dL
            lymphocyte_pct: lymph,
            mcv_fl: mcv,
            rdw_pct: rdw,
            alk_phosphatase_u_l: alp,
            wbc_1000_ul: wbc,
            age_years: age,
        };
        if let Ok(b) = phenoage(&inputs) {
            pairs.push((b.phenoage_years, age));
            deltas.push(b.delta_years);
        }
    }

    let n = pairs.len() as f64;
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in &pairs {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
        syy += (y - my) * (y - my);
    }
    let r = sxy / (sxx.sqrt() * syy.sqrt());
    let mean_delta = deltas.iter().sum::<f64>() / n;
    let within10 = deltas.iter().filter(|d| d.abs() <= 10.0).count() as f64 / n * 100.0;

    println!(
        "\nPhenoAge validated on {} adults (all 9 markers + age present)",
        pairs.len()
    );
    println!(
        "  correlation(PhenoAge, chronological age)  r = {r:.3}   (Levine 2018 reported ~0.94)"
    );
    println!(
        "  mean delta (PhenoAge − age)               = {mean_delta:+.2} yrs   (expect near 0)"
    );
    println!("  within ±10 yrs of chronological age       = {within10:.1}%");
    let pass = r > 0.85 && mean_delta.abs() < 6.0;
    println!(
        "\nADR-034 coefficient gate: {}  (r>0.85 and |mean delta|<6)",
        if pass {
            "✅ PASS — implementation reproduces PhenoAge on its source population"
        } else {
            "✗ review"
        }
    );
}
