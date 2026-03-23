#[path = "admin_grpc_service.rs"]
pub mod admin_grpc_service_v1alpha1;
pub mod admin_grpc_service_v1beta1;
#[path = "command_grpc_service.rs"]
pub mod command_grpc_service_v1alpha1;
pub mod command_grpc_service_v1beta1;
pub mod context_service_compatibility;
pub mod grpc_server;
#[path = "proto_mapping/mod.rs"]
pub mod proto_mapping_v1alpha1;
pub mod proto_mapping_v1beta1;
#[path = "query_grpc_service.rs"]
pub mod query_grpc_service_v1alpha1;
pub mod query_grpc_service_v1beta1;
pub mod support;

#[cfg(test)]
mod tests;

pub use admin_grpc_service_v1alpha1::AdminGrpcService as AdminGrpcServiceV1Alpha1;
pub use admin_grpc_service_v1beta1::AdminGrpcServiceV1Beta1 as AdminGrpcService;
pub use command_grpc_service_v1alpha1::CommandGrpcService as CommandGrpcServiceV1Alpha1;
pub use command_grpc_service_v1beta1::CommandGrpcServiceV1Beta1 as CommandGrpcService;
pub use context_service_compatibility::ContextCompatibilityGrpcService;
pub use grpc_server::GrpcServer;
pub use query_grpc_service_v1alpha1::QueryGrpcService as QueryGrpcServiceV1Alpha1;
pub use query_grpc_service_v1beta1::QueryGrpcServiceV1Beta1 as QueryGrpcService;
