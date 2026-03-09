use rehydration_proto::fleet_context_v1::{ValidateScopeRequest, ValidateScopeResponse};
use tonic::{Request, Response, Status};

use crate::transport::context_service_compatibility::{
    ContextCompatibilityGrpcService, request_mapping::map_validate_scope_query,
    response_mapping::proto_validate_scope_response,
};

pub(crate) async fn handle<G, D, S>(
    service: &ContextCompatibilityGrpcService<G, D, S>,
    request: Request<ValidateScopeRequest>,
) -> Result<Response<ValidateScopeResponse>, Status> {
    let result = service
        .query_application
        .validate_scope(map_validate_scope_query(request.into_inner()));

    Ok(Response::new(proto_validate_scope_response(&result)))
}
