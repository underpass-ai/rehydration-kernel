use std::io;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_STARSHIP_WORKSPACE_DIR: &str = "/workspace-demo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarshipRuntimeMode {
    FileSystem,
    Http,
}

#[derive(Debug, Clone)]
pub struct StarshipDemoConfig {
    pub kernel_grpc_endpoint: String,
    pub nats_url: String,
    pub workspace_dir: PathBuf,
    pub runtime_mode: StarshipRuntimeMode,
    pub runtime_base_url: Option<String>,
    pub reset_workspace: bool,
    pub wait_attempts: usize,
    pub wait_poll_interval: Duration,
    pub run_id: String,
    pub llm_provider: String,
}

impl StarshipDemoConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    fn from_lookup<F>(lookup: F) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str) -> Option<String>,
    {
        let kernel_grpc_endpoint = lookup_or_default(
            &lookup,
            "KERNEL_GRPC_ENDPOINT",
            "http://rehydration-kernel:50054",
        );
        let nats_url = lookup_or_default(&lookup, "NATS_URL", "nats://nats:4222");
        let runtime_mode = parse_runtime_mode(&lookup_or_default(
            &lookup,
            "STARSHIP_RUNTIME_MODE",
            "filesystem",
        ))?;
        let workspace_dir = parse_workspace_dir(&lookup, runtime_mode)?;
        let runtime_base_url = lookup("RUNTIME_BASE_URL").filter(|value| !value.trim().is_empty());
        if runtime_mode == StarshipRuntimeMode::Http && runtime_base_url.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "RUNTIME_BASE_URL is required when STARSHIP_RUNTIME_MODE=http",
            )
            .into());
        }

        let reset_workspace = parse_bool_value(&lookup, "STARSHIP_RESET_WORKSPACE", true)?;
        let wait_attempts = parse_usize_value(&lookup, "STARSHIP_WAIT_ATTEMPTS", 60)?;
        let wait_poll_millis = parse_u64_value(&lookup, "STARSHIP_WAIT_POLL_MILLIS", 500)?;
        let run_id = lookup("STARSHIP_RUN_ID")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(default_run_id);
        let llm_provider = lookup_or_default(&lookup, "LLM_PROVIDER", "vllm").to_lowercase();

        Ok(Self {
            kernel_grpc_endpoint,
            nats_url,
            workspace_dir,
            runtime_mode,
            runtime_base_url,
            reset_workspace,
            wait_attempts,
            wait_poll_interval: Duration::from_millis(wait_poll_millis),
            run_id,
            llm_provider,
        })
    }
}

fn lookup_or_default<F>(lookup: &F, key: &str, default_value: &str) -> String
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key).unwrap_or_else(|| default_value.to_string())
}

fn parse_workspace_dir<F>(lookup: &F, runtime_mode: StarshipRuntimeMode) -> io::Result<PathBuf>
where
    F: Fn(&str) -> Option<String>,
{
    let workspace_dir = lookup_or_default(
        lookup,
        "STARSHIP_WORKSPACE_DIR",
        DEFAULT_STARSHIP_WORKSPACE_DIR,
    );
    if runtime_mode == StarshipRuntimeMode::FileSystem
        && workspace_dir != DEFAULT_STARSHIP_WORKSPACE_DIR
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "STARSHIP_WORKSPACE_DIR must be `{DEFAULT_STARSHIP_WORKSPACE_DIR}` in filesystem mode"
            ),
        ));
    }

    Ok(PathBuf::from(workspace_dir))
}

fn parse_runtime_mode(value: &str) -> io::Result<StarshipRuntimeMode> {
    match value.to_ascii_lowercase().as_str() {
        "filesystem" | "fs" | "localfs" => Ok(StarshipRuntimeMode::FileSystem),
        "http" | "runtime_http" | "underpass-runtime" => Ok(StarshipRuntimeMode::Http),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported STARSHIP_RUNTIME_MODE `{value}`"),
        )),
    }
}

fn parse_bool_value<F>(lookup: &F, key: &str, default_value: bool) -> io::Result<bool>
where
    F: Fn(&str) -> Option<String>,
{
    match lookup(key) {
        None => Ok(default_value),
        Some(value) => match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Ok(true),
            "0" | "false" | "no" => Ok(false),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid boolean `{key}` value `{value}`"),
            )),
        },
    }
}

fn parse_usize_value<F>(lookup: &F, key: &str, default_value: usize) -> io::Result<usize>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key)
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value.parse::<usize>().map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid `{key}` value `{value}`: {error}"),
                )
            })
        })
        .transpose()
        .map(|value| value.unwrap_or(default_value))
}

fn parse_u64_value<F>(lookup: &F, key: &str, default_value: u64) -> io::Result<u64>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key)
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value.parse::<u64>().map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid `{key}` value `{value}`: {error}"),
                )
            })
        })
        .transpose()
        .map(|value| value.unwrap_or(default_value))
}

fn default_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "local".to_string());
    let hostname = hostname
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    format!("{hostname}-{millis}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{StarshipDemoConfig, StarshipRuntimeMode};

    #[test]
    fn config_uses_internal_network_defaults() {
        let config = StarshipDemoConfig::from_lookup(|_| None).expect("defaults should load");

        assert_eq!(
            config.kernel_grpc_endpoint,
            "http://rehydration-kernel:50054"
        );
        assert_eq!(config.nats_url, "nats://nats:4222");
        assert_eq!(config.runtime_mode, StarshipRuntimeMode::FileSystem);
        assert!(config.reset_workspace);
        assert_eq!(config.wait_attempts, 60);
    }

    #[test]
    fn config_requires_runtime_base_url_for_http_mode() {
        let env = BTreeMap::from([("STARSHIP_RUNTIME_MODE".to_string(), "http".to_string())]);

        let error = StarshipDemoConfig::from_lookup(|key| env.get(key).cloned())
            .expect_err("http runtime should require base url");

        assert!(error.to_string().contains("RUNTIME_BASE_URL"));
    }

    #[test]
    fn config_rejects_custom_workspace_dir_in_filesystem_mode() {
        let env = BTreeMap::from([(
            "STARSHIP_WORKSPACE_DIR".to_string(),
            "/tmp/custom-demo".to_string(),
        )]);

        let error = StarshipDemoConfig::from_lookup(|key| env.get(key).cloned())
            .expect_err("filesystem mode should reject custom workspace dir");

        assert!(error.to_string().contains("STARSHIP_WORKSPACE_DIR"));
    }
}
