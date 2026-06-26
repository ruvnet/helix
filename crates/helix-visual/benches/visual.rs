//! Criterion benchmark for ADR-025 visual RAG, run over a real corpus of medical
//! document/image PNGs (lab reports, a trend chart, a chest X-ray, an ECG strip,
//! a skin photo). Measures embed throughput, MaxSim, and end-to-end retrieval,
//! and prints a retrieval ranking so the quality is visible on real images.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use helix_visual::{maxsim, Gray, PerceptualEmbedder, VisualEmbedder, VisualIndex};

const CORPUS: &[(&str, &[u8])] = &[
    ("lab-lipid", include_bytes!("corpus/lab-lipid.png")),
    ("lab-cbc", include_bytes!("corpus/lab-cbc.png")),
    ("chart-trend", include_bytes!("corpus/chart-trend.png")),
    ("xray-chest", include_bytes!("corpus/xray-chest.png")),
    ("ecg-strip", include_bytes!("corpus/ecg-strip.png")),
    ("skin-photo", include_bytes!("corpus/skin-photo.png")),
];

fn load(bytes: &[u8]) -> Gray {
    let img = image::load_from_memory(bytes).expect("decode png").to_luma8();
    let (w, h) = img.dimensions();
    Gray::new(w, h, img.into_raw()).expect("gray")
}

fn bench_visual(c: &mut Criterion) {
    let emb = PerceptualEmbedder::default();
    let imgs: Vec<(&str, Gray)> = CORPUS.iter().map(|(id, b)| (*id, load(b))).collect();

    // Build the index once.
    let mut index = VisualIndex::new();
    for (id, g) in &imgs {
        index.add(*id, emb.embed(g).unwrap());
    }

    // Print a retrieval ranking on the real corpus (quality, visible in bench output).
    let query = emb.embed(&imgs[0].1).unwrap(); // query with the lipid lab report
    let results = index.retrieve(&query, 6).unwrap();
    eprintln!("\n[helix-visual] retrieval for query 'lab-lipid' over the medical corpus:");
    for r in &results {
        eprintln!("    {:<14} score {:.3}", r.doc_id, r.score);
    }
    // Sanity: the other lab report should outrank the x-ray for a lab query.
    let lab_cbc = results.iter().find(|r| r.doc_id == "lab-cbc").unwrap().score;
    let xray = results.iter().find(|r| r.doc_id == "xray-chest").unwrap().score;
    eprintln!("    [check] lab-cbc ({lab_cbc:.3}) > xray-chest ({xray:.3}): {}\n", lab_cbc > xray);

    let big = &imgs[3].1; // the x-ray (largest)
    c.bench_function("embed/xray-chest", |b| b.iter(|| emb.embed(black_box(big)).unwrap()));

    let qe = emb.embed(&imgs[0].1).unwrap();
    let de = emb.embed(&imgs[1].1).unwrap();
    c.bench_function("maxsim/pair", |b| b.iter(|| maxsim(black_box(&qe), black_box(&de)).unwrap()));

    c.bench_function("retrieve/corpus-6", |b| {
        b.iter(|| index.retrieve(black_box(&qe), 6).unwrap())
    });
}

criterion_group!(benches, bench_visual);
criterion_main!(benches);
