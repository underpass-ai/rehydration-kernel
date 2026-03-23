use rehydration_domain::TokenEstimator;

use crate::queries::ContextRenderOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncationMetadata {
    pub budget_requested: u32,
    pub budget_used: u32,
    pub total_before_truncation: u32,
    pub sections_kept: u32,
    pub sections_dropped: u32,
    pub token_estimator: String,
}

/// Limits sections to fit within a token budget.
///
/// Returns the surviving sections and truncation metadata when a budget is set.
/// When no budget is configured, returns all sections with no metadata.
pub(crate) fn limit_sections_by_token_budget(
    sections: Vec<String>,
    options: &ContextRenderOptions,
    estimator: &dyn TokenEstimator,
    total_sections: u32,
) -> (Vec<String>, Option<TruncationMetadata>) {
    let Some(token_budget) = options.token_budget else {
        return (sections, None);
    };

    let mut limited = Vec::new();
    let mut token_count = 0u32;
    let total_before = sections
        .iter()
        .map(|s| estimator.estimate_tokens(s))
        .sum::<u32>();

    for section in sections {
        let section_tokens = estimator.estimate_tokens(&section);
        if limited.is_empty() || token_count + section_tokens <= token_budget {
            token_count += section_tokens;
            limited.push(section);
        } else {
            break;
        }
    }

    let sections_kept = limited.len() as u32;
    let truncation = TruncationMetadata {
        budget_requested: token_budget,
        budget_used: token_count,
        total_before_truncation: total_before,
        sections_kept,
        sections_dropped: total_sections - sections_kept,
        token_estimator: estimator.name().to_string(),
    };

    (limited, Some(truncation))
}
