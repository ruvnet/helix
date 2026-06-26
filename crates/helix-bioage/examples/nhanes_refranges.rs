//! Derive population reference intervals (2.5th–97.5th percentile) from NHANES
//! 2021–2023 for common markers, emitting a Rust const table for `helix-refranges`.
//! Run: `cargo run -p helix-bioage --example nhanes_refranges`
//! These are POPULATION percentiles (general adults), an honest FALLBACK when a lab
//! supplies no reference range — not lab-specific clinical reference intervals.

use std::collections::HashMap;
use std::fs;

// --- minimal SAS XPT v5 reader (same as nhanes_validate) ---------------------
fn ibm_to_f64(raw: &[u8]) -> f64 {
    let mut b = [0u8; 8];
    b[..raw.len().min(8)].copy_from_slice(&raw[..raw.len().min(8)]);
    if b.iter().all(|&x| x == 0) {
        return 0.0;
    }
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
    ntype: i16,
    len: usize,
    pos: usize,
}
fn read_xpt(path: &str, wanted: &[&str]) -> Vec<HashMap<String, f64>> {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("read {path}: {e}");
            std::process::exit(1);
        }
    };
    const NEEDLE: &[u8] = b"HEADER RECORD*******NAMESTR HEADER RECORD";
    let ns_hdr = data
        .windows(NEEDLE.len())
        .position(|w| w == NEEDLE)
        .expect("no NAMESTR");
    let nvars: usize = std::str::from_utf8(&data[ns_hdr + 54..ns_hdr + 58])
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let ns_start = ns_hdr + 80;
    let mut vars = Vec::new();
    for i in 0..nvars {
        let r = &data[ns_start + i * 140..ns_start + i * 140 + 140];
        vars.push(Var {
            ntype: i16::from_be_bytes([r[0], r[1]]),
            len: i16::from_be_bytes([r[4], r[5]]) as usize,
            pos: i32::from_be_bytes([r[84], r[85], r[86], r[87]]) as usize,
            name: String::from_utf8_lossy(&r[8..16]).trim().to_string(),
        });
    }
    let obs_len: usize = vars.iter().map(|v| v.len).sum();
    let data_start = ns_start + (nvars * 140).div_ceil(80) * 80 + 80;
    let want: Vec<&Var> = vars
        .iter()
        .filter(|v| wanted.contains(&v.name.as_str()))
        .collect();
    let mut out = Vec::new();
    let mut off = data_start;
    while off + obs_len <= data.len() {
        let row = &data[off..off + obs_len];
        if row.iter().all(|&x| x == b' ' || x == 0) {
            break;
        }
        let mut m = HashMap::new();
        for v in &want {
            if v.ntype == 1 {
                m.insert(v.name.clone(), ibm_to_f64(&row[v.pos..v.pos + v.len]));
            }
        }
        out.push(m);
        off += obs_len;
    }
    out
}
fn index(path: &str, wanted: &[&str]) -> HashMap<i64, HashMap<String, f64>> {
    let mut all = wanted.to_vec();
    all.push("SEQN");
    read_xpt(path, &all)
        .into_iter()
        .filter_map(|r| {
            let s = r.get("SEQN").copied().filter(|v| v.is_finite())?;
            Some((s as i64, r))
        })
        .collect()
}
fn pct(sorted: &[f64], p: f64) -> f64 {
    let i = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn main() {
    let dir = std::env::var("HELIX_NHANES_DIR").unwrap_or_else(|_| "/tmp/healthdata/nhanes".into());
    let f = |n: &str| format!("{dir}/{n}.XPT");
    let demo = index(&f("DEMO_L"), &["RIDAGEYR"]);

    // (file, nhanes_var, loinc, name, unit)
    let markers: &[(&str, &str, &str, &str, &str)] = &[
        ("BIOPRO_L", "LBXSAL", "1751-7", "Albumin", "g/dL"),
        (
            "BIOPRO_L",
            "LBXSAPSI",
            "6768-6",
            "Alkaline phosphatase",
            "U/L",
        ),
        ("BIOPRO_L", "LBXSATSI", "1742-6", "ALT", "U/L"),
        ("BIOPRO_L", "LBXSASSI", "1920-8", "AST", "U/L"),
        (
            "BIOPRO_L",
            "LBXSBU",
            "3094-0",
            "Urea nitrogen (BUN)",
            "mg/dL",
        ),
        ("BIOPRO_L", "LBXSCA", "17861-6", "Calcium", "mg/dL"),
        ("BIOPRO_L", "LBXSCH", "2093-3", "Total cholesterol", "mg/dL"),
        ("BIOPRO_L", "LBXSCR", "2160-0", "Creatinine", "mg/dL"),
        ("BIOPRO_L", "LBXSGL", "2345-7", "Glucose", "mg/dL"),
        ("BIOPRO_L", "LBXSKSI", "2823-3", "Potassium", "mmol/L"),
        ("BIOPRO_L", "LBXSNASI", "2951-2", "Sodium", "mmol/L"),
        ("BIOPRO_L", "LBXSTP", "2885-2", "Total protein", "g/dL"),
        ("BIOPRO_L", "LBXSTR", "2571-8", "Triglycerides", "mg/dL"),
        ("BIOPRO_L", "LBXSUA", "3084-1", "Uric acid", "mg/dL"),
        ("BIOPRO_L", "LBXSIR", "2498-4", "Iron", "ug/dL"),
        ("BIOPRO_L", "LBXSPH", "2777-1", "Phosphorus", "mg/dL"),
        ("BIOPRO_L", "LBXSTB", "1975-2", "Total bilirubin", "mg/dL"),
        (
            "CBC_L",
            "LBXWBCSI",
            "6690-2",
            "White blood cells",
            "1000/uL",
        ),
        ("CBC_L", "LBXHGB", "718-7", "Hemoglobin", "g/dL"),
        ("CBC_L", "LBXHCT", "4544-3", "Hematocrit", "%"),
        ("CBC_L", "LBXMCVSI", "787-2", "MCV", "fL"),
        ("CBC_L", "LBXRDW", "788-0", "RDW", "%"),
        ("CBC_L", "LBXPLTSI", "777-3", "Platelets", "1000/uL"),
        ("CBC_L", "LBXLYPCT", "736-9", "Lymphocytes", "%"),
        (
            "CBC_L",
            "LBXRBCSI",
            "789-8",
            "Red blood cells",
            "million/uL",
        ),
        ("HSCRP_L", "LBXHSCRP", "1988-5", "hs-CRP", "mg/L"),
        ("GLU_L", "LBXGLU", "1558-6", "Fasting glucose", "mg/dL"),
    ];

    println!("// NHANES 2021–2023 population reference intervals (2.5–97.5 pctile, adults 18+).");
    println!(
        "// Generated by `cargo run -p helix-bioage --example nhanes_refranges`. FALLBACK only."
    );
    let mut files: HashMap<&str, HashMap<i64, HashMap<String, f64>>> = HashMap::new();
    for (file, var, code, name, unit) in markers {
        let idx = files.entry(file).or_insert_with(|| index(&f(file), &[var]));
        // also ensure var present (re-index if first time saw a different var)
        if !idx
            .values()
            .next()
            .map(|m| m.contains_key(*var))
            .unwrap_or(false)
        {
            *idx = index(&f(file), &[var]);
        }
        let mut vals: Vec<f64> = demo
            .iter()
            .filter(|(_, d)| d.get("RIDAGEYR").map(|&a| a >= 18.0).unwrap_or(false))
            .filter_map(|(seqn, _)| idx.get(seqn).and_then(|m| m.get(*var)).copied())
            .filter(|v| v.is_finite() && *v > 0.0)
            .collect();
        if vals.len() < 100 {
            eprintln!("// SKIP {name} ({var}): only {} values", vals.len());
            continue;
        }
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (lo, med, hi) = (pct(&vals, 0.025), pct(&vals, 0.5), pct(&vals, 0.975));
        println!(
            "    R(\"{code}\", \"{name}\", \"{unit}\", {lo:.3}, {hi:.3}, {med:.3}), // n={}",
            vals.len()
        );
    }
}
