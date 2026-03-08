use rehydration_domain::{GraphNeighborhoodReader, NodeDetailReader, SnapshotStore};
use rehydration_proto::fleet_context_v1::{
    GetGraphRelationshipsRequest, GetGraphRelationshipsResponse,
};
use tonic::{Request, Response, Status};

use crate::transport::context_service_compatibility::{
    ContextCompatibilityGrpcService, request_mapping::map_get_graph_relationships_query,
    response_mapping::proto_get_graph_relationships_response,
    status_mapping::map_compatibility_error,
};

pub(crate) async fn handle<G, D, S>(
    service: &ContextCompatibilityGrpcService<G, D, S>,
    request: Request<GetGraphRelationshipsRequest>,
) -> Result<Response<GetGraphRelationshipsResponse>, Status>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    let result = service
        .admin_query_application
        .get_graph_relationships(map_get_graph_relationships_query(request.into_inner()))
        .await
        .map_err(map_compatibility_error)?;

    Ok(Response::new(proto_get_graph_relationships_response(
        &result,
    )))
}
