use std::sync::Arc;

use rehydration_application::{
    CommandApplicationService, UpdateContextChange, UpdateContextCommand,
};
use rehydration_proto::v1alpha1::{
    CommandMetadata, RevisionPrecondition, UpdateContextRequest, UpdateContextResponse,
    context_command_service_server::ContextCommandService,
};
use tonic::{Request, Response, Status};

use crate::transport::proto_mapping_v1alpha1::proto_accepted_version;
use crate::transport::support::map_application_error;

#[derive(Debug, Clone)]
pub struct CommandGrpcService {
    application: Arc<CommandApplicationService>,
}

impl CommandGrpcService {
    pub fn new(application: Arc<CommandApplicationService>) -> Self {
        Self { application }
    }
}

#[tonic::async_trait]
impl ContextCommandService for CommandGrpcService {
    async fn update_context(
        &self,
        request: Request<UpdateContextRequest>,
    ) -> Result<Response<UpdateContextResponse>, Status> {
        let request = request.into_inner();
        let metadata = request.metadata.unwrap_or(CommandMetadata {
            idempotency_key: String::new(),
            correlation_id: String::new(),
            causation_id: String::new(),
            requested_by: String::new(),
            requested_at: None,
        });
        let precondition = request.precondition.unwrap_or(RevisionPrecondition {
            expected_revision: 0,
            expected_content_hash: String::new(),
        });

        let outcome = self
            .application
            .update_context(UpdateContextCommand {
                root_node_id: request.root_node_id,
                role: request.role,
                work_item_id: request.work_item_id,
                changes: request
                    .changes
                    .into_iter()
                    .map(|change| UpdateContextChange {
                        operation: change.operation().as_str_name().to_string(),
                        entity_kind: change.entity_kind,
                        entity_id: change.entity_id,
                        payload_json: change.payload_json,
                        reason: change.reason,
                        scopes: change.scopes,
                    })
                    .collect(),
                expected_revision: (precondition.expected_revision != 0)
                    .then_some(precondition.expected_revision),
                expected_content_hash: (!precondition.expected_content_hash.is_empty())
                    .then_some(precondition.expected_content_hash),
                idempotency_key: (!metadata.idempotency_key.is_empty())
                    .then_some(metadata.idempotency_key),
                requested_by: (!metadata.requested_by.is_empty()).then_some(metadata.requested_by),
                persist_snapshot: request.persist_snapshot,
            })
            .map_err(map_application_error)?;

        Ok(Response::new(UpdateContextResponse {
            accepted_version: Some(proto_accepted_version(&outcome.accepted_version)),
            warnings: outcome.warnings,
            snapshot_persisted: outcome.snapshot_persisted,
            snapshot_id: outcome.snapshot_id.unwrap_or_default(),
        }))
    }
}
