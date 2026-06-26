//! # helix-vision — ADR-028: learned visual encoder (local GPU)
//!
//! A learned visual encoder *by composition*: a GPU vision model constrained to
//! **layout-only** description, embedded by the MiniLM encoder (ADR-027).
//! Image → layout caption → vector. It encodes **appearance, never clinical
//! interpretation** (ADR-025/010): the vision prompt forbids values/findings/
//! diagnoses, and a **value-guard** rejects any caption containing a digit (a
//! layout description has no numbers).
//!
//! On-device: both the vision model and the embedder run on the local GPU; the
//! image never leaves the device (ADR-013). The vector ranks *which documents
//! look alike* (recall, ADR-025) — it asserts nothing clinical.
//!
//! `VisionCaptioner` and the embedder are traits, so a true in-process
//! CLIP/ColPali encoder (candle, GPU) drops in behind the same contract.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use helix_embed::{cosine, TextEmbedder};

/// The layout-only vision prompt (ADR-028 §1). Asks for type/layout, forbids content.
pub const LAYOUT_PROMPT: &str =
    "In 6 words or fewer, name only the document TYPE and visual layout \
(for example: 'lab report table', 'x-ray image', 'line chart', 'ECG strip', 'skin photo'). \
Do NOT mention any medical value, number, finding, or diagnosis.";

/// A neutral fallback when the caption is rejected by the value-guard.
pub const NEUTRAL_TOKEN: &str = "medical document";

/// Produces a layout-only caption for an image (PNG bytes). Production: a GPU
/// vision model; tests: a deterministic stub.
pub trait VisionCaptioner {
    fn caption_layout(&self, png: &[u8]) -> Result<String, VisionError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VisionError {
    #[error("vision backend transport error: {0}")]
    Transport(String),
    #[error("vision backend returned an unexpected response: {0}")]
    BadResponse(String),
}

/// True if `caption` is safe (layout-only): no digit means it did not read a value.
pub fn caption_is_layout_only(caption: &str) -> bool {
    !caption.chars().any(|c| c.is_ascii_digit())
}

/// Minimal, dependency-free base64 (standard alphabet) for embedding image bytes
/// in the vision request.
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | b2 as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// The learned visual encoder: caption (layout-only, guarded) → embed.
pub struct LayoutVisualEncoder<C: VisionCaptioner, E: TextEmbedder> {
    pub captioner: C,
    pub embedder: E,
}

/// What an encode produced — the (guarded) caption and its embedding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualEncoding {
    pub caption: String,
    pub embedding: Vec<f32>,
    /// True if the value-guard rejected the model caption and the neutral token was used.
    pub guarded: bool,
}

impl<C: VisionCaptioner, E: TextEmbedder> LayoutVisualEncoder<C, E> {
    pub fn new(captioner: C, embedder: E) -> Self {
        Self {
            captioner,
            embedder,
        }
    }

    /// Encode an image: caption it layout-only, enforce the value-guard, embed.
    pub fn encode(&self, png: &[u8]) -> Result<VisualEncoding, VisionError> {
        let raw = self.captioner.caption_layout(png)?.trim().to_string();
        let (caption, guarded) = if caption_is_layout_only(&raw) && !raw.is_empty() {
            (raw, false)
        } else {
            (NEUTRAL_TOKEN.to_string(), true)
        };
        let embedding = self
            .embedder
            .embed(&caption)
            .map_err(|e| VisionError::Transport(e.to_string()))?;
        Ok(VisualEncoding {
            caption,
            embedding,
            guarded,
        })
    }
}

/// A visual retrieval match (a similar-LOOKING document, never an interpretation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualMatch {
    pub doc_id: String,
    pub score: f32,
    pub caption: String,
}

/// Rank a corpus of encodings against a query encoding by cosine similarity.
pub fn rank(
    query: &VisualEncoding,
    corpus: &[(String, VisualEncoding)],
    k: usize,
) -> Vec<VisualMatch> {
    let mut out: Vec<VisualMatch> = corpus
        .iter()
        .map(|(id, enc)| VisualMatch {
            doc_id: id.clone(),
            score: cosine(&query.embedding, &enc.embedding),
            caption: enc.caption.clone(),
        })
        .collect();
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    out.truncate(k);
    out
}

/// GPU vision backend: an ollama vision model (e.g. `moondream`) on the local GPU.
#[derive(Debug, Clone)]
pub struct OllamaVision {
    pub url: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl Default for OllamaVision {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:11434/api/generate".to_string(),
            model: "moondream".to_string(),
            timeout_secs: 60,
        }
    }
}

impl VisionCaptioner for OllamaVision {
    fn caption_layout(&self, png: &[u8]) -> Result<String, VisionError> {
        let body = serde_json::json!({
            "model": self.model,
            "prompt": LAYOUT_PROMPT,
            "images": [base64(png)],
            "stream": false,
            "options": { "temperature": 0 },
        });
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .build();
        let resp = agent
            .post(&self.url)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(|e| VisionError::Transport(e.to_string()))?;
        let text = resp
            .into_string()
            .map_err(|e| VisionError::Transport(e.to_string()))?;
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| VisionError::BadResponse(e.to_string()))?;
        v["response"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| VisionError::BadResponse(text.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_embed::EmbedError;

    struct CapStub(&'static str);
    impl VisionCaptioner for CapStub {
        fn caption_layout(&self, _: &[u8]) -> Result<String, VisionError> {
            Ok(self.0.to_string())
        }
    }
    // Deterministic embedder: maps a caption to a vector by first-char buckets,
    // so same-type captions embed identically (enough to test ranking).
    struct EmbStub;
    impl TextEmbedder for EmbStub {
        fn embed(&self, t: &str) -> Result<Vec<f32>, EmbedError> {
            let mut v = vec![0f32; 26];
            for c in t.to_lowercase().chars() {
                if c.is_ascii_alphabetic() {
                    v[(c as u8 - b'a') as usize] += 1.0;
                }
            }
            Ok(v)
        }
    }

    #[test]
    fn base64_known_value() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }

    #[test]
    fn value_guard_blocks_numbers() {
        assert!(caption_is_layout_only("lab report table"));
        assert!(!caption_is_layout_only("ferritin 28 ng/mL")); // read a value → blocked
    }

    #[test]
    fn encode_uses_neutral_token_when_guard_trips() {
        let enc = LayoutVisualEncoder::new(CapStub("LDL 110 mg/dL elevated"), EmbStub);
        let out = enc.encode(b"png").unwrap();
        assert!(out.guarded);
        assert_eq!(out.caption, NEUTRAL_TOKEN);
    }

    #[test]
    fn ranks_same_layout_type_first() {
        let enc = |c: &'static str| {
            LayoutVisualEncoder::new(CapStub(c), EmbStub)
                .encode(b"x")
                .unwrap()
        };
        let lab_a = enc("lab report table");
        let corpus = vec![
            ("lab-b".to_string(), enc("lab report table")), // same type
            ("xray".to_string(), enc("x-ray image")),       // different
        ];
        let out = rank(&lab_a, &corpus, 2);
        assert_eq!(out[0].doc_id, "lab-b");
        assert!(out[0].score > out[1].score);
    }

    #[test]
    fn empty_caption_falls_back() {
        let enc = LayoutVisualEncoder::new(CapStub("   "), EmbStub)
            .encode(b"x")
            .unwrap();
        assert!(enc.guarded);
        assert_eq!(enc.caption, NEUTRAL_TOKEN);
    }
}
