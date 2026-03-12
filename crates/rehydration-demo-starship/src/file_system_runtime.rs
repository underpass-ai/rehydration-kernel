use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::demo_config::DEFAULT_STARSHIP_WORKSPACE_DIR;
use crate::logging::debug_log_value;
use crate::runtime_contract::{AgentRuntime, RuntimeResult, ToolDescriptor, ToolInvocation};
use crate::starship_runtime_tools::{
    STARSHIP_LIST_TOOL, STARSHIP_READ_CAPTAINS_LOG_TOOL, STARSHIP_READ_SCAN_TOOL,
    STARSHIP_WRITE_CAPTAINS_LOG_TOOL, STARSHIP_WRITE_REPAIR_TOOL, STARSHIP_WRITE_ROUTE_TOOL,
    STARSHIP_WRITE_SCAN_TOOL, STARSHIP_WRITE_STATE_TOOL, STARSHIP_WRITE_STATUS_TOOL,
    STARSHIP_WRITE_TEST_TOOL, all_supported_tools, is_write_tool,
};
use crate::{
    CAPTAINS_LOG_PATH, REPAIR_COMMAND_PATH, ROUTE_COMMAND_PATH, SCAN_COMMAND_PATH,
    STARSHIP_STATE_PATH, STARSHIP_TEST_PATH, STATUS_COMMAND_PATH,
};

#[derive(Debug, Clone)]
pub struct FileSystemRuntime {
    workspace_dir: PathBuf,
}

impl FileSystemRuntime {
    pub fn new() -> Self {
        Self::try_new(DEFAULT_STARSHIP_WORKSPACE_DIR)
            .expect("default demo workspace directory should be valid")
    }

    fn try_new(workspace_dir: impl Into<PathBuf>) -> io::Result<Self> {
        let workspace_dir = workspace_dir.into();
        std::fs::create_dir_all(&workspace_dir)?;
        Ok(Self {
            workspace_dir: workspace_dir.canonicalize()?,
        })
    }

    #[cfg(test)]
    pub fn new_for_test(workspace_dir: impl Into<PathBuf>) -> Self {
        Self::try_new(workspace_dir).expect("test workspace directory should be valid")
    }

    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }
}

impl Default for FileSystemRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRuntime for FileSystemRuntime {
    async fn list_tools(&self) -> RuntimeResult<Vec<ToolDescriptor>> {
        Ok(all_supported_tools()
            .into_iter()
            .map(|name| ToolDescriptor {
                name: name.to_string(),
                requires_approval: is_write_tool(name),
            })
            .collect())
    }

    async fn invoke(
        &self,
        tool_name: &str,
        args: Value,
        approved: bool,
    ) -> RuntimeResult<ToolInvocation> {
        debug_log_value("filesystem runtime invoke", tool_name);
        match tool_name {
            STARSHIP_LIST_TOOL => {
                let mut files = known_workspace_files(&self.workspace_dir);
                files.sort();

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: files.join("\n"),
                })
            }
            STARSHIP_WRITE_SCAN_TOOL => {
                ensure_approved(tool_name, approved)?;
                let content = json_string_arg(&args, "content")?;
                let absolute_path = self.workspace_dir.join(SCAN_COMMAND_PATH);
                if let Some(parent) = absolute_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&absolute_path, content)?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: format!("wrote {SCAN_COMMAND_PATH}"),
                })
            }
            STARSHIP_WRITE_REPAIR_TOOL => {
                ensure_approved(tool_name, approved)?;
                let content = json_string_arg(&args, "content")?;
                let absolute_path = self.workspace_dir.join(REPAIR_COMMAND_PATH);
                if let Some(parent) = absolute_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&absolute_path, content)?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: format!("wrote {REPAIR_COMMAND_PATH}"),
                })
            }
            STARSHIP_WRITE_ROUTE_TOOL => {
                ensure_approved(tool_name, approved)?;
                let content = json_string_arg(&args, "content")?;
                let absolute_path = self.workspace_dir.join(ROUTE_COMMAND_PATH);
                if let Some(parent) = absolute_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&absolute_path, content)?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: format!("wrote {ROUTE_COMMAND_PATH}"),
                })
            }
            STARSHIP_WRITE_STATUS_TOOL => {
                ensure_approved(tool_name, approved)?;
                let content = json_string_arg(&args, "content")?;
                let absolute_path = self.workspace_dir.join(STATUS_COMMAND_PATH);
                if let Some(parent) = absolute_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&absolute_path, content)?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: format!("wrote {STATUS_COMMAND_PATH}"),
                })
            }
            STARSHIP_WRITE_STATE_TOOL => {
                ensure_approved(tool_name, approved)?;
                let content = json_string_arg(&args, "content")?;
                let absolute_path = self.workspace_dir.join(STARSHIP_STATE_PATH);
                if let Some(parent) = absolute_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&absolute_path, content)?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: format!("wrote {STARSHIP_STATE_PATH}"),
                })
            }
            STARSHIP_WRITE_TEST_TOOL => {
                ensure_approved(tool_name, approved)?;
                let content = json_string_arg(&args, "content")?;
                let absolute_path = self.workspace_dir.join(STARSHIP_TEST_PATH);
                if let Some(parent) = absolute_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&absolute_path, content)?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: format!("wrote {STARSHIP_TEST_PATH}"),
                })
            }
            STARSHIP_WRITE_CAPTAINS_LOG_TOOL => {
                ensure_approved(tool_name, approved)?;
                let content = json_string_arg(&args, "content")?;
                let absolute_path = self.workspace_dir.join(CAPTAINS_LOG_PATH);
                if let Some(parent) = absolute_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&absolute_path, content)?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: format!("wrote {CAPTAINS_LOG_PATH}"),
                })
            }
            STARSHIP_READ_SCAN_TOOL => {
                let absolute_path = self.workspace_dir.join(SCAN_COMMAND_PATH);
                let content = std::fs::read_to_string(&absolute_path).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("failed to read `{SCAN_COMMAND_PATH}` from workspace: {error}"),
                    )
                })?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: content,
                })
            }
            STARSHIP_READ_CAPTAINS_LOG_TOOL => {
                let absolute_path = self.workspace_dir.join(CAPTAINS_LOG_PATH);
                let content = std::fs::read_to_string(&absolute_path).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("failed to read `{CAPTAINS_LOG_PATH}` from workspace: {error}"),
                    )
                })?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: content,
                })
            }
            _ if is_write_tool(tool_name) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("tool `{tool_name}` is not allowed in the Starship demo workspace"),
            )
            .into()),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("unsupported tool `{tool_name}`"),
            )
            .into()),
        }
    }
}

