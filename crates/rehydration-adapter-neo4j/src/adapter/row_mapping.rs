use std::collections::BTreeMap;

use neo4rs::Row;
use rehydration_ports::{NodeProjection, PortError};

pub(crate) fn serialize_properties(
    properties: &BTreeMap<String, String>,
) -> Result<String, PortError> {
    serde_json::to_string(properties).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j node projection properties could not be serialized: {error}"
        ))
    })
}

fn deserialize_properties(
    payload: &str,
    entity: &str,
) -> Result<BTreeMap<String, String>, PortError> {
    serde_json::from_str(payload).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} properties_json could not be decoded: {error}"
        ))
    })
}

pub(crate) fn row_string(row: &Row, key: &str, entity: &str) -> Result<String, PortError> {
    row.get(key).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} field `{key}` could not be decoded: {error}"
        ))
    })
}

pub(crate) fn row_string_vec(row: &Row, key: &str, entity: &str) -> Result<Vec<String>, PortError> {
    row.get(key).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} field `{key}` could not be decoded: {error}"
        ))
    })
}

pub(crate) fn node_projection_from_row(
    row: &Row,
    prefix: &str,
    entity: &str,
) -> Result<NodeProjection, PortError> {
    Ok(NodeProjection {
        node_id: row_string(row, &format!("{prefix}node_id"), entity)?,
        node_kind: row_string(row, &format!("{prefix}node_kind"), entity)?,
        title: row_string(row, &format!("{prefix}title"), entity)?,
        summary: row_string(row, &format!("{prefix}summary"), entity)?,
        status: row_string(row, &format!("{prefix}status"), entity)?,
        labels: row_string_vec(row, &format!("{prefix}node_labels"), entity)?,
        properties: deserialize_properties(
            &row_string(row, &format!("{prefix}properties_json"), entity)?,
            entity,
        )?,
    })
}
