use rehydration_application::{BundleSnapshotResult, RehydrateSessionResult};
use rehydration_domain::RehydrationBundle;
use rehydration_proto::v1alpha1::{
    BundleSnapshot, GetBundleSnapshotResponse, GraphRoleBundle, RehydrateSessionResponse,
    RehydrationBundle as ProtoRehydrationBundle, RehydrationStats,
};

use crate::transport::proto_mapping::{
    proto_bundle_node, proto_bundle_node_detail, proto_bundle_relationship, proto_bundle_version,
};
use crate::transport::support::{proto_duration, timestamp_from};

pub(crate) fn proto_rehydrate_session_response(
    result: &RehydrateSessionResult,
) -> RehydrateSessionResponse {
    RehydrateSessionResponse {
        bundle: Some(ProtoRehydrationBundle {
            root_node_id: result.root_node_id.clone(),
            bundles: result.bundles.iter().map(proto_graph_role_bundle).collect(),
            stats: Some(proto_session_stats(result)),
            version: Some(proto_bundle_version(&result.version)),
        }),
        snapshot_persisted: result.snapshot_persisted,
        snapshot_id: result.snapshot_id.clone().unwrap_or_default(),
        generated_at: Some(timestamp_from(result.generated_at)),
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

pub(crate) fn proto_bundle_from_single_role(bundle: &RehydrationBundle) -> ProtoRehydrationBundle {
    ProtoRehydrationBundle {
        root_node_id: bundle.root_node_id().as_str().to_string(),
        bundles: vec![proto_graph_role_bundle(bundle)],
        stats: Some(proto_bundle_stats(bundle, 0, 1)),
        version: Some(proto_bundle_version(bundle.metadata())),
    }
}

fn proto_graph_role_bundle(bundle: &RehydrationBundle) -> GraphRoleBundle {
    GraphRoleBundle {
        role: bundle.role().as_str().to_string(),
        root_node: Some(proto_bundle_node(bundle.root_node())),
        neighbor_nodes: bundle
            .neighbor_nodes()
            .iter()
            .map(proto_bundle_node)
            .collect(),
        relationships: bundle
            .relationships()
            .iter()
            .map(proto_bundle_relationship)
            .collect(),
        node_details: bundle
            .node_details()
            .iter()
            .map(proto_bundle_node_detail)
            .collect(),
    }
}

fn proto_session_stats(result: &RehydrateSessionResult) -> RehydrationStats {
    let nodes = result
        .bundles
        .iter()
        .map(|bundle| bundle.stats().selected_nodes())
        .sum();
    let relationships = result
        .bundles
        .iter()
        .map(|bundle| bundle.stats().selected_relationships())
        .sum();
    let detailed_nodes = result
        .bundles
        .iter()
        .map(|bundle| bundle.stats().detailed_nodes())
        .sum();

    RehydrationStats {
        roles: result.bundles.len() as u32,
        nodes,
        relationships,
        detailed_nodes,
        timeline_events: result.timeline_events,
    }
}

fn proto_bundle_stats(
    bundle: &RehydrationBundle,
    timeline_events: u32,
    roles: u32,
) -> RehydrationStats {
    RehydrationStats {
        roles,
        nodes: bundle.stats().selected_nodes(),
        relationships: bundle.stats().selected_relationships(),
        detailed_nodes: bundle.stats().detailed_nodes(),
        timeline_events,
    }
}
