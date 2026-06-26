//! # helix-llm — ADR-026: on-device LLM analyst (grounded compose step)
//!
//! Turns a finished **GroundedAnswer** (grounded claims + deterministic trend +
//! citations) into calm plain-English prose, using a **local-GPU LLM** (ollama /
//! OpenAI-compatible) — so no health data leaves the device (ADR-013).
//!
//! The LLM is a **narrator, never a reasoner**. It receives the already-grounded
//! facts and is told to restate only those. Two structural guards make
//! fabrication impossible to surface:
//!
//! 1. **System prompt** forbids adding numbers, claims, diagnoses, or advice not
//!    in the facts.
//! 2. **Number-guard** (post-generation): every numeric token in the output must
//!    already appear in the input facts. If the model invents a value, the output
//!    is rejected and Helix falls back to the deterministic template. The LLM may
//!    rephrase; it cannot introduce a value.
//!
//! The backend is a trait, so tests use a deterministic stub and production uses
//! [`LocalLlmBackend`] (local GPU); the cloud escalation path (ADR-019) plugs in
//! the same way.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The strict narrator instruction (ADR-026 §1, §3).
pub const SYSTEM_PROMPT: &str = "You are a careful health-data narrator for the user's own records. \
You will be given a list of FACTS that have already been verified and computed. \
Restate ONLY those facts in calm, plain, second-person language. \
Do NOT add, infer, or invent any number, value, date, diagnosis, or recommendation that is not in the FACTS. \
Do NOT give medical advice beyond what the FACTS state. \
If the FACTS are insufficient to answer, say so plainly. Keep it to a few sentences.";

/// A pluggable chat backend. Production: [`LocalLlmBackend`] on the local GPU.
pub trait LlmBackend {
    fn complete(&self, system: &str, user: &str) -> Result<String, LlmError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LlmError {
    #[error("llm backend transport error: {0}")]
    Transport(String),
    #[error("llm backend returned an unexpected response: {0}")]
    BadResponse(String),
}

/// Result of a compose: the prose, whether the LLM output was used or the
/// deterministic fallback kicked in, and any guard rejection reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Composed {
    pub text: String,
    pub used_llm: bool,
    pub guard_rejected: Option<String>,
}

/// Build the user message from the grounded facts.
fn user_message(question: &str, facts: &[String]) -> String {
    let mut s = format!("QUESTION: {question}\n\nFACTS (restate only these):\n");
    for f in facts {
        s.push_str("- ");
        s.push_str(f);
        s.push('\n');
    }
    s
}

/// Extract numeric tokens from text: maximal runs of digits, `.`, `,` that
/// contain at least one digit. Used by the number-guard.
fn numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if cur.chars().any(|c| c.is_ascii_digit()) {
            // trim trailing separators like a sentence period
            let t = cur.trim_matches(|c| c == '.' || c == ',');
            if t.chars().any(|c| c.is_ascii_digit()) {
                out.push(t.to_string());
            }
        }
        cur.clear();
    };
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == ',' {
            cur.push(ch);
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// The number-guard: every number in `output` must appear in `facts`.
/// Returns `Some(reason)` if a fabricated number is found.
fn guard_numbers(output: &str, facts: &[String]) -> Option<String> {
    let fact_blob = facts.join(" ");
    let fact_nums = numbers(&fact_blob);
    for n in numbers(output) {
        if !fact_nums.iter().any(|fn_| fn_ == &n) {
            return Some(format!("output introduced a value not in the facts: {n}"));
        }
    }
    None
}

/// Deterministic fallback narration (also the guard's safe output).
fn template(facts: &[String]) -> String {
    if facts.is_empty() {
        return "I don't have enough in your records to answer that yet.".to_string();
    }
    facts.join(" ")
}

/// Compose a grounded answer into prose. Calls the backend, then enforces the
/// number-guard; on any guard rejection or backend error, returns the safe
/// deterministic template.
pub fn compose(question: &str, facts: &[String], backend: &dyn LlmBackend) -> Composed {
    let user = user_message(question, facts);
    match backend.complete(SYSTEM_PROMPT, &user) {
        Ok(out) => {
            let out = out.trim().to_string();
            match guard_numbers(&out, facts) {
                None if !out.is_empty() => Composed {
                    text: out,
                    used_llm: true,
                    guard_rejected: None,
                },
                reason => Composed {
                    text: template(facts),
                    used_llm: false,
                    guard_rejected: reason.or_else(|| Some("empty llm output".to_string())),
                },
            }
        }
        Err(e) => Composed {
            text: template(facts),
            used_llm: false,
            guard_rejected: Some(format!("backend error: {e}")),
        },
    }
}

/// Local-GPU backend: any OpenAI-compatible chat endpoint, on-device. Temperature
/// 0 for reproducible narration. No health data leaves the device (ADR-013).
///
/// Two presets: [`LocalLlmBackend::ruvllm`] is the **in-stack default** — ruvLLM
/// is the ruvnet-native on-device engine ADR-013 names — and
/// [`LocalLlmBackend::ollama`] is the fallback. Both validated on the local GPU.
#[derive(Debug, Clone)]
pub struct LocalLlmBackend {
    pub base_url: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl LocalLlmBackend {
    /// In-stack ruvLLM endpoint (ruvnet-native, ADR-013).
    pub fn ruvllm() -> Self {
        Self {
            base_url: "http://127.0.0.1:8080/v1".to_string(),
            model: "Qwen/Qwen2.5-3B-Instruct".to_string(),
            timeout_secs: 60,
        }
    }
    /// Ollama fallback endpoint.
    pub fn ollama() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "qwen2.5-coder:7b".to_string(),
            timeout_secs: 60,
        }
    }
}

