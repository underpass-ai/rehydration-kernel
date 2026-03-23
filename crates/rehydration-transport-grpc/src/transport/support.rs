use std::time::SystemTime;

use prost_types::{Duration as ProtoDuration, Timestamp};
use rehydration_application::{ApplicationError, ReplayModeSelection};
use rehydration_proto::v1beta1::ReplayMode;
use tonic::Status;

pub(crate) fn map_replay_mode(value: i32) -> ReplayModeSelection {
    match ReplayMode::try_from(value).unwrap_or(ReplayMode::DryRun) {
        ReplayMode::DryRun => ReplayModeSelection::DryRun,
        ReplayMode::Rebuild => ReplayModeSelection::Rebuild,
        ReplayMode::Unspecified => ReplayModeSelection::DryRun,
    }
}

pub(crate) fn proto_replay_mode(value: ReplayModeSelection) -> ReplayMode {
    match value {
        ReplayModeSelection::DryRun => ReplayMode::DryRun,
        ReplayModeSelection::Rebuild => ReplayMode::Rebuild,
    }
}

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

pub(crate) fn proto_duration(seconds: u64) -> ProtoDuration {
    ProtoDuration {
        seconds: seconds as i64,
        nanos: 0,
    }
}

pub(crate) fn trim_to_option(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
