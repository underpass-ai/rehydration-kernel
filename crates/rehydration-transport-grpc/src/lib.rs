pub mod agentic_reference;
mod transport;

pub use transport::{
    AdminGrpcService, CommandGrpcService, ContextCompatibilityGrpcService, GrpcServer,
    QueryGrpcService,
};