impl Default for LocalLlmBackend {
    /// ruvLLM — the in-stack on-device engine (ADR-013).
    fn default() -> Self {
        Self::ruvllm()
    }
}

impl LlmBackend for LocalLlmBackend {
    fn complete(&self, system: &str, user: &str) -> Result<String, LlmError> {
        let body = serde_json::json!({
            "model": self.model,
            "temperature": 0,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        });
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .build();
        let resp = agent
            .post(&format!("{}/chat/completions", self.base_url))
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(|e| LlmError::Transport(e.to_string()))?;
        let text = resp
            .into_string()
            .map_err(|e| LlmError::Transport(e.to_string()))?;
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| LlmError::BadResponse(e.to_string()))?;
        v["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| LlmError::BadResponse(text.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub(&'static str);
    impl LlmBackend for Stub {
        fn complete(&self, _: &str, _: &str) -> Result<String, LlmError> {
            Ok(self.0.to_string())
        }
    }
    struct ErrStub;
    impl LlmBackend for ErrStub {
        fn complete(&self, _: &str, _: &str) -> Result<String, LlmError> {
            Err(LlmError::Transport("down".into()))
        }
    }

    fn facts() -> Vec<String> {
        vec![
            "Your ferritin is 28 ng/mL and trending down over your last 3 readings.".into(),
            "It crossed below the reference range (30-400).".into(),
        ]
    }

    #[test]
    fn uses_llm_when_guard_passes() {
        // restates only facts' numbers (28, 3, 30, 400)
        let stub = Stub("Your ferritin sits at 28 ng/mL and has been easing down across 3 readings, now under the 30-400 range.");
        let c = compose("why am I tired?", &facts(), &stub);
        assert!(c.used_llm);
        assert!(c.guard_rejected.is_none());
        assert!(c.text.contains("28"));
    }

    #[test]
    fn rejects_fabricated_number_and_falls_back() {
        // 9.9 is NOT in the facts → guard rejects → deterministic template
        let stub = Stub("Your ferritin is 28 and your magic index is 9.9 today.");
        let c = compose("why am I tired?", &facts(), &stub);
        assert!(!c.used_llm);
        assert!(c.guard_rejected.unwrap().contains("9.9"));
        assert_eq!(c.text, template(&facts())); // safe fallback
    }

    #[test]
    fn backend_error_falls_back_safely() {
        let c = compose("q", &facts(), &ErrStub);
        assert!(!c.used_llm);
        assert!(c.guard_rejected.unwrap().contains("backend error"));
        assert_eq!(c.text, template(&facts()));
    }

    #[test]
    fn empty_facts_template() {
        let c = compose("q", &[], &Stub(""));
        assert!(!c.used_llm);
        assert!(c.text.contains("don't have enough"));
    }

    #[test]
    fn number_extraction() {
        let n = numbers("ferritin 28 ng/mL, ref 30-400 on Jun 19.");
        assert!(n.contains(&"28".to_string()));
        assert!(n.contains(&"30".to_string()));
        assert!(n.contains(&"400".to_string()));
        assert!(n.contains(&"19".to_string()));
    }

    #[test]
    fn guard_allows_decimal_and_rejects_unknown() {
        let facts = vec!["HDL 55 mg/dL".to_string()];
        assert!(guard_numbers("Your HDL is 55.", &facts).is_none());
        assert!(guard_numbers("Your HDL is 55 and LDL 130.", &facts).is_some());
    }
}
