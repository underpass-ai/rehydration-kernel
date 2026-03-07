use std::fmt::Write as _;

use rehydration_domain::RehydrationBundle;
use rehydration_ports::{PortError, SnapshotStore};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const DEFAULT_PORT: u16 = 6379;
const DEFAULT_KEY_PREFIX: &str = "rehydration:snapshot";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValkeySnapshotStore {
    endpoint: ValkeyEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValkeyEndpoint {
    raw_uri: String,
    host: String,
    port: u16,
    key_prefix: String,
    ttl_seconds: Option<u64>,
}

impl ValkeySnapshotStore {
    pub fn new(snapshot_uri: impl Into<String>) -> Result<Self, PortError> {
        let endpoint = ValkeyEndpoint::parse(snapshot_uri.into())?;
        Ok(Self { endpoint })
    }

    fn snapshot_key(&self, bundle: &RehydrationBundle) -> String {
        format!(
            "{}:{}:{}",
            self.endpoint.key_prefix,
            bundle.case_id().as_str(),
            bundle.role().as_str()
        )
    }

    fn snapshot_payload(&self, bundle: &RehydrationBundle) -> Result<String, PortError> {
        Ok(serialize_bundle(bundle))
    }

    async fn execute_set_command(&self, key: &str, payload: &str) -> Result<(), PortError> {
        let mut stream = TcpStream::connect(self.endpoint.address())
            .await
            .map_err(|error| {
                PortError::Unavailable(format!(
                    "unable to connect to valkey {}: {error}",
                    self.endpoint.raw_uri
                ))
            })?;

        let frame = encode_set_command(key, payload, self.endpoint.ttl_seconds);
        stream.write_all(&frame).await.map_err(|error| {
            PortError::Unavailable(format!("failed to write snapshot to valkey: {error}"))
        })?;
        stream.flush().await.map_err(|error| {
            PortError::Unavailable(format!("failed to flush snapshot to valkey: {error}"))
        })?;

        let mut response = String::new();
        let mut reader = BufReader::new(stream);
        reader.read_line(&mut response).await.map_err(|error| {
            PortError::Unavailable(format!("failed to read valkey response: {error}"))
        })?;

        map_valkey_response(&response)
    }
}

impl SnapshotStore for ValkeySnapshotStore {
    async fn save_bundle(&self, bundle: &RehydrationBundle) -> Result<(), PortError> {
        let key = self.snapshot_key(bundle);
        let payload = self.snapshot_payload(bundle)?;
        self.execute_set_command(&key, &payload).await
    }
}

impl ValkeyEndpoint {
    fn parse(snapshot_uri: String) -> Result<Self, PortError> {
        if snapshot_uri.trim().is_empty() {
            return Err(PortError::InvalidState(
                "snapshot uri cannot be empty".to_string(),
            ));
        }

        let (scheme, authority, query) = split_uri(&snapshot_uri, "snapshot")?;
        if !matches!(scheme, "redis" | "valkey") {
            return Err(PortError::InvalidState(format!(
                "unsupported snapshot scheme `{scheme}`"
            )));
        }

        let (host, port) = parse_authority(authority, DEFAULT_PORT, "snapshot")?;

        let mut key_prefix = DEFAULT_KEY_PREFIX.to_string();
        let mut ttl_seconds = None;
        if let Some(query) = query {
            for pair in query.split('&') {
                if pair.is_empty() {
                    continue;
                }

                let (key, value) = pair.split_once('=').ok_or_else(|| {
                    PortError::InvalidState(format!(
                        "snapshot uri query parameter `{pair}` is invalid"
                    ))
                })?;

                match key {
                    "key_prefix" => {
                        if value.trim().is_empty() {
                            return Err(PortError::InvalidState(
                                "snapshot key_prefix cannot be empty".to_string(),
                            ));
                        }
                        key_prefix = value.to_string();
                    }
                    "ttl_seconds" => {
                        let ttl = value.parse::<u64>().map_err(|error| {
                            PortError::InvalidState(format!(
                                "snapshot ttl_seconds must be an integer: {error}"
                            ))
                        })?;
                        ttl_seconds = Some(ttl);
                    }
                    _ => {
                        return Err(PortError::InvalidState(format!(
                            "unsupported snapshot uri option `{key}`"
                        )));
                    }
                }
            }
        }

        Ok(Self {
            raw_uri: snapshot_uri,
            host,
            port,
            key_prefix,
            ttl_seconds,
        })
    }

    fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn split_uri<'a>(
    raw_uri: &'a str,
    name: &str,
) -> Result<(&'a str, &'a str, Option<&'a str>), PortError> {
    let (scheme, remainder) = raw_uri
        .split_once("://")
        .ok_or_else(|| PortError::InvalidState(format!("{name} uri must include a scheme")))?;
    if scheme.is_empty() {
        return Err(PortError::InvalidState(format!(
            "{name} uri must include a scheme"
        )));
    }

    let (location, query) = match remainder.split_once('?') {
        Some((location, query)) => (location, Some(query)),
        None => (remainder, None),
    };
    let authority = location.split('/').next().unwrap_or_default().trim();
    if authority.is_empty() {
        return Err(PortError::InvalidState(format!(
            "{name} uri must include a host"
        )));
    }

    Ok((scheme, authority, query))
}

fn parse_authority(
    authority: &str,
    default_port: u16,
    name: &str,
) -> Result<(String, u16), PortError> {
    if authority.contains('@') {
        return Err(PortError::InvalidState(format!(
            "{name} uri auth segments are not supported yet"
        )));
    }

    if authority.starts_with('[') {
        let (host, remainder) = authority.split_once(']').ok_or_else(|| {
            PortError::InvalidState(format!("{name} uri contains an invalid IPv6 host"))
        })?;
        let host = format!("{host}]");
        let port = parse_optional_port(remainder, default_port, name)?;
        return Ok((host, port));
    }

    match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => {
            if host.is_empty() {
                return Err(PortError::InvalidState(format!(
                    "{name} uri must include a host"
                )));
            }
            let port = port.parse::<u16>().map_err(|error| {
                PortError::InvalidState(format!("{name} uri contains an invalid port: {error}"))
            })?;
            Ok((host.to_string(), port))
        }
        Some(_) => Err(PortError::InvalidState(format!(
            "{name} uri IPv6 hosts must use bracket notation"
        ))),
        None => Ok((authority.to_string(), default_port)),
    }
}

