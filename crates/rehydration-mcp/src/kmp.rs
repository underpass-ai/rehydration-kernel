use rehydration_proto::v1beta1::{
    BundleNodeDetail, GetContextResponse, GetNodeDetailResponse, GraphRelationship,
    GraphRelationshipSemanticClass, RenderedContext,
};
use serde_json::{Value, json};

pub(crate) fn wake_from_get_context(
    about: &str,
    intent: &str,
    response: &GetContextResponse,
) -> Value {
    let rendered = response.rendered.as_ref();
    let relationships = context_relationships(response);
    let evidence = context_evidence(response);
    let current_state = rendered_current_state(rendered);
    let has_content = rendered
        .map(|rendered| !rendered.content.trim().is_empty() || !rendered.sections.is_empty())
        .unwrap_or(false);

    json!({
        "summary": rendered
            .map(rendered_summary)
            .unwrap_or_else(|| format!("Live kernel returned no rendered context for {about}.")),
        "wake": {
            "objective": intent,
            "current_state": current_state,
            "causal_spine": relationships
                .iter()
                .take(8)
                .map(|relationship| json!({
                    "claim": format!(
                        "{} -> {}",
                        relationship.get("from").and_then(Value::as_str).unwrap_or("unknown"),
                        relationship.get("to").and_then(Value::as_str).unwrap_or("unknown")
                    ),
                    "because": relationship
                        .get("why")
                        .and_then(Value::as_str)
                        .filter(|why| !why.is_empty())
                        .unwrap_or("Kernel relationship path selected this edge."),
                    "evidence_ref": relationship
                        .get("evidence")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                }))
                .collect::<Vec<_>>(),
            "open_loops": if has_content { Vec::<String>::new() } else { vec!["No rendered live context was returned.".to_string()] },
            "next_actions": [
                "Use kernel_trace for specific relation paths.",
                "Use kernel_inspect for raw node detail."
            ],
            "guardrails": [
                "This wake packet is derived from live GetContext output.",
                "Missing relations or details may limit proof quality."
            ]
        },
        "proof": {
            "path": relationships,
            "evidence": evidence,
            "conflicts": [],
            "missing": if has_content { Vec::<String>::new() } else { vec!["rendered_context".to_string()] },
            "confidence": if has_content { "medium" } else { "unknown" }
        },
        "warnings": live_warnings(rendered, false)
    })
}

pub(crate) fn ask_from_get_context(
    about: &str,
    question: &str,
    response: &GetContextResponse,
) -> Value {
    let rendered = response.rendered.as_ref();
    let relationships = context_relationships(response);
    let evidence = context_evidence(response);
    let has_evidence = !evidence.is_empty()
        || rendered
            .map(|rendered| !rendered.content.trim().is_empty())
            .unwrap_or(false);

    json!({
        "summary": if has_evidence {
            format!("Returned live kernel context for `{about}`. This read-only adapter did not generate a final answer for: {question}")
        } else {
            format!("Live kernel returned no evidence for `{about}`.")
        },
        "answer": Value::Null,
        "because": evidence
            .iter()
            .take(5)
            .map(|item| json!({
                "claim": item.get("source").and_then(Value::as_str).unwrap_or("kernel evidence"),
                "evidence": item.get("text").and_then(Value::as_str).unwrap_or(""),
                "ref": item.get("id").and_then(Value::as_str).unwrap_or("")
            }))
            .collect::<Vec<_>>(),
        "proof": {
            "path": relationships,
            "evidence": evidence,
            "conflicts": [],
            "missing": ["generative_answer"],
            "confidence": if has_evidence { "medium" } else { "unknown" }
        },
        "warnings": [
            "kernel_ask live gRPC mode returns evidence/proof only; final answer generation is not implemented in this adapter."
        ]
    })
}

