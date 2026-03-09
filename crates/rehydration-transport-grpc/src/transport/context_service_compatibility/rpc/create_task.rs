use rehydration_proto::fleet_context_v1::{CreateTaskRequest, CreateTaskResponse};
use tonic::{Request, Response, Status};

use crate::transport::context_service_compatibility::status_mapping::unimplemented_status;

pub(crate) async fn handle(
    _request: Request<CreateTaskRequest>,
) -> Result<Response<CreateTaskResponse>, Status> {
    Err(unimplemented_status("CreateTask"))
}
