use std::collections::BTreeSet;

use serde_json::{Map, Value};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KmpIngestPlan {
    pub(crate) about: String,
    pub(crate) memory_id: String,
    pub(crate) idempotency_key: String,
    pub(crate) requested_by: Option<String>,
    pub(crate) correlation_id: Option<String>,
    pub(crate) causation_id: Option<String>,
    pub(crate) dry_run: bool,
    pub(crate) accepted: AcceptedCounts,
    pub(crate) changes: Vec<KmpIngestChange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedCounts {
    pub(crate) entries: usize,
    pub(crate) relations: usize,
    pub(crate) evidence: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KmpIngestChange {
    pub(crate) entity_kind: String,
    pub(crate) entity_id: String,
    pub(crate) payload_json: String,
    pub(crate) reason: String,
    pub(crate) scopes: Vec<String>,
}

pub(crate) fn build_ingest_plan(arguments: &Value) -> Result<KmpIngestPlan, String> {
    let arguments = arguments
        .as_object()
        .ok_or_else(|| "tool arguments must be a JSON object".to_string())?;
    let about = required_string(arguments.get("about"), "about")?;
    let idempotency_key = required_string(arguments.get("idempotency_key"), "idempotency_key")?;
    let memory = arguments
        .get("memory")
        .and_then(Value::as_object)
        .ok_or_else(|| "missing required object argument `memory`".to_string())?;

    let dimensions = required_array(memory.get("dimensions"), "memory.dimensions")?;
    let entries = required_array(memory.get("entries"), "memory.entries")?;
    let relations = optional_array(memory.get("relations"), "memory.relations")?;
    let evidence = optional_array(memory.get("evidence"), "memory.evidence")?;
    let provenance = arguments.get("provenance").and_then(Value::as_object);
    if let Some(provenance) = provenance {
        validate_provenance(provenance)?;
    }
    let dimension_ids = dimensions
        .iter()
        .filter_map(|dimension| required_object_string(dimension, "memory.dimensions[].id").ok())
        .collect::<BTreeSet<_>>();

    let mut changes = Vec::new();
    for dimension in dimensions {
        let id = required_object_string(dimension, "memory.dimensions[].id")?;
        let _kind = required_object_string(dimension, "memory.dimensions[].kind")?;
        changes.push(KmpIngestChange {
            entity_kind: "memory_dimension".to_string(),
            entity_id: id.to_string(),
            payload_json: stable_payload_json(dimension)?,
            reason: "KMP memory dimension ingest".to_string(),
            scopes: vec![id.to_string()],
        });
    }

    for entry in entries {
        let id = required_object_string(entry, "memory.entries[].id")?;
        let _kind = required_object_string(entry, "memory.entries[].kind")?;
        let _text = required_object_string(entry, "memory.entries[].text")?;
        validate_entry_positions(entry, &dimension_ids)?;
        changes.push(KmpIngestChange {
            entity_kind: "memory_entry".to_string(),
            entity_id: id.to_string(),
            payload_json: stable_payload_json(entry)?,
            reason: "KMP memory entry ingest".to_string(),
            scopes: entry_scopes(entry),
        });
    }

    for relation in relations {
        let from = required_object_string(relation, "memory.relations[].from")?;
        let to = required_object_string(relation, "memory.relations[].to")?;
        let rel = required_object_string(relation, "memory.relations[].rel")?;
        let semantic_class = required_object_string(relation, "memory.relations[].class")?;
        validate_semantic_class(semantic_class)?;
        validate_relation_explanation(relation, semantic_class)?;
        changes.push(KmpIngestChange {
            entity_kind: "memory_relation".to_string(),
            entity_id: format!("relation:{from}:{rel}:{to}"),
            payload_json: stable_payload_json(relation)?,
            reason: relation
                .get("why")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("KMP memory relation ingest")
                .to_string(),
            scopes: vec![from.to_string(), to.to_string()],
        });
    }

    for evidence_item in evidence {
        let id = required_object_string(evidence_item, "memory.evidence[].id")?;
        let _text = required_object_string(evidence_item, "memory.evidence[].text")?;
        validate_evidence_supports(evidence_item)?;
        changes.push(KmpIngestChange {
            entity_kind: "memory_evidence".to_string(),
            entity_id: id.to_string(),
            payload_json: stable_payload_json(evidence_item)?,
            reason: evidence_item
                .get("source")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("KMP memory evidence ingest")
                .to_string(),
            scopes: evidence_scopes(evidence_item),
        });
    }

    Ok(KmpIngestPlan {
        about,
        memory_id: memory_id_from_idempotency_key(&idempotency_key),
        idempotency_key,
        requested_by: provenance
            .and_then(|provenance| {
                provenance
                    .get("source_agent")
                    .or_else(|| provenance.get("source_kind"))
            })
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string),
        correlation_id: provenance
            .and_then(|provenance| provenance.get("correlation_id"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string),
        causation_id: provenance
            .and_then(|provenance| provenance.get("causation_id"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string),
        dry_run: arguments
            .get("dry_run")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        accepted: AcceptedCounts {
            entries: entries.len(),
            relations: relations.len(),
            evidence: evidence.len(),
        },
        changes,
    })
}

fn required_string(value: Option<&Value>, key: &str) -> Result<String, String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing required argument `{key}`"))
}

fn required_array<'a>(value: Option<&'a Value>, key: &str) -> Result<&'a [Value], String> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing required array argument `{key}`"))?;
    if values.is_empty() {
        return Err(format!("required array argument `{key}` must not be empty"));
    }
    Ok(values)
}

