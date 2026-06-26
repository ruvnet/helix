//! # helix-visual — ADR-025: visual RAG over medical documents & images
//!
//! A ColPali-faithful, pure-Rust visual retriever for the kind of artifacts OCR
//! can't handle — lab reports with tables, imaging reports, ECG strips, scanned
//! forms, photos. Adapter for [ruvnet/rupixel](https://github.com/ruvnet/rupixel)
//! ("retrieve over what a page *looks like*"): the document is split into a grid
//! of **tiles**, each tile gets a perceptual descriptor, and retrieval scores a
//! query by **MaxSim** late interaction (for each query tile, its best match in
//! the document, averaged) — so layout and local structure survive.
//!
//! The hard line (ADR-010/023): **visual retrieval finds similar-looking
//! documents — it never interprets or diagnoses an image.** A result surfaces a
//! document for a human and the grounding gate; it asserts nothing about clinical
//! content from pixels.
//!
//! The perceptual descriptor here is the dependency-light, deterministic,
//! benchmarkable reference; a learned ColPali/MiniLM encoder (rupixel on RuVector,
//! WASM at the edge) plugs in behind [`VisualEmbedder`].

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A grayscale image (row-major luminance, 0..=255).
#[derive(Debug, Clone, PartialEq)]
pub struct Gray {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Gray {
    pub fn new(w: u32, h: u32, px: Vec<u8>) -> Result<Self, VisualError> {
        if (w as usize) * (h as usize) != px.len() || w == 0 || h == 0 {
            return Err(VisualError::BadImage);
        }
        Ok(Self { w, h, px })
    }

    /// Build from interleaved RGBA bytes (e.g. a decoded PNG) via Rec.601 luma.
    pub fn from_rgba(w: u32, h: u32, rgba: &[u8]) -> Result<Self, VisualError> {
        if rgba.len() != (w as usize) * (h as usize) * 4 {
            return Err(VisualError::BadImage);
        }
        let px = rgba
            .chunks_exact(4)
            .map(|p| {
                (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32).round() as u8
            })
            .collect();
        Self::new(w, h, px)
    }
}

/// One document's multi-vector embedding: one descriptor per tile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocEmbedding {
    /// `grid*grid` tile descriptors, each `cells*cells` long and L2-normalized.
    pub tiles: Vec<Vec<f32>>,
    pub grid: u32,
    pub cells: u32,
}

/// Pluggable visual embedder. The in-crate [`PerceptualEmbedder`] is the
/// reference; a learned ColPali/MiniLM encoder plugs in here.
pub trait VisualEmbedder {
    fn embed(&self, img: &Gray) -> Result<DocEmbedding, VisualError>;
}

/// Configuration for the reference perceptual embedder.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PerceptualEmbedder {
    /// Tiles per side (e.g. 4 → 16 tiles, ColPali-style patches).
    pub grid: u32,
    /// Descriptor cells per tile side (e.g. 6 → 36-dim descriptor).
    pub cells: u32,
}

impl Default for PerceptualEmbedder {
    fn default() -> Self {
        Self { grid: 4, cells: 6 }
    }
}

impl VisualEmbedder for PerceptualEmbedder {
    fn embed(&self, img: &Gray) -> Result<DocEmbedding, VisualError> {
        if self.grid == 0 || self.cells == 0 {
            return Err(VisualError::BadConfig);
        }
        let (g, c) = (self.grid, self.cells);
        let mut tiles = Vec::with_capacity((g * g) as usize);
        for ty in 0..g {
            for tx in 0..g {
                // Tile pixel bounds.
                let x0 = (tx * img.w) / g;
                let x1 = ((tx + 1) * img.w) / g;
                let y0 = (ty * img.h) / g;
                let y1 = ((ty + 1) * img.h) / g;
                tiles.push(tile_descriptor(img, x0, x1, y0, y1, c));
            }
        }
        Ok(DocEmbedding {
            tiles,
            grid: g,
            cells: c,
        })
    }
}

