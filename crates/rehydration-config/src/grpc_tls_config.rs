use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GrpcTlsMode {
    #[default]
    Disabled,
    Server,
    Mutual,
}

impl GrpcTlsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Server => "server",
            Self::Mutual => "mutual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GrpcTlsConfig {
    pub mode: GrpcTlsMode,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub client_ca_path: Option<PathBuf>,
}

impl GrpcTlsConfig {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub(crate) fn from_lookup<F>(lookup: &F) -> io::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let mode = lookup("REHYDRATION_GRPC_TLS_MODE")
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_mode(&value))
            .transpose()?
            .unwrap_or_default();

        let cert_path = lookup_optional_path(lookup, "REHYDRATION_GRPC_TLS_CERT_PATH");
        let key_path = lookup_optional_path(lookup, "REHYDRATION_GRPC_TLS_KEY_PATH");
        let client_ca_path = lookup_optional_path(lookup, "REHYDRATION_GRPC_TLS_CLIENT_CA_PATH");

        validate_required_path(mode, "REHYDRATION_GRPC_TLS_CERT_PATH", &cert_path)?;
        validate_required_path(mode, "REHYDRATION_GRPC_TLS_KEY_PATH", &key_path)?;
        if mode == GrpcTlsMode::Mutual {
            validate_required_client_ca_path(&client_ca_path)?;
        }

        Ok(Self {
            mode,
            cert_path,
            key_path,
            client_ca_path,
        })
    }
}

fn parse_mode(value: &str) -> io::Result<GrpcTlsMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" | "plaintext" => Ok(GrpcTlsMode::Disabled),
        "server" | "tls" => Ok(GrpcTlsMode::Server),
        "mutual" | "mtls" => Ok(GrpcTlsMode::Mutual),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unsupported REHYDRATION_GRPC_TLS_MODE `{value}`; expected one of disabled, server, mutual"
            ),
        )),
    }
}

fn lookup_optional_path<F>(lookup: &F, key: &str) -> Option<PathBuf>
where
    F: Fn(&str) -> Option<String>,
{
    lookup(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn validate_required_path(mode: GrpcTlsMode, key: &str, path: &Option<PathBuf>) -> io::Result<()> {
    if mode == GrpcTlsMode::Disabled || path.is_some() {
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "{key} is required when REHYDRATION_GRPC_TLS_MODE={}",
            mode.as_str()
        ),
    ))
}

fn validate_required_client_ca_path(path: &Option<PathBuf>) -> io::Result<()> {
    if path.is_some() {
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "REHYDRATION_GRPC_TLS_CLIENT_CA_PATH is required when REHYDRATION_GRPC_TLS_MODE=mutual",
    ))
}
