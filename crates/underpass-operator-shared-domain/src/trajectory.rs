use serde_json::Value;

use crate::{
    AboutId, AllowedTools, DomainError, DomainResult, OperatorAction, OperatorMode, StepId,
    TaskFamily,
};

#[derive(Debug, Clone, PartialEq)]
pub struct VisibleState(Value);

impl VisibleState {
    pub fn parse(value: Value) -> DomainResult<Self> {
        if !value.is_object() {
            return Err(DomainError::InvalidActionArguments {
                context: "visible_state".to_string(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrainingTrajectory {
    step_id: StepId,
    about: AboutId,
    mode: OperatorMode,
    task_family: TaskFamily,
    allowed_tools: AllowedTools,
    visible_state: VisibleState,
    target_action: OperatorAction,
}

impl TrainingTrajectory {
    pub fn new(
        step_id: StepId,
        about: AboutId,
        mode: OperatorMode,
        task_family: TaskFamily,
        allowed_tools: AllowedTools,
        visible_state: VisibleState,
        target_action: OperatorAction,
    ) -> DomainResult<Self> {
        if let Some(tool) = target_action.tool()
            && !allowed_tools.contains(tool)
        {
            return Err(DomainError::TargetToolNotAllowed {
                tool: tool.as_str().to_string(),
            });
        }
        Ok(Self {
            step_id,
            about,
            mode,
            task_family,
            allowed_tools,
            visible_state,
            target_action,
        })
    }

    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }

    pub fn about(&self) -> &AboutId {
        &self.about
    }

    pub fn mode(&self) -> OperatorMode {
        self.mode
    }

    pub fn task_family(&self) -> &TaskFamily {
        &self.task_family
    }

    pub fn allowed_tools(&self) -> &AllowedTools {
        &self.allowed_tools
    }

    pub fn visible_state(&self) -> &VisibleState {
        &self.visible_state
    }

    pub fn target_action(&self) -> &OperatorAction {
        &self.target_action
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{ActionArguments, KernelTool};

    use super::*;

    #[test]
    fn target_tool_must_be_allowed() {
        let mode = OperatorMode::Read;
        let allowed = AllowedTools::parse(mode, vec![KernelTool::Inspect]).expect("valid tools");
        let action = OperatorAction::tool_call(
            KernelTool::Near,
            ActionArguments::parse(json!({ "around": { "ref": "node-1" }})).expect("args"),
        );

        let error = TrainingTrajectory::new(
            StepId::parse("step-1").expect("step"),
            AboutId::parse("about-1").expect("about"),
            mode,
            TaskFamily::parse("read.near").expect("task"),
            allowed,
            VisibleState::parse(json!({})).expect("visible state"),
            action,
        )
        .expect_err("target outside allowed tools must fail");

        assert_eq!(
            error,
            DomainError::TargetToolNotAllowed {
                tool: "kernel_near".to_string()
            }
        );
    }
}
