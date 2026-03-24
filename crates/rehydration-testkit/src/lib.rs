mod container_runtime;
pub mod dataset_generator;
mod in_memory_stores;
pub mod llm_evaluator;
pub mod seed_publisher;

pub use container_runtime::ensure_testcontainers_runtime;
pub use dataset_generator::{
    Domain, GeneratedNode, GeneratedRelation, GeneratedSeed, GraphSeedConfig, RelationMix,
    generate_seed,
};
pub use in_memory_stores::{
    InMemoryContextEventStore, InMemoryGraphNeighborhoodReader, InMemoryNodeDetailReader,
    InMemoryProcessedEventStore, InMemoryProjectionCheckpointStore, InMemoryProjectionWriter,
    NoopSnapshotStore,
};
pub use llm_evaluator::{
    EvaluationGroundTruth, LlmEvaluationResult, LlmEvaluatorConfig, evaluate_with_llm,
};

#[cfg(test)]
fn seed_bundle(
    case_id: rehydration_domain::CaseId,
    role: &str,
) -> rehydration_domain::RehydrationBundle {
    let role = rehydration_domain::Role::new(role).expect("role must be valid");
    rehydration_domain::RehydrationBundle::new(
        case_id.clone(),
        role.clone(),
        rehydration_domain::BundleNode::new(
            case_id.as_str(),
            "capability",
            format!("Node {}", case_id.as_str()),
            format!(
                "bundle for node {} role {}",
                case_id.as_str(),
                role.as_str()
            ),
            "ACTIVE",
            vec!["projection-node".to_string()],
            std::collections::BTreeMap::new(),
        ),
        Vec::new(),
        Vec::new(),
        vec![rehydration_domain::BundleNodeDetail::new(
            case_id.as_str(),
            format!(
                "bundle for node {} role {}",
                case_id.as_str(),
                role.as_str()
            ),
            "pending",
            1,
        )],
        rehydration_domain::BundleMetadata::initial("0.1.0"),
    )
    .expect("seed bundle should be valid")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rehydration_domain::{
        CaseId, GraphNeighborhoodReader, NodeDetailProjection, NodeDetailReader, NodeNeighborhood,
        NodeProjection, ProcessedEventStore, ProjectionCheckpoint, ProjectionCheckpointStore,
        ProjectionMutation, ProjectionWriter, SnapshotStore,
    };

    use super::{
        InMemoryGraphNeighborhoodReader, InMemoryNodeDetailReader, InMemoryProcessedEventStore,
        InMemoryProjectionCheckpointStore, InMemoryProjectionWriter, NoopSnapshotStore,
        seed_bundle,
    };

    #[tokio::test]
    async fn in_memory_graph_reader_returns_seeded_neighborhood() {
        let reader = InMemoryGraphNeighborhoodReader::with_neighborhood(NodeNeighborhood {
            root: NodeProjection {
                node_id: "node-123".to_string(),
                node_kind: "capability".to_string(),
                title: "Projection".to_string(),
                summary: String::new(),
                status: "ACTIVE".to_string(),
                labels: vec!["projection".to_string()],
                properties: BTreeMap::new(),
            },
            neighbors: Vec::new(),
            relations: Vec::new(),
        });

        let loaded = reader
            .load_neighborhood("node-123", 1)
            .await
            .expect("load should succeed");

        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn in_memory_node_detail_reader_returns_seeded_detail() {
        let reader = InMemoryNodeDetailReader::with_details([NodeDetailProjection {
            node_id: "node-123".to_string(),
            detail: "Expanded detail".to_string(),
            content_hash: "hash-1".to_string(),
            revision: 1,
        }]);

        let loaded = reader
            .load_node_detail("node-123")
            .await
            .expect("load should succeed");

        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn in_memory_projection_writer_records_mutations() {
        let writer = InMemoryProjectionWriter::default();
        writer
            .apply_mutations(vec![
                ProjectionMutation::UpsertNode(NodeProjection {
                    node_id: "node-123".to_string(),
                    node_kind: "task".to_string(),
                    title: "Task 123".to_string(),
                    summary: "Projection updated".to_string(),
                    status: "ACTIVE".to_string(),
                    labels: vec!["work-item".to_string()],
                    properties: BTreeMap::new(),
                }),
                ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
                    node_id: "node-123".to_string(),
                    detail: "expanded node detail".to_string(),
                    content_hash: "hash-1".to_string(),
                    revision: 1,
                }),
            ])
            .await
            .expect("write should succeed");

        assert_eq!(writer.mutations().await.len(), 2);
    }

    #[tokio::test]
    async fn in_memory_processed_event_store_tracks_deduplication() {
        let store = InMemoryProcessedEventStore::default();
        assert!(
            !store
                .has_processed("context-projection", "evt-1")
                .await
                .expect("lookup should succeed")
        );

        store
            .record_processed("context-projection", "evt-1")
            .await
            .expect("record should succeed");

        assert!(
            store
                .has_processed("context-projection", "evt-1")
                .await
                .expect("lookup should succeed")
        );
        assert_eq!(store.processed().await.len(), 1);
    }

    #[tokio::test]
    async fn in_memory_checkpoint_store_persists_latest_checkpoint() {
        let store = InMemoryProjectionCheckpointStore::default();
        store
            .save_checkpoint(ProjectionCheckpoint {
                consumer_name: "context-projection".to_string(),
                stream_name: "rehydration.events".to_string(),
                last_subject: "graph.node.materialized".to_string(),
                last_event_id: "evt-1".to_string(),
                last_correlation_id: "corr-1".to_string(),
                last_occurred_at: "2026-03-07T00:00:00Z".to_string(),
                processed_events: 1,
                updated_at: std::time::SystemTime::UNIX_EPOCH,
            })
            .await
            .expect("save should succeed");

        let checkpoint = store
            .checkpoint("context-projection", "rehydration.events")
            .await
            .expect("checkpoint should exist");
        assert_eq!(checkpoint.last_event_id, "evt-1");
    }

    #[tokio::test]
    async fn noop_snapshot_store_accepts_bundle() {
        let bundle = seed_bundle(
            CaseId::new("case-123").expect("case id is valid"),
            "developer",
        );

        NoopSnapshotStore
            .save_bundle(&bundle)
            .await
            .expect("noop snapshot store should accept bundles");
    }
}