fn parse_optional_port(remainder: &str, default_port: u16, name: &str) -> Result<u16, PortError> {
    if remainder.is_empty() {
        return Ok(default_port);
    }
    let port = remainder.strip_prefix(':').ok_or_else(|| {
        PortError::InvalidState(format!("{name} uri contains an invalid port separator"))
    })?;
    port.parse::<u16>().map_err(|error| {
        PortError::InvalidState(format!("{name} uri contains an invalid port: {error}"))
    })
}

fn serialize_bundle(bundle: &RehydrationBundle) -> String {
    let sections = bundle
        .sections()
        .iter()
        .map(|section| format!("\"{}\"", escape_json(section)))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        concat!(
            "{{",
            "\"case_id\":\"{}\",",
            "\"role\":\"{}\",",
            "\"sections\":[{}],",
            "\"metadata\":{{",
            "\"revision\":{},",
            "\"content_hash\":\"{}\",",
            "\"generator_version\":\"{}\"",
            "}}",
            "}}"
        ),
        escape_json(bundle.case_id().as_str()),
        escape_json(bundle.role().as_str()),
        sections,
        bundle.metadata().revision,
        escape_json(&bundle.metadata().content_hash),
        escape_json(&bundle.metadata().generator_version),
    )
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(&mut escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }

    escaped
}

