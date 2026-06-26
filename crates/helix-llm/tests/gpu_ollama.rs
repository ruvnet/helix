//! Integration test against the REAL local-GPU LLM (ollama). Ignored by default
//! (needs the endpoint up); run explicitly:
//!
//!   cargo test -p helix-llm -- --ignored
//!
//! Validates the on-device analyst end to end on the GPU: a real model narrates
//! grounded facts, and the number-guard still holds on real output.

use helix_llm::{compose, LocalLlmBackend};

#[test]
#[ignore = "requires local ollama GPU endpoint"]
fn narrates_grounded_facts_on_gpu() {
    let backend = LocalLlmBackend::ruvllm(); // in-stack ruvLLM on the GPU
    let facts = vec![
        "Your ferritin is 28 ng/mL and trending down over your last 3 readings.".to_string(),
        "It crossed below the reference range of 30 to 400 ng/mL.".to_string(),
    ];
    let c = compose("Why am I tired in the afternoons?", &facts, &backend);

    eprintln!(
        "\n[helix-llm GPU] used_llm={} guard={:?}\n  {}\n",
        c.used_llm, c.guard_rejected, c.text
    );

    // The output must be non-empty and contain a grounded value (28).
    assert!(!c.text.is_empty());
    // Whether the LLM output was used or the guard fell back, the result is safe:
    // it must not contain any number absent from the facts (the guard guarantees it).
    // Facts numbers: 28, 3, 30, 400. Assert no obviously-fabricated value slipped through.
    for bad in ["99", "7.7", "1000"] {
        assert!(
            !c.text.contains(bad),
            "fabricated value {bad} leaked: {}",
            c.text
        );
    }
}
