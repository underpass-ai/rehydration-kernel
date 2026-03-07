use std::fmt::Write as _;

use rehydration_domain::RehydrationBundle;
use rehydration_ports::{
    NodeDetailProjection, PortError, ProjectionMutation, ProjectionWriter, SnapshotStore,
};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const DEFAULT_PORT: u16 = 6379;
const DEFAULT_KEY_PREFIX: &str = "rehydration:snapshot";
const DEFAULT_NODE_DETAIL_KEY_PREFIX: &str = "rehydration:node-detail";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValkeySnapshotStore {
    endpoint: ValkeyEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValkeyNodeDetailStore {
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
        execute_set_command(&self.endpoint, key, payload).await
    }
}

impl ValkeyNodeDetailStore {
    pub fn new(detail_uri: impl Into<String>) -> Result<Self, PortError> {
        let endpoint = ValkeyEndpoint::parse_with_default_key_prefix(
            detail_uri.into(),
            "detail",
            DEFAULT_NODE_DETAIL_KEY_PREFIX,
        )?;
        Ok(Self { endpoint })
    }

    fn detail_key(&self, node_id: &str) -> String {
        format!("{}:{}", self.endpoint.key_prefix, node_id)
    }

    fn detail_payload(&self, detail: &NodeDetailProjection) -> Result<String, PortError> {
        serialize_node_detail(detail)
    }

    async fn execute_set_command(&self, key: &str, payload: &str) -> Result<(), PortError> {
        execute_set_command(&self.endpoint, key, payload).await
    }
}

async fn execute_set_command(
    endpoint: &ValkeyEndpoint,
    key: &str,
    payload: &str,
) -> Result<(), PortError> {
    let mut stream = TcpStream::connect(endpoint.address())
        .await
        .map_err(|error| {
            PortError::Unavailable(format!(
                "unable to connect to valkey {}: {error}",
                endpoint.raw_uri
            ))
        })?;

    let frame = encode_set_command(key, payload, endpoint.ttl_seconds);
    stream.write_all(&frame).await.map_err(|error| {
        PortError::Unavailable(format!("failed to write valkey payload: {error}"))
    })?;
    stream.flush().await.map_err(|error| {
        PortError::Unavailable(format!("failed to flush valkey payload: {error}"))
    })?;

    let mut response = String::new();
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut response).await.map_err(|error| {
        PortError::Unavailable(format!("failed to read valkey response: {error}"))
    })?;

    map_valkey_response(&response)
}

impl SnapshotStore for ValkeySnapshotStore {
    async fn save_bundle(&self, bundle: &RehydrationBundle) -> Result<(), PortError> {
        let key = self.snapshot_key(bundle);
        let payload = self.snapshot_payload(bundle)?;
        self.execute_set_command(&key, &payload).await
    }
}

impl ProjectionWriter for ValkeyNodeDetailStore {
    async fn apply_mutations(&self, mutations: Vec<ProjectionMutation>) -> Result<(), PortError> {
        for mutation in mutations {
            match mutation {
                ProjectionMutation::UpsertNodeDetail(detail) => {
                    let key = self.detail_key(&detail.node_id);
                    let payload = self.detail_payload(&detail)?;
                    self.execute_set_command(&key, &payload).await?;
                }
                ProjectionMutation::UpsertNode(node) => {
                    return Err(PortError::InvalidState(format!(
                        "valkey detail store does not persist graph node `{}`",
                        node.node_id
                    )));
                }
                ProjectionMutation::UpsertNodeRelation(relation) => {
                    return Err(PortError::InvalidState(format!(
                        "valkey detail store does not persist graph relation `{} -> {}`",
                        relation.source_node_id, relation.target_node_id
                    )));
                }
            }
        }

        Ok(())
    }
}

impl ValkeyEndpoint {
    fn parse(snapshot_uri: String) -> Result<Self, PortError> {
        Self::parse_with_default_key_prefix(snapshot_uri, "snapshot", DEFAULT_KEY_PREFIX)
    }

