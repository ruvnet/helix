//! Integration test against the REAL local-GPU MiniLM embedder (ollama all-minilm).
//! Ignored by default; run explicitly:  cargo test -p helix-embed -- --ignored
//!
//! Validates real semantic recall on the GPU: a fatigue query embeds closer to a
//! fatigue-related sentence than to an unrelated one.

use helix_embed::{cosine, LocalEmbedder, TextEmbedder};

#[test]
#[ignore = "requires local ollama all-minilm GPU endpoint"]
fn semantic_similarity_on_gpu() {
    let e = LocalEmbedder::default();
    let q = e
        .embed("why am I tired and low on energy in the afternoons")
        .unwrap();
    let related = e
        .embed("fatigue and low iron / ferritin causing exhaustion")
        .unwrap();
    let unrelated = e
        .embed("the potassium level in a banana smoothie recipe")
        .unwrap();

    assert_eq!(q.len(), 384, "all-MiniLM-L6-v2 is 384-dim");
    let s_rel = cosine(&q, &related);
    let s_unrel = cosine(&q, &unrelated);
    eprintln!("\n[helix-embed GPU] sim(related)={s_rel:.3}  sim(unrelated)={s_unrel:.3}\n");
    assert!(
        s_rel > s_unrel,
        "fatigue query should be closer to fatigue text ({s_rel} vs {s_unrel})"
    );
}
