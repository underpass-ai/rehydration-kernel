use std::collections::BTreeSet;

use rehydration_application::{
    GetContextPathResult, GetContextResult, GetNodeDetailResult, MemoryAnswerPolicy,
    TemporalMemoryResult,
};
use rehydration_domain::TemporalDirection;
use rehydration_proto::v1beta1::{
    AnswerReason, AskResponse, InspectResponse, InspectedLinks, InspectedObject, MemoryConfidence,
    MemoryEvidence, TemporalEntry as ProtoTemporalEntry, TemporalMoveResponse, TemporalState,
    TraceResponse, WakeClaim, WakePacket, WakeResponse,
};

use super::bundle_views::{
    memory_evidence_from_bundle, memory_relations_from_bundle, proof, proto_coordinate_from_domain,
    rendered_current_state, rendered_summary, temporal_evidence_from_bundle,
    temporal_relations_from_bundle,
};
use super::dimensions::proto_dimension_selection_from_domain;
use super::scalars::proto_direction;

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
    let evidence = memory_evidence_from_bundle(&result.bundle);
    let answer = rendered_summary(&result.rendered);
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
        | MemoryAnswerPolicy::BestEffort => answer,
    };

    AskResponse {
        summary: if answer.trim().is_empty() || answer == "UNKNOWN" {
            format!("No deterministic memory answer found for: {question}")
        } else {
            answer.clone()
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
    }
}

pub(crate) fn trace_response_from_result(result: GetContextPathResult) -> TraceResponse {
    TraceResponse {
        summary: rendered_summary(&result.rendered),
        trace: memory_relations_from_bundle(&result.path_bundle),
        warnings: Vec::new(),
    }
}

pub(crate) fn inspect_response_from_result(
    result: GetNodeDetailResult,
    include_details: bool,
) -> InspectResponse {
    let text = if include_details {
        result
            .detail
            .as_ref()
            .map(|detail| detail.detail.clone())
            .filter(|detail| !detail.trim().is_empty())
            .unwrap_or_else(|| result.node.summary.clone())
    } else {
        result.node.summary.clone()
    };
    let evidence = if include_details {
        result.detail.as_ref().map_or_else(Vec::new, |detail| {
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

    InspectResponse {
        summary: format!("Found live kernel node `{}`.", result.node.node_id),
        object: Some(InspectedObject {
            r#ref: result.node.node_id,
            kind: result.node.node_kind,
            text,
        }),
        links: Some(InspectedLinks {
            incoming: Vec::new(),
            outgoing: Vec::new(),
        }),
        evidence,
        warnings: Vec::new(),
    }
}
