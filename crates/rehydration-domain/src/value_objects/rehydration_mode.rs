use crate::DomainError;

/// Rehydration mode controlling how context is rendered under budget pressure.
///
/// - `Auto`: let the kernel choose based on token pressure heuristic
/// - `ResumeFocused`: prune non-causal branches, keep causal spine only
/// - `ReasonPreserving`: keep all explanatory metadata (default for generous budgets)
/// - `TemporalDelta`: reserved for future use
/// - `GlobalSummary`: reserved for future use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RehydrationMode {
    #[default]
    Auto,
    ResumeFocused,
    ReasonPreserving,
    TemporalDelta,
    GlobalSummary,
}

impl RehydrationMode {
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value.trim() {
            "auto" => Ok(Self::Auto),
            "resume_focused" => Ok(Self::ResumeFocused),
            "reason_preserving" => Ok(Self::ReasonPreserving),
            "temporal_delta" => Ok(Self::TemporalDelta),
            "global_summary" => Ok(Self::GlobalSummary),
            other => Err(DomainError::InvalidState(format!(
                "invalid rehydration mode `{other}`"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::ResumeFocused => "resume_focused",
            Self::ReasonPreserving => "reason_preserving",
            Self::TemporalDelta => "temporal_delta",
            Self::GlobalSummary => "global_summary",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Auto,
            Self::ResumeFocused,
            Self::ReasonPreserving,
            Self::TemporalDelta,
            Self::GlobalSummary,
        ]
    }

    /// Returns true for modes that are reserved but not yet implemented.
    pub fn is_placeholder(&self) -> bool {
        matches!(self, Self::TemporalDelta | Self::GlobalSummary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        for mode in RehydrationMode::all() {
            let parsed = RehydrationMode::parse(mode.as_str()).expect("valid mode");
            assert_eq!(&parsed, mode);
        }
    }

    #[test]
    fn parse_invalid_returns_error() {
        assert!(RehydrationMode::parse("invalid").is_err());
    }

    #[test]
    fn default_is_auto() {
        assert_eq!(RehydrationMode::default(), RehydrationMode::Auto);
    }

    #[test]
    fn placeholder_modes() {
        assert!(!RehydrationMode::Auto.is_placeholder());
        assert!(!RehydrationMode::ResumeFocused.is_placeholder());
        assert!(!RehydrationMode::ReasonPreserving.is_placeholder());
        assert!(RehydrationMode::TemporalDelta.is_placeholder());
        assert!(RehydrationMode::GlobalSummary.is_placeholder());
    }
}