fn known_workspace_files(workspace_root: &Path) -> Vec<String> {
    known_workspace_paths()
        .into_iter()
        .filter(|path| resolve_known_path(workspace_root, path).is_file())
        .map(ToString::to_string)
        .collect()
}

fn known_workspace_paths() -> [&'static str; 7] {
    [
        SCAN_COMMAND_PATH,
        REPAIR_COMMAND_PATH,
        ROUTE_COMMAND_PATH,
        STATUS_COMMAND_PATH,
        STARSHIP_STATE_PATH,
        STARSHIP_TEST_PATH,
        CAPTAINS_LOG_PATH,
    ]
}

fn resolve_known_path(workspace_root: &Path, relative_path: &'static str) -> PathBuf {
    workspace_root.join(relative_path)
}

fn json_string_arg(args: &Value, key: &str) -> RuntimeResult<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("missing string arg `{key}`"),
            )
            .into()
        })
}

fn ensure_approved(tool_name: &str, approved: bool) -> RuntimeResult<()> {
    if approved {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{tool_name} requires approval"),
        )
        .into())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::{FileSystemRuntime, resolve_known_path};
    use crate::runtime_contract::AgentRuntime;
    use crate::starship_runtime_tools::{
        STARSHIP_LIST_TOOL, STARSHIP_READ_SCAN_TOOL, STARSHIP_WRITE_SCAN_TOOL,
    };

    fn unique_workspace_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rehydration-starship-runtime-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should work")
                .as_nanos()
        ))
    }

    #[tokio::test]
    async fn runtime_writes_reads_and_lists_files() {
        let workspace = unique_workspace_dir();
        let runtime = FileSystemRuntime::new_for_test(&workspace);

        runtime
            .invoke(
                STARSHIP_WRITE_SCAN_TOOL,
                json!({
                    "content": "pub fn scan() {}",
                }),
                true,
            )
            .await
            .expect("write should succeed");

        let file = runtime
            .invoke(STARSHIP_READ_SCAN_TOOL, json!({}), false)
            .await
            .expect("read should succeed");
        assert!(file.output.contains("scan"));

        let listing = runtime
            .invoke(STARSHIP_LIST_TOOL, json!({}), false)
            .await
            .expect("list should succeed");
        assert!(listing.output.contains("src/commands/scan.rs"));

        std::fs::remove_dir_all(workspace).expect("workspace cleanup should succeed");
    }

    #[tokio::test]
    async fn runtime_rejects_parent_traversal_paths() {
        let workspace = unique_workspace_dir();
        let runtime = FileSystemRuntime::new_for_test(&workspace);

        let error = runtime
            .invoke(
                "starship.fs.write.unknown",
                json!({
                    "content": "nope",
                }),
                true,
            )
            .await
            .expect_err("parent traversal must be rejected");

        assert!(error.to_string().contains("unsupported tool"));
    }

    #[tokio::test]
    async fn runtime_rejects_absolute_paths() {
        let workspace = unique_workspace_dir();
        let runtime = FileSystemRuntime::new_for_test(&workspace);

        let error = runtime
            .invoke("starship.fs.read.unknown", json!({}), false)
            .await
            .expect_err("absolute paths must be rejected");

        assert!(error.to_string().contains("unsupported tool"));
    }

    #[tokio::test]
    async fn known_workspace_paths_resolve_existing_files() {
        let workspace = unique_workspace_dir();
        let runtime = FileSystemRuntime::new_for_test(&workspace);

        runtime
            .invoke(
                STARSHIP_WRITE_SCAN_TOOL,
                json!({
                    "content": "pub fn scan() {}",
                }),
                true,
            )
            .await
            .expect("write should succeed");

        let path = resolve_known_path(runtime.workspace_dir(), "src/commands/scan.rs");
        assert!(path.exists());

        std::fs::remove_dir_all(workspace).expect("workspace cleanup should succeed");
    }
}
