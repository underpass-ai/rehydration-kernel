use std::io;
use std::path::PathBuf;

use crate::env_bool::parse_bool_value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NatsTlsMode {
    #[default]
    Disabled,
    Server,
    Mutual,
}

impl NatsTlsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Server => "server",
            Self::Mutual => "mutual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NatsTlsConfig {
    pub mode: NatsTlsMode,
    pub ca_path: Option<PathBuf>,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub tls_first: bool,
}

impl NatsTlsConfig {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub(crate) fn from_lookup<F>(lookup: &F) -> io::Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let mode = lookup("NATS_TLS_MODE")
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_mode(&value))
            .transpose()?
            .unwrap_or_default();

        let ca_path = lookup_optional_path(lookup, "NATS_TLS_CA_PATH");
        let cert_path = lookup_optional_path(lookup, "NATS_TLS_CERT_PATH");
        let key_path = lookup_optional_path(lookup, "NATS_TLS_KEY_PATH");
        let tls_first = lookup("NATS_TLS_FIRST")
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_bool_value(&value))
            .unwrap_or(false);

        validate_tls_first(mode, tls_first)?;
        validate_client_cert_mode(mode, &cert_path, &key_path)?;
        validate_mutual_client_cert_pair(mode, &cert_path, &key_path)?;

        Ok(Self {
            mode,
            ca_path,
            cert_path,
            key_path,
            tls_first,
        })
    }
}

fn parse_mode(value: &str) -> io::Result<NatsTlsMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" | "plaintext" => Ok(NatsTlsMode::Disabled),
        "server" | "tls" => Ok(NatsTlsMode::Server),
        "mutual" | "mtls" => Ok(NatsTlsMode::Mutual),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unsupported NATS_TLS_MODE `{value}`; expected one of disabled, server, mutual"
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

fn validate_tls_first(mode: NatsTlsMode, tls_first: bool) -> io::Result<()> {
    if !tls_first || mode != NatsTlsMode::Disabled {
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "NATS_TLS_FIRST requires NATS_TLS_MODE=server or mutual",
    ))
}

fn validate_client_cert_mode(
    mode: NatsTlsMode,
    cert_path: &Option<PathBuf>,
    key_path: &Option<PathBuf>,
) -> io::Result<()> {
    if mode == NatsTlsMode::Mutual || (cert_path.is_none() && key_path.is_none()) {
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "NATS_TLS_CERT_PATH and NATS_TLS_KEY_PATH are only supported when NATS_TLS_MODE=mutual",
    ))
}

fn validate_mutual_client_cert_pair(
    mode: NatsTlsMode,
    cert_path: &Option<PathBuf>,
    key_path: &Option<PathBuf>,
) -> io::Result<()> {
    if mode != NatsTlsMode::Mutual {
        return Ok(());
    }

    if cert_path.is_some() && key_path.is_some() {
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "NATS_TLS_CERT_PATH and NATS_TLS_KEY_PATH are required when NATS_TLS_MODE=mutual",
    ))
}
