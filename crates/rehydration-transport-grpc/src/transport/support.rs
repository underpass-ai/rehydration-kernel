use std::time::SystemTime;

#[cfg(test)]
use prost_types::Duration as ProtoDuration;
use prost_types::Timestamp;
use rehydration_application::ApplicationError;
use tonic::Status;

pub(crate) fn map_application_error(error: ApplicationError) -> Status {
    match error {
        ApplicationError::Domain(domain_error) => {
            Status::invalid_argument(domain_error.to_string())
        }
        ApplicationError::Ports(port_error) => match port_error {
            rehydration_ports::PortError::InvalidState(message) => {
                Status::failed_precondition(message)
            }
            rehydration_ports::PortError::Unavailable(message) => Status::unavailable(message),
            rehydration_ports::PortError::Conflict(message) => Status::aborted(message),
        },
        ApplicationError::NotFound(message) => Status::not_found(message),
        ApplicationError::Validation(message) => Status::invalid_argument(message),
    }
}

pub(crate) fn timestamp_from(value: SystemTime) -> Timestamp {
    Timestamp::from(value)
}

#[cfg(test)]
pub(crate) fn proto_duration(seconds: u64) -> ProtoDuration {
    ProtoDuration {
        seconds: seconds as i64,
        nanos: 0,
    }
}

#[cfg(test)]
pub(crate) fn trim_to_option(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
