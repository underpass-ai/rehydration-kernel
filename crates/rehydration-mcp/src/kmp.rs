use std::collections::{BTreeMap, BTreeSet};

use rehydration_proto::v1beta1::{
    BundleNodeDetail, GetContextResponse, GetNodeDetailResponse, GraphRelationship,
    GraphRelationshipExplanation, GraphRelationshipSemanticClass, RenderedContext,
};
use serde_json::{Map, Value, json};

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

pub(crate) fn temporal_from_get_context(
    direction: &str,
    arguments: &Value,
    response: &GetContextResponse,
) -> Value {
    let about = arguments
        .get("about")
        .and_then(Value::as_str)
        .unwrap_or("memory");
    let relationships = context_relationships(response);
    let evidence = context_evidence(response);
    let nodes = response
        .bundle
        .as_ref()
        .map(bundle_nodes_by_id)
        .unwrap_or_default();
    let requested_dimensions = arguments
        .get("dimensions")
        .cloned()
        .unwrap_or_else(|| json!({"mode": "all"}));
    let mut positions = relationships
        .iter()
        .filter_map(|relationship| temporal_position_from_relationship(relationship, &nodes))
        .filter(|position| dimension_is_requested(position, &requested_dimensions))
        .collect::<Vec<_>>();
    positions.sort_by(|left, right| left.sort_key.cmp(&right.sort_key));

    let cursor = temporal_cursor(direction, arguments, &positions);
    let mut selected = match direction {
        "goto" => positions
            .iter()
            .filter(|position| cursor.as_ref().is_none_or(|cursor| *position <= cursor))
            .cloned()
            .collect::<Vec<_>>(),
        "near" => near_positions(&positions, cursor.as_ref(), arguments),
        "rewind" => positions
            .iter()
            .filter(|position| cursor.as_ref().is_none_or(|cursor| *position < cursor))
            .cloned()
            .collect::<Vec<_>>(),
        "forward" => positions
            .iter()
            .filter(|position| cursor.as_ref().is_none_or(|cursor| *position > cursor))
            .cloned()
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    if direction == "goto" || direction == "rewind" {
        selected.reverse();
    }
    selected.truncate(limit_entries(arguments, direction));
    if direction == "goto" || direction == "rewind" {
        selected.reverse();
    }

    let included = selected
        .iter()
        .filter_map(|position| position.dimension.as_deref())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    json!({
        "summary": format!(
            "Live kernel returned {} temporal {} {} for `{about}`.",
            selected.len(),
            if selected.len() == 1 { "entry" } else { "entries" },
            direction
        ),
        "temporal": temporal_request_json(direction, arguments, cursor.as_ref()),
        "coverage": {
            "requested": requested_dimensions,
            "included": included,
            "missing": []
        },
        "entries": selected.iter().map(TemporalPosition::to_entry_json).collect::<Vec<_>>(),
        "proof": {
            "path": relationships
                .into_iter()
                .filter(|relationship| relationship.get("rel").and_then(Value::as_str) != Some("contains_entry"))
                .collect::<Vec<_>>(),
            "evidence": evidence,
            "conflicts": [],
            "missing": if selected.is_empty() { vec!["temporal_positions"] } else { Vec::<&str>::new() },
            "confidence": if selected.is_empty() { "unknown" } else { "medium" }
        },
        "warnings": live_warnings(response.rendered.as_ref(), false)
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

    let mut relationship = json!({
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
    });

    if let Some(coordinate) = explanation.and_then(coordinate_json) {
        relationship["coordinate"] = coordinate;
    }

    relationship
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

fn bundle_nodes_by_id(
    bundle: &rehydration_proto::v1beta1::RehydrationBundle,
) -> BTreeMap<String, (String, String)> {
    let mut nodes = BTreeMap::new();

    for role_bundle in &bundle.bundles {
        if let Some(root) = role_bundle.root_node.as_ref() {
            nodes.insert(root.node_id.clone(), node_text(root));
        }
        for node in &role_bundle.neighbor_nodes {
            nodes.insert(node.node_id.clone(), node_text(node));
        }
    }

    nodes
}

fn node_text(node: &rehydration_proto::v1beta1::GraphNode) -> (String, String) {
    let text = if node.summary.trim().is_empty() {
        node.title.clone()
    } else {
        node.summary.clone()
    };
    (node.node_kind.clone(), text)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TemporalPosition {
    ref_id: String,
    text: String,
    kind: String,
    dimension: Option<String>,
    scope_id: Option<String>,
    sequence: Option<u32>,
    rank: Option<u32>,
    occurred_at: Option<String>,
    observed_at: Option<String>,
    ingested_at: Option<String>,
    valid_from: Option<String>,
    valid_until: Option<String>,
    sort_key: String,
}

impl PartialOrd for TemporalPosition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TemporalPosition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_key
            .cmp(&other.sort_key)
            .then_with(|| self.dimension.cmp(&other.dimension))
            .then_with(|| self.scope_id.cmp(&other.scope_id))
            .then_with(|| self.sequence.cmp(&other.sequence))
            .then_with(|| self.rank.cmp(&other.rank))
            .then_with(|| self.ref_id.cmp(&other.ref_id))
    }
}

impl TemporalPosition {
    fn to_entry_json(&self) -> Value {
        json!({
            "ref": self.ref_id,
            "kind": self.kind,
            "text": self.text,
            "coordinates": [self.coordinate_json()]
        })
    }

    fn coordinate_json(&self) -> Value {
        let mut coordinate = Map::new();
        insert_optional_json(&mut coordinate, "dimension", self.dimension.as_deref());
        insert_optional_json(&mut coordinate, "scope_id", self.scope_id.as_deref());
        insert_optional_u32_json(&mut coordinate, "sequence", self.sequence);
        insert_optional_u32_json(&mut coordinate, "rank", self.rank);
        insert_optional_json(&mut coordinate, "occurred_at", self.occurred_at.as_deref());
        insert_optional_json(&mut coordinate, "observed_at", self.observed_at.as_deref());
        insert_optional_json(&mut coordinate, "ingested_at", self.ingested_at.as_deref());
        insert_optional_json(&mut coordinate, "valid_from", self.valid_from.as_deref());
        insert_optional_json(&mut coordinate, "valid_until", self.valid_until.as_deref());
        Value::Object(coordinate)
    }
}

fn temporal_position_from_relationship(
    relationship: &Value,
    nodes: &BTreeMap<String, (String, String)>,
) -> Option<TemporalPosition> {
    if relationship.get("rel").and_then(Value::as_str) != Some("contains_entry") {
        return None;
    }

    let ref_id = relationship.get("to").and_then(Value::as_str)?.to_string();
    let coordinate = relationship.get("coordinate")?;
    let (kind, text) = nodes
        .get(&ref_id)
        .cloned()
        .unwrap_or_else(|| ("entry".to_string(), ref_id.clone()));
    let sequence = coordinate
        .get("sequence")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let rank = coordinate
        .get("rank")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let occurred_at = optional_json_string(coordinate, "occurred_at");
    let valid_from = optional_json_string(coordinate, "valid_from");
    let observed_at = optional_json_string(coordinate, "observed_at");
    let ingested_at = optional_json_string(coordinate, "ingested_at");
    let sort_key = occurred_at
        .as_ref()
        .or(valid_from.as_ref())
        .or(observed_at.as_ref())
        .or(ingested_at.as_ref())
        .cloned()
        .or_else(|| sequence.map(|sequence| format!("sequence:{sequence:010}")))
        .or_else(|| rank.map(|rank| format!("rank:{rank:010}")))
        .unwrap_or_else(|| ref_id.clone());

    Some(TemporalPosition {
        ref_id,
        text,
        kind,
        dimension: optional_json_string(coordinate, "dimension"),
        scope_id: optional_json_string(coordinate, "scope_id"),
        sequence,
        rank,
        occurred_at,
        observed_at,
        ingested_at,
        valid_from,
        valid_until: optional_json_string(coordinate, "valid_until"),
        sort_key,
    })
}

fn coordinate_json(explanation: &GraphRelationshipExplanation) -> Option<Value> {
    let mut coordinate = Map::new();
    insert_optional_json(
        &mut coordinate,
        "dimension",
        non_empty(&explanation.dimension),
    );
    insert_optional_json(
        &mut coordinate,
        "scope_id",
        non_empty(&explanation.scope_id),
    );
    insert_optional_u32_json(&mut coordinate, "sequence", non_zero(explanation.sequence));
    insert_optional_u32_json(&mut coordinate, "rank", non_zero(explanation.rank));
    insert_optional_json(
        &mut coordinate,
        "occurred_at",
        non_empty(&explanation.occurred_at),
    );
    insert_optional_json(
        &mut coordinate,
        "observed_at",
        non_empty(&explanation.observed_at),
    );
    insert_optional_json(
        &mut coordinate,
        "ingested_at",
        non_empty(&explanation.ingested_at),
    );
    insert_optional_json(
        &mut coordinate,
        "valid_from",
        non_empty(&explanation.valid_from),
    );
    insert_optional_json(
        &mut coordinate,
        "valid_until",
        non_empty(&explanation.valid_until),
    );

    (!coordinate.is_empty()).then_some(Value::Object(coordinate))
}

fn dimension_is_requested(position: &TemporalPosition, requested: &Value) -> bool {
    let mode = requested
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("all");
    let dimension = position.dimension.as_deref().unwrap_or("");
    match mode {
        "only" => requested
            .get("include")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|candidate| candidate == dimension),
        "except" => !requested
            .get("exclude")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|candidate| candidate == dimension),
        _ => true,
    }
}

fn temporal_cursor(
    direction: &str,
    arguments: &Value,
    positions: &[TemporalPosition],
) -> Option<TemporalPosition> {
    let cursor_object = match direction {
        "goto" => arguments.get("at"),
        "near" => arguments.get("around"),
        "rewind" | "forward" => arguments.get("from"),
        _ => None,
    }?;

    if let Some(ref_id) = cursor_object.get("ref").and_then(Value::as_str)
        && let Some(position) = positions.iter().find(|position| position.ref_id == ref_id)
    {
        return Some(position.clone());
    }

    let sort_key = cursor_object
        .get("time")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            cursor_object
                .get("sequence")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .map(|value| format!("sequence:{value:010}"))
        })?;

    Some(TemporalPosition {
        ref_id: "cursor".to_string(),
        text: String::new(),
        kind: "cursor".to_string(),
        dimension: None,
        scope_id: None,
        sequence: None,
        rank: None,
        occurred_at: cursor_object
            .get("time")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        observed_at: None,
        ingested_at: None,
        valid_from: None,
        valid_until: None,
        sort_key,
    })
}

