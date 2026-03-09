use std::collections::BTreeMap;
use std::sync::Arc;

use rehydration_adapter_nats::ContextAsyncApplication;
use rehydration_application::{
    CommandApplicationService, QueryApplicationService, UpdateContextUseCase,
};
use rehydration_domain::{NodeDetailProjection, NodeNeighborhood, NodeProjection};
use rehydration_testkit::{
    InMemoryGraphNeighborhoodReader, InMemoryNodeDetailReader, NoopSnapshotStore,
};

pub(crate) fn seeded_service() -> ContextAsyncApplication<
    InMemoryGraphNeighborhoodReader,
    InMemoryNodeDetailReader,
    NoopSnapshotStore,
> {
    let command_application = Arc::new(CommandApplicationService::new(Arc::new(
        UpdateContextUseCase::new("0.1.0"),
    )));
    let graph_reader = Arc::new(InMemoryGraphNeighborhoodReader::with_neighborhood(
        NodeNeighborhood {
            root: NodeProjection {
                node_id: "case-123".to_string(),
                node_kind: "story".to_string(),
                title: "Story".to_string(),
                summary: "Story summary".to_string(),
                status: "ACTIVE".to_string(),
                labels: vec!["Story".to_string()],
                properties: BTreeMap::new(),
            },
            neighbors: vec![NodeProjection {
                node_id: "decision-1".to_string(),
                node_kind: "decision".to_string(),
                title: "Decision".to_string(),
                summary: "Decision summary".to_string(),
                status: "ACTIVE".to_string(),
                labels: vec!["Decision".to_string()],
                properties: BTreeMap::new(),
            }],
            relations: Vec::new(),
        },
    ));
    let detail_reader = Arc::new(InMemoryNodeDetailReader::with_details([
        NodeDetailProjection {
            node_id: "case-123".to_string(),
            detail: "Expanded detail".to_string(),
            content_hash: "hash-1".to_string(),
            revision: 1,
        },
    ]));
    let query_application = Arc::new(QueryApplicationService::new(
        graph_reader,
        detail_reader,
        Arc::new(NoopSnapshotStore),
        "0.1.0",
    ));

    ContextAsyncApplication::new(command_application, query_application)
}