fn optional_array<'a>(value: Option<&'a Value>, key: &str) -> Result<&'a [Value], String> {
    match value {
        Some(value) => value
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| format!("argument `{key}` must be an array")),
        None => Ok(&[]),
    }
}

fn required_object_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .as_object()
        .and_then(|object| object.get(key.rsplit('.').next().unwrap_or(key)))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing required argument `{key}`"))
}

fn required_map_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing required argument `{path}`"))
}

fn validate_provenance(provenance: &Map<String, Value>) -> Result<(), String> {
    validate_source_kind(required_map_string(
        provenance,
        "source_kind",
        "provenance.source_kind",
    )?)?;
    let _source_agent = required_map_string(provenance, "source_agent", "provenance.source_agent")?;
    let _observed_at = required_map_string(provenance, "observed_at", "provenance.observed_at")?;
    Ok(())
}

fn validate_source_kind(value: &str) -> Result<(), String> {
    match value.trim() {
        "human" | "agent" | "projection" | "derived" => Ok(()),
        other => Err(format!("invalid memory provenance source_kind `{other}`")),
    }
}

fn validate_semantic_class(value: &str) -> Result<(), String> {
    match value.trim() {
        "structural" | "causal" | "motivational" | "procedural" | "evidential" | "constraint" => {
            Ok(())
        }
        other => Err(format!("invalid memory relation class `{other}`")),
    }
}

fn validate_confidence(value: &str) -> Result<(), String> {
    match value.trim() {
        "high" | "medium" | "low" | "unknown" => Ok(()),
        other => Err(format!("invalid memory relation confidence `{other}`")),
    }
}

fn validate_relation_explanation(relation: &Value, semantic_class: &str) -> Result<(), String> {
    let confidence = relation
        .get("confidence")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    if let Some(confidence) = confidence {
        validate_confidence(confidence)?;
    }
    if semantic_class != "structural" {
        if confidence.is_none() {
            return Err("non-structural memory relations require confidence".to_string());
        }
        let has_why = relation
            .get("why")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        let has_evidence = relation
            .get("evidence")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
        if !has_why && !has_evidence {
            return Err("non-structural memory relations require why or evidence".to_string());
        }
    }
    Ok(())
}

fn validate_evidence_supports(evidence_item: &Value) -> Result<(), String> {
    for (index, support) in evidence_item
        .get("supports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        if support
            .as_str()
            .is_none_or(|support| support.trim().is_empty())
        {
            return Err(format!(
                "argument `memory.evidence[].supports[{index}]` must be a non-empty string"
            ));
        }
    }
    Ok(())
}

fn entry_scopes(entry: &Value) -> Vec<String> {
    entry
        .get("coordinates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|coordinate| coordinate.get("scope_id"))
        .filter_map(Value::as_str)
        .filter(|scope| !scope.trim().is_empty())
        .map(ToString::to_string)
        .collect()
}

fn validate_entry_positions(entry: &Value, dimension_ids: &BTreeSet<&str>) -> Result<(), String> {
    let positions = entry
        .get("coordinates")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "missing required array argument `memory.entries[].coordinates`".to_string()
        })?;
    if positions.is_empty() {
        return Err(
            "required array argument `memory.entries[].coordinates` must not be empty".to_string(),
        );
    }

    for position in positions {
        let Some(dimension) = position.get("dimension").and_then(Value::as_str) else {
            return Err(
                "memory.entries[].coordinates[] is missing required `dimension`".to_string(),
            );
        };
        if dimension.trim().is_empty() {
            return Err("memory.entries[].coordinates[].dimension must not be empty".to_string());
        }
        let Some(scope_id) = position.get("scope_id").and_then(Value::as_str) else {
            return Err(
                "memory.entries[].coordinates[] is missing required `scope_id`".to_string(),
            );
        };
        if scope_id.trim().is_empty() {
            return Err("memory.entries[].coordinates[].scope_id must not be empty".to_string());
        }
        if !dimension_ids.contains(scope_id) {
            return Err(format!(
                "memory entry coordinate references unknown dimension scope `{scope_id}`"
            ));
        }
    }

    Ok(())
}

