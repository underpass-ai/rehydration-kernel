use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::demo_config::DEFAULT_STARSHIP_WORKSPACE_DIR;
use crate::logging::debug_log_value;
use crate::runtime_contract::{AgentRuntime, RuntimeResult, ToolDescriptor, ToolInvocation};
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
        Ok(vec![
            ToolDescriptor {
                name: "fs.write".to_string(),
                requires_approval: true,
            },
            ToolDescriptor {
                name: "fs.read".to_string(),
                requires_approval: false,
            },
            ToolDescriptor {
                name: "fs.list".to_string(),
                requires_approval: false,
            },
        ])
    }

    async fn invoke(
        &self,
        tool_name: &str,
        args: Value,
        approved: bool,
    ) -> RuntimeResult<ToolInvocation> {
        debug_log_value("filesystem runtime invoke", tool_name);
        match tool_name {
            "fs.write" => {
                if !approved {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "fs.write requires approval",
                    )
                    .into());
                }

                let path = StarshipWorkspacePath::from_args(&args, "path")?;
                let content = json_string_arg(&args, "content")?;
                let absolute_path = path.resolve_for_write(&self.workspace_dir);
                if let Some(parent) = absolute_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&absolute_path, content)?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: format!("wrote {}", path.display()),
                })
            }
            "fs.read" => {
                let path = StarshipWorkspacePath::from_args(&args, "path")?;
                let absolute_path = path.resolve_existing(&self.workspace_dir)?;
                let content = std::fs::read_to_string(&absolute_path).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "failed to read `{}` from workspace: {error}",
                            path.display()
                        ),
                    )
                })?;

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: content,
                })
            }
            "fs.list" => {
                let mut files = known_workspace_files(&self.workspace_dir);
                files.sort();

                Ok(ToolInvocation {
                    tool_name: tool_name.to_string(),
                    output: files.join("\n"),
                })
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("unsupported tool `{tool_name}`"),
            )
            .into()),
        }
    }
}

fn known_workspace_files(workspace_root: &Path) -> Vec<String> {
    StarshipWorkspacePath::all()
        .into_iter()
        .filter(|path| path.resolve_existing(workspace_root).is_ok())
        .map(|path| path.display().to_string())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarshipWorkspacePath {
    ScanCommand,
    RepairCommand,
    RouteCommand,
    StatusCommand,
    StateFile,
    TestFile,
    CaptainsLog,
}

impl StarshipWorkspacePath {
    fn from_args(args: &Value, key: &str) -> RuntimeResult<Self> {
        let raw = json_string_arg(args, key)?;
        Self::parse(&raw)
    }

    fn parse(raw: &str) -> RuntimeResult<Self> {
        match raw {
            SCAN_COMMAND_PATH => Ok(Self::ScanCommand),
            REPAIR_COMMAND_PATH => Ok(Self::RepairCommand),
            ROUTE_COMMAND_PATH => Ok(Self::RouteCommand),
            STATUS_COMMAND_PATH => Ok(Self::StatusCommand),
            STARSHIP_STATE_PATH => Ok(Self::StateFile),
            STARSHIP_TEST_PATH => Ok(Self::TestFile),
            CAPTAINS_LOG_PATH => Ok(Self::CaptainsLog),
            _ => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("path `{raw}` is not allowed in the Starship demo workspace"),
            )
            .into()),
        }
    }

    fn all() -> [Self; 7] {
        [
            Self::ScanCommand,
            Self::RepairCommand,
            Self::RouteCommand,
            Self::StatusCommand,
            Self::StateFile,
            Self::TestFile,
            Self::CaptainsLog,
        ]
    }

    fn display(self) -> &'static str {
        match self {
            Self::ScanCommand => SCAN_COMMAND_PATH,
            Self::RepairCommand => REPAIR_COMMAND_PATH,
            Self::RouteCommand => ROUTE_COMMAND_PATH,
            Self::StatusCommand => STATUS_COMMAND_PATH,
            Self::StateFile => STARSHIP_STATE_PATH,
            Self::TestFile => STARSHIP_TEST_PATH,
            Self::CaptainsLog => CAPTAINS_LOG_PATH,
        }
    }

    fn resolve_for_write(self, workspace_root: &Path) -> PathBuf {
        workspace_root.join(self.display())
    }

    fn resolve_existing(self, workspace_root: &Path) -> io::Result<PathBuf> {
        let path = self.resolve_for_write(workspace_root);
        if path.exists() {
            Ok(path)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "file `{}` does not exist in the Starship demo workspace",
                    self.display()
                ),
            ))
        }
    }
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::{FileSystemRuntime, StarshipWorkspacePath};
    use crate::runtime_contract::AgentRuntime;

    #[tokio::test]
    async fn runtime_writes_reads_and_lists_files() {
        let workspace = std::env::temp_dir().join(format!(
            "rehydration-starship-runtime-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should work")
                .as_millis()
        ));
        let runtime = FileSystemRuntime::new_for_test(&workspace);

        runtime
            .invoke(
                "fs.write",
                json!({
                    "path": "src/commands/scan.rs",
                    "content": "pub fn scan() {}",
                }),
                true,
            )
            .await
            .expect("write should succeed");

        let file = runtime
            .invoke("fs.read", json!({ "path": "src/commands/scan.rs" }), false)
            .await
            .expect("read should succeed");
        assert!(file.output.contains("scan"));

        let listing = runtime
            .invoke("fs.list", json!({ "path": "." }), false)
            .await
            .expect("list should succeed");
        assert!(listing.output.contains("src/commands/scan.rs"));

        std::fs::remove_dir_all(workspace).expect("workspace cleanup should succeed");
    }

    #[tokio::test]
    async fn runtime_rejects_parent_traversal_paths() {
        let workspace = std::env::temp_dir().join(format!(
            "rehydration-starship-runtime-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should work")
                .as_millis()
        ));
        let runtime = FileSystemRuntime::new_for_test(&workspace);

        let error = runtime
            .invoke(
                "fs.write",
                json!({
                    "path": "../outside.txt",
                    "content": "nope",
                }),
                true,
            )
            .await
            .expect_err("parent traversal must be rejected");

        assert!(error.to_string().contains("not allowed"));
    }

    #[tokio::test]
    async fn runtime_rejects_absolute_paths() {
        let workspace = std::env::temp_dir().join(format!(
            "rehydration-starship-runtime-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should work")
                .as_millis()
        ));
        let runtime = FileSystemRuntime::new_for_test(&workspace);

        let error = runtime
            .invoke(
                "fs.read",
                json!({
                    "path": "/tmp/escape.txt",
                }),
                false,
            )
            .await
            .expect_err("absolute paths must be rejected");

        assert!(error.to_string().contains("not allowed"));
    }

    #[test]
    fn workspace_relative_path_rejects_escape_components() {
        let error = StarshipWorkspacePath::parse("../outside.txt")
            .expect_err("parent traversal must be rejected");

        assert!(error.to_string().contains("not allowed"));
    }

    #[test]
    fn workspace_path_accepts_known_starship_deliverables() {
        assert_eq!(
            StarshipWorkspacePath::parse("src/commands/scan.rs")
                .expect("known deliverable should be accepted")
                .display(),
            "src/commands/scan.rs"
        );
    }
}
