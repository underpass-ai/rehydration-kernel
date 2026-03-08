use rehydration_application::ApplicationError;
use tonic::Status;

pub(crate) fn map_compatibility_error(error: ApplicationError) -> Status {
    match error {
        ApplicationError::Domain(domain_error) => {
            Status::invalid_argument(domain_error.to_string())
        }
        ApplicationError::Validation(message) => Status::invalid_argument(message),
        ApplicationError::Ports(port_error) => Status::internal(port_error.to_string()),
    }
}

pub(crate) fn unimplemented_status(operation: &str) -> Status {
    Status::unimplemented(format!("{operation} compatibility is not implemented yet"))
}

#[cfg(test)]
mod tests {
    use rehydration_application::ApplicationError;
    use rehydration_domain::{DomainError, PortError};

    use super::{map_compatibility_error, unimplemented_status};

    #[test]
    fn compatibility_status_mapping_matches_phase_one_boundary_rules() {
        assert_eq!(
            map_compatibility_error(ApplicationError::Validation("bad".to_string())).code(),
            tonic::Code::InvalidArgument
        );
        assert_eq!(
            map_compatibility_error(ApplicationError::Domain(DomainError::EmptyValue("node_id")))
                .code(),
            tonic::Code::InvalidArgument
        );
        assert_eq!(
            map_compatibility_error(ApplicationError::Ports(PortError::Unavailable(
                "down".to_string()
            )))
            .code(),
            tonic::Code::Internal
        );
        assert_eq!(
            unimplemented_status("ValidateScope").code(),
            tonic::Code::Unimplemented
        );
    }
}
