//! Integration test against the REAL local-GPU vision encoder (moondream) + the
//! MiniLM embedder (all-minilm). Ignored by default; run explicitly:
//!   cargo test -p helix-vision -- --ignored
//!
//! Validates the learned visual encoder end to end on the GPU over the real
//! medical-image corpus: lab reports embed closer to each other than to an x-ray,
//! and the value-guard keeps every caption layout-only (no numbers).

use helix_embed::LocalEmbedder;
use helix_vision::{caption_is_layout_only, rank, LayoutVisualEncoder, OllamaVision};

const CORPUS: &[(&str, &[u8])] = &[
    (
        "lab-lipid",
        include_bytes!("../../helix-visual/benches/corpus/lab-lipid.png"),
    ),
    (
        "lab-cbc",
        include_bytes!("../../helix-visual/benches/corpus/lab-cbc.png"),
    ),
    (
        "xray-chest",
        include_bytes!("../../helix-visual/benches/corpus/xray-chest.png"),
    ),
    (
        "ecg-strip",
        include_bytes!("../../helix-visual/benches/corpus/ecg-strip.png"),
    ),
];

#[test]
#[ignore = "requires local ollama moondream + all-minilm GPU endpoints"]
fn visual_retrieval_on_gpu() {
    let enc = LayoutVisualEncoder::new(OllamaVision::default(), LocalEmbedder::default());

    let encoded: Vec<(String, _)> = CORPUS
        .iter()
        .map(|(id, png)| {
            let e = enc.encode(png).expect("encode");
            eprintln!(
                "[helix-vision GPU] {id:<12} caption={:?} guarded={}",
                e.caption, e.guarded
            );
            // every caption must be layout-only (the encoder's safety property)
            assert!(
                caption_is_layout_only(&e.caption),
                "caption read a value: {}",
                e.caption
            );
            (id.to_string(), e)
        })
        .collect();

    // Query with the lipid lab report; the other lab report should outrank the x-ray.
    let query = encoded[0].1.clone();
    let results = rank(&query, &encoded[1..], 3);
    eprintln!("\n[helix-vision GPU] retrieval for 'lab-lipid':");
    for r in &results {
        eprintln!("    {:<12} {:.3}  ({})", r.doc_id, r.score, r.caption);
    }
    let lab_cbc = results
        .iter()
        .find(|r| r.doc_id == "lab-cbc")
        .unwrap()
        .score;
    let xray = results
        .iter()
        .find(|r| r.doc_id == "xray-chest")
        .unwrap()
        .score;
    assert!(
        lab_cbc >= xray,
        "lab-cbc ({lab_cbc}) should be >= xray ({xray})"
    );
}