/// Downscale a tile region to `cells*cells` mean-luminance, mean-subtract for
/// brightness invariance, then L2-normalize. A flat region → all zeros.
fn tile_descriptor(img: &Gray, x0: u32, x1: u32, y0: u32, y1: u32, cells: u32) -> Vec<f32> {
    let n = (cells * cells) as usize;
    let mut acc = vec![0f32; n];
    let mut cnt = vec![0u32; n];
    let (tw, th) = ((x1 - x0).max(1), (y1 - y0).max(1));
    // Tile bounds are derived from image dimensions, so x<w and y<h hold here —
    // no per-pixel bounds clamp needed on the hot path.
    let y_end = y1.max(y0 + 1);
    let x_end = x1.max(x0 + 1);
    for y in y0..y_end {
        let cy = (((y - y0) * cells) / th).min(cells - 1);
        let row = (y as usize) * (img.w as usize);
        for x in x0..x_end {
            let cx = (((x - x0) * cells) / tw).min(cells - 1);
            let idx = (cy * cells + cx) as usize;
            acc[idx] += img.px[row + x as usize] as f32;
            cnt[idx] += 1;
        }
    }
    let mut v: Vec<f32> = acc
        .iter()
        .zip(&cnt)
        .map(|(a, &c)| if c > 0 { a / c as f32 } else { 0.0 })
        .collect();
    // Mean-subtract (contrast, not brightness).
    let mean = v.iter().sum::<f32>() / n as f32;
    for x in &mut v {
        *x -= mean;
    }
    // L2-normalize.
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-6 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

/// Cosine of two L2-normalized vectors (== dot product).
#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// ColPali MaxSim late interaction: for each query tile, its best cosine over the
/// document's tiles, averaged. Clamped to `[0,1]`. Requires matching descriptor
/// shape (same grid/cells).
pub fn maxsim(query: &DocEmbedding, doc: &DocEmbedding) -> Result<f32, VisualError> {
    if query.cells != doc.cells {
        return Err(VisualError::ShapeMismatch);
    }
    if query.tiles.is_empty() || doc.tiles.is_empty() {
        return Ok(0.0);
    }
    let mut sum = 0.0;
    for q in &query.tiles {
        let mut best = f32::NEG_INFINITY;
        for d in &doc.tiles {
            let s = dot(q, d);
            if s > best {
                best = s;
            }
        }
        sum += best.clamp(0.0, 1.0);
    }
    Ok(sum / query.tiles.len() as f32)
}

/// A visual retrieval result — a *similar-looking* document, never an interpretation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualMatch {
    pub doc_id: String,
    pub score: f32,
    /// Constant reminder of what this is (and isn't).
    pub note: String,
}

/// An index of document embeddings. RuVector HNSW/IVF caps the candidate set
/// before MaxSim in production; here we score the corpus directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisualIndex {
    docs: Vec<(String, DocEmbedding)>,
}