fn near_positions(
    positions: &[TemporalPosition],
    cursor: Option<&TemporalPosition>,
    arguments: &Value,
) -> Vec<TemporalPosition> {
    let Some(cursor) = cursor else {
        return positions.to_vec();
    };
    let before_limit = arguments
        .get("window")
        .and_then(|window| window.get("before_entries"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(2);
    let after_limit = arguments
        .get("window")
        .and_then(|window| window.get("after_entries"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(2);
    let mut before = positions
        .iter()
        .filter(|position| *position < cursor)
        .rev()
        .take(before_limit)
        .cloned()
        .collect::<Vec<_>>();
    before.reverse();
    let after = positions
        .iter()
        .filter(|position| *position > cursor)
        .take(after_limit)
        .cloned();

    before.into_iter().chain(after).collect()
}

fn limit_entries(arguments: &Value, direction: &str) -> usize {
    let default = if direction == "goto" { 1 } else { 5 };
    arguments
        .get("limit")
        .and_then(|limit| limit.get("entries"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(default)
}

fn temporal_request_json(
    direction: &str,
    arguments: &Value,
    cursor: Option<&TemporalPosition>,
) -> Value {
    let cursor_key = match direction {
        "near" => "around",
        "goto" => "at",
        _ => "from",
    };
    let mut temporal = Map::new();
    temporal.insert("direction".to_string(), json!(direction));
    temporal.insert(
        cursor_key.to_string(),
        arguments.get(cursor_key).cloned().unwrap_or(Value::Null),
    );
    temporal.insert(
        "resolved".to_string(),
        cursor
            .map(|cursor| cursor.coordinate_json())
            .unwrap_or(Value::Null),
    );
    Value::Object(temporal)
}

fn optional_json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn insert_optional_json(object: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        object.insert(key.to_string(), json!(value));
    }
}

fn insert_optional_u32_json(object: &mut Map<String, Value>, key: &str, value: Option<u32>) {
    if let Some(value) = value {
        object.insert(key.to_string(), json!(value));
    }
}

fn non_empty(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then_some(value)
}

fn non_zero(value: u32) -> Option<u32> {
    (value != 0).then_some(value)
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
