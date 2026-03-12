use std::io;

use crate::{
    CAPTAINS_LOG_PATH, REPAIR_COMMAND_PATH, ROUTE_COMMAND_PATH, SCAN_COMMAND_PATH,
    STARSHIP_STATE_PATH, STARSHIP_TEST_PATH, STATUS_COMMAND_PATH,
};

pub const STARSHIP_LIST_TOOL: &str = "starship.fs.list";
pub const STARSHIP_WRITE_SCAN_TOOL: &str = "starship.fs.write.scan";
pub const STARSHIP_WRITE_REPAIR_TOOL: &str = "starship.fs.write.repair";
pub const STARSHIP_WRITE_ROUTE_TOOL: &str = "starship.fs.write.route";
pub const STARSHIP_WRITE_STATUS_TOOL: &str = "starship.fs.write.status";
pub const STARSHIP_WRITE_STATE_TOOL: &str = "starship.fs.write.state";
pub const STARSHIP_WRITE_TEST_TOOL: &str = "starship.fs.write.test";
pub const STARSHIP_WRITE_CAPTAINS_LOG_TOOL: &str = "starship.fs.write.captains_log";
pub const STARSHIP_READ_SCAN_TOOL: &str = "starship.fs.read.scan";
pub const STARSHIP_READ_CAPTAINS_LOG_TOOL: &str = "starship.fs.read.captains_log";

pub fn all_supported_tools() -> [&'static str; 10] {
    [
        STARSHIP_LIST_TOOL,
        STARSHIP_WRITE_SCAN_TOOL,
        STARSHIP_WRITE_REPAIR_TOOL,
        STARSHIP_WRITE_ROUTE_TOOL,
        STARSHIP_WRITE_STATUS_TOOL,
        STARSHIP_WRITE_STATE_TOOL,
        STARSHIP_WRITE_TEST_TOOL,
        STARSHIP_WRITE_CAPTAINS_LOG_TOOL,
        STARSHIP_READ_SCAN_TOOL,
        STARSHIP_READ_CAPTAINS_LOG_TOOL,
    ]
}

pub fn write_tool_name_for_deliverable(deliverable: &str) -> io::Result<&'static str> {
    match deliverable {
        SCAN_COMMAND_PATH => Ok(STARSHIP_WRITE_SCAN_TOOL),
        REPAIR_COMMAND_PATH => Ok(STARSHIP_WRITE_REPAIR_TOOL),
        ROUTE_COMMAND_PATH => Ok(STARSHIP_WRITE_ROUTE_TOOL),
        STATUS_COMMAND_PATH => Ok(STARSHIP_WRITE_STATUS_TOOL),
        STARSHIP_STATE_PATH => Ok(STARSHIP_WRITE_STATE_TOOL),
        STARSHIP_TEST_PATH => Ok(STARSHIP_WRITE_TEST_TOOL),
        CAPTAINS_LOG_PATH => Ok(STARSHIP_WRITE_CAPTAINS_LOG_TOOL),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported Starship deliverable `{deliverable}`"),
        )),
    }
}

pub fn read_tool_name_for_deliverable(deliverable: &str) -> io::Result<&'static str> {
    match deliverable {
        SCAN_COMMAND_PATH => Ok(STARSHIP_READ_SCAN_TOOL),
        CAPTAINS_LOG_PATH => Ok(STARSHIP_READ_CAPTAINS_LOG_TOOL),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported Starship readable deliverable `{deliverable}`"),
        )),
    }
}

pub fn path_for_tool_name(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        STARSHIP_WRITE_SCAN_TOOL | STARSHIP_READ_SCAN_TOOL => Some(SCAN_COMMAND_PATH),
        STARSHIP_WRITE_REPAIR_TOOL => Some(REPAIR_COMMAND_PATH),
        STARSHIP_WRITE_ROUTE_TOOL => Some(ROUTE_COMMAND_PATH),
        STARSHIP_WRITE_STATUS_TOOL => Some(STATUS_COMMAND_PATH),
        STARSHIP_WRITE_STATE_TOOL => Some(STARSHIP_STATE_PATH),
        STARSHIP_WRITE_TEST_TOOL => Some(STARSHIP_TEST_PATH),
        STARSHIP_WRITE_CAPTAINS_LOG_TOOL | STARSHIP_READ_CAPTAINS_LOG_TOOL => {
            Some(CAPTAINS_LOG_PATH)
        }
        _ => None,
    }
}

pub fn is_write_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        STARSHIP_WRITE_SCAN_TOOL
            | STARSHIP_WRITE_REPAIR_TOOL
            | STARSHIP_WRITE_ROUTE_TOOL
            | STARSHIP_WRITE_STATUS_TOOL
            | STARSHIP_WRITE_STATE_TOOL
            | STARSHIP_WRITE_TEST_TOOL
            | STARSHIP_WRITE_CAPTAINS_LOG_TOOL
    )
}

pub fn is_read_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        STARSHIP_READ_SCAN_TOOL | STARSHIP_READ_CAPTAINS_LOG_TOOL
    )
}

#[cfg(test)]
mod tests {
    use super::{
        STARSHIP_LIST_TOOL, STARSHIP_READ_CAPTAINS_LOG_TOOL, STARSHIP_WRITE_ROUTE_TOOL,
        all_supported_tools, path_for_tool_name, read_tool_name_for_deliverable,
        write_tool_name_for_deliverable,
    };

    #[test]
    fn tool_helpers_map_deliverables_to_static_tool_names() {
        assert_eq!(
            write_tool_name_for_deliverable("src/commands/route.rs")
                .expect("route should be writable"),
            STARSHIP_WRITE_ROUTE_TOOL
        );
        assert_eq!(
            read_tool_name_for_deliverable("captains-log.md")
                .expect("captains log should be readable"),
            STARSHIP_READ_CAPTAINS_LOG_TOOL
        );
        assert_eq!(
            path_for_tool_name(STARSHIP_WRITE_ROUTE_TOOL),
            Some("src/commands/route.rs")
        );
        assert!(all_supported_tools().contains(&STARSHIP_LIST_TOOL));
    }
}
