use std::io;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::logging::debug_log_value;
use crate::runtime_contract::{AgentRuntime, RuntimeResult, ToolDescriptor, ToolInvocation};

#[derive(Debug, Clone)]
pub struct FileSystemRuntime {
    workspace_dir: PathBuf,
}

impl FileSystemRuntime {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
        }
    }

    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    fn workspace_root(&self) -> io::Result<PathBuf> {
        if self.workspace_dir.exists() {
            self.workspace_dir.canonicalize()
        } else {
            Ok(self.workspace_dir.clone())
        }
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

                let path = WorkspaceRelativePath::from_args(&args, "path")?;
                let content = json_string_arg(&args, "content")?;
                let workspace_root = self.workspace_root()?;
                let absolute_path = path.resolve_for_write(&workspace_root)?;
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
                let path = WorkspaceRelativePath::from_args(&args, "path")?;
                let workspace_root = self.workspace_root()?;
                let absolute_path = path.resolve_existing(&workspace_root)?;
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
                let mut files = Vec::new();
                collect_files(&self.workspace_dir, &self.workspace_dir, &mut files)?;
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

fn collect_files(root: &Path, current: &Path, files: &mut Vec<String>) -> io::Result<()> {
    if !current.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .to_string_lossy()
            .replace('\\', "/");
        files.push(relative);
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceRelativePath {
    value: PathBuf,
}

impl WorkspaceRelativePath {
    fn from_args(args: &Value, key: &str) -> RuntimeResult<Self> {
        let raw = json_string_arg(args, key)?;
        Self::parse(raw)
    }

    fn parse(raw: String) -> RuntimeResult<Self> {
        let candidate = PathBuf::from(&raw);
        if candidate.as_os_str().is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "path cannot be empty").into());
        }

        for component in candidate.components() {
            match component {
                Component::Normal(_) | Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!("path `{raw}` escapes the workspace"),
                    )
                    .into());
                }
            }
        }

        Ok(Self { value: candidate })
    }

    fn display(&self) -> String {
        self.value.to_string_lossy().into_owned()
    }

    fn resolve_for_write(&self, workspace_root: &Path) -> io::Result<PathBuf> {
        let path = workspace_root.join(&self.value);
        if let Some(parent) = path.parent() {
            let parent = if parent.exists() {
                parent.canonicalize()?
            } else {
                canonicalize_parent_chain(parent, workspace_root)?
            };
            ensure_within_root(workspace_root, &parent)?;
        }
        Ok(path)
    }

    fn resolve_existing(&self, workspace_root: &Path) -> io::Result<PathBuf> {
        let path = workspace_root.join(&self.value);
        let canonical = path.canonicalize()?;
        ensure_within_root(workspace_root, &canonical)?;
        Ok(canonical)
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

fn ensure_within_root(workspace_root: &Path, path: &Path) -> io::Result<()> {
    if path.starts_with(workspace_root) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("path `{}` escapes the workspace", path.to_string_lossy()),
        ))
    }
}

fn canonicalize_parent_chain(path: &Path, workspace_root: &Path) -> io::Result<PathBuf> {
    if path == workspace_root {
        return Ok(workspace_root.to_path_buf());
    }

    if path.exists() {
        return path.canonicalize();
    }

    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("path `{}` escapes the workspace", path.to_string_lossy()),
        )
    })?;
    let canonical_parent = canonicalize_parent_chain(parent, workspace_root)?;
    let final_component = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("path `{}` escapes the workspace", path.to_string_lossy()),
        )
    })?;
    Ok(canonical_parent.join(final_component))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::{FileSystemRuntime, WorkspaceRelativePath};
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
        let runtime = FileSystemRuntime::new(&workspace);

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
        let runtime = FileSystemRuntime::new(&workspace);

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

        assert!(error.to_string().contains("escapes the workspace"));
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
        let runtime = FileSystemRuntime::new(&workspace);

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

        assert!(error.to_string().contains("escapes the workspace"));
    }

    #[test]
    fn workspace_relative_path_rejects_escape_components() {
        let error = WorkspaceRelativePath::parse("../outside.txt".to_string())
            .expect_err("parent traversal must be rejected");

        assert!(error.to_string().contains("escapes the workspace"));
    }
}
