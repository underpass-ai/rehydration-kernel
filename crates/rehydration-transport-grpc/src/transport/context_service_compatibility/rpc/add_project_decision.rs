use rehydration_proto::fleet_context_v1::{AddProjectDecisionRequest, AddProjectDecisionResponse};
use tonic::{Request, Response, Status};

use crate::transport::context_service_compatibility::status_mapping::unimplemented_status;

pub(crate) async fn handle(
    _request: Request<AddProjectDecisionRequest>,
) -> Result<Response<AddProjectDecisionResponse>, Status> {
    Err(unimplemented_status("AddProjectDecision"))
}
