use rehydration_proto::v1beta1::{
    AskRequest, InspectRequest, TemporalMoveRequest, TemporalNearRequest, TraceRequest, WakeRequest,
};
use serde_json::Value;

use crate::args::{optional_string, required_string, validate_required_arguments};

use super::common::{answer_policy_from_object, memory_budget_from_arguments, object};
use super::dimensions::dimension_selection_from_arguments;
use super::temporal::{
    inspect_include_from_arguments, temporal_cursor_from_arguments,
    temporal_include_from_arguments, temporal_limit_from_arguments, temporal_window_from_arguments,
};

pub(in crate::grpc) fn wake_request_from_arguments(
    arguments: &Value,
) -> Result<WakeRequest, String> {
    validate_required_arguments(arguments, &["about"])?;
    Ok(WakeRequest {
        about: required_string(arguments, "about")?,
        role: optional_string(arguments, "role").unwrap_or_default(),
        intent: optional_string(arguments, "intent").unwrap_or_default(),
        budget: Some(memory_budget_from_arguments(arguments, 1600, 2)?),
        dimensions: dimension_selection_from_arguments(arguments)?,
    })
}

pub(in crate::grpc) fn ask_request_from_arguments(arguments: &Value) -> Result<AskRequest, String> {
    validate_required_arguments(arguments, &["about", "question"])?;
    let arguments_object = object(arguments, "tool arguments")?;
    Ok(AskRequest {
        about: required_string(arguments, "about")?,
        question: required_string(arguments, "question")?,
        answer_policy: answer_policy_from_object(arguments_object)?,
        budget: Some(memory_budget_from_arguments(arguments, 2400, 2)?),
        dimensions: dimension_selection_from_arguments(arguments)?,
    })
}

pub(in crate::grpc) fn temporal_move_request_from_arguments(
    arguments: &Value,
    direction: &str,
) -> Result<TemporalMoveRequest, String> {
    validate_required_arguments(arguments, &["about"])?;
    let cursor_key = match direction {
        "goto" => "at",
        "rewind" | "forward" => "from",
        _ => return Err(format!("unknown temporal direction `{direction}`")),
    };

    Ok(TemporalMoveRequest {
        about: required_string(arguments, "about")?,
        cursor: Some(temporal_cursor_from_arguments(arguments, cursor_key)?),
        dimensions: dimension_selection_from_arguments(arguments)?,
        window: temporal_window_from_arguments(arguments)?,
        limit: temporal_limit_from_arguments(arguments)?,
        include: temporal_include_from_arguments(arguments)?,
        budget: Some(memory_budget_from_arguments(arguments, 2400, 3)?),
    })
}

pub(in crate::grpc) fn temporal_near_request_from_arguments(
    arguments: &Value,
) -> Result<TemporalNearRequest, String> {
    validate_required_arguments(arguments, &["about"])?;
    Ok(TemporalNearRequest {
        about: required_string(arguments, "about")?,
        around: Some(temporal_cursor_from_arguments(arguments, "around")?),
        dimensions: dimension_selection_from_arguments(arguments)?,
        window: temporal_window_from_arguments(arguments)?,
        limit: temporal_limit_from_arguments(arguments)?,
        include: temporal_include_from_arguments(arguments)?,
        budget: Some(memory_budget_from_arguments(arguments, 2400, 3)?),
    })
}

pub(in crate::grpc) fn trace_request_from_arguments(
    arguments: &Value,
) -> Result<TraceRequest, String> {
    validate_required_arguments(arguments, &["from", "to"])?;
    Ok(TraceRequest {
        from: required_string(arguments, "from")?,
        to: required_string(arguments, "to")?,
        goal: optional_string(arguments, "goal")
            .or_else(|| optional_string(arguments, "role"))
            .unwrap_or_default(),
        budget: Some(memory_budget_from_arguments(arguments, 1600, 1)?),
    })
}

pub(in crate::grpc) fn inspect_request_from_arguments(
    arguments: &Value,
) -> Result<InspectRequest, String> {
    validate_required_arguments(arguments, &["ref"])?;
    Ok(InspectRequest {
        r#ref: required_string(arguments, "ref")?,
        include: inspect_include_from_arguments(arguments)?,
    })
}
