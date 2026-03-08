use std::time::SystemTime;

use rehydration_application::{
    AcceptedVersion, BundleSnapshotResult, GetContextResult, GetGraphRelationshipsResult,
    GetProjectionStatusResult, GetRehydrationDiagnosticsResult, GraphNodeView,
    GraphRelationshipView, ProjectionStatusView, RehydrateSessionResult, RehydrationDiagnosticView,
    ReplayProjectionOutcome, ScopeValidation,
};
use rehydration_domain::{BundleMetadata, RehydrationBundle};
use rehydration_proto::v1alpha1::{
    BundleRenderFormat, BundleSection, BundleSnapshot, BundleVersion, CaseHeader, Decision,
    DecisionRelation, GetBundleSnapshotResponse, GetGraphRelationshipsResponse,
    GetProjectionStatusResponse, GetRehydrationDiagnosticsResponse, GraphNode, GraphRelationship,
    Milestone, PlanHeader, ProjectionStatus, RehydrateSessionResponse,
    RehydrationBundle as ProtoRehydrationBundle, RehydrationDiagnostic, RehydrationStats,
    RenderedContext as ProtoRenderedContext, ReplayProjectionResponse, RoleContextPack,
    ScopeValidationResult, TaskImpact, WorkItem,
};

use crate::transport::support::{proto_duration, proto_replay_mode, timestamp_from};

pub(crate) fn proto_rehydrate_session_response(
    result: &RehydrateSessionResult,
) -> RehydrateSessionResponse {
    let decisions = result
        .bundles
        .iter()
        .map(|bundle| bundle.pack().decisions().len() as u32)
        .sum();
    let decision_relations = result
        .bundles
        .iter()
        .map(|bundle| bundle.pack().decision_relations().len() as u32)
        .sum();
    let impacts = result
        .bundles
        .iter()
        .map(|bundle| bundle.pack().impacts().len() as u32)
        .sum();
    let milestones = result
        .bundles
        .iter()
        .map(|bundle| bundle.pack().milestones().len() as u32)
        .sum();

    RehydrateSessionResponse {
        bundle: Some(ProtoRehydrationBundle {
            root_node_id: result.root_node_id.clone(),
            packs: result
                .bundles
                .iter()
                .map(proto_role_pack_from_domain)
                .collect(),
            stats: Some(RehydrationStats {
                roles: result.bundles.len() as u32,
                decisions,
                decision_relations,
                impacts,
                milestones,
                timeline_events: result.timeline_events,
            }),
            version: Some(proto_bundle_version(&result.version)),
        }),
        snapshot_persisted: result.snapshot_persisted,
        snapshot_id: result.snapshot_id.clone().unwrap_or_default(),
        generated_at: Some(timestamp_from(result.generated_at)),
    }
}

pub(crate) fn proto_projection_status_response(
    result: &GetProjectionStatusResult,
) -> GetProjectionStatusResponse {
    GetProjectionStatusResponse {
        projections: result
            .projections
            .iter()
            .map(proto_projection_status)
            .collect(),
        observed_at: Some(timestamp_from(result.observed_at)),
    }
}

pub(crate) fn proto_replay_projection_response(
    result: &ReplayProjectionOutcome,
) -> ReplayProjectionResponse {
    ReplayProjectionResponse {
        replay_id: result.replay_id.clone(),
        consumer_name: result.consumer_name.clone(),
        replay_mode: proto_replay_mode(result.replay_mode) as i32,
        accepted_events: result.accepted_events,
        requested_at: Some(timestamp_from(result.requested_at)),
    }
}

pub(crate) fn proto_bundle_snapshot_response(
    result: &BundleSnapshotResult,
) -> GetBundleSnapshotResponse {
    GetBundleSnapshotResponse {
        snapshot: Some(BundleSnapshot {
            snapshot_id: result.snapshot_id.clone(),
            root_node_id: result.root_node_id.clone(),
            role: result.role.clone(),
            bundle: Some(proto_bundle_from_single_role(&result.bundle)),
            created_at: Some(timestamp_from(result.created_at)),
            expires_at: Some(timestamp_from(result.expires_at)),
            ttl: Some(proto_duration(result.ttl_seconds)),
        }),
    }
}

