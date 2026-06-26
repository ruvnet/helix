//! # helix-embed — ADR-027: learned MiniLM text embeddings (local GPU)
//!
//! The real semantic encoder behind `helix-retrieval`'s `Embedder` seam
//! (ADR-023). Default backend is **all-MiniLM-L6-v2** (384-dim, the ruvnet stack
//! standard) served on the local GPU — no health text leaves the device
//! (ADR-013). Embeddings drive *recall*; the grounding gate (ADR-005) keeps
//! assertion strict.
//!
//! `TextEmbedder` is a trait (deterministic stub in tests, GPU backend in prod).
//! [`LearnedEmbedder`] adapts it to `helix_retrieval::Embedder`, degrading to an
//! empty vector on backend failure rather than erroring the whole answer.

use thiserror::Error;

use helix_provenance::ProvRecord;

/// Produces a fixed-dimension embedding for a piece of text.
pub trait TextEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EmbedError {
    #[error("embedding backend transport error: {0}")]
    Transport(String),
    #[error("embedding backend returned an unexpected response: {0}")]
    BadResponse(String),
}

/// Cosine similarity between two equal-length vectors. Returns 0 for empty or
/// mismatched inputs (safe-degradation contract).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut na = 0.0;
    let mut nb = 0.0;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Compact, provenance-preserving rendering of a record for embedding. Never adds
/// anything the record doesn't already hold.
pub fn record_text(r: &ProvRecord) -> String {
    format!("{} {} {}", r.concept, r.value, r.unit)
}

/// Local-GPU embeddings backend (OpenAI-/ollama-style embeddings endpoint).
/// Default: all-MiniLM-L6-v2 served by ollama on the GPU.
#[derive(Debug, Clone)]
pub struct LocalEmbedder {
    /// e.g. "http://127.0.0.1:11434/api/embeddings"
    pub url: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl Default for LocalEmbedder {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:11434/api/embeddings".to_string(),
            model: "all-minilm".to_string(),
            timeout_secs: 30,
        }
    }
}

impl TextEmbedder for LocalEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let body = serde_json::json!({ "model": self.model, "prompt": text });
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .build();
        let resp = agent
            .post(&self.url)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(|e| EmbedError::Transport(e.to_string()))?;
        let text = resp
            .into_string()
            .map_err(|e| EmbedError::Transport(e.to_string()))?;
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| EmbedError::BadResponse(e.to_string()))?;
        let arr = v["embedding"]
            .as_array()
            .ok_or_else(|| EmbedError::BadResponse(text.clone()))?;
        Ok(arr
            .iter()
            .filter_map(|x| x.as_f64().map(|f| f as f32))
            .collect())
    }
}

/// Adapts a [`TextEmbedder`] to `helix_retrieval::Embedder`. On backend failure
/// it returns an empty vector (no spurious matches) so one embedding error never
/// fails the whole answer.
pub struct LearnedEmbedder<E: TextEmbedder> {
    pub inner: E,
}

impl<E: TextEmbedder> LearnedEmbedder<E> {
    pub fn new(inner: E) -> Self {
        Self { inner }
    }
}

impl<E: TextEmbedder> helix_retrieval::Embedder for LearnedEmbedder<E> {
    fn embed_query(&self, query: &str) -> Vec<f32> {
        self.inner.embed(query).unwrap_or_default()
    }
    fn embed_record(&self, record: &ProvRecord) -> Vec<f32> {
        self.inner.embed(&record_text(record)).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_provenance::{Confidence, MeasurementMethod, RecordId, ReferenceRange};
    use helix_retrieval::Embedder;

    struct Stub(Vec<f32>);
    impl TextEmbedder for Stub {
        fn embed(&self, _: &str) -> Result<Vec<f32>, EmbedError> {
            Ok(self.0.clone())
        }
    }
    struct ErrStub;
    impl TextEmbedder for ErrStub {
        fn embed(&self, _: &str) -> Result<Vec<f32>, EmbedError> {
            Err(EmbedError::Transport("down".into()))
        }
    }

    fn rec() -> ProvRecord {
        ProvRecord {
            id: RecordId::from("r"),
            source: "Quest".into(),
            measured_at: 1000,
            method: MeasurementMethod::LabFeed,
            code: Some("2276-4".into()),
            concept: "Ferritin".into(),
            value: 28.0,
            unit: "ng/mL".into(),
            reference_range: Some(ReferenceRange::new(Some(30.0), Some(400.0))),
            confidence: Confidence::FULL,
        }
    }

    #[test]
    fn cosine_basics() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert_eq!(cosine(&[], &[]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0); // shape mismatch → 0
    }

    #[test]
    fn record_text_is_compact_and_provenance_safe() {
        assert_eq!(record_text(&rec()), "Ferritin 28 ng/mL");
    }

    #[test]
    fn learned_embedder_adapts_to_retrieval() {
        let e = LearnedEmbedder::new(Stub(vec![0.1, 0.2, 0.3]));
        assert_eq!(e.embed_query("why am I tired?"), vec![0.1, 0.2, 0.3]);
        assert_eq!(e.embed_record(&rec()), vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn backend_failure_degrades_to_empty() {
        let e = LearnedEmbedder::new(ErrStub);
        assert!(e.embed_query("x").is_empty()); // safe: no spurious match
        assert!(e.embed_record(&rec()).is_empty());
    }
}
