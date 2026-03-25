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

pub(crate) fn deserialize_properties(
    payload: &str,
    entity: &str,
) -> Result<BTreeMap<String, String>, PortError> {
    serde_json::from_str(payload).map_err(|error| {
        PortError::InvalidState(format!(
            "neo4j {entity} properties_json could not be decoded: {error}"
        ))
    })
}

pub(crate) fn row_properties(
    row: &Row,
    key: &str,
    entity: &str,
) -> Result<BTreeMap<String, String>, PortError> {
    deserialize_properties(&row_string(row, key, entity)?, entity)
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
        provenance: None, // TODO: read from Neo4j when persisted
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use neo4rs::{BoltList, BoltType, Row};
    use rehydration_ports::PortError;

    use super::{node_projection_from_row, row_string, serialize_properties};

    #[test]
    fn serialize_properties_emits_json_payloads() {
        let payload = serialize_properties(
            &[("phase".to_string(), "build".to_string())]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
        )
        .expect("properties should serialize");

        assert_eq!(payload, "{\"phase\":\"build\"}");
    }

    #[test]
    fn node_projection_from_row_maps_complete_rows() {
        let row = row(vec![
            ("node_id", BoltType::from("node-123")),
            ("node_kind", BoltType::from("capability")),
            ("title", BoltType::from("Root node")),
            ("summary", BoltType::from("expanded context")),
            ("status", BoltType::from("ACTIVE")),
            (
                "node_labels",
                BoltType::List(BoltList::from(vec![
                    BoltType::from("Capability"),
                    BoltType::from("ProjectionNode"),
                ])),
            ),
            ("properties_json", BoltType::from("{\"phase\":\"build\"}")),
        ]);

        let projection =
            node_projection_from_row(&row, "", "node").expect("rows should map successfully");

        assert_eq!(projection.node_id, "node-123");
        assert_eq!(projection.node_kind, "capability");
        assert_eq!(projection.labels, vec!["Capability", "ProjectionNode"]);
        assert_eq!(
            projection.properties,
            [("phase".to_string(), "build".to_string())]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
        );
    }

    #[test]
    fn row_mapping_surfaces_missing_fields_and_invalid_json() {
        let missing_field =
            row_string(&row(vec![]), "node_id", "node").expect_err("missing node ids must fail");
        let invalid_json = node_projection_from_row(
            &row(vec![
                ("node_id", BoltType::from("node-123")),
                ("node_kind", BoltType::from("capability")),
                ("title", BoltType::from("Root node")),
                ("summary", BoltType::from("expanded context")),
                ("status", BoltType::from("ACTIVE")),
                (
                    "node_labels",
                    BoltType::List(BoltList::from(Vec::<BoltType>::new())),
                ),
                ("properties_json", BoltType::from("{not-json}")),
            ]),
            "",
            "node",
        )
        .expect_err("invalid json must fail");

        assert!(
            missing_field
                .to_string()
                .starts_with("neo4j node field `node_id` could not be decoded:")
        );
        assert!(matches!(
            invalid_json,
            PortError::InvalidState(message)
                if message.starts_with("neo4j node properties_json could not be decoded:")
        ));
    }

    fn row(values: Vec<(&str, BoltType)>) -> Row {
        let fields = BoltList::from(
            values
                .iter()
                .map(|(key, _)| BoltType::from(*key))
                .collect::<Vec<_>>(),
        );
        let data = BoltList::from(
            values
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<_>>(),
        );

        Row::new(fields, data)
    }
}