impl VisualIndex {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, id: impl Into<String>, emb: DocEmbedding) {
        self.docs.push((id.into(), emb));
    }
    pub fn len(&self) -> usize {
        self.docs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Rank documents by MaxSim against `query`, return the top `k`.
    pub fn retrieve(
        &self,
        query: &DocEmbedding,
        k: usize,
    ) -> Result<Vec<VisualMatch>, VisualError> {
        let mut scored: Vec<VisualMatch> = self
            .docs
            .iter()
            .map(|(id, emb)| {
                maxsim(query, emb).map(|score| VisualMatch {
                    doc_id: id.clone(),
                    score,
                    note:
                        "visual similarity — a document that looks like this, not an interpretation"
                            .to_string(),
                })
            })
            .collect::<Result<_, _>>()?;
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        scored.truncate(k);
        Ok(scored)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VisualError {
    #[error("image dimensions do not match the pixel buffer")]
    BadImage,
    #[error("grid and cells must be >= 1")]
    BadConfig,
    #[error("query and document descriptors have different shapes")]
    ShapeMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A "document": white page with evenly spaced dark horizontal text bands.
    fn doc_image(w: u32, h: u32, phase: u8) -> Gray {
        let mut px = vec![245u8; (w * h) as usize];
        for y in 0..h {
            if ((y as u8).wrapping_add(phase)) % 12 < 3 {
                for x in 0..w {
                    px[(y * w + x) as usize] = 40;
                }
            }
        }
        Gray::new(w, h, px).unwrap()
    }

    /// An "x-ray": smooth radial gradient (no text structure).
    fn xray_image(w: u32, h: u32) -> Gray {
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let maxd = (cx * cx + cy * cy).sqrt();
        let mut px = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                px[(y * w + x) as usize] = (255.0 * (1.0 - d / maxd)).clamp(0.0, 255.0) as u8;
            }
        }
        Gray::new(w, h, px).unwrap()
    }

    fn emb(img: &Gray) -> DocEmbedding {
        PerceptualEmbedder::default().embed(img).unwrap()
    }

    #[test]
    fn identical_image_scores_near_one() {
        let d = doc_image(120, 160, 0);
        let e = emb(&d);
        let s = maxsim(&e, &e).unwrap();
        assert!(s > 0.99, "identical maxsim {s}");
    }

    #[test]
    fn retrieves_same_class_over_different_class() {
        let mut idx = VisualIndex::new();
        idx.add("lab-A", emb(&doc_image(120, 160, 0)));
        idx.add("lab-B", emb(&doc_image(120, 160, 1)));
        idx.add("xray", emb(&xray_image(120, 160)));

        // Query: a document-like page → a lab report must rank above the x-ray.
        let q = emb(&doc_image(120, 160, 2));
        let out = idx.retrieve(&q, 3).unwrap();
        assert_eq!(out.len(), 3);
        assert!(out[0].doc_id.starts_with("lab"), "top: {}", out[0].doc_id);
        let xray = out.iter().find(|m| m.doc_id == "xray").unwrap();
        assert!(
            out[0].score > xray.score,
            "doc {} vs xray {}",
            out[0].score,
            xray.score
        );
        assert!(out[0].note.contains("not an interpretation"));
    }

    #[test]
    fn xray_query_retrieves_xray() {
        let mut idx = VisualIndex::new();
        idx.add("lab", emb(&doc_image(120, 160, 0)));
        idx.add("xray", emb(&xray_image(120, 160)));
        let q = emb(&xray_image(120, 160));
        let out = idx.retrieve(&q, 1).unwrap();
        assert_eq!(out[0].doc_id, "xray");
    }

    #[test]
    fn from_rgba_matches_luma() {
        // a 2x1 image: black, white
        let rgba = [0, 0, 0, 255, 255, 255, 255, 255];
        let g = Gray::from_rgba(2, 1, &rgba).unwrap();
        assert_eq!(g.px[0], 0);
        assert_eq!(g.px[1], 255);
    }

    #[test]
    fn bad_inputs_rejected() {
        assert_eq!(Gray::new(2, 2, vec![0, 0, 0]), Err(VisualError::BadImage));
        let bad = PerceptualEmbedder { grid: 0, cells: 4 };
        assert_eq!(
            bad.embed(&doc_image(10, 10, 0)),
            Err(VisualError::BadConfig)
        );
    }

    #[test]
    fn descriptor_is_normalized() {
        let e = emb(&doc_image(120, 160, 0));
        assert_eq!(e.tiles.len(), 16); // grid 4x4
        for t in &e.tiles {
            let norm = t.iter().map(|x| x * x).sum::<f32>().sqrt();
            // either unit-normalized (~1) or a flat/zero tile (~0)
            let normalized = (0.999..=1.0001).contains(&norm);
            assert!(normalized || norm < 1e-5, "norm {norm}");
        }
    }
}
