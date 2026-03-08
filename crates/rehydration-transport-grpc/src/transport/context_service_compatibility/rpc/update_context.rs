use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, SnapshotStore};
use rehydration_proto::fleet_context_v1::{UpdateContextRequest, UpdateContextResponse};
use tonic::{Request, Response, Status};

use crate::transport::context_service_compatibility::{
    ContextCompatibilityGrpcService, request_mapping::map_update_context_command,
    response_mapping::proto_update_context_response, status_mapping::map_compatibility_error,
};

pub(crate) async fn handle<G, D, S>(
    service: &ContextCompatibilityGrpcService<G, D, S>,
    request: Request<UpdateContextRequest>,
) -> Result<Response<UpdateContextResponse>, Status>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    let outcome = service
        .command_application
        .update_context(map_update_context_command(request.into_inner()))
        .map_err(map_compatibility_error)?;

    Ok(Response::new(proto_update_context_response(&outcome)))
}
