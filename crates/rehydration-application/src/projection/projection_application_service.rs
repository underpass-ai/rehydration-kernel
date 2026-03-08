use rehydration_domain::{
    NodeDetailProjection, NodeProjection, NodeRelationProjection, PortError, ProcessedEventStore,
    ProjectionCheckpoint, ProjectionCheckpointStore, ProjectionMutation, ProjectionWriter,
};

use crate::ApplicationError;
use crate::projection::{
    ProjectionEvent, ProjectionEventHandler, ProjectionHandlingRequest, ProjectionHandlingResult,
};

#[derive(Debug)]
pub struct ProjectionApplicationService<W, P, C> {
    projection_writer: W,
    processed_event_store: P,
    checkpoint_store: C,
}

impl<W, P, C> ProjectionApplicationService<W, P, C>
where
    W: ProjectionWriter + Send + Sync,
    P: ProcessedEventStore + Send + Sync,
    C: ProjectionCheckpointStore + Send + Sync,
{
    pub fn new(projection_writer: W, processed_event_store: P, checkpoint_store: C) -> Self {
        Self {
            projection_writer,
            processed_event_store,
            checkpoint_store,
        }
    }

    async fn mutations_for_event(
        &self,
        event: &ProjectionEvent,
    ) -> Result<Vec<ProjectionMutation>, PortError> {
        Ok(match event {
            ProjectionEvent::GraphNodeMaterialized(event) => {
                let mut mutations = vec![ProjectionMutation::UpsertNode(NodeProjection {
                    node_id: event.data.node_id.clone(),
                    node_kind: event.data.node_kind.clone(),
                    title: event.data.title.clone(),
                    summary: event.data.summary.clone(),
                    status: event.data.status.clone(),
                    labels: event.data.labels.clone(),
                    properties: event.data.properties.clone(),
                })];
                mutations.extend(event.data.related_nodes.iter().map(|reference| {
                    ProjectionMutation::UpsertNodeRelation(NodeRelationProjection {
                        source_node_id: event.data.node_id.clone(),
                        target_node_id: reference.node_id.clone(),
                        relation_type: reference.relation_type.clone(),
                    })
                }));
                mutations
            }
            ProjectionEvent::NodeDetailMaterialized(event) => {
                vec![ProjectionMutation::UpsertNodeDetail(NodeDetailProjection {
                    node_id: event.data.node_id.clone(),
                    detail: event.data.detail.clone(),
                    content_hash: event.data.content_hash.clone(),
                    revision: event.data.revision,
                })]
            }
        })
    }
}

impl<W, P, C> ProjectionEventHandler for ProjectionApplicationService<W, P, C>
where
    W: ProjectionWriter + Send + Sync,
    P: ProcessedEventStore + Send + Sync,
    C: ProjectionCheckpointStore + Send + Sync,
{
    async fn handle_projection_event(
        &self,
        request: ProjectionHandlingRequest,
    ) -> Result<ProjectionHandlingResult, ApplicationError> {
        let event_id = request.event.event_id().to_string();
        if self
            .processed_event_store
            .has_processed(&request.consumer_name, &event_id)
            .await?
        {
            return Ok(ProjectionHandlingResult {
                event_id,
                subject: request.subject,
                duplicate: true,
                applied_mutations: 0,
                checkpoint: None,
            });
        }

        let mutations = self.mutations_for_event(&request.event).await?;
        self.projection_writer
            .apply_mutations(mutations.clone())
            .await?;
        self.processed_event_store
            .record_processed(&request.consumer_name, &event_id)
            .await?;
        let checkpoint = ProjectionCheckpoint {
            consumer_name: request.consumer_name,
            stream_name: request.stream_name,
            last_subject: request.subject.clone(),
            last_event_id: event_id.clone(),
            last_correlation_id: request.event.envelope().correlation_id.clone(),
            last_occurred_at: request.event.envelope().occurred_at.clone(),
            processed_events: 1,
            updated_at: std::time::SystemTime::now(),
        };
        self.checkpoint_store
            .save_checkpoint(checkpoint.clone())
            .await?;

        Ok(ProjectionHandlingResult {
            event_id,
            subject: request.subject,
            duplicate: false,
            applied_mutations: mutations.len(),
            checkpoint: Some(checkpoint),
        })
    }
}