fn encode_set_command(key: &str, payload: &str, ttl_seconds: Option<u64>) -> Vec<u8> {
    let mut arguments = vec!["SET".to_string(), key.to_string(), payload.to_string()];
    if let Some(ttl_seconds) = ttl_seconds {
        arguments.push("EX".to_string());
        arguments.push(ttl_seconds.to_string());
    }

    let mut command = Vec::new();
    command.extend_from_slice(format!("*{}\r\n", arguments.len()).as_bytes());
    for argument in arguments {
        command.extend_from_slice(format!("${}\r\n", argument.len()).as_bytes());
        command.extend_from_slice(argument.as_bytes());
        command.extend_from_slice(b"\r\n");
    }

    command
}

fn map_valkey_response(response: &str) -> Result<(), PortError> {
    match response.trim_end() {
        "+OK" => Ok(()),
        response if response.starts_with('-') => Err(PortError::Unavailable(format!(
            "valkey rejected snapshot write: {}",
            response.trim_start_matches('-')
        ))),
        response => Err(PortError::Unavailable(format!(
            "unexpected valkey response: {response}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::str;

    use super::{ValkeySnapshotStore, encode_set_command, map_valkey_response};
    use rehydration_domain::{CaseId, RehydrationBundle, Role};

    #[test]
    fn snapshot_store_encodes_a_resp_set_command() {
        let store = ValkeySnapshotStore::new(
            "redis://localhost:6379?key_prefix=rehydration:test&ttl_seconds=60",
        )
        .expect("uri should be accepted");
        let bundle = RehydrationBundle::empty(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("reviewer").expect("role is valid"),
            "0.1.0",
        );

        let key = store.snapshot_key(&bundle);
        let payload = store
            .snapshot_payload(&bundle)
            .expect("snapshot payload should serialize");
        let request = parse_resp_array(&encode_set_command(
            &key,
            &payload,
            store.endpoint.ttl_seconds,
        ))
        .expect("resp command should be parsed");

        assert_eq!(request[0], "SET");
        assert_eq!(request[1], "rehydration:test:case-123:reviewer");
        assert_eq!(request[3], "EX");
        assert_eq!(request[4], "60");
        assert_eq!(
            request[2],
            "{\"case_id\":\"case-123\",\"role\":\"reviewer\",\"sections\":[],\"metadata\":{\"revision\":1,\"content_hash\":\"pending\",\"generator_version\":\"0.1.0\"}}"
        );
    }

    #[test]
    fn snapshot_store_surfaces_server_errors() {
        let error = map_valkey_response("-ERR read only replica\r\n")
            .expect_err("server errors must surface");

        assert_eq!(
            error,
            rehydration_ports::PortError::Unavailable(
                "valkey rejected snapshot write: ERR read only replica".to_string()
            )
        );
    }

    #[test]
    fn snapshot_store_rejects_invalid_scheme() {
        let error = ValkeySnapshotStore::new("http://localhost:6379")
            .expect_err("unsupported schemes must fail");

        assert_eq!(
            error,
            rehydration_ports::PortError::InvalidState(
                "unsupported snapshot scheme `http`".to_string()
            )
        );
    }

    fn parse_resp_array(
        buffer: &[u8],
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let payload = str::from_utf8(buffer)?;
        let mut segments = payload.split("\r\n");
        let header = segments.next().ok_or("resp array header is missing")?;
        let argument_count = header
            .strip_prefix('*')
            .ok_or("resp array header is missing")?
            .parse::<usize>()?;

        let mut arguments = Vec::with_capacity(argument_count);
        for _ in 0..argument_count {
            let length_line = segments
                .next()
                .ok_or("resp bulk string header is missing")?;
            let length = length_line
                .strip_prefix('$')
                .ok_or("resp bulk string header is missing")?
                .parse::<usize>()?;
            let argument = segments
                .next()
                .ok_or("resp bulk string payload is missing")?;
            if argument.len() != length {
                return Err("resp bulk string payload has an unexpected length".into());
            }
            arguments.push(argument.to_string());
        }

        Ok(arguments)
    }
}