pub(crate) fn inspect_from_get_node_detail(
    ref_id: &str,
    response: &GetNodeDetailResponse,
) -> Value {
    let object = response.node.as_ref().map_or_else(
        || {
            json!({
                "ref": ref_id,
                "kind": "unknown"
            })
        },
        |node| {
            json!({
                "ref": node.node_id,
                "kind": node.node_kind,
                "text": if node.summary.is_empty() { node.title.clone() } else { node.summary.clone() }
            })
        },
    );
    let evidence = response
        .detail
        .as_ref()
        .map_or_else(Vec::new, |detail| vec![evidence_from_detail(detail)]);

    json!({
        "summary": if response.node.is_some() {
            format!("Found live kernel node `{ref_id}`.")
        } else {
            format!("No live kernel node metadata returned for `{ref_id}`.")
        },
        "object": object,
        "links": {
            "incoming": [],
            "outgoing": []
        },
        "evidence": evidence,
        "warnings": if response.detail.is_some() { Vec::<String>::new() } else { vec!["No node detail returned.".to_string()] }
    })
}

fn context_relationships(response: &GetContextResponse) -> Vec<Value> {
    response
        .bundle
        .as_ref()
        .map(bundle_relationships)
        .unwrap_or_default()
}

pub(crate) fn bundle_relationships(
    bundle: &rehydration_proto::v1beta1::RehydrationBundle,
) -> Vec<Value> {
    bundle
        .bundles
        .iter()
        .flat_map(|role_bundle| role_bundle.relationships.iter())
        .map(relationship_json)
        .collect()
}

pub(crate) fn relationships_is_empty(relationships: &[Value]) -> bool {
    relationships.is_empty()
}

fn relationship_json(relationship: &GraphRelationship) -> Value {
    let explanation = relationship.explanation.as_ref();
    let relationship_type = if relationship.relationship_type.trim().is_empty() {
        "related"
    } else {
        relationship.relationship_type.as_str()
    };
    let why = explanation
        .map(|explanation| {
            first_non_empty([
                explanation.rationale.as_str(),
                explanation.motivation.as_str(),
                explanation.method.as_str(),
            ])
        })
        .filter(|why| !why.trim().is_empty())
        .unwrap_or_else(|| "Kernel relationship path selected this edge.".to_string());
    let evidence = explanation
        .map(|explanation| explanation.evidence.clone())
        .filter(|evidence| !evidence.trim().is_empty())
        .unwrap_or_else(|| why.clone());

    json!({
        "from": relationship.source_node_id,
        "to": relationship.target_node_id,
        "rel": relationship_type,
        "class": explanation
            .map(|explanation| semantic_class_label(explanation.semantic_class))
            .unwrap_or("structural"),
        "why": why,
        "evidence": evidence,
        "confidence": explanation
            .map(|explanation| if explanation.confidence.is_empty() { "unknown".to_string() } else { explanation.confidence.clone() })
            .unwrap_or_else(|| "unknown".to_string())
    })
}

fn semantic_class_label(value: i32) -> &'static str {
    match GraphRelationshipSemanticClass::try_from(value) {
        Ok(GraphRelationshipSemanticClass::Structural) => "structural",
        Ok(GraphRelationshipSemanticClass::Causal) => "causal",
        Ok(GraphRelationshipSemanticClass::Motivational) => "motivational",
        Ok(GraphRelationshipSemanticClass::Procedural) => "procedural",
        Ok(GraphRelationshipSemanticClass::Evidential) => "evidential",
        Ok(GraphRelationshipSemanticClass::Constraint) => "constraint",
        _ => "structural",
    }
}

fn first_non_empty(values: [&str; 3]) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or("")
        .to_string()
}

fn context_evidence(response: &GetContextResponse) -> Vec<Value> {
    response
        .bundle
        .as_ref()
        .map(|bundle| {
            bundle
                .bundles
                .iter()
                .flat_map(|role_bundle| role_bundle.node_details.iter())
                .map(evidence_from_detail)
                .collect()
        })
        .unwrap_or_default()
}

