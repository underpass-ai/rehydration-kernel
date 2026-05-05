use std::collections::{BTreeMap, BTreeSet};

use rehydration_application::{
    GetContextPathResult, GetContextResult, GraphRelationshipView, InspectMemoryResult,
    MemoryAnswerPolicy, TemporalMemoryResult,
};
use rehydration_domain::{BundleNodeDetail, RehydrationBundle, TemporalDirection};
use rehydration_proto::v1beta1::{
    AnswerReason, AskResponse, InspectResponse, InspectedLinks, InspectedObject, MemoryConfidence,
    MemoryEvidence, MemoryRelation, RawMemoryRef, TemporalEntry as ProtoTemporalEntry,
    TemporalMoveResponse, TemporalState, TraceResponse, WakeClaim, WakePacket, WakeResponse,
};

use super::bundle_views::{
    answer_evidence_from_bundle, memory_evidence_from_bundle, memory_relations_from_bundle, proof,
    proto_coordinate_from_domain, rendered_current_state, rendered_summary,
    temporal_evidence_from_bundle, temporal_relations_from_bundle,
};
use super::dimensions::proto_dimension_selection_from_domain;
use super::scalars::{proto_confidence, proto_direction, proto_semantic_class};

pub(crate) fn wake_response_from_result(intent: &str, result: GetContextResult) -> WakeResponse {
    let relationships = memory_relations_from_bundle(&result.bundle);
    let evidence = memory_evidence_from_bundle(&result.bundle);
    let current_state = rendered_current_state(&result.rendered);
    let summary = rendered_summary(&result.rendered);

    WakeResponse {
        summary,
        wake: Some(WakePacket {
            objective: intent.to_string(),
            current_state,
            causal_spine: relationships
                .iter()
                .take(8)
                .map(|relationship| WakeClaim {
                    claim: format!("{} -> {}", relationship.source_ref, relationship.target_ref),
                    because: if relationship.why.is_empty() {
                        "Kernel relationship path selected this edge.".to_string()
                    } else {
                        relationship.why.clone()
                    },
                    evidence_ref: relationship.evidence.clone(),
                })
                .collect(),
            open_loops: Vec::new(),
            next_actions: Vec::new(),
            guardrails: Vec::new(),
        }),
        proof: Some(proof(
            relationships,
            evidence,
            Vec::new(),
            MemoryConfidence::Medium,
        )),
        warnings: Vec::new(),
    }
}

pub(crate) fn ask_response_from_result(
    question: &str,
    policy: MemoryAnswerPolicy,
    result: GetContextResult,
) -> AskResponse {
    let evidence = answer_evidence_from_bundle(&result.bundle);
    let because = evidence
        .iter()
        .take(5)
        .map(|item| AnswerReason {
            claim: item.source.clone(),
            evidence: item.text.clone(),
            r#ref: item.id.clone(),
        })
        .collect::<Vec<_>>();
    let confidence = if because.is_empty() {
        MemoryConfidence::Unknown
    } else {
        MemoryConfidence::Medium
    };

    let answer = match policy {
        MemoryAnswerPolicy::EvidenceOrUnknown if because.is_empty() => "UNKNOWN".to_string(),
        MemoryAnswerPolicy::EvidenceOrUnknown
        | MemoryAnswerPolicy::ShowConflicts
        | MemoryAnswerPolicy::BestEffort => deterministic_answer_from_reasons(&because),
    };
    let answer = if answer.trim().is_empty() {
        "UNKNOWN".to_string()
    } else {
        answer
    };

    AskResponse {
        summary: if answer == "UNKNOWN" {
            format!("No deterministic memory answer found for: {question}")
        } else {
            format!(
                "Deterministic memory answer from {} evidence {} for: {question}",
                because.len(),
                if because.len() == 1 { "item" } else { "items" }
            )
        },
        answer,
        because,
        proof: Some(proof(
            memory_relations_from_bundle(&result.bundle),
            evidence,
            Vec::new(),
            confidence,
        )),
        warnings: Vec::new(),
    }
}

