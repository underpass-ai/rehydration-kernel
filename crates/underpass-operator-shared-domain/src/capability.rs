use crate::{KernelTool, OperatorAction, OperatorMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KmpMcpCapability {
    tool: KernelTool,
}

impl KmpMcpCapability {
    pub const fn new(tool: KernelTool) -> Self {
        Self { tool }
    }

    pub fn all() -> &'static [Self] {
        &ALL_KMP_MCP_CAPABILITIES
    }

    pub fn from_tool(tool: KernelTool) -> Self {
        Self { tool }
    }

    pub fn from_action(action: &OperatorAction) -> Option<Self> {
        action.tool().map(Self::from_tool)
    }

    pub fn tool(self) -> KernelTool {
        self.tool
    }

    pub fn name(self) -> &'static str {
        self.tool.as_str()
    }

    pub fn mode(self) -> OperatorMode {
        match self.tool {
            KernelTool::Ingest | KernelTool::WriteMemory => OperatorMode::Write,
            _ => OperatorMode::Read,
        }
    }
}

const ALL_KMP_MCP_CAPABILITIES: [KmpMcpCapability; 10] = [
    KmpMcpCapability::new(KernelTool::Wake),
    KmpMcpCapability::new(KernelTool::Ask),
    KmpMcpCapability::new(KernelTool::Near),
    KmpMcpCapability::new(KernelTool::Goto),
    KmpMcpCapability::new(KernelTool::Rewind),
    KmpMcpCapability::new(KernelTool::Forward),
    KmpMcpCapability::new(KernelTool::Trace),
    KmpMcpCapability::new(KernelTool::Inspect),
    KmpMcpCapability::new(KernelTool::Ingest),
    KmpMcpCapability::new(KernelTool::WriteMemory),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_complete_kmp_mcp_tool_inventory_in_stable_order() {
        let names = KmpMcpCapability::all()
            .iter()
            .copied()
            .map(KmpMcpCapability::name)
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "kernel_wake",
                "kernel_ask",
                "kernel_near",
                "kernel_goto",
                "kernel_rewind",
                "kernel_forward",
                "kernel_trace",
                "kernel_inspect",
                "kernel_ingest",
                "kernel_write_memory",
            ]
        );
    }

    #[test]
    fn write_capabilities_are_explicitly_classified() {
        assert_eq!(
            KmpMcpCapability::from_tool(KernelTool::WriteMemory).mode(),
            OperatorMode::Write
        );
        assert_eq!(
            KmpMcpCapability::from_tool(KernelTool::Near).mode(),
            OperatorMode::Read
        );
    }
}