fn evidence_from_detail(detail: &BundleNodeDetail) -> Value {
    json!({
        "id": format!("detail:{}", detail.node_id),
        "supports": [detail.node_id.clone()],
        "text": detail.detail,
        "source": detail.node_id
    })
}

fn rendered_current_state(rendered: Option<&RenderedContext>) -> Vec<String> {
    let Some(rendered) = rendered else {
        return Vec::new();
    };

    let from_sections = rendered
        .sections
        .iter()
        .take(5)
        .map(|section| {
            if section.title.is_empty() {
                section.content.clone()
            } else {
                format!("{}: {}", section.title, section.content)
            }
        })
        .filter(|state| !state.trim().is_empty())
        .collect::<Vec<_>>();

    if !from_sections.is_empty() {
        return from_sections;
    }

    if rendered.content.trim().is_empty() {
        Vec::new()
    } else {
        vec![truncate(&rendered.content, 1200)]
    }
}

pub(crate) fn rendered_summary(rendered: &RenderedContext) -> String {
    rendered
        .tiers
        .iter()
        .find(|tier| !tier.content.trim().is_empty())
        .map(|tier| truncate(&tier.content, 500))
        .or_else(|| {
            rendered
                .sections
                .iter()
                .find(|section| !section.content.trim().is_empty())
                .map(|section| truncate(&section.content, 500))
        })
        .unwrap_or_else(|| truncate(&rendered.content, 500))
}

pub(crate) fn live_warnings(rendered: Option<&RenderedContext>, missing_path: bool) -> Vec<String> {
    let mut warnings = Vec::new();

    if rendered
        .map(|rendered| rendered.content.trim().is_empty() && rendered.sections.is_empty())
        .unwrap_or(true)
    {
        warnings.push("No rendered context was returned by the live kernel.".to_string());
    }

    if missing_path {
        warnings.push("No relationship path was returned by the live kernel.".to_string());
    }

    warnings
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut truncated = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        truncated.push_str("...");
    }
    truncated
}

#[cfg(test)]
mod tests {
    use rehydration_proto::v1beta1::{
        BundleRenderFormat, BundleSection, RehydrationMode, RenderedTier, ResolutionTier,
    };

    use super::*;

    #[test]
    fn live_warnings_report_missing_rendered_context_and_path() {
        assert_eq!(
            live_warnings(None, true),
            vec![
                "No rendered context was returned by the live kernel.".to_string(),
                "No relationship path was returned by the live kernel.".to_string()
            ]
        );

        let rendered = rendered_with_content("visible context");
        assert!(live_warnings(Some(&rendered), false).is_empty());
    }

    #[test]
    fn rendered_summary_prefers_tiers_then_sections_then_content() {
        let mut rendered = rendered_with_content("fallback content");
        rendered.sections.push(BundleSection {
            key: "state".to_string(),
            title: "State".to_string(),
            content: "section summary".to_string(),
            token_count: 2,
            scopes: Vec::new(),
        });
        rendered.tiers.push(RenderedTier {
            tier: ResolutionTier::L0Summary as i32,
            content: "tier summary".to_string(),
            token_count: 2,
            sections: Vec::new(),
        });

        assert_eq!(rendered_summary(&rendered), "tier summary");

        rendered.tiers.clear();
        assert_eq!(rendered_summary(&rendered), "section summary");

        rendered.sections.clear();
        assert_eq!(rendered_summary(&rendered), "fallback content");
    }

    fn rendered_with_content(content: &str) -> RenderedContext {
        RenderedContext {
            format: BundleRenderFormat::Structured as i32,
            content: content.to_string(),
            token_count: 1,
            sections: Vec::new(),
            tiers: Vec::new(),
            resolved_mode: RehydrationMode::ResumeFocused as i32,
            quality: None,
            truncation: None,
            content_hash: "sha256:test".to_string(),
        }
    }
}
