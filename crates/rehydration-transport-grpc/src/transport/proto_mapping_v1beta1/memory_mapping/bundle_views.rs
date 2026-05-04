use std::collections::BTreeSet;

use rehydration_application::RenderedContext;
use rehydration_domain::{RehydrationBundle, TemporalCoordinate};
use rehydration_proto::v1beta1::{
    MemoryConfidence, MemoryEvidence, MemoryRelation, Proof,
    TemporalCoordinate as ProtoTemporalCoordinate,
};

use super::scalars::{proto_confidence, proto_semantic_class, timestamp_from_sort_or_rfc3339};

pub(super) fn memory_relations_from_bundle(bundle: &RehydrationBundle) -> Vec<MemoryRelation> {
    bundle
        .relationships()
        .iter()
        .map(|relationship| {
            let explanation = relationship.explanation();
            MemoryRelation {
                source_ref: relationship.source_node_id().to_string(),
                target_ref: relationship.target_node_id().to_string(),
                rel: relationship.relationship_type().to_string(),
                semantic_class: proto_semantic_class(explanation.semantic_class()) as i32,
                why: explanation.rationale().unwrap_or_default().to_string(),
                evidence: explanation.evidence().unwrap_or_default().to_string(),
                confidence: proto_confidence(explanation.confidence()) as i32,
                sequence: explanation.sequence(),
            }
        })
        .collect()
}

pub(super) fn memory_evidence_from_bundle(bundle: &RehydrationBundle) -> Vec<MemoryEvidence> {
    bundle
        .node_details()
        .iter()
        .map(|detail| MemoryEvidence {
            id: format!("detail:{}", detail.node_id()),
            supports: vec![detail.node_id().to_string()],
            text: detail.detail().to_string(),
            source: detail.node_id().to_string(),
            time: None,
            metadata: Default::default(),
        })
        .collect()
}

pub(super) fn temporal_relations_from_bundle(
    bundle: &RehydrationBundle,
    selected_refs: &BTreeSet<String>,
) -> Vec<MemoryRelation> {
    memory_relations_from_bundle(bundle)
        .into_iter()
        .filter(|relationship| {
            selected_refs.contains(&relationship.source_ref)
                || selected_refs.contains(&relationship.target_ref)
        })
        .collect()
}

pub(super) fn temporal_evidence_from_bundle(
    bundle: &RehydrationBundle,
    selected_refs: &BTreeSet<String>,
) -> Vec<MemoryEvidence> {
    let mut evidence_refs = selected_refs.clone();
    for relationship in bundle.relationships().iter().filter(|relationship| {
        relationship.relationship_type() == "supports"
            && selected_refs.contains(relationship.target_node_id())
    }) {
        evidence_refs.insert(relationship.source_node_id().to_string());
    }

    bundle
        .node_details()
        .iter()
        .filter(|detail| evidence_refs.contains(detail.node_id()))
        .map(|detail| MemoryEvidence {
            id: format!("detail:{}", detail.node_id()),
            supports: vec![detail.node_id().to_string()],
            text: detail.detail().to_string(),
            source: detail.node_id().to_string(),
            time: None,
            metadata: Default::default(),
        })
        .collect()
}

pub(super) fn proof(
    path: Vec<MemoryRelation>,
    evidence: Vec<MemoryEvidence>,
    missing: Vec<String>,
    confidence: MemoryConfidence,
) -> Proof {
    Proof {
        path,
        evidence,
        conflicts: Vec::new(),
        missing,
        confidence: confidence as i32,
    }
}

pub(super) fn rendered_summary(rendered: &RenderedContext) -> String {
    rendered
        .tiers
        .iter()
        .find(|tier| !tier.content.trim().is_empty())
        .map(|tier| tier.content.clone())
        .or_else(|| {
            rendered
                .sections
                .iter()
                .find(|section| !section.content.trim().is_empty())
                .map(|section| section.content.clone())
        })
        .unwrap_or_else(|| rendered.content.clone())
}

pub(super) fn rendered_current_state(rendered: &RenderedContext) -> Vec<String> {
    let sections = rendered
        .sections
        .iter()
        .take(5)
        .map(|section| section.content.clone())
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    if sections.is_empty() && !rendered.content.trim().is_empty() {
        vec![rendered.content.clone()]
    } else {
        sections
    }
}

pub(super) fn proto_coordinate_from_domain(
    coordinate: &TemporalCoordinate,
) -> ProtoTemporalCoordinate {
    ProtoTemporalCoordinate {
        dimension: coordinate.dimension().to_string(),
        scope_id: coordinate.scope_id().to_string(),
        occurred_at: timestamp_from_sort_or_rfc3339(coordinate.occurred_at()),
        observed_at: timestamp_from_sort_or_rfc3339(coordinate.observed_at()),
        ingested_at: timestamp_from_sort_or_rfc3339(coordinate.ingested_at()),
        valid_from: timestamp_from_sort_or_rfc3339(coordinate.valid_from()),
        valid_until: timestamp_from_sort_or_rfc3339(coordinate.valid_until()),
        sequence: coordinate.sequence(),
        rank: coordinate.rank(),
        metadata: Default::default(),
    }
}