pub(crate) fn proto_graph_relationships_response(
    result: &GetGraphRelationshipsResult,
) -> GetGraphRelationshipsResponse {
    GetGraphRelationshipsResponse {
        root: Some(proto_graph_node(&result.root)),
        neighbors: result.neighbors.iter().map(proto_graph_node).collect(),
        relationships: result
            .relationships
            .iter()
            .map(proto_graph_relationship)
            .collect(),
        observed_at: Some(timestamp_from(result.observed_at)),
    }
}

pub(crate) fn proto_rehydration_diagnostics_response(
    result: &GetRehydrationDiagnosticsResult,
) -> GetRehydrationDiagnosticsResponse {
    GetRehydrationDiagnosticsResponse {
        diagnostics: result.diagnostics.iter().map(proto_diagnostic).collect(),
        observed_at: Some(timestamp_from(result.observed_at)),
    }
}

pub(crate) fn proto_bundle_from_single_role(bundle: &RehydrationBundle) -> ProtoRehydrationBundle {
    ProtoRehydrationBundle {
        root_node_id: bundle.root_node_id().as_str().to_string(),
        packs: vec![proto_role_pack_from_domain(bundle)],
        stats: Some(RehydrationStats {
            roles: 1,
            decisions: bundle.pack().decisions().len() as u32,
            decision_relations: bundle.pack().decision_relations().len() as u32,
            impacts: bundle.pack().impacts().len() as u32,
            milestones: bundle.pack().milestones().len() as u32,
            timeline_events: 0,
        }),
        version: Some(proto_bundle_version(bundle.metadata())),
    }
}

pub(crate) fn proto_role_pack_from_domain(bundle: &RehydrationBundle) -> RoleContextPack {
    let pack = bundle.pack();
    let case_header = pack.case_header();

    RoleContextPack {
        role: pack.role().as_str().to_string(),
        case_header: Some(CaseHeader {
            root_node_id: case_header.case_id().as_str().to_string(),
            title: case_header.title().to_string(),
            summary: case_header.summary().to_string(),
            status: case_header.status().to_string(),
            created_at: Some(timestamp_from(case_header.created_at())),
            created_by: case_header.created_by().to_string(),
        }),
        plan_header: pack.plan_header().map(|plan| PlanHeader {
            plan_id: plan.plan_id().to_string(),
            revision: plan.revision(),
            status: plan.status().to_string(),
            work_items_total: plan.work_items_total(),
            work_items_completed: plan.work_items_completed(),
        }),
        work_items: pack
            .work_items()
            .iter()
            .map(|work_item| WorkItem {
                work_item_id: work_item.work_item_id().to_string(),
                title: work_item.title().to_string(),
                summary: work_item.summary().to_string(),
                role: work_item.role().to_string(),
                phase: work_item.phase().to_string(),
                status: work_item.status().to_string(),
                dependency_ids: work_item.dependency_ids().to_vec(),
                priority: work_item.priority(),
            })
            .collect(),
        decisions: pack
            .decisions()
            .iter()
            .map(|decision| Decision {
                decision_id: decision.decision_id().to_string(),
                title: decision.title().to_string(),
                rationale: decision.rationale().to_string(),
                status: decision.status().to_string(),
                owner: decision.owner().to_string(),
                decided_at: Some(timestamp_from(decision.decided_at())),
            })
            .collect(),
        decision_relations: pack
            .decision_relations()
            .iter()
            .map(|relation| DecisionRelation {
                source_decision_id: relation.source_decision_id().to_string(),
                target_decision_id: relation.target_decision_id().to_string(),
                relation_type: relation.relation_type().to_string(),
            })
            .collect(),
        impacts: pack
            .impacts()
            .iter()
            .map(|impact| TaskImpact {
                decision_id: impact.decision_id().to_string(),
                work_item_id: impact.work_item_id().to_string(),
                title: impact.title().to_string(),
                impact_type: impact.impact_type().to_string(),
            })
            .collect(),
        milestones: pack
            .milestones()
            .iter()
            .map(|milestone| Milestone {
                milestone_type: milestone.milestone_type().to_string(),
                description: milestone.description().to_string(),
                occurred_at: Some(timestamp_from(milestone.occurred_at())),
                actor: milestone.actor().to_string(),
            })
            .collect(),
        latest_summary: pack.latest_summary().to_string(),
        token_budget_hint: pack.token_budget_hint(),
    }
}

