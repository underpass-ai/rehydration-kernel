use rehydration_proto::fleet_context_v1::{ValidateScopeRequest, ValidateScopeResponse};
use tonic::{Request, Response, Status};

use crate::transport::context_service_compatibility::status_mapping::unimplemented_status;

pub(crate) async fn handle(
    _request: Request<ValidateScopeRequest>,
) -> Result<Response<ValidateScopeResponse>, Status> {
    Err(unimplemented_status("ValidateScope"))
}
