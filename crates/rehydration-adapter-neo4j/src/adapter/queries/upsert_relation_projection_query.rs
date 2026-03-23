use neo4rs::{Query, query};
use rehydration_ports::{NodeRelationProjection, PortError};

use super::super::row_mapping::serialize_properties;

pub(crate) fn upsert_relation_projection_query(
    relation: &NodeRelationProjection,
) -> Result<Query, PortError> {
    Ok(query(
        "
MERGE (source:ProjectionNode {node_id: $source_node_id})
ON CREATE SET source.node_kind = 'unknown',
              source.title = '',
              source.summary = '',
              source.status = 'STATUS_UNSPECIFIED',
              source.node_labels = [],
              source.properties_json = '{}'
MERGE (target:ProjectionNode {node_id: $target_node_id})
ON CREATE SET target.node_kind = 'unknown',
              target.title = '',
              target.summary = '',
              target.status = 'STATUS_UNSPECIFIED',
              target.node_labels = [],
              target.properties_json = '{}'
MERGE (source)-[edge:RELATED_TO {relation_type: $relation_type}]->(target)
SET edge.properties_json = $properties_json
        ",
    )
    .param("source_node_id", relation.source_node_id.as_str())
    .param("target_node_id", relation.target_node_id.as_str())
    .param("relation_type", relation.relation_type.as_str())
    .param(
        "properties_json",
        serialize_properties(&relation.explanation.to_properties())?,
    ))
}
