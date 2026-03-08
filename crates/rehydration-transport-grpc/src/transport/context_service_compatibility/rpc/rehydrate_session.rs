use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, SnapshotStore};
use rehydration_proto::fleet_context_v1::{RehydrateSessionRequest, RehydrateSessionResponse};
use tonic::{Request, Response, Status};

use crate::transport::context_service_compatibility::{
    ContextCompatibilityGrpcService, request_mapping::map_rehydrate_session_query,
    response_mapping::proto_rehydrate_session_response, status_mapping::map_compatibility_error,
};

pub(crate) async fn handle<G, D, S>(
    service: &ContextCompatibilityGrpcService<G, D, S>,
    request: Request<RehydrateSessionRequest>,
) -> Result<Response<RehydrateSessionResponse>, Status>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    let result = service
        .query_application
        .rehydrate_session(map_rehydrate_session_query(request.into_inner()))
        .await
        .map_err(map_compatibility_error)?;

    Ok(Response::new(proto_rehydrate_session_response(&result)))
}
