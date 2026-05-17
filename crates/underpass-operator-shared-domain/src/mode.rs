use std::collections::BTreeSet;

use crate::{DomainError, DomainResult, KernelTool};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperatorMode {
    Read,
    WriteContextRead,
    Write,
}

impl OperatorMode {
    pub fn parse(value: &str) -> DomainResult<Self> {
        match value {
            "read" => Ok(Self::Read),
            "write_context_read" => Ok(Self::WriteContextRead),
            "write" => Ok(Self::Write),
            other => Err(DomainError::UnsupportedMode {
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::WriteContextRead => "write_context_read",
            Self::Write => "write",
        }
    }

    pub fn allowed_tools(self) -> &'static [KernelTool] {
        match self {
            Self::Read => &[
                KernelTool::Wake,
                KernelTool::Ask,
                KernelTool::Near,
                KernelTool::Goto,
                KernelTool::Rewind,
                KernelTool::Forward,
                KernelTool::Trace,
                KernelTool::Inspect,
            ],
            Self::WriteContextRead => &[KernelTool::Near, KernelTool::Trace, KernelTool::Inspect],
            Self::Write => &[KernelTool::Ingest, KernelTool::WriteMemory],
        }
    }

    pub fn allows_tool(self, tool: KernelTool) -> bool {
        self.allowed_tools().contains(&tool)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedTools {
    mode: OperatorMode,
    tools: Vec<KernelTool>,
}

impl AllowedTools {
    pub fn all_for_mode(mode: OperatorMode) -> Self {
        Self {
            mode,
            tools: mode.allowed_tools().to_vec(),
        }
    }

    pub fn parse(mode: OperatorMode, tools: Vec<KernelTool>) -> DomainResult<Self> {
        let mut seen = BTreeSet::new();
        for tool in &tools {
            if !seen.insert(*tool) {
                return Err(DomainError::DuplicateAllowedTool {
                    tool: tool.as_str().to_string(),
                });
            }
            if !mode.allows_tool(*tool) {
                return Err(DomainError::ToolOutsideMode {
                    mode: mode.as_str().to_string(),
                    tool: tool.as_str().to_string(),
                });
            }
        }
        Ok(Self { mode, tools })
    }

    pub fn mode(&self) -> OperatorMode {
        self.mode
    }

    pub fn contains(&self, tool: KernelTool) -> bool {
        self.tools.contains(&tool)
    }

    pub fn iter(&self) -> impl Iterator<Item = KernelTool> + '_ {
        self.tools.iter().copied()
    }

    pub fn as_slice(&self) -> &[KernelTool] {
        &self.tools
    }
}

pub fn read_tool_names() -> Vec<String> {
    tool_names(OperatorMode::Read.allowed_tools())
}

pub fn writer_context_read_tool_names() -> Vec<String> {
    tool_names(OperatorMode::WriteContextRead.allowed_tools())
}

pub fn write_tool_names() -> Vec<String> {
    tool_names(OperatorMode::Write.allowed_tools())
}

pub fn full_tool_names() -> Vec<String> {
    let mut tools = read_tool_names();
    tools.extend(write_tool_names());
    tools
}

pub fn allowed_tool_names_for_mode(mode: &str) -> DomainResult<Vec<String>> {
    Ok(tool_names(OperatorMode::parse(mode)?.allowed_tools()))
}

pub fn parse_allowed_tools_for_mode(
    mode: &str,
    allowed_tools: &[String],
) -> DomainResult<AllowedTools> {
    let mode = OperatorMode::parse(mode)?;
    let tools = allowed_tools
        .iter()
        .map(|tool| KernelTool::parse(tool))
        .collect::<DomainResult<Vec<_>>>()?;
    AllowedTools::parse(mode, tools)
}

pub fn validate_allowed_tools_for_mode(mode: &str, allowed_tools: &[String]) -> DomainResult<()> {
    parse_allowed_tools_for_mode(mode, allowed_tools).map(|_| ())
}

fn tool_names(tools: &[KernelTool]) -> Vec<String> {
    tools
        .iter()
        .copied()
        .map(KernelTool::as_str)
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_tool_outside_mode() {
        let error = validate_allowed_tools_for_mode(
            "read",
            &["kernel_near".to_string(), "kernel_write_memory".to_string()],
        )
        .expect_err("mode boundary must fail");

        assert_eq!(
            error,
            DomainError::ToolOutsideMode {
                mode: "read".to_string(),
                tool: "kernel_write_memory".to_string()
            }
        );
    }

    #[test]
    fn writer_context_read_profile_is_bounded_to_navigation_tools() {
        let tools = writer_context_read_tool_names();

        assert_eq!(
            tools,
            vec![
                "kernel_near".to_string(),
                "kernel_trace".to_string(),
                "kernel_inspect".to_string()
            ]
        );
    }

    #[test]
    fn read_profile_exposes_existing_read_tools_in_stable_order() {
        assert_eq!(
            read_tool_names(),
            vec![
                "kernel_wake".to_string(),
                "kernel_ask".to_string(),
                "kernel_near".to_string(),
                "kernel_goto".to_string(),
                "kernel_rewind".to_string(),
                "kernel_forward".to_string(),
                "kernel_trace".to_string(),
                "kernel_inspect".to_string(),
            ]
        );
    }

    #[test]
    fn write_profile_exposes_existing_write_tools_in_stable_order() {
        assert_eq!(
            write_tool_names(),
            vec![
                "kernel_ingest".to_string(),
                "kernel_write_memory".to_string(),
            ]
        );
    }

    #[test]
    fn full_profile_matches_legacy_read_plus_write_surface() {
        assert_eq!(
            full_tool_names(),
            vec![
                "kernel_wake".to_string(),
                "kernel_ask".to_string(),
                "kernel_near".to_string(),
                "kernel_goto".to_string(),
                "kernel_rewind".to_string(),
                "kernel_forward".to_string(),
                "kernel_trace".to_string(),
                "kernel_inspect".to_string(),
                "kernel_ingest".to_string(),
                "kernel_write_memory".to_string(),
            ]
        );
    }
}
