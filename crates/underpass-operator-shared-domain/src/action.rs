use serde_json::Value;

use crate::{DomainError, DomainResult, MemoryRef, NonEmptyString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KernelTool {
    Wake,
    Ingest,
    Ask,
    Goto,
    Near,
    Rewind,
    Forward,
    Trace,
    Inspect,
    WriteMemory,
}

impl KernelTool {
    pub fn parse(value: &str) -> DomainResult<Self> {
        match value {
            "kernel_wake" => Ok(Self::Wake),
            "kernel_ingest" => Ok(Self::Ingest),
            "kernel_ask" => Ok(Self::Ask),
            "kernel_goto" => Ok(Self::Goto),
            "kernel_near" => Ok(Self::Near),
            "kernel_rewind" => Ok(Self::Rewind),
            "kernel_forward" => Ok(Self::Forward),
            "kernel_trace" => Ok(Self::Trace),
            "kernel_inspect" => Ok(Self::Inspect),
            "kernel_write_memory" => Ok(Self::WriteMemory),
            other => Err(DomainError::UnsupportedTool {
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wake => "kernel_wake",
            Self::Ingest => "kernel_ingest",
            Self::Ask => "kernel_ask",
            Self::Goto => "kernel_goto",
            Self::Near => "kernel_near",
            Self::Rewind => "kernel_rewind",
            Self::Forward => "kernel_forward",
            Self::Trace => "kernel_trace",
            Self::Inspect => "kernel_inspect",
            Self::WriteMemory => "kernel_write_memory",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerPolicy {
    EvidenceOrUnknown,
    ShowConflicts,
    BestEffort,
}

impl AnswerPolicy {
    pub fn parse(value: &str) -> DomainResult<Self> {
        match value {
            "evidence_or_unknown" => Ok(Self::EvidenceOrUnknown),
            "show_conflicts" => Ok(Self::ShowConflicts),
            "best_effort" => Ok(Self::BestEffort),
            other => Err(DomainError::UnsupportedAnswerPolicy {
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::EvidenceOrUnknown => "evidence_or_unknown",
            Self::ShowConflicts => "show_conflicts",
            Self::BestEffort => "best_effort",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedPayloadSource {
    DraftWritePreparedArguments,
    CanonicalPayload,
}

impl PreparedPayloadSource {
    pub fn parse(tool: KernelTool, value: &str) -> DomainResult<Self> {
        match (tool, value) {
            (KernelTool::WriteMemory, "draft_write.prepared_arguments") => {
                Ok(Self::DraftWritePreparedArguments)
            }
            (KernelTool::Ingest, "canonical_payload") => Ok(Self::CanonicalPayload),
            _ => Err(DomainError::UnsupportedPreparedPayloadSource {
                tool: tool.as_str().to_string(),
                source: value.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DraftWritePreparedArguments => "draft_write.prepared_arguments",
            Self::CanonicalPayload => "canonical_payload",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionArguments(Value);

impl ActionArguments {
    pub fn parse(value: Value) -> DomainResult<Self> {
        if !value.is_object() {
            return Err(DomainError::InvalidActionArguments {
                context: "action.arguments".to_string(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }

    pub fn into_value(self) -> Value {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperatorAction {
    ToolCall(ToolCallAction),
    PreparedToolCall {
        tool: KernelTool,
        source: PreparedPayloadSource,
    },
    Stop(StopAction),
}

impl OperatorAction {
    pub fn tool_call(tool: KernelTool, arguments: ActionArguments) -> Self {
        Self::ToolCall(ToolCallAction { tool, arguments })
    }

    pub fn prepared_tool_call(tool: KernelTool, source: PreparedPayloadSource) -> Self {
        Self::PreparedToolCall { tool, source }
    }

    pub fn stop(
        answer_policy: AnswerPolicy,
        final_refs: Vec<MemoryRef>,
        reason: NonEmptyString,
    ) -> Self {
        Self::Stop(StopAction {
            answer_policy,
            final_refs,
            reason,
        })
    }

    pub fn tool(&self) -> Option<KernelTool> {
        match self {
            Self::ToolCall(action) => Some(action.tool()),
            Self::PreparedToolCall { tool, .. } => Some(*tool),
            Self::Stop(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCallAction {
    tool: KernelTool,
    arguments: ActionArguments,
}

impl ToolCallAction {
    pub fn tool(&self) -> KernelTool {
        self.tool
    }

    pub fn arguments(&self) -> &ActionArguments {
        &self.arguments
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StopAction {
    answer_policy: AnswerPolicy,
    final_refs: Vec<MemoryRef>,
    reason: NonEmptyString,
}

impl StopAction {
    pub fn answer_policy(&self) -> AnswerPolicy {
        self.answer_policy
    }

    pub fn final_refs(&self) -> &[MemoryRef] {
        &self.final_refs
    }

    pub fn reason(&self) -> &NonEmptyString {
        &self.reason
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn action_arguments_must_be_object() {
        let error = ActionArguments::parse(json!("not-object")).expect_err("must fail");

        assert_eq!(
            error,
            DomainError::InvalidActionArguments {
                context: "action.arguments".to_string()
            }
        );
    }

    #[test]
    fn prepared_source_must_match_tool() {
        let error = PreparedPayloadSource::parse(KernelTool::Near, "canonical_payload")
            .expect_err("wrong source must fail");

        assert_eq!(
            error,
            DomainError::UnsupportedPreparedPayloadSource {
                tool: "kernel_near".to_string(),
                source: "canonical_payload".to_string()
            }
        );
    }
}
