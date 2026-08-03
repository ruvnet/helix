use serde::{Deserialize, Serialize};

/// Independent staged controls for the physically separate research build.
/// Every capability is disabled by default; these flags never replace the
/// build, trust, consent, or one-way observation boundaries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeuroSleepResearchFlags {
    #[serde(default)]
    pub import_v1: bool,
    #[serde(default)]
    pub shadow_v1: bool,
    #[serde(default)]
    pub research_ui_v1: bool,
    #[serde(default)]
    pub rvf_v1: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_research_capability_defaults_off() {
        let flags: NeuroSleepResearchFlags = serde_json::from_str("{}").unwrap();
        assert_eq!(flags, NeuroSleepResearchFlags::default());
        assert!(!flags.import_v1);
        assert!(!flags.shadow_v1);
        assert!(!flags.research_ui_v1);
        assert!(!flags.rvf_v1);
        assert!(serde_json::from_str::<NeuroSleepResearchFlags>(r#"{"diagnostic":true}"#).is_err());
    }
}