    fn parse_with_default_key_prefix(
        raw_uri: String,
        name: &str,
        default_key_prefix: &str,
    ) -> Result<Self, PortError> {
        if raw_uri.trim().is_empty() {
            return Err(PortError::InvalidState(format!(
                "{name} uri cannot be empty"
            )));
        }

        let (scheme, authority, query) = split_uri(&raw_uri, name)?;
        if !matches!(scheme, "redis" | "valkey") {
            return Err(PortError::InvalidState(format!(
                "unsupported {name} scheme `{scheme}`"
            )));
        }

        let (host, port) = parse_authority(authority, DEFAULT_PORT, name)?;

        let mut key_prefix = default_key_prefix.to_string();
        let mut ttl_seconds = None;
        if let Some(query) = query {
            for pair in query.split('&') {
                if pair.is_empty() {
                    continue;
                }

                let (key, value) = pair.split_once('=').ok_or_else(|| {
                    PortError::InvalidState(format!(
                        "{name} uri query parameter `{pair}` is invalid"
                    ))
                })?;

                match key {
                    "key_prefix" => {
                        if value.trim().is_empty() {
                            return Err(PortError::InvalidState(format!(
                                "{name} key_prefix cannot be empty"
                            )));
                        }
                        key_prefix = value.to_string();
                    }
                    "ttl_seconds" => {
                        let ttl = value.parse::<u64>().map_err(|error| {
                            PortError::InvalidState(format!(
                                "{name} ttl_seconds must be an integer: {error}"
                            ))
                        })?;
                        ttl_seconds = Some(ttl);
                    }
                    _ => {
                        return Err(PortError::InvalidState(format!(
                            "unsupported {name} uri option `{key}`"
                        )));
                    }
                }
            }
        }

        Ok(Self {
            raw_uri,
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

fn serialize_node_detail(detail: &NodeDetailProjection) -> Result<String, PortError> {
    serde_json::to_string(&json!({
        "node_id": detail.node_id,
        "detail": detail.detail,
        "content_hash": detail.content_hash,
        "revision": detail.revision,
    }))
    .map_err(|error| {
        PortError::InvalidState(format!(
            "node detail could not be serialized for valkey: {error}"
        ))
    })
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
            "valkey rejected write: {}",
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

    use super::{
        ValkeyNodeDetailStore, ValkeySnapshotStore, encode_set_command, escape_json,
        map_valkey_response, parse_authority, parse_optional_port, serialize_node_detail,
        split_uri,
    };
    use rehydration_domain::{CaseId, RehydrationBundle, Role};
    use rehydration_ports::{NodeDetailProjection, ProjectionMutation, ProjectionWriter};

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
            "{\"case_id\":\"case-123\",\"role\":\"reviewer\",\"sections\":[\"bundle for case case-123 role reviewer\"],\"metadata\":{\"revision\":1,\"content_hash\":\"pending\",\"generator_version\":\"0.1.0\"}}"
        );
    }

    #[test]
    fn snapshot_store_surfaces_server_errors() {
        let error = map_valkey_response("-ERR read only replica\r\n")
            .expect_err("server errors must surface");

        assert_eq!(
            error,
            rehydration_ports::PortError::Unavailable(
                "valkey rejected write: ERR read only replica".to_string()
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

    #[test]
    fn snapshot_store_parses_valid_runtime_options() {
        let store = ValkeySnapshotStore::new(
            "valkey://cache.internal?key_prefix=rehydration:it&ttl_seconds=15",
        )
        .expect("valkey uri should be accepted");

        assert_eq!(store.endpoint.host, "cache.internal");
        assert_eq!(store.endpoint.port, 6379);
        assert_eq!(store.endpoint.key_prefix, "rehydration:it");
        assert_eq!(store.endpoint.ttl_seconds, Some(15));
    }

    #[test]
    fn node_detail_store_uses_dedicated_default_prefix() {
        let store = ValkeyNodeDetailStore::new("redis://cache.internal")
            .expect("detail uri should be accepted");

        assert_eq!(store.endpoint.key_prefix, "rehydration:node-detail");
        assert_eq!(
            store.detail_key("node-123"),
            "rehydration:node-detail:node-123"
        );
    }

    #[test]
    fn snapshot_store_rejects_invalid_query_options() {
        let invalid_pair = ValkeySnapshotStore::new("redis://localhost:6379?ttl_seconds")
            .expect_err("query pairs must contain =");
        let invalid_ttl = ValkeySnapshotStore::new("redis://localhost:6379?ttl_seconds=soon")
            .expect_err("ttl must be numeric");
        let empty_prefix = ValkeySnapshotStore::new("redis://localhost:6379?key_prefix=   ")
            .expect_err("key prefix cannot be empty");
        let unsupported_option = ValkeySnapshotStore::new("redis://localhost:6379?database=1")
            .expect_err("unsupported options must fail");

        assert_eq!(
            invalid_pair,
            rehydration_ports::PortError::InvalidState(
                "snapshot uri query parameter `ttl_seconds` is invalid".to_string()
            )
        );
        assert!(
            invalid_ttl
                .to_string()
                .starts_with("snapshot ttl_seconds must be an integer:")
        );
        assert_eq!(
            empty_prefix,
            rehydration_ports::PortError::InvalidState(
                "snapshot key_prefix cannot be empty".to_string()
            )
        );
        assert_eq!(
            unsupported_option,
            rehydration_ports::PortError::InvalidState(
                "unsupported snapshot uri option `database`".to_string()
            )
        );
    }

    #[test]
    fn parser_helpers_cover_ipv6_and_error_branches() {
        let (scheme, authority, query) =
            split_uri("redis://[::1]:6380/cache?ttl_seconds=5", "snapshot")
                .expect("uri should parse");
        let (host, port) =
            parse_authority(authority, 6379, "snapshot").expect("authority should parse");

        assert_eq!(scheme, "redis");
        assert_eq!(authority, "[::1]:6380");
        assert_eq!(query, Some("ttl_seconds=5"));
        assert_eq!(host, "[::1]");
        assert_eq!(port, 6380);

        let missing_scheme =
            split_uri("localhost:6379", "snapshot").expect_err("scheme is required");
        let missing_host = split_uri("redis://", "snapshot").expect_err("host is required");
        let auth_segment = parse_authority("user@localhost:6379", 6379, "snapshot")
            .expect_err("auth is not supported");
        let invalid_ipv6 =
            parse_authority("[::1", 6379, "snapshot").expect_err("ipv6 host must be complete");
        let invalid_port = parse_authority("localhost:not-a-port", 6379, "snapshot")
            .expect_err("ports must be numeric");
        let invalid_separator = parse_optional_port("6380", 6379, "snapshot")
            .expect_err("port separators must be explicit");

        assert_eq!(
            missing_scheme,
            rehydration_ports::PortError::InvalidState(
                "snapshot uri must include a scheme".to_string()
            )
        );
        assert_eq!(
            missing_host,
            rehydration_ports::PortError::InvalidState(
                "snapshot uri must include a host".to_string()
            )
        );
        assert_eq!(
            auth_segment,
            rehydration_ports::PortError::InvalidState(
                "snapshot uri auth segments are not supported yet".to_string()
            )
        );
        assert_eq!(
            invalid_ipv6,
            rehydration_ports::PortError::InvalidState(
                "snapshot uri contains an invalid IPv6 host".to_string()
            )
        );
        assert!(
            invalid_port
                .to_string()
                .starts_with("snapshot uri contains an invalid port:")
        );
        assert_eq!(
            invalid_separator,
            rehydration_ports::PortError::InvalidState(
                "snapshot uri contains an invalid port separator".to_string()
            )
        );
    }

    #[test]
    fn serializer_and_response_helpers_cover_escape_paths() {
        let bundle = RehydrationBundle::new(
            CaseId::new("case-123").expect("case id is valid"),
            Role::new("reviewer").expect("role is valid"),
            vec!["quote \" slash \\ newline\n tab\t".to_string()],
            rehydration_domain::BundleMetadata {
                revision: 2,
                content_hash: "hash\rvalue".to_string(),
                generator_version: "0.1.0".to_string(),
            },
        );
        let store = ValkeySnapshotStore::new("redis://localhost").expect("uri should be accepted");
        let payload = store
            .snapshot_payload(&bundle)
            .expect("snapshot payload should serialize");

        assert!(payload.contains("\\\""));
        assert!(payload.contains("\\\\"));
        assert!(payload.contains("\\n"));
        assert!(payload.contains("\\t"));
        assert!(payload.contains("\\r"));
        assert_eq!(escape_json("\u{0001}"), "\\u0001");
        assert!(matches!(map_valkey_response("+OK\r\n"), Ok(())));
        assert_eq!(
            map_valkey_response("?wat\r\n").expect_err("unexpected response must fail"),
            rehydration_ports::PortError::Unavailable(
                "unexpected valkey response: ?wat".to_string()
            )
        );
    }

    #[test]
    fn node_detail_serializer_emits_expected_shape() {
        let payload = serialize_node_detail(&NodeDetailProjection {
            node_id: "node-123".to_string(),
            detail: "Expanded detail".to_string(),
            content_hash: "hash-123".to_string(),
            revision: 4,
        })
        .expect("detail should serialize");

        assert_eq!(
            payload,
            "{\"content_hash\":\"hash-123\",\"detail\":\"Expanded detail\",\"node_id\":\"node-123\",\"revision\":4}"
        );
    }

    #[tokio::test]
    async fn detail_store_rejects_graph_mutations() {
        let store =
            ValkeyNodeDetailStore::new("redis://cache.internal").expect("detail uri should parse");

        let error = store
            .apply_mutations(vec![ProjectionMutation::UpsertNode(
                rehydration_ports::NodeProjection {
                    node_id: "node-123".to_string(),
                    node_kind: "capability".to_string(),
                    title: "Projection".to_string(),
                    summary: String::new(),
                    status: "ACTIVE".to_string(),
                    labels: Vec::new(),
                    properties: std::collections::BTreeMap::new(),
                },
            )])
            .await
            .expect_err("graph nodes should be rejected by detail store");

        assert_eq!(
            error,
            rehydration_ports::PortError::InvalidState(
                "valkey detail store does not persist graph node `node-123`".to_string()
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
