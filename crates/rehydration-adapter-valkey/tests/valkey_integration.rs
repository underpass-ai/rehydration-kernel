use std::error::Error;

use rehydration_adapter_valkey::{ValkeyNodeDetailStore, ValkeySnapshotStore};
use rehydration_domain::{
    BundleMetadata, BundleNode, BundleNodeDetail, CaseId, RehydrationBundle, Role,
    SnapshotSaveOptions,
};
use rehydration_ports::{
    NodeDetailProjection, NodeDetailReader, ProjectionMutation, ProjectionWriter, SnapshotStore,
};
use serde_json::{Value, json};
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

    let case_id = CaseId::new("node-123")?;
    let role = Role::new("reviewer")?;
    let bundle = RehydrationBundle::new(
        case_id.clone(),
        role.clone(),
        BundleNode::new(
            case_id.as_str(),
            "capability",
            format!("Node {}", case_id.as_str()),
            "expanded context",
            "ACTIVE",
            vec!["projection-node".to_string()],
            std::collections::BTreeMap::new(),
        ),
        Vec::new(),
        Vec::new(),
        vec![BundleNodeDetail::new(
            case_id.as_str(),
            "expanded context",
            "abc123",
            7,
        )],
        BundleMetadata {
            revision: 7,
            content_hash: "abc123".to_string(),
            generator_version: "integration-test".to_string(),
        },
    );
    let bundle = bundle?;

    store.save_bundle(&bundle).await?;

    let key = "rehydration:it:node-123:reviewer";
    let snapshot = send_command(&address, &["GET", key]).await?;
    let ttl = send_command(&address, &["TTL", key]).await?;

    match snapshot {
        RespValue::BulkString(Some(payload)) => {
            assert_eq!(
                serde_json::from_str::<Value>(&payload)?,
                json!({
                    "root_node_id": "node-123",
                    "role": "reviewer",
                    "root_node": {
                        "node_id": "node-123",
                        "node_kind": "capability",
                        "title": "Node node-123",
                        "summary": "expanded context",
                        "status": "ACTIVE",
                        "labels": ["projection-node"],
                        "properties": {}
                    },
                    "neighbor_nodes": [],
                    "relationships": [],
                    "node_details": [{
                        "node_id": "node-123",
                        "detail": "expanded context",
                        "content_hash": "abc123",
                        "revision": 7
                    }],
                    "stats": {
                        "selected_nodes": 1,
                        "selected_relationships": 0,
                        "detailed_nodes": 1
                    },
                    "metadata": {
                        "revision": 7,
                        "content_hash": "abc123",
                        "generator_version": "integration-test"
                    }
                })
            );
        }
        other => panic!("expected snapshot payload, got {other:?}"),
    }

    match ttl {
        RespValue::Integer(value) => {
            assert!((1..=120).contains(&value));
        }
        other => panic!("expected integer TTL response, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn save_bundle_respects_request_ttl_override() -> Result<(), Box<dyn Error + Send + Sync>> {
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
    let case_id = CaseId::new("node-123")?;
    let role = Role::new("reviewer")?;
    let bundle = RehydrationBundle::new(
        case_id.clone(),
        role.clone(),
        BundleNode::new(
            case_id.as_str(),
            "capability",
            format!("Node {}", case_id.as_str()),
            "expanded context",
            "ACTIVE",
            vec!["projection-node".to_string()],
            std::collections::BTreeMap::new(),
        ),
        Vec::new(),
        Vec::new(),
        vec![BundleNodeDetail::new(
            case_id.as_str(),
            "expanded context",
            "abc123",
            7,
        )],
        BundleMetadata {
            revision: 7,
            content_hash: "abc123".to_string(),
            generator_version: "integration-test".to_string(),
        },
    )?;

    store
        .save_bundle_with_options(&bundle, SnapshotSaveOptions::new(Some(15)))
        .await?;

    match send_command(&address, &["TTL", "rehydration:it:node-123:reviewer"]).await? {
        RespValue::Integer(value) => assert!((1..=15).contains(&value)),
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