fn deterministic_answer_from_reasons(reasons: &[AnswerReason]) -> String {
    let mut seen = BTreeSet::new();
    let evidence = reasons
        .iter()
        .filter_map(|reason| {
            let text = reason.evidence.trim();
            if text.is_empty() || !seen.insert(text.to_string()) {
                None
            } else {
                Some(text.to_string())
            }
        })
        .collect::<Vec<_>>();

    match evidence.as_slice() {
        [] => String::new(),
        [single] => single.clone(),
        many => many
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

pub(crate) fn temporal_response_from_result(
    requested_cursor: rehydration_proto::v1beta1::TemporalCursor,
    direction: TemporalDirection,
    result: TemporalMemoryResult,
) -> TemporalMoveResponse {
    let traversal = result.traversal;
    let entries = traversal
        .entries()
        .iter()
        .map(|entry| ProtoTemporalEntry {
            r#ref: entry.ref_id().to_string(),
            kind: entry.kind().to_string(),
            text: entry.text().to_string(),
            coordinates: entry
                .coordinates()
                .iter()
                .map(proto_coordinate_from_domain)
                .collect(),
        })
        .collect::<Vec<_>>();
    let selected_refs = entries
        .iter()
        .map(|entry| entry.r#ref.clone())
        .collect::<BTreeSet<_>>();
    let relationships = if result.include.relations {
        temporal_relations_from_bundle(&result.source_bundle, &selected_refs)
    } else {
        Vec::new()
    };
    let evidence = if result.include.evidence {
        temporal_evidence_from_bundle(&result.source_bundle, &selected_refs)
    } else {
        Vec::new()
    };
    let count = entries.len();
    let raw_refs = if result.include.raw_refs {
        raw_refs_from_temporal_entries(&entries, &result.source_bundle)
    } else {
        Vec::new()
    };

    TemporalMoveResponse {
        summary: format!(
            "Returned {count} temporal {}.",
            if count == 1 { "entry" } else { "entries" }
        ),
        temporal: Some(TemporalState {
            direction: proto_direction(direction) as i32,
            requested: Some(requested_cursor),
            resolved: Some(proto_coordinate_from_domain(traversal.resolved_cursor())),
        }),
        coverage: Some(rehydration_proto::v1beta1::TemporalCoverage {
            requested: Some(proto_dimension_selection_from_domain(
                traversal.requested_dimensions(),
            )),
            included: traversal.included_dimensions().to_vec(),
            missing: traversal.missing_dimensions().to_vec(),
        }),
        entries,
        proof: Some(proof(
            relationships,
            evidence,
            traversal.missing().to_vec(),
            if count == 0 {
                MemoryConfidence::Unknown
            } else {
                MemoryConfidence::Medium
            },
        )),
        warnings: Vec::new(),
        raw_refs,
    }
}

pub(crate) fn trace_response_from_result(result: GetContextPathResult) -> TraceResponse {
    TraceResponse {
        summary: rendered_summary(&result.rendered),
        trace: memory_relations_from_bundle(&result.path_bundle),
        warnings: Vec::new(),
    }
}

pub(crate) fn inspect_response_from_result(result: InspectMemoryResult) -> InspectResponse {
    let node_ref = result.detail.node.node_id.clone();
    let node_kind = result.detail.node.node_kind.clone();
    let text = if result.include_details {
        result
            .detail
            .detail
            .as_ref()
            .map(|detail| detail.detail.clone())
            .filter(|detail| !detail.trim().is_empty())
            .unwrap_or_else(|| result.detail.node.summary.clone())
    } else {
        result.detail.node.summary.clone()
    };
    let evidence = if result.include_details {
        result
            .detail
            .detail
            .as_ref()
            .map_or_else(Vec::new, |detail| {
                vec![MemoryEvidence {
                    id: format!("detail:{}", detail.node_id),
                    supports: vec![detail.node_id.clone()],
                    text: detail.detail.clone(),
                    source: detail.node_id.clone(),
                    time: None,
                    metadata: Default::default(),
                }]
            })
    } else {
        Vec::new()
    };
    let incoming = result
        .incoming
        .iter()
        .map(memory_relation_from_graph_relationship)
        .collect();
    let outgoing = result
        .outgoing
        .iter()
        .map(memory_relation_from_graph_relationship)
        .collect();
    let raw = if result.include_raw {
        vec![RawMemoryRef {
            r#ref: node_ref.clone(),
            kind: node_kind.clone(),
            text: result.detail.node.summary.clone(),
            coordinates: result
                .raw_coordinates
                .iter()
                .map(proto_coordinate_from_domain)
                .collect(),
            detail: result
                .detail
                .detail
                .as_ref()
                .map(|detail| detail.detail.clone())
                .unwrap_or_default(),
            content_hash: result
                .detail
                .detail
                .as_ref()
                .map(|detail| detail.content_hash.clone())
                .unwrap_or_default(),
            revision: result
                .detail
                .detail
                .as_ref()
                .map(|detail| detail.revision)
                .unwrap_or_default(),
        }]
    } else {
        Vec::new()
    };

    InspectResponse {
        summary: format!("Found live kernel node `{}`.", node_ref),
        object: Some(InspectedObject {
            r#ref: node_ref,
            kind: node_kind,
            text,
        }),
        links: Some(InspectedLinks { incoming, outgoing }),
        evidence,
        warnings: Vec::new(),
        raw,
    }
}

fn raw_refs_from_temporal_entries(
    entries: &[ProtoTemporalEntry],
    bundle: &RehydrationBundle,
) -> Vec<RawMemoryRef> {
    let detail_by_ref = bundle
        .node_details()
        .iter()
        .map(|detail| (detail.node_id(), detail))
        .collect::<BTreeMap<_, _>>();

    entries
        .iter()
        .map(|entry| {
            let detail = detail_by_ref.get(entry.r#ref.as_str()).copied();
            raw_ref_from_temporal_entry(entry, detail)
        })
        .collect()
}

fn raw_ref_from_temporal_entry(
    entry: &ProtoTemporalEntry,
    detail: Option<&BundleNodeDetail>,
) -> RawMemoryRef {
    RawMemoryRef {
        r#ref: entry.r#ref.clone(),
        kind: entry.kind.clone(),
        text: entry.text.clone(),
        coordinates: entry.coordinates.clone(),
        detail: detail
            .map(|detail| detail.detail().to_string())
            .unwrap_or_default(),
        content_hash: detail
            .map(|detail| detail.content_hash().to_string())
            .unwrap_or_default(),
        revision: detail.map(BundleNodeDetail::revision).unwrap_or_default(),
    }
}

fn memory_relation_from_graph_relationship(relationship: &GraphRelationshipView) -> MemoryRelation {
    let explanation = &relationship.explanation;
    MemoryRelation {
        source_ref: relationship.source_node_id.clone(),
        target_ref: relationship.target_node_id.clone(),
        rel: relationship.relationship_type.clone(),
        semantic_class: proto_semantic_class(explanation.semantic_class()) as i32,
        why: explanation.rationale().unwrap_or_default().to_string(),
        evidence: explanation.evidence().unwrap_or_default().to_string(),
        confidence: proto_confidence(explanation.confidence()) as i32,
        sequence: explanation.sequence(),
    }
}
