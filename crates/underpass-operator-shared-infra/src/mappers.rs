use serde_json::Value;
use underpass_operator_shared_contract::{
    OperatorActionDto, PreparedToolCallActionDto, RawTrajectoryDto, StopActionDto,
    ToolCallActionDto,
};
use underpass_operator_shared_domain::{
    AboutId, ActionArguments, AllowedTools, AnswerPolicy, KernelTool, MemoryRef, NonEmptyString,
    OperatorAction, OperatorMode, PreparedPayloadSource, StepId, TaskFamily, TrainingTrajectory,
    VisibleState,
};

use crate::InfraResult;

pub struct TrainingTrajectoryMapper;

impl TrainingTrajectoryMapper {
    pub fn from_json(value: Value) -> InfraResult<TrainingTrajectory> {
        let dto = serde_json::from_value::<RawTrajectoryDto>(value)?;
        Self::from_dto(dto)
    }

    pub fn to_json(trajectory: &TrainingTrajectory) -> InfraResult<Value> {
        Ok(serde_json::to_value(Self::to_dto(trajectory))?)
    }

    pub fn from_dto(dto: RawTrajectoryDto) -> InfraResult<TrainingTrajectory> {
        let mode = OperatorMode::parse(&dto.mode)?;
        let allowed_tools = dto
            .allowed_tools
            .iter()
            .map(|tool| KernelTool::parse(tool))
            .collect::<Result<Vec<_>, _>>()?;
        let allowed_tools = AllowedTools::parse(mode, allowed_tools)?;
        let target_action = OperatorActionMapper::from_dto(dto.target_action)?;
        Ok(TrainingTrajectory::new(
            StepId::parse(dto.step_id)?,
            AboutId::parse(dto.about)?,
            mode,
            TaskFamily::parse(dto.task_family)?,
            allowed_tools,
            VisibleState::parse(dto.visible_state)?,
            target_action,
        )?)
    }

    pub fn to_dto(trajectory: &TrainingTrajectory) -> RawTrajectoryDto {
        RawTrajectoryDto {
            step_id: trajectory.step_id().as_str().to_string(),
            about: trajectory.about().as_str().to_string(),
            mode: trajectory.mode().as_str().to_string(),
            task_family: trajectory.task_family().as_str().to_string(),
            allowed_tools: trajectory
                .allowed_tools()
                .iter()
                .map(|tool| tool.as_str().to_string())
                .collect(),
            visible_state: trajectory.visible_state().as_value().clone(),
            target_action: OperatorActionMapper::to_dto(trajectory.target_action()),
        }
    }
}

pub struct OperatorActionMapper;

impl OperatorActionMapper {
    pub fn from_dto(dto: OperatorActionDto) -> InfraResult<OperatorAction> {
        match dto {
            OperatorActionDto::ToolCall(action) => {
                let tool = KernelTool::parse(&action.tool)?;
                let arguments = ActionArguments::parse(action.arguments)?;
                Ok(OperatorAction::tool_call(tool, arguments))
            }
            OperatorActionDto::PreparedToolCall(action) => {
                let tool = KernelTool::parse(&action.tool)?;
                let source = PreparedPayloadSource::parse(tool, &action.source)?;
                Ok(OperatorAction::prepared_tool_call(tool, source))
            }
            OperatorActionDto::Stop(action) => {
                let answer_policy = AnswerPolicy::parse(&action.answer_policy)?;
                let final_refs = action
                    .final_refs
                    .into_iter()
                    .map(MemoryRef::parse)
                    .collect::<Result<Vec<_>, _>>()?;
                let reason = NonEmptyString::parse(action.reason, "action.reason")?;
                Ok(OperatorAction::stop(answer_policy, final_refs, reason))
            }
        }
    }

    pub fn to_dto(action: &OperatorAction) -> OperatorActionDto {
        match action {
            OperatorAction::ToolCall(action) => OperatorActionDto::ToolCall(ToolCallActionDto {
                tool: action.tool().as_str().to_string(),
                arguments: action.arguments().as_value().clone(),
            }),
            OperatorAction::PreparedToolCall { tool, source } => {
                OperatorActionDto::PreparedToolCall(PreparedToolCallActionDto {
                    tool: tool.as_str().to_string(),
                    source: source.as_str().to_string(),
                })
            }
            OperatorAction::Stop(action) => OperatorActionDto::Stop(StopActionDto {
                answer_policy: action.answer_policy().as_str().to_string(),
                final_refs: action
                    .final_refs()
                    .iter()
                    .map(|value| value.as_str().to_string())
                    .collect(),
                reason: action.reason().as_str().to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use underpass_operator_shared_domain::{DomainError, OperatorAction};

    use super::*;

    #[test]
    fn maps_json_dto_to_domain_and_back_to_json() {
        let raw = json!({
            "step_id": "step-1",
            "about": "about:incident-1",
            "mode": "read",
            "task_family": "contract.read.near",
            "allowed_tools": ["kernel_near", "kernel_inspect"],
            "visible_state": {
                "cursor": { "ref": "node-1" }
            },
            "target_action": {
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {
                    "around": { "ref": "node-1" }
                }
            }
        });

        let trajectory = TrainingTrajectoryMapper::from_json(raw).expect("json maps to domain");
        let mapped = TrainingTrajectoryMapper::to_json(&trajectory).expect("domain maps to json");

        assert_eq!(mapped["step_id"], "step-1");
        assert_eq!(mapped["target_action"]["tool"], "kernel_near");
    }

    #[test]
    fn mapper_fails_fast_when_tool_is_outside_allowed_tools() {
        let raw = json!({
            "step_id": "step-1",
            "about": "about:incident-1",
            "mode": "read",
            "task_family": "contract.read.near",
            "allowed_tools": ["kernel_inspect"],
            "visible_state": {},
            "target_action": {
                "type": "tool_call",
                "tool": "kernel_near",
                "arguments": {}
            }
        });

        let error = TrainingTrajectoryMapper::from_json(raw).expect_err("must fail");

        assert!(matches!(
            error,
            crate::InfraError::Domain(DomainError::TargetToolNotAllowed { .. })
        ));
    }

    #[test]
    fn action_mapper_preserves_stop_action() {
        let dto = OperatorActionDto::Stop(StopActionDto {
            answer_policy: "evidence_or_unknown".to_string(),
            final_refs: vec!["node-1".to_string()],
            reason: "evidence complete".to_string(),
        });

        let action = OperatorActionMapper::from_dto(dto).expect("dto maps to domain");
        let mapped = OperatorActionMapper::to_dto(&action);

        assert_eq!(
            mapped,
            OperatorActionDto::Stop(StopActionDto {
                answer_policy: "evidence_or_unknown".to_string(),
                final_refs: vec!["node-1".to_string()],
                reason: "evidence complete".to_string()
            })
        );
        assert!(matches!(action, OperatorAction::Stop(_)));
    }
}