fn evidence_scopes(evidence_item: &Value) -> Vec<String> {
    evidence_item
        .get("supports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|scope| !scope.trim().is_empty())
        .map(ToString::to_string)
        .collect()
}

fn stable_payload_json(value: &Value) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("failed to encode ingest payload: {error}"))
}

fn memory_id_from_idempotency_key(idempotency_key: &str) -> String {
    idempotency_key
        .strip_prefix("ingest:")
        .map(|suffix| format!("memory:{suffix}"))
        .unwrap_or_else(|| format!("memory:{idempotency_key}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn build_ingest_plan_translates_memory_to_command_changes() {
        let plan = build_ingest_plan(&sample_ingest_request()).expect("ingest plan should build");

        assert_eq!(plan.about, "question:830ce83f");
        assert_eq!(plan.memory_id, "memory:830ce83f:1");
        assert_eq!(plan.idempotency_key, "ingest:830ce83f:1");
        assert_eq!(plan.requested_by.as_deref(), Some("longmemeval-adapter"));
        assert_eq!(plan.correlation_id.as_deref(), Some("corr:830ce83f"));
        assert_eq!(plan.causation_id.as_deref(), Some("eval:item:830ce83f"));
        assert_eq!(
            plan.accepted,
            AcceptedCounts {
                entries: 1,
                relations: 1,
                evidence: 1
            }
        );
        assert_eq!(
            plan.changes
                .iter()
                .map(|change| change.entity_kind.as_str())
                .collect::<Vec<_>>(),
            vec![
                "memory_dimension",
                "memory_entry",
                "memory_relation",
                "memory_evidence"
            ]
        );
        assert_eq!(plan.changes[1].scopes, vec!["conversation:rachel"]);
        assert_eq!(
            plan.changes[2].entity_id,
            "relation:claim:rachel-austin:supersedes:claim:rachel-denver"
        );
    }

    #[test]
    fn build_ingest_plan_rejects_missing_memory_shape() {
        let error = build_ingest_plan(&json!({
            "about": "question:830ce83f",
            "idempotency_key": "ingest:830ce83f:1"
        }))
        .expect_err("missing memory should fail");

        assert_eq!(error, "missing required object argument `memory`");
    }

    #[test]
    fn build_ingest_plan_rejects_unknown_entry_position_scope() {
        let error = build_ingest_plan(&json!({
            "about": "question:830ce83f",
            "memory": {
                "dimensions": [
                    {
                        "id": "conversation:rachel",
                        "kind": "conversation"
                    }
                ],
                "entries": [
                    {
                        "id": "claim:rachel-austin",
                        "kind": "claim",
                        "text": "Rachel moved to Austin.",
                        "coordinates": [
                            {
                                "dimension": "conversation",
                                "scope_id": "conversation:missing",
                                "sequence": 1
                            }
                        ]
                    }
                ]
            },
            "idempotency_key": "ingest:830ce83f:1"
        }))
        .expect_err("unknown position scope should fail");

        assert_eq!(
            error,
            "memory entry coordinate references unknown dimension scope `conversation:missing`"
        );
    }

    fn sample_ingest_request() -> Value {
        json!({
            "about": "question:830ce83f",
            "memory": {
                "dimensions": [
                    {
                        "id": "conversation:rachel",
                        "kind": "conversation"
                    }
                ],
                "entries": [
                    {
                        "id": "claim:rachel-austin",
                        "kind": "claim",
                        "text": "Rachel moved to Austin.",
                        "coordinates": [
                            {
                                "dimension": "conversation",
                                "scope_id": "conversation:rachel",
                                "sequence": 1
                            }
                        ]
                    }
                ],
                "relations": [
                    {
                        "from": "claim:rachel-austin",
                        "to": "claim:rachel-denver",
                        "rel": "supersedes",
                        "class": "evidential",
                        "why": "Later statement corrects earlier statement.",
                        "evidence": "Rachel corrected the destination.",
                        "confidence": "high"
                    }
                ],
                "evidence": [
                    {
                        "id": "evidence:rachel",
                        "supports": ["claim:rachel-austin"],
                        "text": "Rachel corrected the destination.",
                        "source": "conversation"
                    }
                ]
            },
            "provenance": {
                "source_kind": "agent",
                "source_agent": "longmemeval-adapter",
                "observed_at": "2026-05-04T10:00:00Z",
                "correlation_id": "corr:830ce83f",
                "causation_id": "eval:item:830ce83f"
            },
            "idempotency_key": "ingest:830ce83f:1"
        })
    }
}
