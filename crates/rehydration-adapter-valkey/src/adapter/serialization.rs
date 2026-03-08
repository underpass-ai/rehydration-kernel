use std::fmt::Write as _;

use rehydration_domain::RehydrationBundle;
use rehydration_ports::{NodeDetailProjection, PortError};
use serde_json::json;

pub(crate) fn serialize_node_detail(detail: &NodeDetailProjection) -> Result<String, PortError> {
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

pub(crate) fn deserialize_node_detail(
    payload: &str,
) -> Result<Option<NodeDetailProjection>, PortError> {
    #[derive(serde::Deserialize)]
    struct RawNodeDetailProjection {
        node_id: String,
        detail: String,
        content_hash: String,
        revision: u64,
    }

    serde_json::from_str::<RawNodeDetailProjection>(payload)
        .map(|detail| {
            Some(NodeDetailProjection {
                node_id: detail.node_id,
                detail: detail.detail,
                content_hash: detail.content_hash,
                revision: detail.revision,
            })
        })
        .map_err(|error| {
            PortError::InvalidState(format!(
                "node detail could not be deserialized from valkey: {error}"
            ))
        })
}

pub(crate) fn serialize_bundle(bundle: &RehydrationBundle) -> String {
    let sections = bundle
        .sections()
        .iter()
        .map(|section| format!("\"{}\"", escape_json(section)))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        concat!(
            "{{",
            "\"root_node_id\":\"{}\",",
            "\"role\":\"{}\",",
            "\"sections\":[{}],",
            "\"metadata\":{{",
            "\"revision\":{},",
            "\"content_hash\":\"{}\",",
            "\"generator_version\":\"{}\"",
            "}}",
            "}}"
        ),
        escape_json(bundle.root_node_id().as_str()),
        escape_json(bundle.role().as_str()),
        sections,
        bundle.metadata().revision,
        escape_json(&bundle.metadata().content_hash),
        escape_json(&bundle.metadata().generator_version),
    )
}

pub(crate) fn escape_json(value: &str) -> String {
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
