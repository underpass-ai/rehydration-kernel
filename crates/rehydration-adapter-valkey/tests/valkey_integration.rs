use std::error::Error;

use rehydration_adapter_valkey::{ValkeyNodeDetailStore, ValkeySnapshotStore};
use rehydration_domain::{
    BundleMetadata, CaseHeader, CaseId, RehydrationBundle, Role, RoleContextPack,
};
use rehydration_ports::{
    NodeDetailProjection, NodeDetailReader, ProjectionMutation, ProjectionWriter, SnapshotStore,
};
use testcontainers::{
    GenericImage,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const VALKEY_INTERNAL_PORT: u16 = 6379;

#[tokio::test]
async fn save_bundle_persists_snapshot_in_valkey() -> Result<(), Box<dyn Error + Send + Sync>> {
    let container = GenericImage::new("docker.io/valkey/valkey", "8.1.5-alpine")
        .with_exposed_port(VALKEY_INTERNAL_PORT.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .start()
        .await?;

    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(VALKEY_INTERNAL_PORT).await?;
    let address = format!("{host}:{port}");

    let store = ValkeySnapshotStore::new(format!(
        "redis://{address}?key_prefix=rehydration:it&ttl_seconds=120"
    ))?;

    let case_id = CaseId::new("case-123")?;
    let role = Role::new("reviewer")?;
    let bundle = RehydrationBundle::new(
        RoleContextPack::new(
            role.clone(),
            CaseHeader::new(
                case_id.clone(),
                "Node case-123",
                "prior decision",
                "ACTIVE",
                std::time::SystemTime::UNIX_EPOCH,
                "integration-test",
            ),
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            "prior decision",
            4096,
        ),
        vec!["prior decision".to_string(), "active milestone".to_string()],
        BundleMetadata {
            revision: 7,
            content_hash: "abc123".to_string(),
            generator_version: "integration-test".to_string(),
        },
    );

    store.save_bundle(&bundle).await?;

    let key = "rehydration:it:case-123:reviewer";
    let snapshot = send_command(&address, &["GET", key]).await?;
    let ttl = send_command(&address, &["TTL", key]).await?;

    assert_eq!(
        snapshot,
        RespValue::BulkString(Some(
            "{\"root_node_id\":\"case-123\",\"role\":\"reviewer\",\"sections\":[\"prior decision\",\"active milestone\"],\"metadata\":{\"revision\":7,\"content_hash\":\"abc123\",\"generator_version\":\"integration-test\"}}".to_string()
        ))
    );

    match ttl {
        RespValue::Integer(value) => {
            assert!((1..=120).contains(&value));
        }
        other => panic!("expected integer TTL response, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn node_detail_roundtrip_reads_expanded_detail_from_valkey()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let container = GenericImage::new("docker.io/valkey/valkey", "8.1.5-alpine")
        .with_exposed_port(VALKEY_INTERNAL_PORT.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
        .start()
        .await?;

    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(VALKEY_INTERNAL_PORT).await?;
    let address = format!("{host}:{port}");

    let store = ValkeyNodeDetailStore::new(format!(
        "redis://{address}?key_prefix=rehydration:detail&ttl_seconds=120"
    ))?;

    store
        .apply_mutations(vec![ProjectionMutation::UpsertNodeDetail(
            NodeDetailProjection {
                node_id: "node-123".to_string(),
                detail: "Expanded node detail".to_string(),
                content_hash: "hash-123".to_string(),
                revision: 3,
            },
        )])
        .await?;

    let loaded = store
        .load_node_detail("node-123")
        .await?
        .expect("detail should exist");
    let ttl = send_command(&address, &["TTL", "rehydration:detail:node-123"]).await?;

    assert_eq!(loaded.node_id, "node-123");
    assert_eq!(loaded.detail, "Expanded node detail");
    assert_eq!(loaded.content_hash, "hash-123");
    assert_eq!(loaded.revision, 3);

    match ttl {
        RespValue::Integer(value) => assert!((1..=120).contains(&value)),
        other => panic!("expected integer TTL response, got {other:?}"),
    }

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum RespValue {
    SimpleString(String),
    BulkString(Option<String>),
    Integer(i64),
    Error(String),
}

async fn send_command(
    address: &str,
    arguments: &[&str],
) -> Result<RespValue, Box<dyn Error + Send + Sync>> {
    let mut stream = TcpStream::connect(address).await?;
    let payload = encode_command(arguments);
    stream.write_all(&payload).await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    read_response(&mut reader).await
}

fn encode_command(arguments: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(format!("*{}\r\n", arguments.len()).as_bytes());
    for argument in arguments {
        payload.extend_from_slice(format!("${}\r\n", argument.len()).as_bytes());
        payload.extend_from_slice(argument.as_bytes());
        payload.extend_from_slice(b"\r\n");
    }

    payload
}

async fn read_response(
    reader: &mut BufReader<TcpStream>,
) -> Result<RespValue, Box<dyn Error + Send + Sync>> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let line = line.trim_end_matches("\r\n");
    let (prefix, remainder) = line.split_at(1);

    match prefix {
        "+" => Ok(RespValue::SimpleString(remainder.to_string())),
        "-" => Ok(RespValue::Error(remainder.to_string())),
        ":" => Ok(RespValue::Integer(remainder.parse()?)),
        "$" => {
            let length: isize = remainder.parse()?;
            if length == -1 {
                return Ok(RespValue::BulkString(None));
            }

            let mut buffer = vec![0_u8; length as usize];
            reader.read_exact(&mut buffer).await?;
            let mut crlf = [0_u8; 2];
            reader.read_exact(&mut crlf).await?;
            Ok(RespValue::BulkString(Some(String::from_utf8(buffer)?)))
        }
        other => Err(format!("unsupported RESP prefix `{other}`").into()),
    }
}
