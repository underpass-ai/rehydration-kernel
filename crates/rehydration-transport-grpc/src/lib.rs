pub mod agentic_reference;
pub mod starship_e2e;
mod transport;

pub use transport::{
    AdminGrpcService, CommandGrpcService, ContextCompatibilityGrpcService, GrpcServer,
    QueryGrpcService,
};