pub(crate) fn proto_rendered_context_from_result(
    result: &GetContextResult,
) -> ProtoRenderedContext {
    ProtoRenderedContext {
        format: BundleRenderFormat::Structured as i32,
        content: result.rendered.content.clone(),
        token_count: result.rendered.token_count,
        sections: result
            .rendered
            .sections
            .iter()
            .enumerate()
            .map(|(index, section)| BundleSection {
                key: format!("section_{index}"),
                title: format!("Section {}", index + 1),
                content: section.clone(),
                token_count: section.split_whitespace().count() as u32,
                scopes: result.scope_validation.provided_scopes.clone(),
            })
            .collect(),
    }
}

pub(crate) fn proto_scope_validation(result: &ScopeValidation) -> ScopeValidationResult {
    ScopeValidationResult {
        allowed: result.allowed,
        required_scopes: result.required_scopes.clone(),
        provided_scopes: result.provided_scopes.clone(),
        missing_scopes: result.missing_scopes.clone(),
        extra_scopes: result.extra_scopes.clone(),
        reason: result.reason.clone(),
        diagnostics: result.diagnostics.clone(),
    }
}

pub(crate) fn proto_projection_status(view: &ProjectionStatusView) -> ProjectionStatus {
    ProjectionStatus {
        consumer_name: view.consumer_name.clone(),
        stream_name: view.stream_name.clone(),
        projection_watermark: view.projection_watermark.clone(),
        processed_events: view.processed_events,
        pending_events: view.pending_events,
        last_event_at: Some(timestamp_from(view.last_event_at)),
        updated_at: Some(timestamp_from(view.updated_at)),
        healthy: view.healthy,
        warnings: view.warnings.clone(),
    }
}

pub(crate) fn proto_graph_node(node: &GraphNodeView) -> GraphNode {
    GraphNode {
        node_id: node.node_id.clone(),
        node_kind: node.node_kind.clone(),
        title: node.title.clone(),
        labels: node.labels.clone(),
        properties: node.properties.clone().into_iter().collect(),
    }
}

pub(crate) fn proto_graph_relationship(relationship: &GraphRelationshipView) -> GraphRelationship {
    GraphRelationship {
        source_node_id: relationship.source_node_id.clone(),
        target_node_id: relationship.target_node_id.clone(),
        relationship_type: relationship.relationship_type.clone(),
        properties: relationship.properties.clone().into_iter().collect(),
    }
}

pub(crate) fn proto_diagnostic(diagnostic: &RehydrationDiagnosticView) -> RehydrationDiagnostic {
    RehydrationDiagnostic {
        role: diagnostic.role.clone(),
        version: Some(proto_bundle_version(&diagnostic.version)),
        selected_decisions: diagnostic.selected_decisions,
        selected_impacts: diagnostic.selected_impacts,
        selected_milestones: diagnostic.selected_milestones,
        estimated_tokens: diagnostic.estimated_tokens,
        notes: diagnostic.notes.clone(),
    }
}

pub(crate) fn proto_accepted_version(version: &AcceptedVersion) -> BundleVersion {
    BundleVersion {
        revision: version.revision,
        content_hash: version.content_hash.clone(),
        schema_version: "v1alpha1".to_string(),
        projection_watermark: format!("rev-{}", version.revision),
        generated_at: Some(timestamp_from(SystemTime::now())),
        generator_version: version.generator_version.clone(),
    }
}

pub(crate) fn proto_bundle_version(metadata: &BundleMetadata) -> BundleVersion {
    BundleVersion {
        revision: metadata.revision,
        content_hash: metadata.content_hash.clone(),
        schema_version: "v1alpha1".to_string(),
        projection_watermark: format!("rev-{}", metadata.revision),
        generated_at: Some(timestamp_from(SystemTime::now())),
        generator_version: metadata.generator_version.clone(),
    }
}
